use crate::{
    activity,
    ai::{
        identity_guard,
        model_manager::{AiStatus, ModelManager},
        prompts,
    },
    auth, excel, locks,
    models::*,
    pairing, peers, search, shares,
    sql::engine::SqlEngine,
    versions, AppState,
};
use base64::Engine as _;
use tauri::Emitter;
use tauri::State;

type CmdResult<T> = Result<T, String>;
fn e(err: impl std::fmt::Display) -> String {
    err.to_string()
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_user_exists(state: State<'_, AppState>) -> CmdResult<bool> {
    let db = state.db.lock().map_err(e)?;
    auth::user_exists(&db).map_err(e)
}

#[tauri::command]
pub fn cmd_create_user(
    username: String,
    password: String,
    state: State<'_, AppState>,
) -> CmdResult<User> {
    let db = state.db.lock().map_err(e)?;
    let user = auth::create_user(&db, &username, &password).map_err(e)?;
    activity::log_activity(&db, &user.username, "account_created", "local", None).ok();
    Ok(user)
}

#[tauri::command]
pub fn cmd_login(
    username: String,
    password: String,
    state: State<'_, AppState>,
) -> CmdResult<User> {
    let db = state.db.lock().map_err(e)?;
    let user = auth::verify_user(&db, &username, &password).map_err(e)?;
    activity::log_activity(&db, &user.username, "login", "local", None).ok();
    Ok(user)
}

#[tauri::command]
pub fn cmd_list_users(state: State<'_, AppState>) -> CmdResult<Vec<User>> {
    let db = state.db.lock().map_err(e)?;
    auth::list_users(&db).map_err(e)
}

#[tauri::command]
pub fn cmd_update_user(
    user_id: String,
    username: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> CmdResult<User> {
    let db = state.db.lock().map_err(e)?;
    let user = auth::update_user(&db, &user_id, &username, password.as_deref()).map_err(e)?;
    activity::log_activity(&db, &user.username, "account_updated", "local", None).ok();
    Ok(user)
}

// ── Pairing ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_generate_pairing_code(state: State<'_, AppState>) -> CmdResult<PairingCodeWithQr> {
    let db = state.db.lock().map_err(e)?;

    // Get node addr ticket synchronously from the stored node_id
    let node_id = state
        .iroh_node
        .try_lock()
        .ok()
        .and_then(|g| g.as_ref().map(|n| n.node_id.clone()));

    // Get the full EndpointAddr for embedding in the code
    let endpoint_addr_json = state
        .iroh_node
        .try_lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|n| {
                let addr = n.endpoint.addr();
                serde_json::to_string(&addr).ok()
            })
        })
        .flatten();

    let code = pairing::generate_pairing_code(&db, endpoint_addr_json.as_deref()).map_err(e)?;

    // QR encodes the full code (including embedded addr)
    let qr_b64 = generate_qr_base64(&code.code).unwrap_or_default();

    // Extract just the short part for display (XXXX-XXXX)
    let short_display = code
        .code
        .split(':')
        .next()
        .unwrap_or(&code.code)
        .to_string();

    Ok(PairingCodeWithQr {
        id: code.id,
        code: code.code.clone(),
        short_code: short_display,
        created_at: code.created_at,
        expires_at: code.expires_at,
        qr_base64: qr_b64,
        node_id,
    })
}

#[tauri::command]
pub fn cmd_consume_pairing_code(
    code: String,
    display_name: String,
    state: State<'_, AppState>,
) -> CmdResult<Option<String>> {
    // Returns the embedded endpoint addr if present.  The DB lock must be
    // released before dialing because a connection may take a moment.
    let endpoint_addr = {
        let db = state.db.lock().map_err(e)?;
        pairing::consume_pairing_code(&db, &code).map_err(e)?
    };

    // If we got an endpoint addr, connect to that peer immediately.
    if let Some(ref addr_json) = endpoint_addr {
        let remote_node_id = pairing::endpoint_node_id(addr_json).map_err(e)?;
        let local_display_name = {
            let db = state.db.lock().map_err(e)?;
            db.query_row(
                "SELECT username FROM users ORDER BY created_at ASC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "This device".to_string())
        };
        let node = state
            .iroh_node
            .lock()
            .map_err(e)?
            .as_ref()
            .cloned()
            .ok_or_else(|| "Networking is still starting; try again in a moment.".to_string())?;

        // Do not report success until the authenticated iroh handshake and
        // Hello message have actually completed.
        state
            .rt
            .block_on(node.connect_to_peer(addr_json, &local_display_name))
            .map_err(e)?;

        let db = state.db.lock().map_err(e)?;
        let peer =
            peers::add_peer(&db, &remote_node_id, &display_name, &remote_node_id).map_err(e)?;
        pairing::update_peer_endpoint(&db, &remote_node_id, addr_json).map_err(e)?;
        activity::log_activity(&db, "system", "peer_added", &peer.display_name, None).ok();
    }

    Ok(endpoint_addr)
}

#[tauri::command]
pub fn cmd_add_peer(
    node_id: String,
    display_name: String,
    public_key: String,
    endpoint_addr: Option<String>,
    state: State<'_, AppState>,
) -> CmdResult<Peer> {
    let db = state.db.lock().map_err(e)?;
    let peer = peers::add_peer(&db, &node_id, &display_name, &public_key).map_err(e)?;
    // Store endpoint addr if provided
    if let Some(ref addr) = endpoint_addr {
        pairing::update_peer_endpoint(&db, &node_id, addr).ok();
    }
    activity::log_activity(&db, "system", "peer_added", &display_name, None).ok();
    Ok(peer)
}

/// Reconnect to all previously paired peers using stored endpoint addrs.
/// Call this on app startup for persistent long-lived sessions.
#[tauri::command]
pub fn cmd_reconnect_peers(state: State<'_, AppState>) -> CmdResult<usize> {
    let db = state.db.lock().map_err(e)?;
    let reconnectable = pairing::get_reconnectable_peers(&db).map_err(e)?;
    let local_display_name = db
        .query_row(
            "SELECT username FROM users ORDER BY created_at ASC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "This device".to_string());
    let count = reconnectable.len();

    if let Ok(node_guard) = state.iroh_node.try_lock() {
        if let Some(node) = node_guard.as_ref() {
            for (node_id, addr_json) in reconnectable {
                let node = node.clone();
                let name = local_display_name.clone();
                state.rt.spawn(async move {
                    match node.connect_to_peer(&addr_json, &name).await {
                        Ok(_) => eprintln!("[reconnect] connected to {}", node_id),
                        Err(e) => eprintln!("[reconnect] failed {}: {}", node_id, e),
                    }
                });
            }
        }
    }
    Ok(count)
}

// ── Sync triggers ─────────────────────────────────────────────────────────────

fn local_display_name(state: &AppState) -> String {
    state
        .db
        .lock()
        .ok()
        .and_then(|db| {
            db.query_row(
                "SELECT username FROM users ORDER BY created_at ASC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
        })
        .unwrap_or_else(|| "This device".to_string())
}

fn trigger_pushes_for_share(state: &AppState, share_id: &str) {
    // Dial every granted peer that has a stored endpoint address and run a
    // sync session. Session de-dup inside the sync engine makes concurrent
    // pushes of the same share to the same peer a no-op.
    let Some(node) = state.iroh_node.lock().ok().and_then(|g| g.clone()) else {
        return;
    };
    let granted = state
        .db
        .lock()
        .ok()
        .and_then(|db| peers::list_granted_peer_addrs(&db, share_id).ok());
    let Some(granted) = granted else { return };
    let local_name = local_display_name(state);
    for (_peer_name, addr) in granted {
        let node = node.clone();
        let local_name = local_name.clone();
        state.rt.spawn(async move {
            let _ = node.connect_to_peer(&addr, &local_name).await;
        });
    }
}

// ── Peers ─────────────────────────────────────────────────────────────────────
// ── Peers ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_list_peers(state: State<'_, AppState>) -> CmdResult<Vec<Peer>> {
    let db = state.db.lock().map_err(e)?;
    peers::list_peers(&db).map_err(e)
}

#[tauri::command]
pub fn cmd_remove_peer(peer_id: String, state: State<'_, AppState>) -> CmdResult<()> {
    let node_id: Option<String> = {
        let db = state.db.lock().map_err(e)?;
        let node_id: Option<String> = db
            .query_row(
                "SELECT node_id FROM peers WHERE id = ?1",
                rusqlite::params![peer_id],
                |row| row.get(0),
            )
            .ok();
        peers::remove_peer(&db, &peer_id).map_err(e)?;
        activity::log_activity(&db, "system", "peer_removed", &peer_id, None).ok();
        node_id
    };
    // Tell the gossip swarm so the removed device (and every other member)
    // forgets the pairing — modules §1: un-pairing revokes access everywhere
    // and is visible on the removed side.
    if let Some(node_id) = node_id {
        if let Ok(node_guard) = state.iroh_node.try_lock() {
            if let Some(node) = node_guard.as_ref() {
                let node = node.clone();
                state.rt.spawn(async move {
                    node.broadcast_peer_removed(&node_id).await;
                });
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_rename_peer(
    peer_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    peers::rename_peer(&db, &peer_id, &new_name).map_err(e)
}

#[tauri::command]
pub fn cmd_set_permission(
    share_id: String,
    peer_id: String,
    level: String,
    state: State<'_, AppState>,
) -> CmdResult<SharePermission> {
    let perm = {
        let db = state.db.lock().map_err(e)?;
        let perm = peers::set_permission(
            &db,
            &share_id,
            &peer_id,
            PermissionLevel::from(level.as_str()),
        )
        .map_err(e)?;
        activity::log_activity(
            &db,
            "system",
            "permission_changed",
            &share_id,
            Some(&format!("peer={peer_id} level={level}")),
        )
        .ok();
        perm
    };
    if level != "none" {
        trigger_pushes_for_share(&state, &share_id);
    }
    Ok(perm)
}
/// Current permission rows for one peer (drives the Peers screen dropdowns).
#[tauri::command]
pub fn cmd_get_peer_permissions(
    peer_id: String,
    state: State<'_, AppState>,
) -> CmdResult<Vec<SharePermission>> {
    let db = state.db.lock().map_err(e)?;
    peers::list_permissions_for_peer(&db, &peer_id).map_err(e)
}

// ── Shares ────────────────────────────────────────────────────────────────────

/// Rename (or move) a file inside an owned share: the disk file, the sync
/// index, the FTS row, stored embeddings, and version history all follow the
/// new path (modules §3: the file browser renames files).
#[tauri::command]
pub fn cmd_rename_file(
    share_id: String,
    old_path: String,
    new_path: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    let (is_owner, share_root): (i64, String) = db
        .query_row(
            "SELECT is_owner, path FROM shares WHERE id = ?1",
            rusqlite::params![share_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| e(anyhow::anyhow!("Share not found")))?;
    if is_owner != 1 {
        return Err("Only the device that owns this share can rename its files".to_string());
    }
    if !crate::network::sync::is_safe_relative_path(&new_path) {
        return Err("Unsafe file path".to_string());
    }
    let from = std::path::Path::new(&share_root).join(&old_path);
    let to = std::path::Path::new(&share_root).join(&new_path);
    if !from.exists() {
        return Err("File not found on disk".to_string());
    }
    if to.exists() {
        return Err("A file with that name already exists".to_string());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(e)?;
    }
    std::fs::rename(&from, &to).map_err(e)?;
    db.execute(
        "UPDATE file_index SET relative_path = ?1 WHERE share_id = ?2 AND relative_path = ?3",
        rusqlite::params![new_path, share_id, old_path],
    )
    .map_err(e)?;
    db.execute(
        "UPDATE file_fts SET file_path = ?1 WHERE share_id = ?2 AND file_path = ?3",
        rusqlite::params![new_path, share_id, old_path],
    )
    .map_err(e)?;
    db.execute(
        "UPDATE file_embeddings SET relative_path = ?1 WHERE share_id = ?2 AND relative_path = ?3",
        rusqlite::params![new_path, share_id, old_path],
    )
    .map_err(e)?;
    db.execute(
        "UPDATE versions SET file_path = ?1 WHERE file_path = ?2",
        rusqlite::params![to.to_string_lossy().to_string(), from.to_string_lossy().to_string()],
    )
    .map_err(e)?;
    activity::log_activity(&db, "system", "file_renamed", &new_path, Some(&old_path)).ok();
    Ok(())
}

/// Selective-sync "fetch on open" (technical spec §12): make sure a file from
/// a received share exists locally, pulling it from the owner over the live
/// connection when it is metadata-only. Returns true when a pull happened.
#[tauri::command]
pub async fn cmd_ensure_local_file(
    share_id: String,
    path: String,
    state: tauri::State<'_, AppState>,
) -> CmdResult<bool> {
    let (receiver_root, owner_node_id) = {
        let db = state.db.lock().map_err(e)?;
        db.query_row(
            "SELECT path, owner_node_id FROM shares WHERE id = ?1 AND is_owner = 0",
            rusqlite::params![share_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|_| e(anyhow::anyhow!("This file belongs to a share you receive, not one you own")))?
    };
    let Some(owner_node_id) = owner_node_id else {
        return Err(
            "This share predates pull support — ask the owner to refresh the share".to_string(),
        );
    };
    let target = std::path::Path::new(&receiver_root).join(&path);
    if target.exists() {
        return Ok(false);
    }
    let node = state
        .iroh_node
        .lock()
        .map_err(e)?
        .clone()
        .ok_or_else(|| e(anyhow::anyhow!("Network is not running yet")))?;
    let rt = state.rt.clone();
    drop(state);
    rt.spawn(async move {
        crate::network::sync::fetch_remote_file(&node, &owner_node_id, &share_id, &path).await
    })
    .await
                .map_err(|je| e(anyhow::anyhow!(je.to_string())))?
    .map_err(e)?;
    Ok(true)
}

/// The app-data directory that owns the SQLite database and models folder.
fn data_dir_from_state(
    state: &tauri::State<'_, AppState>,
) -> Result<std::path::PathBuf, String> {
    Ok(state
        .versions_dir
        .lock()
        .map_err(e)?
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf())
}

/// Semantic search (AI spec §6.5): rank indexed files by embedding similarity
/// to the query. Requires the embedding model; returns [] until it is ready.
#[derive(serde::Serialize)]
pub struct SemanticSearchHit {
    pub file_path: String,
    pub share_id: String,
    pub score: f32,
}

#[tauri::command]
pub async fn cmd_semantic_search(
    query: String,
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> CmdResult<Vec<SemanticSearchHit>> {
    let limit = limit.unwrap_or(12);
    let data_dir = data_dir_from_state(&state)?;
    let db_path = data_dir.join("soffit-commit.sqlite");
    drop(state);
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<SemanticSearchHit>> {
        let model_path = ModelManager::new(&data_dir).embed_model_path();
        if !model_path.exists() {
            return Ok(Vec::new());
        }
        let conn = rusqlite::Connection::open(&db_path)?;
        let query_vector = crate::ai::embeddings::embed_text(&model_path, &query)?;
        let rows = search::all_embeddings(&conn)?;
        let mut candidates: Vec<(usize, f32)> = rows
            .iter()
            .enumerate()
            .map(|(idx, (_, _, vector))| {
                (
                    idx,
                    crate::ai::embeddings::cosine_similarity(&query_vector, vector),
                )
            })
            .filter(|(_, score)| *score >= 0.35)
            .collect();
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(candidates
            .into_iter()
            .take(limit)
            .filter_map(|(idx, score)| {
                let (share_id, rel, _) = rows.get(idx)?;
                Some(SemanticSearchHit {
                    file_path: rel.clone(),
                    share_id: share_id.clone(),
                    score,
                })
            })
            .collect())
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    Ok(result)
}

#[tauri::command]
pub fn cmd_create_share(
    path: String,
    display_name: String,
    state: State<'_, AppState>,
) -> CmdResult<Share> {
    let share = {
        let db = state.db.lock().map_err(e)?;
        let share = shares::create_share(&db, &path, &display_name).map_err(e)?;
        activity::log_activity(&db, "system", "share_created", &display_name, None).ok();
        share
    };
    trigger_pushes_for_share(&state, &share.id);
    Ok(share)
}

#[tauri::command]
pub fn cmd_list_shares(state: State<'_, AppState>) -> CmdResult<Vec<Share>> {
    let db = state.db.lock().map_err(e)?;
    shares::list_shares(&db).map_err(e)
}

#[tauri::command]
pub fn cmd_delete_share(share_id: String, state: State<'_, AppState>) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    shares::delete_share(&db, &share_id).map_err(e)?;
    crate::search::remove_share(&db, &share_id).ok();
    activity::log_activity(&db, "system", "share_deleted", &share_id, None).ok();
    Ok(())
}

#[tauri::command]
pub fn cmd_list_files(share_id: String, state: State<'_, AppState>) -> CmdResult<Vec<FileIndex>> {
    let db = state.db.lock().map_err(e)?;
    shares::list_files_in_share(&db, &share_id).map_err(e)
}

#[tauri::command]
pub fn cmd_refresh_share(share_id: String, state: State<'_, AppState>) -> CmdResult<()> {
    {
        let db = state.db.lock().map_err(e)?;
        let share_path = db
            .query_row(
                "SELECT path FROM shares WHERE id = ?1",
                rusqlite::params![share_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(e)?;
        shares::index_share_files(&db, &share_id, &share_path).map_err(e)?;
    }
    trigger_pushes_for_share(&state, &share_id);
    Ok(())
}

#[tauri::command]
pub fn cmd_set_selective_sync(
    share_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    shares::set_selective_sync(&db, &share_id, enabled).map_err(e)
}

// ── Locks ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_acquire_lock(
    file_path: String,
    peer_id: String,
    peer_name: String,
    state: State<'_, AppState>,
) -> CmdResult<Lock> {
    let db = state.db.lock().map_err(e)?;
    let lock = locks::acquire_lock(&db, &file_path, &peer_id, &peer_name).map_err(e)?;
    activity::log_activity(&db, &peer_name, "file_locked", &file_path, None).ok();
    // Broadcast via gossip using the app's tokio runtime
    if let Ok(node_guard) = state.iroh_node.try_lock() {
        if let Some(node) = node_guard.as_ref() {
            let node = node.clone();
            let fp = file_path.clone();
            let pn = peer_name.clone();
            state.rt.spawn(async move {
                node.broadcast_lock(&fp, &pn).await.ok();
            });
        }
    }
    Ok(lock)
}

#[tauri::command]
pub fn cmd_release_lock(
    file_path: String,
    peer_id: String,
    state: State<'_, AppState>,
) -> CmdResult<bool> {
    let db = state.db.lock().map_err(e)?;
    let released = locks::release_lock(&db, &file_path, &peer_id).map_err(e)?;
    if released {
        activity::log_activity(&db, &peer_id, "file_unlocked", &file_path, None).ok();
        if let Ok(node_guard) = state.iroh_node.try_lock() {
            if let Some(node) = node_guard.as_ref() {
                let node = node.clone();
                let fp = file_path.clone();
                state.rt.spawn(async move {
                    node.broadcast_unlock(&fp).await.ok();
                });
            }
        }
    }
    Ok(released)
}

#[tauri::command]
pub fn cmd_get_lock(file_path: String, state: State<'_, AppState>) -> CmdResult<Option<Lock>> {
    let db = state.db.lock().map_err(e)?;
    locks::get_lock(&db, &file_path).map_err(e)
}

#[tauri::command]
pub fn cmd_list_active_locks(state: State<'_, AppState>) -> CmdResult<Vec<Lock>> {
    let db = state.db.lock().map_err(e)?;
    locks::list_active_locks(&db).map_err(e)
}

// ── Activity ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_list_activity(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> CmdResult<Vec<ActivityEntry>> {
    let db = state.db.lock().map_err(e)?;
    activity::list_activity(&db, limit.unwrap_or(50)).map_err(e)
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_dashboard_stats(state: State<'_, AppState>) -> CmdResult<DashboardStats> {
    let db = state.db.lock().map_err(e)?;

    let storage_used_bytes: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(size_bytes),0) FROM file_index",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let files_synced: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM file_index WHERE sync_status='synced'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let now = chrono::Utc::now();
    let week_start = (now - chrono::Duration::days(7)).to_rfc3339();
    let two_weeks_start = (now - chrono::Duration::days(14)).to_rfc3339();
    let month_start = format!(
        "{}-{:02}-01T00:00:00Z",
        now.format("%Y"),
        now.format("%m").to_string().parse::<u32>().unwrap_or(1)
    );

    let files_edited_this_month: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM activity_log WHERE action='file_saved' AND timestamp>=?1",
            rusqlite::params![month_start],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let conflicts_resolved: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM activity_log WHERE action='conflict_resolved'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Week-over-week deltas
    let files_this_week: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM activity_log WHERE action='file_saved' AND timestamp>=?1",
            rusqlite::params![week_start],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let files_last_week: i64 = db.query_row(
        "SELECT COUNT(*) FROM activity_log WHERE action='file_saved' AND timestamp>=?1 AND timestamp<?2",
        rusqlite::params![two_weeks_start, week_start], |r| r.get(0)).unwrap_or(0);

    let conflicts_this_week: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM activity_log WHERE action='conflict_resolved' AND timestamp>=?1",
            rusqlite::params![week_start],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let conflicts_last_week: i64 = db.query_row(
        "SELECT COUNT(*) FROM activity_log WHERE action='conflict_resolved' AND timestamp>=?1 AND timestamp<?2",
        rusqlite::params![two_weeks_start, week_start], |r| r.get(0)).unwrap_or(0);

    let pct_delta = |cur: i64, prev: i64| -> i64 {
        if prev == 0 {
            if cur > 0 {
                100
            } else {
                0
            }
        } else {
            (cur - prev) * 100 / prev
        }
    };

    let files_edited_delta_pct = pct_delta(files_this_week, files_last_week);
    let conflicts_delta_pct = pct_delta(conflicts_this_week, conflicts_last_week);
    let total_files: i64 = db
        .query_row("SELECT COUNT(*) FROM file_index", [], |r| r.get(0))
        .unwrap_or(0)
        .max(1);
    let sync_health_percent = (files_synced as f64 / total_files as f64) * 100.0;

    let excel_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM file_index WHERE file_kind='excel'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let sql_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM file_index WHERE file_kind='sql'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let online_peers: i64 = db
        .query_row("SELECT COUNT(*) FROM peers WHERE is_online=1", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let total_peers: i64 = db
        .query_row("SELECT COUNT(*) FROM peers", [], |r| r.get(0))
        .unwrap_or(0);

    Ok(DashboardStats {
        storage_used_bytes,
        files_synced,
        files_edited_this_month,
        conflicts_resolved,
        sync_health_percent,
        file_type_breakdown: FileTypeBreakdown {
            excel: excel_count,
            sql: sql_count,
            other: (total_files - excel_count - sql_count).max(0),
        },
        online_peers,
        total_peers,
        storage_delta_pct: 0, // would need snapshot history to compute
        files_synced_delta_pct: 0,
        files_edited_delta_pct,
        conflicts_delta_pct,
    })
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_get_setting(key: String, state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let db = state.db.lock().map_err(e)?;
    match db.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        rusqlite::params![key],
        |row| row.get(0),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub fn cmd_set_setting(key: String, value: String, state: State<'_, AppState>) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    db.execute(
        "INSERT INTO app_settings(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=?2",
        rusqlite::params![key, value],
    )
    .map_err(e)?;
    Ok(())
}

// ── Excel ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_excel_open(path: String) -> CmdResult<Vec<excel::SheetInfo>> {
    excel::open_workbook(std::path::Path::new(&path)).map_err(e)
}

#[tauri::command]
pub fn cmd_excel_get_sheet(path: String, sheet_index: usize) -> CmdResult<excel::SheetData> {
    excel::get_sheet_data(std::path::Path::new(&path), sheet_index).map_err(e)
}

#[tauri::command]
pub fn cmd_excel_set_cell(
    path: String,
    sheet_index: usize,
    row: u32,
    col: u32,
    value: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let file_path = std::path::Path::new(&path);
    excel::set_cell(file_path, sheet_index, row, col, &value).map_err(e)?;
    let db = state.db.lock().map_err(e)?;
    let vdir = state.versions_dir.lock().map_err(e)?;
    versions::snapshot_file(&db, &vdir, file_path, "local").ok();
    activity::log_activity(&db, "local", "file_saved", &path, None).ok();
    Ok(())
}

#[tauri::command]
pub fn cmd_excel_detect_form(path: String, sheet_index: usize) -> CmdResult<excel::FormLayout> {
    excel::detect_form_layout(std::path::Path::new(&path), sheet_index).map_err(e)
}

#[tauri::command]
pub fn cmd_excel_get_record(
    path: String,
    layout: excel::FormLayout,
    record_index: u32,
) -> CmdResult<excel::FormRecord> {
    excel::get_record(std::path::Path::new(&path), &layout, record_index).map_err(e)
}

#[tauri::command]
pub fn cmd_excel_save_record(
    path: String,
    layout: excel::FormLayout,
    record_index: u32,
    updates: Vec<(u32, String)>,
    state: State<'_, AppState>,
) -> CmdResult<Vec<String>> {
    let file_path = std::path::Path::new(&path);
    let errors = excel::save_record(file_path, &layout, record_index, updates).map_err(e)?;
    if errors.is_empty() {
        let db = state.db.lock().map_err(e)?;
        let vdir = state.versions_dir.lock().map_err(e)?;
        versions::snapshot_file(&db, &vdir, file_path, "local").ok();
        activity::log_activity(&db, "local", "file_saved", &path, None).ok();
    }
    Ok(errors)
}

// ── SQL ───────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_sql_execute(
    sql: String,
    state: State<'_, AppState>,
) -> CmdResult<crate::sql::engine::QueryResult> {
    let engine = state.sql_engine.lock().map_err(e)?;
    Ok(engine.execute(&sql))
}

#[tauri::command]
pub fn cmd_sql_execute_script(
    sql: String,
    state: State<'_, AppState>,
) -> CmdResult<Vec<crate::sql::engine::ScriptStatementResult>> {
    let engine = state.sql_engine.lock().map_err(e)?;
    Ok(engine.execute_script(&sql))
}

#[tauri::command]
pub fn cmd_sql_import_dump(
    file_path: String,
    state: State<'_, AppState>,
) -> CmdResult<crate::sql::engine::ImportResult> {
    let engine = state.sql_engine.lock().map_err(e)?;
    engine
        .import_sql_dump(std::path::Path::new(&file_path))
        .map_err(e)
}

#[tauri::command]
pub fn cmd_sql_query_file(
    file_path: String,
    sql: String,
    state: State<'_, AppState>,
) -> CmdResult<crate::sql::engine::QueryResult> {
    let engine = state.sql_engine.lock().map_err(e)?;
    Ok(engine.query_file(std::path::Path::new(&file_path), &sql))
}

#[tauri::command]
pub fn cmd_sql_attach_file(
    file_path: String,
    alias: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let engine = state.sql_engine.lock().map_err(e)?;
    engine
        .attach_file(std::path::Path::new(&file_path), &alias)
        .map_err(e)
}

#[tauri::command]
pub fn cmd_sql_generate_edit(
    table: String,
    pk_col: String,
    pk_val: serde_json::Value,
    changes: Vec<(String, serde_json::Value)>,
) -> CmdResult<String> {
    Ok(SqlEngine::generate_edit_sql(
        &table, &pk_col, &pk_val, &changes,
    ))
}

// ── Version history ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_list_versions(
    file_path: String,
    state: State<'_, AppState>,
) -> CmdResult<Vec<versions::Version>> {
    let db = state.db.lock().map_err(e)?;
    versions::list_versions(&db, &file_path).map_err(e)
}

#[tauri::command]
pub fn cmd_restore_version(version_id: String, state: State<'_, AppState>) -> CmdResult<String> {
    let db = state.db.lock().map_err(e)?;
    let path = versions::restore_version(&db, &version_id).map_err(e)?;
    activity::log_activity(&db, "local", "version_restored", &path, Some(&version_id)).ok();
    Ok(path)
}

// ── Conflicts ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_list_conflicts(
    state: State<'_, AppState>,
) -> CmdResult<Vec<crate::conflicts::Conflict>> {
    let db = state.db.lock().map_err(e)?;
    crate::conflicts::list_conflicts(&db).map_err(e)
}

#[tauri::command]
pub fn cmd_resolve_conflict(
    conflict_id: String,
    resolution: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    let (file_path, _): (String, String) = db
        .query_row(
            "SELECT file_path, remote_hash FROM conflicts WHERE id=?1",
            rusqlite::params![conflict_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(e)?;

    let res = match resolution.as_str() {
        "keep_theirs" => crate::conflicts::ConflictResolution::KeepTheirs,
        "keep_both" => crate::conflicts::ConflictResolution::KeepBoth,
        _ => crate::conflicts::ConflictResolution::KeepMine,
    };
    crate::conflicts::resolve_conflict(
        &db,
        &conflict_id,
        res,
        std::path::Path::new(&file_path),
        None,
    )
    .map_err(e)?;
    activity::log_activity(
        &db,
        "local",
        "conflict_resolved",
        &file_path,
        Some(&resolution),
    )
    .ok();
    Ok(())
}

// ── Search ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_search(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> CmdResult<Vec<crate::search::SearchResult>> {
    let db = state.db.lock().map_err(e)?;
    crate::search::search(&db, &query, limit.unwrap_or(20)).map_err(e)
}

#[tauri::command]
pub fn cmd_index_file(
    share_id: String,
    rel_path: String,
    abs_path: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    crate::search::index_file(&db, &share_id, &rel_path, std::path::Path::new(&abs_path)).map_err(e)
}

// ── Network ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn cmd_get_node_id(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let node = state.iroh_node.lock().map_err(e)?;
    Ok(node.as_ref().map(|n| n.node_id.clone()))
}

#[tauri::command]
pub async fn cmd_get_node_ticket(state: tauri::State<'_, AppState>) -> CmdResult<Option<String>> {
    let node_opt = {
        let guard = state.iroh_node.lock().map_err(e)?;
        guard.clone()
    };
    match node_opt {
        Some(node) => node.node_addr_ticket().await.map(Some).map_err(e),
        None => Ok(None),
    }
}

// ── File history & favourites ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    pub id: String,
    pub file_path: String,
    pub opened_at: String,
    pub file_kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileFavourite {
    pub id: String,
    pub file_path: String,
    pub display_name: String,
    pub file_kind: String,
    pub added_at: String,
}

#[tauri::command]
pub fn cmd_record_file_open(
    file_path: String,
    file_kind: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    // Keep last 50 entries per file — delete older ones
    db.execute(
        "DELETE FROM file_history WHERE file_path = ?1",
        rusqlite::params![file_path],
    )
    .ok();
    db.execute(
        "INSERT INTO file_history (id, file_path, opened_at, file_kind) VALUES (?1,?2,?3,?4)",
        rusqlite::params![id, file_path, now, file_kind],
    )
    .map_err(e)?;
    Ok(())
}

#[tauri::command]
pub fn cmd_get_file_history(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> CmdResult<Vec<FileHistoryEntry>> {
    let db = state.db.lock().map_err(e)?;
    let lim = limit.unwrap_or(20) as i64;
    let mut stmt = db.prepare(
        "SELECT id, file_path, opened_at, file_kind FROM file_history ORDER BY opened_at DESC LIMIT ?1"
    ).map_err(e)?;
    let items = stmt
        .query_map(rusqlite::params![lim], |row| {
            Ok(FileHistoryEntry {
                id: row.get(0)?,
                file_path: row.get(1)?,
                opened_at: row.get(2)?,
                file_kind: row.get(3)?,
            })
        })
        .map_err(e)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

#[tauri::command]
pub fn cmd_add_favourite(
    file_path: String,
    display_name: String,
    file_kind: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    db.execute(
        "INSERT INTO file_favourites (id, file_path, display_name, file_kind, added_at) VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(file_path) DO UPDATE SET display_name=?3",
        rusqlite::params![id, file_path, display_name, file_kind, now],
    ).map_err(e)?;
    Ok(())
}

#[tauri::command]
pub fn cmd_remove_favourite(file_path: String, state: State<'_, AppState>) -> CmdResult<()> {
    let db = state.db.lock().map_err(e)?;
    db.execute(
        "DELETE FROM file_favourites WHERE file_path = ?1",
        rusqlite::params![file_path],
    )
    .map_err(e)?;
    Ok(())
}

#[tauri::command]
pub fn cmd_get_favourites(state: State<'_, AppState>) -> CmdResult<Vec<FileFavourite>> {
    let db = state.db.lock().map_err(e)?;
    let mut stmt = db.prepare(
        "SELECT id, file_path, display_name, file_kind, added_at FROM file_favourites ORDER BY added_at DESC"
    ).map_err(e)?;
    let items = stmt
        .query_map([], |row| {
            Ok(FileFavourite {
                id: row.get(0)?,
                file_path: row.get(1)?,
                display_name: row.get(2)?,
                file_kind: row.get(3)?,
                added_at: row.get(4)?,
            })
        })
        .map_err(e)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

/// Read any file as base64 for previewing in the frontend
#[tauri::command]
pub fn cmd_read_file_base64(file_path: String) -> CmdResult<String> {
    let bytes = std::fs::read(&file_path).map_err(e)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Write raw bytes to a file path (used for saving Excel files from frontend)
#[tauri::command]
pub fn cmd_write_file_bytes(
    file_path: String,
    data: Vec<u8>,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    std::fs::write(&file_path, &data).map_err(e)?;
    let db = state.db.lock().map_err(e)?;
    let vdir = state.versions_dir.lock().map_err(e)?;
    let path = std::path::Path::new(&file_path);
    versions::snapshot_file(&db, &vdir, path, "local").ok();
    activity::log_activity(&db, "local", "file_saved", &file_path, None).ok();
    // Broadcast save event to peers via gossip
    if let Ok(node_guard) = state.iroh_node.try_lock() {
        if let Some(node) = node_guard.as_ref() {
            let node = node.clone();
            let fp = file_path.clone();
            state.rt.spawn(async move {
                node.broadcast_lock(&fp, "saved").await.ok();
            });
        }
    }
    Ok(())
}

/// Read a text file as UTF-8 string
#[tauri::command]
pub fn cmd_read_file_text(file_path: String) -> CmdResult<String> {
    std::fs::read_to_string(&file_path).map_err(e)
}

fn generate_qr_base64(data: &str) -> anyhow::Result<String> {
    use image::Luma;
    use qrcode::{EcLevel, QrCode};
    let code = QrCode::with_error_correction_level(data, EcLevel::M)?;
    let img = code.render::<Luma<u8>>().min_dimensions(200, 200).build();
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(img).write_to(&mut buf, image::ImageFormat::Png)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

// ── Extra response types ──────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PairingCodeWithQr {
    pub id: String,
    pub code: String,       // full code with embedded addr
    pub short_code: String, // just XXXX-XXXX for display
    pub created_at: String,
    pub expires_at: String,
    pub qr_base64: String,
    pub node_id: Option<String>,
}

// ── AI ────────────────────────────────────────────────────────────────────────

/// Get AI model status (downloaded / not downloaded / size)
#[tauri::command]
pub fn cmd_ai_status(state: State<'_, AppState>) -> CmdResult<AiStatus> {
    let mgr = ModelManager::new(
        &state
            .versions_dir
            .lock()
            .map_err(e)?
            .parent()
            .unwrap_or(std::path::Path::new(".")),
    );
    Ok(mgr.status())
}

/// Start downloading a model. Emits "ai-download-progress" events: {filename, bytes_done, bytes_total}
#[tauri::command]
pub async fn cmd_ai_download_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<()> {
    let data_dir = state
        .versions_dir
        .lock()
        .map_err(e)?
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    drop(state); // must drop before .await
    let mgr = std::sync::Arc::new(crate::ai::model_manager::ModelManager::new(&data_dir));

    for (url, filename, sha256) in [
        (
            crate::ai::model_manager::CHAT_MODEL_URL,
            crate::ai::model_manager::CHAT_MODEL_FILENAME,
            crate::ai::model_manager::CHAT_MODEL_SHA256,
        ),
        (
            crate::ai::model_manager::EMBED_MODEL_URL,
            crate::ai::model_manager::EMBED_MODEL_FILENAME,
            crate::ai::model_manager::EMBED_MODEL_SHA256,
        ),
    ] {
        // A previous failed download may have left an error document at the
        // destination. Only skip files that pass the GGUF validation check.
        if (filename == crate::ai::model_manager::CHAT_MODEL_FILENAME && mgr.chat_ready())
            || (filename == crate::ai::model_manager::EMBED_MODEL_FILENAME && mgr.embed_ready())
        {
            continue;
        }
        std::fs::remove_file(mgr.models_dir.join(filename)).ok();
        let app2 = app.clone();
        let fn2 = filename.to_string();
        mgr.download(url, filename, sha256, move |done, total| {
            app2.emit(
                "ai-download-progress",
                serde_json::json!({
                    "filename": fn2, "bytes_done": done, "bytes_total": total
                }),
            )
            .ok();
        })
        .await
        .map_err(e)?;
    }
    Ok(())
}

/// Remove downloaded model files
#[tauri::command]
pub fn cmd_ai_remove_models(state: State<'_, AppState>) -> CmdResult<()> {
    let data_dir = state
        .versions_dir
        .lock()
        .map_err(e)?
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    ModelManager::new(&data_dir).remove_models().map_err(e)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiTextResult {
    pub text: String,
    pub guarded: bool,
}

/// 6.1 Explain a sync conflict
#[tauri::command]
pub async fn cmd_ai_explain_conflict(
    file_path: String,
    local_meta: String,
    remote_meta: String,
    diff_snippet: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<AiTextResult> {
    let model_path = ModelManager::new(
        &state
            .versions_dir
            .lock()
            .map_err(e)?
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
    )
    .chat_model_path();
    drop(state); // release State before await
    let system = prompts::conflict_explain(
        &file_path,
        &local_meta,
        &remote_meta,
        diff_snippet.as_deref(),
    );
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::ai::llm::complete(
            &model_path,
            &system,
            "Explain this conflict.",
            None,
            move |tok| {
                app2.emit("ai-token", tok).ok();
                true
            },
        )
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    let guarded = result == identity_guard::canned();
    Ok(AiTextResult {
        text: result,
        guarded,
    })
}

/// 6.4 Activity summary
#[tauri::command]
pub async fn cmd_ai_activity_summary(
    time_window: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<AiTextResult> {
    // Extract everything from state before any await point
    let (log_text, model_path) = {
        let db = state.db.lock().map_err(e)?;
        let entries = activity::list_activity(&db, 100).map_err(e)?;
        let log = entries
            .iter()
            .map(|a| format!("[{}] {} {} {}", a.timestamp, a.actor, a.action, a.target))
            .collect::<Vec<_>>()
            .join("\n");
        let mp = ModelManager::new(
            &state
                .versions_dir
                .lock()
                .map_err(e)?
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
        )
        .chat_model_path();
        (log, mp)
    };
    drop(state);
    let system = prompts::activity_summary(&log_text, &time_window);
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::ai::llm::complete(&model_path, &system, "Summarize.", None, move |tok| {
            app2.emit("ai-token", tok).ok();
            true
        })
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    let guarded = result == identity_guard::canned();
    Ok(AiTextResult {
        text: result,
        guarded,
    })
}

/// AI NL→SQL (§6.3): draft a query from a natural-language question. Output is
/// grammar-constrained to {"sql","explanation"} JSON and NEVER executed here —
/// the UI drops the draft into the editor/diff for human review.
#[derive(serde::Serialize)]
pub struct AiSqlDraft {
    pub sql: String,
    pub explanation: String,
    pub guarded: bool,
}

#[tauri::command]
pub async fn cmd_ai_nl_sql(
    schema: String,
    question: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<AiSqlDraft> {
    let (model_path, embed_model_path) = {
        let data_dir = data_dir_from_state(&state)?;
        let manager = ModelManager::new(&data_dir);
        (manager.chat_model_path(), manager.embed_model_path())
    };
    // Scope gate (§8.2) — needs the embedding model; fails open without it.
    if embed_model_path.exists() {
        match crate::ai::scope_guard::gate_message(&embed_model_path, &question) {
            Ok(false) => {
                return Ok(AiSqlDraft {
                    sql: String::new(),
                    explanation: crate::ai::scope_guard::out_of_scope_response().to_string(),
                    guarded: true,
                });
            }
            Ok(true) | Err(_) => {}
        }
    }
    drop(state);
    let system = prompts::nl_to_sql(&schema, &question);
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::ai::llm::complete(
            &model_path,
            &system,
            &question,
            Some(crate::ai::grammar::NL_TO_SQL_GRAMMAR),
            move |tok| {
                app2.emit("ai-token", tok).ok();
                true
            },
        )
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    if let Some((sql, explanation)) = crate::ai::grammar::validate_nl_sql_output(&result) {
        Ok(AiSqlDraft {
            sql,
            explanation,
            guarded: false,
        })
    } else {
        // The grammar makes this unreachable; stay defensive anyway and hand
        // the raw text to the human-review flow rather than executing anything.
        Ok(AiSqlDraft {
            sql: String::new(),
            explanation: result,
            guarded: false,
        })
    }
}

/// 6.6 Data insights — stats computed in Rust, model only phrases them
#[tauri::command]
pub async fn cmd_ai_data_insights(
    stats_json: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<AiTextResult> {
    let model_path = ModelManager::new(
        &state
            .versions_dir
            .lock()
            .map_err(e)?
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
    )
    .chat_model_path();
    drop(state);
    let system = prompts::data_insights(&stats_json);
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::ai::llm::complete(
            &model_path,
            &system,
            "Describe these stats.",
            None,
            move |tok| {
                app2.emit("ai-token", tok).ok();
                true
            },
        )
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    let guarded = result == identity_guard::canned();
    Ok(AiTextResult {
        text: result,
        guarded,
    })
}

/// Ask a question grounded in a bounded excerpt of a local document.
#[tauri::command]
pub async fn cmd_ai_chat_document(
    file_path: String,
    file_kind: String,
    question: String,
    document_text: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> CmdResult<AiTextResult> {
    // The complete document remains local. This is the working context passed
    // to one inference call; documents larger than this are ranked by the
    // user's question so they never fail merely for being large.
    // Keep the prompt evaluation under the 3K inference context. This is an
    // input budget, not a document-size limit: ranked excerpts cover documents
    // of any length without asking llama.cpp to ingest all 30 pages at once.
    // A 0.6B CPU model needs a deliberately tight retrieval budget. Eight
    // thousand characters became ~2,000 prompt tokens on a six-page SLA and
    // dominated first-token latency. 3,600 characters leaves room for the
    // question and a concise cited answer in the 3K context.
    const CONTEXT_CHARS: usize = 3_600;
    let path = std::path::Path::new(&file_path);
    if !matches!(
        file_kind.as_str(),
        "excel" | "sql" | "csv" | "text" | "generic" | "pdf"
    ) {
        return Err(
            "Document chat currently supports PDFs, spreadsheets, and text-based files."
                .to_string(),
        );
    }
    // Scope gate (§8.2): out-of-scope questions never reach the generation
    // model. Fails open while the embedding model is not downloaded yet.
    let embed_model_path = data_dir_from_state(&state)?;
    let embed_model_path = ModelManager::new(&embed_model_path).embed_model_path();
    if embed_model_path.exists() {
        match crate::ai::scope_guard::gate_message(&embed_model_path, &question) {
            Ok(false) => {
                return Ok(AiTextResult {
                    text: crate::ai::scope_guard::out_of_scope_response().to_string(),
                    guarded: true,
                });
            }
            Ok(true) | Err(_) => {}
        }
    }
    let excerpt = if let Some(text) = document_text.filter(|text| !text.trim().is_empty()) {
        text
    } else if file_kind == "excel" {
        let sheets = excel::open_workbook(path).map_err(e)?;
        let mut text = String::new();
        for sheet in sheets.iter().take(3) {
            let data = excel::get_sheet_data(path, sheet.index).map_err(e)?;
            text.push_str(&format!("Sheet: {}\n", data.sheet_name));
            for row in data.rows.iter().take(100) {
                let values = row
                    .iter()
                    .map(|value| match value {
                        excel::CellValue::Text(v) => v.clone(),
                        excel::CellValue::Number(v) => v.to_string(),
                        excel::CellValue::Bool(v) => v.to_string(),
                        excel::CellValue::Empty => String::new(),
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                if !values.trim().is_empty() {
                    text.push_str(&values);
                    text.push('\n');
                }
                if text.len() >= CONTEXT_CHARS {
                    break;
                }
            }
            if text.len() >= CONTEXT_CHARS {
                break;
            }
        }
        text
    } else if file_kind == "pdf" {
        pdf_extract::extract_text(path).map_err(|_| {
            "This PDF has no extractable text. If it is a scan, OCR it first.".to_string()
        })?
    } else {
        std::fs::read_to_string(path)
            .map_err(|err| format!("Document chat needs a UTF-8 text-based file: {err}"))?
    };
    let excerpt = document_context(&excerpt, &question, CONTEXT_CHARS);
    if excerpt.trim().is_empty() {
        return Err(e(anyhow::anyhow!("This document has no readable text.")));
    }
    let model_path = ModelManager::new(
        &state
            .versions_dir
            .lock()
            .map_err(e)?
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
    )
    .chat_model_path();
    drop(state);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("document")
        .to_string();
    let system = prompts::document_chat(&name, &excerpt);
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::ai::llm::complete_document(&model_path, &system, &question, move |token| {
            app2.emit("ai-token", token).ok();
            true
        })
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    let guarded = result == identity_guard::canned();
    Ok(AiTextResult {
        text: result,
        guarded,
    })
}

/// Select document passages that are useful for a question without ever
/// exposing the model's finite context window to the user. Every byte stays on
/// device; this is a local relevance pass, not a network request.
fn document_context(document: &str, question: &str, max_chars: usize) -> String {
    if document.len() <= max_chars {
        return document.to_string();
    }

    let terms: Vec<String> = question
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|term| term.len() > 2)
        .map(str::to_string)
        .collect();
    let mut text_chunks = Vec::new();
    let mut current = String::new();
    for line in document.lines() {
        if current.chars().count() + line.chars().count() + 1 > 1_200 && !current.is_empty() {
            text_chunks.push(std::mem::take(&mut current));
        }
        // Long unbroken lines are split on Unicode character boundaries.
        if line.chars().count() > 1_200 {
            for segment in line.chars().collect::<Vec<_>>().chunks(1_200) {
                text_chunks.push(segment.iter().collect());
            }
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.is_empty() {
        text_chunks.push(current);
    }
    let mut chunks: Vec<(usize, usize, String)> = text_chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            let lower = chunk.to_lowercase();
            let score = terms
                .iter()
                .filter(|term| lower.contains(term.as_str()))
                .count();
            (score, index, chunk)
        })
        .collect();
    chunks.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

    let mut selected = Vec::new();
    let mut used = 0;
    for (_, _, chunk) in chunks {
        let chunk_len = chunk.len();
        if used + chunk_len > max_chars {
            continue;
        }
        selected.push(chunk);
        used += chunk_len;
        if used >= max_chars {
            break;
        }
    }
    selected.join("\n\n")
}

/// Walk every share folder (owned and received) and embed text files that do
/// not have a stored vector yet (AI spec §6.5). Bounded per call so it can
/// run at startup without stalling the app.
#[tauri::command]
pub async fn cmd_reindex_embeddings(state: tauri::State<'_, AppState>) -> CmdResult<usize> {
    let data_dir = data_dir_from_state(&state)?;
    let db_path = data_dir.join("soffit-commit.sqlite");
    drop(state);
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<usize> {
        let model_path = ModelManager::new(&data_dir).embed_model_path();
        if !model_path.exists() {
            anyhow::bail!("Enable AI and download the embedding model first");
        }
        let conn = rusqlite::Connection::open(&db_path)?;
        let text_exts = [
            "txt", "md", "rs", "ts", "js", "py", "sql", "json", "toml", "yaml", "yml", "csv",
            "html", "css",
        ];
        let mut roots: Vec<(String, std::path::PathBuf)> = {
            let mut stmt = conn.prepare("SELECT id, path FROM shares")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .map(|(id, path)| (id, std::path::PathBuf::from(path)))
                .collect();
            rows
        };
        let mut done = 0usize;
        'outer: for (share_id, root) in roots {
            if !root.exists() {
                continue;
            }
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    if name.ends_with(".soffit-part") || name.ends_with(".sha256") {
                        continue;
                    }
                    let rel = match path.strip_prefix(&root) {
                        Ok(rel) => rel.to_string_lossy().to_string(),
                        Err(_) => continue,
                    };
                    if search::has_embedding(&conn, &share_id, &rel) {
                        continue;
                    }
                    let ext = path
                        .extension()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !text_exts.contains(&ext.as_str()) {
                        continue;
                    }
                    let Ok(bytes) = std::fs::read(&path) else {
                        continue;
                    };
                    let Ok(text) = String::from_utf8(bytes[..bytes.len().min(400_000)].to_vec())
                    else {
                        continue;
                    };
                    match crate::ai::embeddings::embed_text(&model_path, &text) {
                        Ok(vector) => {
                            search::upsert_embedding(&conn, &share_id, &rel, &vector).ok();
                            done += 1;
                        }
                        Err(err) => eprintln!("[ai-index] {rel}: {err}"),
                    }
                    if done >= 50 {
                        break 'outer;
                    }
                }
            }
        }
        Ok(done)
    })
    .await
    .map_err(e)?;
    result.map_err(e)
}
