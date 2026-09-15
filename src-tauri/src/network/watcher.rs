//! Realtime change detection: watch every linked folder (owned shares and
//! received mirrors) with inotify so peers hear about edits in well under a
//! second instead of on the SYNC_INTERVAL poll tick. The 8-second poll stays
//! as a fallback for anything the watcher misses (network races, cold paths).
//!
//! Design: one OS watcher thread feeds raw event paths into an mpsc channel;
//! an async task debounces bursts (300 ms), maps changed paths back to their
//! share via the database, then (a) wakes the local push loops through
//! `node.change_notify` and (b) sends a `LocalChange` control message to every
//! connected peer so their push loops wake too. Failures are non-fatal — the
//! poll cadence still guarantees eventual sync.
use crate::network::sync::send_control;
use crate::network::IrohNode;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// How long to let a burst of filesystem events settle before acting.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Creates the watcher and spawns the debounce/announce task. Watch paths are
/// (re)discovered from the database, so shares added later are picked up on
/// the periodic rescan below.
pub async fn spawn_watchers(node: Arc<IrohNode>) {
    let (tx, mut rx) = mpsc::channel::<PathBuf>(4096);
    let mut watcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            for path in event.paths {
                // Non-blocking: a full channel means the debouncer is far
                // behind and the poll loop will cover us anyway.
                let _ = tx.try_send(path);
            }
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[watcher] unavailable, falling back to polling: {e}");
            return;
        }
    };

    // Seed with everything currently in the database, then rescan so later
    // shares get watched too. Any network event (e.g. ShareGranted) also
    // triggers an immediate rescan so brand-new mirrors are watched at once.
    let mut watched: HashSet<PathBuf> = HashSet::new();
    let mut net_events = node.event_tx.subscribe();
    loop {
        watch_new_paths(&node, &mut watcher, &mut watched);
        tokio::select! {
            maybe_path = rx.recv() => {
                let Some(_first) = maybe_path else { break };
                // Absorb the rest of the burst for DEBOUNCE, then announce.
                let deadline = tokio::time::Instant::now() + DEBOUNCE;
                loop {
                    match tokio::time::timeout_at(deadline, rx.recv()).await {
                        Ok(Some(_)) => continue,
                        _ => break,
                    }
                }
                announce_changes(&node).await;
            }
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                // Rescan for newly created shares/mirrors.
            }
            _ = net_events.recv() => {
                // ShareGranted etc.: pick up the new folder immediately.
            }
        }
    }
}

/// Watch every share folder that is not watched yet. Owned share roots and
/// received mirror directories both count: an owner edit must push instantly,
/// and a mirror edit must nudge the owner instantly.
fn watch_new_paths(
    node: &Arc<IrohNode>,
    watcher: &mut RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
) {
    let Ok(conn) = rusqlite::Connection::open(&node.db_path) else {
        return;
    };
    let Ok(mut stmt) = conn.prepare("SELECT path FROM shares") else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
        return;
    };
    for path in rows.flatten() {
        let root = PathBuf::from(path);
        if !root.is_dir() || !watched.insert(root.clone()) {
            continue;
        }
        if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
            // Non-fatal: the poll loop still syncs this share.
            eprintln!("[watcher] cannot watch {}: {e}", root.display());
            watched.remove(&root);
        }
    }
}

/// Map a debounce batch back to shares (the events themselves are ignored —
/// the rescan+diff in push_owned_shares decides what to transfer) and wake
/// everyone. Cheap: unchanged files reuse their stored hash.
async fn announce_changes(node: &Arc<IrohNode>) {
    // Wake every live connection's push loop (local + peers via LocalChange).
    node.change_notify.notify_waiters();
    let peers: Vec<_> = node
        .conns
        .lock()
        .expect("connection registry poisoned")
        .keys()
        .cloned()
        .collect();
    for peer in peers {
        let conn = node
            .conns
            .lock()
            .expect("connection registry poisoned")
            .get(&peer)
            .map(|(c, _)| c.clone());
        if let Some(conn) = conn {
            let _ = send_control(
                &conn,
                &crate::network::protocol::SyncMessage::LocalChange {
                    // Empty share_id means "rescan everything" — one message
                    // per burst, not per share.
                    share_id: String::new(),
                },
            )
            .await;
        }
    }
}

