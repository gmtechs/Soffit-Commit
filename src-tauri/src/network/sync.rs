//! File synchronization engine.
//!
//! A share's owner pushes its contents to every peer that holds a granted
//! permission on that share. Each live QUIC connection runs two concurrent
//! parts (see run_peer_connection):
//!
//! * a reader task that services inbound control messages (ShareGrant,
//!   FileIndexRequest, FileIndex, FileInfo) and writes verified files
//!   into <app_data_dir>/shared/<share_id>/,
//! * a pusher that, for every share this device owns, exchanges file
//!   indexes with the peer, diffs them by relative path + SHA-256, and
//!   transfers the files the peer is missing or has changed.
//!
//! Connections are kept alive until the peer closes them. Each connection
//! independently pushes every owned share the peer is granted; overlapping
//! connections each push in their own task.
use crate::{
    activity, conflicts,
    models::{FileKind, PermissionLevel, SyncStatus},
    network::{
        node::{IrohNode, NetworkEvent},
        protocol::{IndexedFile, SyncMessage},
    },
    peers, shares,
};
use anyhow::{bail, Result};
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    sync::{oneshot, Mutex},
    time::{timeout, Duration},
};

/// Control messages (and the JSON header of a file stream) are length-
/// prefixed with a u32 BE length and capped to keep frame parsing bounded.
const MAX_CONTROL_FRAME: usize = 16 * 1024 * 1024;
/// How long to wait for a peer's FileIndex response before giving up and
/// pushing anyway (always safe: writes are idempotent and hash-checked).
const INDEX_TIMEOUT: Duration = Duration::from_secs(15);
/// How long to wait between automatic re-sync cycles on a live connection.
/// While a peer stays online, owned shares are re-indexed and diffs re-pushed
/// every <SYNC_INTERVAL>, so edits propagate in near real time. Re-indexing
/// is cheap: unchanged files reuse their stored hash (see shares::index_dir).
const SYNC_INTERVAL: Duration = Duration::from_secs(8);

/// State shared between the reader task and the pusher of one connection.
pub(crate) struct SharedCtx {
    /// oneshot senders awaiting a FileIndex response, keyed by share_id.
    pub(crate) pending_index: Mutex<HashMap<String, oneshot::Sender<Vec<IndexedFile>>>>,
    /// oneshot senders awaiting a ResumeInfo response, keyed by `share:path`.
    pub(crate) pending_resume: Mutex<HashMap<String, oneshot::Sender<u64>>>,
    /// oneshot senders awaiting a pulled file to finish landing, keyed by
    /// `share:path` (pull-mode selective sync, "fetch on open").
    pub(crate) pending_fetch: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl Default for SharedCtx {
    fn default() -> Self {
        SharedCtx {
            pending_index: Mutex::new(HashMap::new()),
            pending_resume: Mutex::new(HashMap::new()),
            pending_fetch: Mutex::new(HashMap::new()),
        }
    }
}
/// Drive one peer connection for its whole lifetime:
/// 1. spawn a reader that serves inbound sync requests and writes received files,
/// 2. push every owned share the peer is granted on this connection,
/// 3. keep the connection alive until the peer closes it.
pub async fn run_peer_connection(
    conn: iroh::endpoint::Connection,
    node: &Arc<IrohNode>,
    peer_node_id: &str,
    _peer_display_name: &str,
    shared: Arc<SharedCtx>,
) {
    node.conns
        .lock()
        .expect("connection registry poisoned")
        .insert(peer_node_id.to_string(), (conn.clone(), shared.clone()));
    {
        let conn_reader = conn.clone();
        let node_reader = node.clone();
        let shared_reader = shared.clone();
        let peer_reader = peer_node_id.to_string();
        tokio::spawn(async move {
            service_peer_streams(conn_reader, &node_reader, &shared_reader, &peer_reader).await;
        });
    }

    // Keep serving inbound streams (e.g. this device receiving a push the
    // other side is sending) and re-sync whenever a watched folder changes
    // (realtime) or on the SYNC_INTERVAL poll cadence (fallback) while the
    // peer stays connected. The task ends when the peer closes the connection.
    loop {
        let _ = push_owned_shares(&conn, node, &shared, peer_node_id).await;
        tokio::select! {
            _ = conn.closed() => break,
            _ = node.change_notify.notified() => {}
            _ = tokio::time::sleep(SYNC_INTERVAL) => {}
        }
    }
    node.conns
        .lock()
        .expect("connection registry poisoned")
        .remove(peer_node_id);
}

/// Reader task: loop accepting uni streams and dispatching by message type.
async fn service_peer_streams(
    conn: iroh::endpoint::Connection,
    node: &Arc<IrohNode>,
    shared: &Arc<SharedCtx>,
    peer_node_id: &str,
) {
    loop {
        let Ok(mut recv) = conn.accept_uni().await else {
            break;
        };
        // A single truncated or abandoned stream must not end the reader:
        // dropping out here would silence every later reply for the rest of
        // the connection's life, which stalls the pusher into re-sending.
        // Skip the bad stream instead and keep serving; a dead connection
        // surfaces as an error from accept_uni above.
        let mut len_buf = [0u8; 4];
        if recv.read_exact(&mut len_buf).await.is_err() {
            continue;
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len == 0 || len > MAX_CONTROL_FRAME {
            continue;
        }
        let mut buf = vec![0u8; len];
        if recv.read_exact(&mut buf).await.is_err() {
            continue;
        }
        let Ok(msg) = serde_json::from_slice::<SyncMessage>(&buf) else {
            continue;
        };

        match msg {
            SyncMessage::FileInfo {
                share_id,
                path,
                size,
                hash,
                resume_from,
            } => {
                // The raw file payload follows in the same stream: an
                // 8-byte BE length (which must equal the remaining size)
                // followed by the bytes.
                let mut payload_len_buf = [0u8; 8];
                if recv.read_exact(&mut payload_len_buf).await.is_err() {
                    // The sender abandoned this file stream — skip it, but
                    // keep the reader (and therefore the connection) alive.
                    continue;
                }
                let payload_len = u64::from_be_bytes(payload_len_buf);
                let expected_payload = size.saturating_sub(resume_from);
                if payload_len != expected_payload {
                    emit_sync_error(
                        node,
                        &path,
                        format!("size mismatch: expected {expected_payload}, got {payload_len}"),
                    );
                    continue;
                }
                // Stream the payload straight to disk (bounded memory) and
                // verify integrity before it is installed.
                receive_file(
                    node,
                    peer_node_id,
                    &share_id,
                    &path,
                    size,
                    resume_from,
                    &hash,
                    &mut recv,
                )
                .await;
                // A pull ("fetch on open") may be waiting for this exact file.
                let key = format!("{share_id}:{path}");
                if let Some(tx) = shared.pending_fetch.lock().await.remove(&key) {
                    let _ = tx.send(());
                }
            }
            SyncMessage::FileIndexRequest { share_id } => {
                // Re-scan the folder first so the reply reflects the disk right
                // now — realtime nudges (LocalChange → owner push → index
                // request) compare against this reply, and a stale index would
                // make the owner skip a needed transfer.
                if let Ok(db) = rusqlite::Connection::open(&node.db_path) {
                    let path: Option<String> = db
                        .query_row(
                            "SELECT path FROM shares WHERE id = ?1",
                            rusqlite::params![share_id],
                            |row| row.get(0),
                        )
                        .ok();
                    if let Some(path) = path {
                        shares::index_share_files(&db, &share_id, &path).ok();
                    }
                }
                let files = current_local_index(&node.db_path, &share_id);
                let _ = send_control(&conn, &SyncMessage::FileIndex { share_id, files }).await;
            }
            SyncMessage::LocalChange { share_id: _ } => {
                // The peer's folder changed; wake our push loops so the diff
                // runs immediately. The actual re-index + transfer decision
                // happens in push_owned_shares.
                node.change_notify.notify_waiters();
            }
            SyncMessage::FileIndex { share_id, files } => {
                if let Some(tx) = shared.pending_index.lock().await.remove(&share_id) {
                    let _ = tx.send(files.clone());
                }
                // Remember the owner's index as metadata-only rows for shares
                // this device receives, so selective-sync shares list their
                // files (marked Pending) before any content is pulled.
                if let Ok(db) = rusqlite::Connection::open(&node.db_path) {
                    let is_received: bool = db
                        .query_row(
                            "SELECT COUNT(*) FROM shares WHERE id = ?1 AND is_owner = 0",
                            rusqlite::params![share_id],
                            |row| row.get::<_, i64>(0),
                        )
                        .map(|n| n > 0)
                        .unwrap_or(false);
                    if is_received {
                        for file in files {
                            let kind = FileKind::from_extension(
                                Path::new(&file.relative_path)
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or(""),
                            );
                            shares::upsert_pending_file_entry(
                                &db,
                                &share_id,
                                &file.relative_path,
                                file.size_bytes,
                                file.modified_at,
                                Some(file.content_hash.as_str()),
                                kind,
                            );
                        }
                    }
                }
            }
            SyncMessage::ShareGrant {
                share_id,
                display_name,
                selective_sync,
            } => {
                if let Ok(share_db) = rusqlite::Connection::open(&node.db_path) {
                    let _ = shares::ensure_receiver_share(
                        &share_db,
                        &node.shared_root,
                        &share_id,
                        &display_name,
                        selective_sync,
                        peer_node_id,
                    );
                }
                // The share is now visible locally, so the UI can refresh.
                let _ = node
                    .event_tx
                    .send(NetworkEvent::ShareGranted { share_id });
            }
            SyncMessage::RequestFile { share_id, path } => {
                // Pull-mode selective sync ("fetch on open"): the OWNER serves
                // an individual file when the requester holds at least view
                // permission on the share.
                let deny = |reason: String| SyncMessage::PermissionDenied { reason };
                let db = match rusqlite::Connection::open(&node.db_path) {
                    Ok(db) => db,
                    Err(_) => {
                        let _ = send_control(
                            &conn,
                            &deny("local database unavailable".to_string()),
                        )
                        .await;
                        continue;
                    }
                };
                let level = peers::get_permission_by_node(&db, &share_id, peer_node_id);
                let is_owner = db
                    .query_row(
                        "SELECT is_owner FROM shares WHERE id = ?1",
                        rusqlite::params![share_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .map(|v| v == 1)
                    .unwrap_or(false);
                if level == PermissionLevel::None || !is_owner {
                    let _ = send_control(
                        &conn,
                        &deny("files are only served by the share owner to granted peers".to_string()),
                    )
                    .await;
                    continue;
                }
                if !is_safe_relative_path(&path) {
                    let _ = send_control(&conn, &deny("unsafe file path rejected".to_string())).await;
                    continue;
                }
                let owner_folder: Option<String> = db
                    .query_row(
                        "SELECT path FROM shares WHERE id = ?1",
                        rusqlite::params![share_id],
                        |row| row.get(0),
                    )
                    .ok();
                let Some(owner_folder) = owner_folder else {
                    let _ = send_control(&conn, &deny("share not found".to_string())).await;
                    continue;
                };
                let full_path = std::path::Path::new(&owner_folder).join(&path);
                // Hash the current content so the receiver can verify the pull.
                let Ok(hash) = conflicts::hash_file(&full_path) else {
                    let _ = send_control(&conn, &deny("file unreadable".to_string())).await;
                    continue;
                };
                if let Err(err) =
                    send_file(&conn, &shared, &share_id, &path, &hash, &full_path).await
                {
                    let _ = send_control(&conn, &deny(format!("cannot serve file: {err}"))).await;
                }
            }
            SyncMessage::ResumeQuery {
                share_id,
                path,
                hash,
            } => {
                // How much of exactly this content version does the receiver
                // already hold? 0 when the on-disk part file is missing or
                // belongs to a different version of the file.
                let have_bytes =
                    resume_part_bytes(&node.shared_root, &share_id, &path, &hash);
                let _ = send_control(
                    &conn,
                    &SyncMessage::ResumeInfo {
                        share_id,
                        path,
                        have_bytes,
                    },
                )
                .await;
            }
            SyncMessage::ResumeInfo {
                share_id,
                path,
                have_bytes,
            } => {
                let key = format!("{share_id}:{path}");
                if let Some(tx) = shared.pending_resume.lock().await.remove(&key) {
                    let _ = tx.send(have_bytes);
                }
            }
            SyncMessage::PeerRemoved {
                remover_node_id,
                removed_node_id,
            } => {
                apply_peer_removal(node, &remover_node_id, &removed_node_id);
            }
            _ => {}
        }
    }
}

/// Pusher: for each share this device owns that the peer is granted, refresh
/// the local index, exchange file indexes, and transfer files the peer is
/// missing or whose content hash differs.
async fn push_owned_shares(
    conn: &iroh::endpoint::Connection,
    node: &Arc<IrohNode>,
    shared: &Arc<SharedCtx>,
    peer_node_id: &str,
) -> Result<()> {
    // Phase 1 (synchronous): which shares do we own? The rusqlite Connection
    // is not Sync, so it must never be held across an await point.
    let owned = {
        let db = rusqlite::Connection::open(&node.db_path)?;
        shares::list_owned_shares(&db)?
    };

    for share in owned {
        // Phase 2 (synchronous): permission check, fresh index, local listing.
        let local = {
            let db = rusqlite::Connection::open(&node.db_path)?;
            if peers::get_permission_by_node(&db, &share.id, peer_node_id)
                == PermissionLevel::None
            {
                continue;
            }
            // Re-index so sizes and hashes in the diff are current.
            shares::index_share_files(&db, &share.id, &share.path).ok();
            shares::list_files_in_share(&db, &share.id).unwrap_or_default()
        };

        // Selective-sync shares are metadata-only: the peer sees the file list
        // and pulls individual files when it opens them (technical spec §12).
        if share.selective_sync {
            send_control(
                conn,
                &SyncMessage::ShareGrant {
                    share_id: share.id.clone(),
                    display_name: share.display_name.clone(),
                    selective_sync: true,
                },
            )
            .await?;
            let (tx, rx) = oneshot::channel();
            shared
                .pending_index
                .lock()
                .await
                .insert(share.id.clone(), tx);
            send_control(
                conn,
                &SyncMessage::FileIndexRequest {
                    share_id: share.id.clone(),
                },
            )
            .await?;
            // The reply is recorded receiver-side as Pending metadata rows;
            // no content is pushed for selective shares.
            let _: Vec<IndexedFile> = timeout(INDEX_TIMEOUT, rx)
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();
            continue;
        }

        // Grant visibility + ask the peer for its local index so we only
        // transfer what is actually missing or changed.
        let transfer: Result<()> = async {
            send_control(
                conn,
                &SyncMessage::ShareGrant {
                    share_id: share.id.clone(),
                    display_name: share.display_name.clone(),
                    selective_sync: share.selective_sync,
                },
            )
            .await?;

            let (tx, rx) = oneshot::channel();
            shared
                .pending_index
                .lock()
                .await
                .insert(share.id.clone(), tx);
            send_control(
                conn,
                &SyncMessage::FileIndexRequest {
                    share_id: share.id.clone(),
                },
            )
            .await?;
            let remote: Vec<IndexedFile> = timeout(INDEX_TIMEOUT, rx)
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();

            for file in local {
                let Some(hash) = file.content_hash.filter(|h| !h.is_empty()) else {
                    continue;
                };
                let already_have = remote.iter().any(|r| {
                    r.relative_path == file.relative_path && r.content_hash == hash
                });
                if already_have {
                    continue;
                }

                let full_path = Path::new(&share.path).join(&file.relative_path);
                if let Err(err) =
                    send_file(conn, shared, &share.id, &file.relative_path, &hash, &full_path).await
                {
                    emit_sync_error(node, &file.relative_path, format!("send failed: {err}"));
                    continue;
                }

                let _ = node.event_tx.send(NetworkEvent::SyncComplete {
                    share_id: share.id.clone(),
                    path: file.relative_path.clone(),
                });
            }
            Ok(())
        }
        .await;
        transfer?;
    }
    Ok(())
}

/// Serve a FileIndexRequest from the local file_index rows, if any.
fn current_local_index(db_path: &Path, share_id: &str) -> Vec<IndexedFile> {
    let Ok(conn) = rusqlite::Connection::open(db_path) else {
        return Vec::new();
    };
    shares::list_files_in_share(&conn, share_id)
        .map(|files| {
            files
                .into_iter()
                .filter_map(|f| {
                    Some(IndexedFile {
                        relative_path: f.relative_path,
                        size_bytes: f.size_bytes,
                        content_hash: f.content_hash?,
                        modified_at: f.modified_at,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
/// Reject wire-supplied relative paths that could escape the share folder:
/// traversal segments, absolute paths, and Windows drive prefixes.
pub(crate) fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && path.split(['/', '\\']).all(|seg| seg != "..")
}

/// Sidecar recording which content version a partial transfer belongs to, so
/// a resume query never appends bytes from a different version of the file.
fn part_hash_sidecar(part_path: &Path) -> std::path::PathBuf {
    let mut sidecar = part_path.as_os_str().to_os_string();
    sidecar.push(".hash");
    std::path::PathBuf::from(sidecar)
}

fn remove_part_hash_sidecar(part_path: &Path) {
    std::fs::remove_file(part_hash_sidecar(part_path)).ok();
}

/// Bytes of a partial transfer for exactly this content version, or 0 when
/// nothing usable is on disk (missing part, or a part from another version).
fn resume_part_bytes(shared_root: &Path, share_id: &str, relative_path: &str, hash: &str) -> u64 {
    let target = shared_root.join(share_id).join(relative_path);
    let Some(parent) = target.parent() else {
        return 0;
    };
    let Some(file_name) = target.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return 0;
    };
    let part = parent.join(format!("{file_name}.soffit-part"));
    let sidecar = parent.join(format!("{file_name}.soffit-part.hash"));
    if !part.exists() || !sidecar.exists() {
        return 0;
    }
    let Ok(recorded) = std::fs::read_to_string(&sidecar) else {
        return 0;
    };
    if recorded.trim() != hash {
        return 0;
    }
    std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0)
}

/// A peer removed someone from the swarm. If we were removed, forget the
/// remover too; otherwise forget the removed device. Access is revoked by
/// deleting the peer row and every permission that referenced it (modules §1).
fn apply_peer_removal(node: &Arc<IrohNode>, remover_node_id: &str, removed_node_id: &str) {
    let Ok(db) = rusqlite::Connection::open(&node.db_path) else {
        return;
    };
    let (drop_node, action) = if removed_node_id == node.node_id.as_str() {
        (remover_node_id.to_string(), "removed_by_peer")
    } else {
        (removed_node_id.to_string(), "peer_removed")
    };
    let Ok(peer_row_id) = db.query_row(
        "SELECT id FROM peers WHERE node_id = ?1",
        rusqlite::params![drop_node],
        |row| row.get::<_, String>(0),
    ) else {
        return;
    };
    let _ = db.execute(
        "DELETE FROM share_permissions WHERE peer_id = ?1",
        rusqlite::params![peer_row_id],
    );
    let _ = db.execute(
        "DELETE FROM peers WHERE id = ?1",
        rusqlite::params![peer_row_id],
    );
    activity::log_activity(&db, "system", action, &drop_node, None).ok();
    let _ = node.event_tx.send(NetworkEvent::PeerOffline { node_id: drop_node });
}

/// Pull one file from its owner on demand (pull-mode selective sync). The
/// file lands in the local share folder; completion is awaited so an "open"
/// action can continue once the content is on disk.
pub async fn fetch_remote_file(
    node: &Arc<IrohNode>,
    owner_node_id: &str,
    share_id: &str,
    relative_path: &str,
) -> Result<()> {
    let (conn, shared) = {
        let registry = node.conns.lock().expect("connection registry poisoned");
        match registry.get(owner_node_id) {
            Some((conn, shared)) => (conn.clone(), shared.clone()),
            None => bail!("Peer is not connected right now — open the Peers screen to reconnect"),
        }
    };
    let key = format!("{share_id}:{relative_path}");
    let (tx, rx) = oneshot::channel();
    shared.pending_fetch.lock().await.insert(key.clone(), tx);
    if let Err(err) = send_control(
        &conn,
        &SyncMessage::RequestFile {
            share_id: share_id.to_string(),
            path: relative_path.to_string(),
        },
    )
    .await
    {
        shared.pending_fetch.lock().await.remove(&key);
        bail!("Could not request the file: {err}");
    }
    timeout(Duration::from_secs(300), rx)
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for the file"))?
        .map_err(|_| anyhow::anyhow!("Transfer was cancelled"))?;
    Ok(())
}

/// Receive one file: stream the payload into a `<name>.soffit-part` temp file
/// (bounded memory, the real target is never partially written), verify the
/// SHA-256 against the advertised hash, then finalize. A nonzero `resume_from`
/// appends to a matching partial instead of starting over (modules §4).
async fn receive_file(
    node: &Arc<IrohNode>,
    peer_node_id: &str,
    share_id: &str,
    path: &str,
    size: u64,
    resume_from: u64,
    hash: &str,
    recv: &mut iroh::endpoint::RecvStream,
) {
    // Defence in depth: the relative path travels over the wire.
    if !is_safe_relative_path(path) {
        emit_sync_error(node, path, "unsafe file path rejected");
        return;
    }

    let Ok(db) = rusqlite::Connection::open(&node.db_path) else {
        emit_sync_error(node, path, "cannot open local database");
        return;
    };

    // Make sure a local share folder exists (covers share grant + file
    // arriving in any order, and shares granted before this app version).
    let Ok(share_path) = shares::ensure_receiver_share(
        &db,
        &node.shared_root,
        share_id,
        share_id,
        false,
        peer_node_id,
    ) else {
        emit_sync_error(node, path, "cannot create local share folder");
        return;
    };
    let target = share_path.join(path);
    let Some(parent) = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .or_else(|| Some(share_path.clone()))
    else {
        emit_sync_error(node, path, "cannot resolve target folder");
        return;
    };
    if std::fs::create_dir_all(&parent).is_err() {
        emit_sync_error(node, path, "cannot create target folder");
        return;
    }
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let part_path = parent.join(format!("{file_name}.soffit-part"));

    // Stream to the part file. An existing partial for exactly this content
    // version is appended to (modules §4 resume); anything else starts over
    // and records which version the partial belongs to.
    let io_ok: Result<()> = async {
        let mut out = if resume_from > 0 {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&part_path)
                .await?
        } else {
            let file = tokio::fs::File::create(&part_path).await?;
            std::fs::write(part_hash_sidecar(&part_path), hash).ok();
            file
        };
        let mut buf = vec![0u8; 64 * 1024];
        let mut remaining = size.saturating_sub(resume_from);
        while remaining > 0 {
            let chunk = buf.len().min(remaining as usize);
            recv.read_exact(&mut buf[..chunk]).await?;
            out.write_all(&buf[..chunk]).await?;
            remaining -= chunk as u64;
        }
        out.flush().await?;
        Ok(())
    }
    .await;

    if let Err(e) = io_ok {
        std::fs::remove_file(&part_path).ok();
        remove_part_hash_sidecar(&part_path);
        emit_sync_error(node, path, format!("transfer failed: {e}"));
        return;
    }
    // Hash the assembled file (existing prefix + new bytes) so a resumed
    // transfer gets the same integrity guarantee as a fresh one.
    let digest = match conflicts::hash_file(&part_path) {
        Ok(d) => d,
        Err(e) => {
            std::fs::remove_file(&part_path).ok();
            remove_part_hash_sidecar(&part_path);
            emit_sync_error(node, path, format!("cannot hash transfer: {e}"));
            return;
        }
    };
    if digest != hash {
        std::fs::remove_file(&part_path).ok();
        remove_part_hash_sidecar(&part_path);
        emit_sync_error(node, path, "hash mismatch after transfer");
        return;
    }

    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let kind = FileKind::from_extension(ext);

    // Conflict detection: when a local file with the same relative path holds
    // different content, decide whether this is a diverged local edit (keep
    // the local copy and surface the conflict) or a replica of an older
    // version of the same file (install the newer, verified copy).
    if target.exists() {
        match conflicts::hash_file(&target) {
            Ok(local_hash) if local_hash == hash => {
                // Same content already on disk; drop the part file and just
                // make sure the index row is current. The marker records that
                // this content came from the peer, so future updates of the
                // file are installed instead of being flagged as conflicts.
                std::fs::remove_file(&part_path).ok();
                remove_part_hash_sidecar(&part_path);
                let _ = shares::upsert_file_entry(
                    &db,
                    share_id,
                    path,
                    size as i64,
                    Some(chrono::Utc::now().to_rfc3339()),
                    hash,
                    kind,
                    SyncStatus::Synced,
                );
                let _ = shares::set_file_synced_hash(&db, share_id, path, hash);
                let _ = node.event_tx.send(NetworkEvent::SyncComplete {
                    share_id: share_id.to_string(),
                    path: path.to_string(),
                });
                return;
            }
            Ok(local_hash) => {
                // A differing hash is only a diverged local edit when it no
                // longer matches the content this device last accepted from a
                // peer (`synced_hash`). When it still matches, the local file is
                // a plain replica of an older version of this share and the
                // verified incoming copy must be installed — otherwise every
                // remote update would be recorded as a conflict and the replica
                // could never converge. A file that was never received (NULL
                // marker) keeps the conservative conflict behaviour.
                let accepted = db
                    .query_row(
                        "SELECT synced_hash FROM file_index
                         WHERE share_id = ?1 AND relative_path = ?2",
                        rusqlite::params![share_id, path],
                        |row| row.get::<_, Option<String>>(0),
                    )
                    .ok()
                    .flatten();
                if accepted.as_deref() != Some(local_hash.as_str()) {
                    // Record the conflict once per divergence, not once per push.
                    let target_str = target.to_string_lossy().to_string();
                    let already_recorded = db
                        .query_row(
                            "SELECT COUNT(*) FROM conflicts
                             WHERE file_path = ?1 AND remote_hash = ?2 AND resolved = 0",
                            rusqlite::params![target_str, hash],
                            |row| row.get::<_, i64>(0),
                        )
                        .map(|count| count > 0)
                        .unwrap_or(false);
                    if !already_recorded {
                        let _ = conflicts::record_conflict(
                            &db,
                            &target_str,
                            &local_hash,
                            hash,
                            peer_node_id,
                        );
                    }
                    let _ = shares::set_file_sync_status(&db, share_id, path, SyncStatus::Conflict);
                    let _ = activity::log_activity(
                        &db,
                        "system",
                        "conflict_detected",
                        path,
                        Some(&format!("share={share_id} peer={peer_node_id}")),
                    );
                    std::fs::remove_file(&part_path).ok();
                    remove_part_hash_sidecar(&part_path);
                    emit_sync_error(node, path, "conflict: local copy differs; local kept");
                    return;
                }
                // Plain replica of the old version: fall through and install.
            }
            Err(_) => {
                // Local copy unreadable — the verified remote copy wins.
            }
        }
    }

    // Install the verified file. The rename replaces the target atomically
    // (or creates it), so the target is never left partially written.
    if let Err(e) = std::fs::rename(&part_path, &target) {
        std::fs::remove_file(&part_path).ok();
        remove_part_hash_sidecar(&part_path);
        emit_sync_error(node, path, format!("install failed: {e}"));
        return;
    }
    remove_part_hash_sidecar(&part_path);
    let modified = std::fs::metadata(&target)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        });
    let _ = shares::upsert_file_entry(
        &db,
        share_id,
        path,
        size as i64,
        modified,
        hash,
        kind,
        SyncStatus::Synced,
    );
    // Record the accepted content so a later remote edit replaces this replica
    // in place rather than being treated as a conflict.
    let _ = shares::set_file_synced_hash(&db, share_id, path, hash);
    activity::log_activity(&db, "system", "file_synced", path, Some(share_id)).ok();
    let _ = node.event_tx.send(NetworkEvent::SyncComplete {
        share_id: share_id.to_string(),
        path: path.to_string(),
    });
}
/// Broadcast a sync error event for the notification bridge / UI.
fn emit_sync_error(node: &IrohNode, path: &str, error: impl std::fmt::Display) {
    let _ = node.event_tx.send(NetworkEvent::SyncError {
        path: path.to_string(),
        error: error.to_string(),
    });
}

/// Send a length-prefixed control message on a fresh uni stream.
pub(crate) async fn send_control(conn: &iroh::endpoint::Connection, msg: &SyncMessage) -> Result<()> {
    let bytes = serde_json::to_vec(msg)?;
    if bytes.is_empty() || bytes.len() > MAX_CONTROL_FRAME {
        bail!("control frame too large: {} bytes", bytes.len());
    }
    let mut send = conn.open_uni().await?;
    send.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    send.write_all(&bytes).await?;
    send.finish()?;
    Ok(())
}

/// Stream one file: a FileInfo control header (carrying `resume_from`), the
/// 8-byte BE payload length, then the raw bytes chunked from disk — the whole
/// file is never buffered in memory. Before sending, the receiver is asked how
/// many bytes of exactly this content version it already holds, so interrupted
/// transfers resume instead of restarting (modules §4).
async fn send_file(
    conn: &iroh::endpoint::Connection,
    shared: &Arc<SharedCtx>,
    share_id: &str,
    relative_path: &str,
    hash: &str,
    full_path: &Path,
) -> Result<()> {
    use tokio::io::AsyncSeekExt;

    // Ask the receiver how much it already holds (best effort — a timeout or
    // missing reply simply means a full transfer).
    let mut resume_from: u64 = 0;
    let key = format!("{share_id}:{relative_path}");
    let (tx, rx) = oneshot::channel();
    shared.pending_resume.lock().await.insert(key.clone(), tx);
    if send_control(
        conn,
        &SyncMessage::ResumeQuery {
            share_id: share_id.to_string(),
            path: relative_path.to_string(),
            hash: hash.to_string(),
        },
    )
    .await
    .is_ok()
    {
        resume_from = timeout(Duration::from_secs(5), rx)
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(0);
    } else {
        shared.pending_resume.lock().await.remove(&key);
    }

    let mut file = tokio::fs::File::open(full_path).await?;
    let size = file.metadata().await?.len();
    if resume_from >= size {
        // The receiver already holds this exact version.
        return Ok(());
    }
    file.seek(std::io::SeekFrom::Start(resume_from)).await?;

    let header = serde_json::to_vec(&SyncMessage::FileInfo {
        share_id: share_id.to_string(),
        path: relative_path.to_string(),
        size,
        hash: hash.to_string(),
        resume_from,
    })?;
    if header.len() > MAX_CONTROL_FRAME {
        bail!("file header too large");
    }
    let mut send = conn.open_uni().await?;
    send.write_all(&(header.len() as u32).to_be_bytes()).await?;
    send.write_all(&header).await?;
    send.write_all(&(size - resume_from).to_be_bytes()).await?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        send.write_all(&buf[..n]).await?;
    }
    send.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_safe_relative_path;

    #[test]
    fn normal_relative_paths_are_accepted() {
        assert!(is_safe_relative_path("budget.xlsx"));
        assert!(is_safe_relative_path("sub/dir/file.txt"));
        assert!(is_safe_relative_path("a/./b/c.sql"));
        assert!(is_safe_relative_path("deep/nested/folder/name.csv"));
    }

    #[test]
    fn traversal_attempts_are_rejected() {
        assert!(!is_safe_relative_path("../escape"));
        assert!(!is_safe_relative_path("a/../../escape"));
        assert!(!is_safe_relative_path("safe/../../../etc/passwd"));
        assert!(!is_safe_relative_path(".."));
    }

    #[test]
    fn absolute_and_windows_paths_are_rejected() {
        assert!(!is_safe_relative_path("/etc/passwd"));
        assert!(!is_safe_relative_path("\\Windows\\evil"));
        assert!(!is_safe_relative_path("C:\\evil\\path"));
        assert!(!is_safe_relative_path("C:/evil/path"));
    }

    #[test]
    fn empty_paths_are_rejected() {
        assert!(!is_safe_relative_path(""));
    }

    /// Full lifecycle test against two real iroh endpoints over real QUIC:
    /// initial push, incremental diff push, and conflict handling.
    #[tokio::test]
    async fn owner_pushes_files_to_a_granted_peer_end_to_end() {
        use crate::models::PermissionLevel;
        use crate::network::IrohNode;
        use sha2::Digest as _;
        use std::time::Duration;

        let base = std::env::temp_dir().join(format!("soffit-e2e-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let owner_folder = base.join("owner-docs");
        std::fs::create_dir_all(&owner_folder).unwrap();
        let receiver_shared_root = base.join("receiver-shared");
        std::fs::create_dir_all(&receiver_shared_root).unwrap();

        let db_a = base.join("owner.sqlite");
        let db_b = base.join("receiver.sqlite");
        for db_path in [&db_a, &db_b] {
            let conn = crate::db::init_db(db_path).unwrap();
            crate::conflicts::ensure_conflicts_table(&conn).unwrap();
        }

        let key_a = crate::network::identity::load_or_create(&base.join("owner.key")).unwrap();
        let key_b = crate::network::identity::load_or_create(&base.join("receiver.key")).unwrap();
        let node_a = IrohNode::start(key_a, db_a.clone(), base.join("a-shared")).await.unwrap();
        let node_b =
            IrohNode::start(key_b, db_b.clone(), receiver_shared_root.clone()).await.unwrap();

        std::fs::write(owner_folder.join("budget.xlsx"), b"v1-content").unwrap();
        std::fs::create_dir_all(owner_folder.join("sub")).unwrap();
        std::fs::write(owner_folder.join("sub/query.sql"), b"SELECT 1;").unwrap();

        let share_id: String = {
            let conn = rusqlite::Connection::open(&db_a).unwrap();
            let share =
                crate::shares::create_share(&conn, owner_folder.to_str().unwrap(), "Docs")
                    .unwrap();
            let addr_json = serde_json::to_string(&node_b.endpoint.addr()).unwrap();
            let peer = crate::peers::add_peer(&conn, &node_b.node_id, "Receiver", "pk-b").unwrap();
            crate::pairing::update_peer_endpoint(&conn, &node_b.node_id, &addr_json).unwrap();
            crate::peers::set_permission(&conn, &share.id, &peer.id, PermissionLevel::Edit)
                .unwrap();
            share.id
        };

        let addr_b = serde_json::to_string(&node_b.endpoint.addr()).unwrap();
        node_a.connect_to_peer(&addr_b, "Receiver").await.unwrap();

        async fn wait_for_file(path: &std::path::Path, expected: &[u8]) {
            for _ in 0..300 {
                if std::fs::read(path).map(|bytes| bytes == expected).unwrap_or(false) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            panic!("timed out waiting for {}", path.display());
        }

        async fn wait_for_count(db_path: &std::path::Path, sql: &str, expected: i64) -> i64 {
            for _ in 0..300 {
                if let Ok(conn) = rusqlite::Connection::open(db_path) {
                    if let Ok(count) = conn.query_row(sql, [], |row| row.get::<_, i64>(0)) {
                        if count == expected {
                            return count;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            panic!("timed out waiting for count {expected}: {sql}");
        }

        let b_share_dir = receiver_shared_root.join(&share_id);

        // Phase 1: initial push transfers the whole folder, nested dirs included.
        wait_for_file(&b_share_dir.join("budget.xlsx"), b"v1-content").await;
        wait_for_file(&b_share_dir.join("sub/query.sql"), b"SELECT 1;").await;
        {
            let conn = rusqlite::Connection::open(&db_b).unwrap();
            let is_owner: i64 = conn
                .query_row(
                    "SELECT is_owner FROM shares WHERE id = ?1",
                    rusqlite::params![share_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(is_owner, 0, "receiver share record must not be an owner");
            let (status, hash): (String, String) = conn
                .query_row(
                    "SELECT sync_status, content_hash FROM file_index
                     WHERE share_id = ?1 AND relative_path = 'budget.xlsx'",
                    rusqlite::params![share_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(status, "synced");
            assert_eq!(hash, format!("{:x}", sha2::Sha256::digest(b"v1-content")));
            let synced_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM file_index WHERE share_id = ?1 AND sync_status = 'synced'",
                    rusqlite::params![share_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(synced_count, 2, "both files synced");
        }

        // Phase 2: a newly added file is pushed; files the peer already has
        // (matching hash) are skipped by the index diff.
        std::fs::write(owner_folder.join("new.txt"), b"added").unwrap();
        {
            let conn = rusqlite::Connection::open(&db_a).unwrap();
            crate::shares::index_share_files(
                &conn,
                &share_id,
                owner_folder.to_str().unwrap(),
            )
            .unwrap();
        }
        node_a.connect_to_peer(&addr_b, "Receiver").await.unwrap();
        wait_for_file(&b_share_dir.join("new.txt"), b"added").await;

        // Phase 2b: the owner now modifies a file the receiver has not touched.
        // The receiver only holds a replica, so the newer verified copy must be
        // installed in place — never recorded as a conflict.
        std::fs::write(owner_folder.join("new.txt"), b"added-v2").unwrap();
        {
            let conn = rusqlite::Connection::open(&db_a).unwrap();
            crate::shares::index_share_files(
                &conn,
                &share_id,
                owner_folder.to_str().unwrap(),
            )
            .unwrap();
        }
        node_a.connect_to_peer(&addr_b, "Receiver").await.unwrap();
        wait_for_file(&b_share_dir.join("new.txt"), b"added-v2").await;
        {
            let conn = rusqlite::Connection::open(&db_b).unwrap();
            let (status, hash): (String, String) = conn
                .query_row(
                    "SELECT sync_status, content_hash FROM file_index
                     WHERE share_id = ?1 AND relative_path = 'new.txt'",
                    rusqlite::params![share_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(status, "synced", "updating a replica is not a conflict");
            assert_eq!(hash, format!("{:x}", sha2::Sha256::digest(b"added-v2")));
            let conflicts: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM conflicts WHERE resolved = 0",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(conflicts, 0, "no conflict for an untouched replica");
        }

        // Phase 3: the receiver edits its local copy; the owner's re-push
        // diverges → conflict recorded, local edit kept.
        std::fs::write(b_share_dir.join("budget.xlsx"), b"local-edit").unwrap();
        {
            let conn = rusqlite::Connection::open(&db_b).unwrap();
            crate::shares::index_share_files(
                &conn,
                &share_id,
                b_share_dir.to_str().unwrap(),
            )
            .unwrap();
        }
        node_a.connect_to_peer(&addr_b, "Receiver").await.unwrap();
        wait_for_count(&db_b, "SELECT COUNT(*) FROM conflicts WHERE resolved = 0", 1).await;
        assert_eq!(
            std::fs::read(b_share_dir.join("budget.xlsx")).unwrap(),
            b"local-edit"
        );
        {
            let conn = rusqlite::Connection::open(&db_b).unwrap();
            let status: String = conn
                .query_row(
                    "SELECT sync_status FROM file_index
                     WHERE share_id = ?1 AND relative_path = 'budget.xlsx'",
                    rusqlite::params![share_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(status, "conflict");
        }

        // Phase 4: repeat the push (with an unrelated new file so completion
        // is observable) — the conflict row must NOT be duplicated.
        std::fs::write(owner_folder.join("new2.txt"), b"added-two").unwrap();
        {
            let conn = rusqlite::Connection::open(&db_a).unwrap();
            crate::shares::index_share_files(
                &conn,
                &share_id,
                owner_folder.to_str().unwrap(),
            )
            .unwrap();
        }
        node_a.connect_to_peer(&addr_b, "Receiver").await.unwrap();
        wait_for_file(&b_share_dir.join("new2.txt"), b"added-two").await;
        // The conflicting file was re-attempted during this push; give the
        // session a moment, then verify suppression.
        tokio::time::sleep(Duration::from_secs(2)).await;
        let conflicts = wait_for_count(
            &db_b,
            "SELECT COUNT(*) FROM conflicts WHERE resolved = 0",
            1,
        )
        .await;
        assert_eq!(conflicts, 1, "conflict rows must not duplicate");

        drop(node_a);
        drop(node_b);
        std::fs::remove_dir_all(&base).ok();
    }
}