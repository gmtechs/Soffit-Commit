#![allow(dead_code)]
mod activity;
mod ai;
mod auth;
mod commands;
mod conflicts;
mod db;
mod excel;
mod locks;
mod models;
mod network;
mod notifications;
mod pairing;
mod peers;
mod search;
mod shares;
mod sql;
mod versions;

use network::IrohNode;
use sql::engine::SqlEngine;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub iroh_node: Mutex<Option<Arc<IrohNode>>>,
    pub sql_engine: Mutex<SqlEngine>,
    pub versions_dir: Mutex<PathBuf>,
    pub rt: tokio::runtime::Handle,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let rt_handle = rt.handle().clone();

    std::thread::spawn(move || {
        rt.block_on(std::future::pending::<()>());
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(move |app| {
            let data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&data_dir).expect("failed to create app data dir");

            // SQLite
            let db_path = data_dir.join("soffit-commit.sqlite");
            let conn = db::init_db(&db_path).expect("failed to init database");
            versions::ensure_versions_table(&conn).expect("failed to init versions table");
            conflicts::ensure_conflicts_table(&conn).expect("failed to init conflicts table");
            search::ensure_fts_table(&conn).expect("failed to init FTS table");
            db::ensure_extra_tables(&conn).expect("failed to init extra tables");
            // Online is an observed connection state, not a remembered value
            // from a previous app run.
            peers::mark_all_offline(&conn).expect("failed to reset peer presence");
            // Expired locks are cleared at startup so stale rows never block a
            // fresh acquire after a crash (locks also expire lazily on read).
            locks::cleanup_expired_locks(&conn).ok();

            // SQL engine (SQLite in-memory)
            let sql_engine = SqlEngine::new().expect("failed to init SQL engine");

            // Snapshot dir
            let versions_dir = data_dir.join("versions");
            std::fs::create_dir_all(&versions_dir).ok();

            // Remote shares are stored under <data_dir>/shared/<share_id>
            let shared_root = data_dir.join("shared");
            std::fs::create_dir_all(&shared_root).ok();

            // iroh node
            let key_path = data_dir.join("identity.key");
            let (iroh_tx, iroh_rx) = std::sync::mpsc::channel::<Arc<IrohNode>>();
            let rt_clone = rt_handle.clone();
            let iroh_db_path = db_path.clone();

            std::thread::spawn(move || {
                rt_clone.block_on(async move {
                    match network::identity::load_or_create(&key_path) {
                        Ok(key) => match IrohNode::start(key, iroh_db_path, shared_root).await {
                            Ok(node) => {
                                iroh_tx.send(node).ok();
                            }
                            Err(e) => eprintln!("[iroh] start failed: {e}"),
                        },
                        Err(e) => eprintln!("[iroh] identity error: {e}"),
                    }
                });
            });

            let iroh_node = iroh_rx.recv_timeout(std::time::Duration::from_secs(5)).ok();
            let reconnect_node = iroh_node.clone();
            let reconnect_rt = rt_handle.clone();

            // Network event → OS notification bridge
            if let Some(ref node) = iroh_node {
                let mut rx = node.event_tx.subscribe();
                let app_handle = app.handle().clone();
                let rt_notif = rt_handle.clone();
                let event_db_path = db_path.clone();
                let bridge_shared_root = node.shared_root.clone();
                std::thread::spawn(move || {
                    rt_notif.block_on(async move {
                        while let Ok(event) = rx.recv().await {
                            use network::NetworkEvent::*;
                            match event {
                                PeerOnline {
                                    node_id,
                                    display_name,
                                    endpoint_addr,
                                } => {
                                    // The receiving device learns about its peer from the
                                    // authenticated Hello, so pairing is persisted on both sides.
                                    if let Ok(conn) = rusqlite::Connection::open(&event_db_path) {
                                        if let Ok(peer) = peers::add_peer(
                                            &conn,
                                            &node_id,
                                            &display_name,
                                            &node_id,
                                        ) {
                                            pairing::update_peer_endpoint(
                                                &conn,
                                                &node_id,
                                                &endpoint_addr,
                                            )
                                            .ok();
                                            activity::log_activity(
                                                &conn,
                                                "system",
                                                "peer_added",
                                                &peer.display_name,
                                                None,
                                            )
                                            .ok();
                                        }
                                    }
                                    notifications::notify(
                                        &app_handle,
                                        "Peer online",
                                        &format!("{display_name} is now online"),
                                    );
                                    let _ = app_handle.emit(
                                        "app://sync-event",
                                        serde_json::json!({
                                            "event": "peer_online",
                                            "node_id": node_id,
                                            "display_name": display_name,
                                        }),
                                    );
                                }
                                LockAcquired {
                                    file_path,
                                    peer_name,
                                } => notifications::notify(
                                    &app_handle,
                                    "File locked",
                                    &format!(
                                        "{peer_name} locked {}",
                                        file_path.split('/').last().unwrap_or(&file_path)
                                    ),
                                ),
                                LockReleased { file_path } => notifications::notify(
                                    &app_handle,
                                    "Lock released",
                                    &format!(
                                        "{} is now available",
                                        file_path.split('/').last().unwrap_or(&file_path)
                                    ),
                                ),
                                ShareGranted { share_id } => {
                                    // A remote share became visible; let open
                                    // views refresh their share list.
                                    if let Ok(conn) = rusqlite::Connection::open(&event_db_path) {
                                        activity::log_activity(
                                            &conn,
                                            "system",
                                            "share_granted",
                                            &share_id,
                                            None,
                                        )
                                        .ok();
                                    }
                                    let _ = app_handle.emit(
                                        "app://sync-event",
                                        serde_json::json!({
                                            "event": "share_granted",
                                            "share_id": share_id,
                                        }),
                                    );
                                }
                                SyncComplete { share_id, path } => {
                                    if let Ok(conn) = rusqlite::Connection::open(&event_db_path) {
                                        activity::log_activity(
                                            &conn,
                                            "system",
                                            "file_synced",
                                            &path,
                                            Some(&share_id),
                                        )
                                        .ok();
                                        // Keep local FTS search current for files
                                        // received from peers.
                                        let target = bridge_shared_root.join(&share_id).join(&path);
                                        if target.exists() {
                                            search::index_file(&conn, &share_id, &path, &target)
                                                .ok();
                                        }
                                    }
                                    let _ = app_handle.emit(
                                        "app://sync-event",
                                        serde_json::json!({
                                            "event": "sync_complete",
                                            "share_id": share_id,
                                            "path": path,
                                        }),
                                    );
                                }
                                SyncError { path, error } => {
                                    notifications::notify(
                                        &app_handle,
                                        "Sync error",
                                        &format!(
                                            "{}: {error}",
                                            path.split('/').last().unwrap_or(&path)
                                        ),
                                    );
                                    let _ = app_handle.emit(
                                        "app://sync-event",
                                        serde_json::json!({
                                            "event": "sync_error",
                                            "path": path,
                                            "error": error,
                                        }),
                                    );
                                }
                                _ => {}
                            }
                        }
                    });
                });
            }

            let lock_watch_rt = rt_handle.clone();
            app.manage(AppState {
                db: Mutex::new(conn),
                iroh_node: Mutex::new(iroh_node),
                sql_engine: Mutex::new(sql_engine),
                versions_dir: Mutex::new(versions_dir),
                rt: rt_handle,
            });

            // Lock housekeeping (modules §4): warn the holder shortly before
            // an idle lock auto-releases, and sweep expired rows periodically.
            {
                let warn_app = app.handle().clone();
                let warn_db = db_path.clone();
                lock_watch_rt.spawn(async move {
                    let mut warned: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        let Ok(conn) = rusqlite::Connection::open(&warn_db) else {
                            continue;
                        };
                        let _ = locks::cleanup_expired_locks(&conn);
                        let now = chrono::Utc::now();
                        let Ok(active) = locks::list_active_locks(&conn) else {
                            continue;
                        };
                        for lock in active {
                            let Ok(expires) =
                                chrono::DateTime::parse_from_rfc3339(&lock.expires_at)
                            else {
                                continue;
                            };
                            let remaining = expires.signed_duration_since(now);
                            if remaining > chrono::Duration::zero()
                                && remaining <= chrono::Duration::minutes(2)
                                && warned.insert(lock.id.clone())
                            {
                                notifications::notify(
                                    &warn_app,
                                    "Lock expiring soon",
                                    &format!(
                                        "{} will unlock in about a minute — save your changes",
                                        lock.file_path
                                            .split('/')
                                            .last()
                                            .unwrap_or(&lock.file_path)
                                    ),
                                );
                            }
                            if remaining <= chrono::Duration::zero() {
                                warned.remove(&lock.id);
                            }
                        }
                    }
                });
            }

            // Keep paired peers reachable for the whole session so the peer
            // list settles without the user visiting the Peers page: presence
            // is refreshed by every pass, and a dropped QUIC connection is
            // retried instead of ending sync until the next launch.
            if let Some(node) = reconnect_node {
                let reconnect_db_path = db_path.clone();
                let rt_reconnect = reconnect_rt;
                std::thread::spawn(move || {
                    rt_reconnect.block_on(async move {
                        // Long enough not to churn against the per-connection
                        // sync cadence, short enough to heal a dropped link
                        // promptly.
                        const RECONNECT_INTERVAL: std::time::Duration =
                            std::time::Duration::from_secs(15);
                        let local_name = rusqlite::Connection::open(&reconnect_db_path)
                            .ok()
                            .and_then(|conn| {
                                conn.query_row(
                                    "SELECT username FROM users ORDER BY created_at ASC LIMIT 1",
                                    [],
                                    |row| row.get::<_, String>(0),
                                )
                                .ok()
                            })
                            .unwrap_or_else(|| "This device".to_string());
                        loop {
                            let reconnectable = rusqlite::Connection::open(&reconnect_db_path)
                                .ok()
                                .and_then(|conn| pairing::get_reconnectable_peers(&conn).ok())
                                .unwrap_or_default();
                            for (node_id, endpoint_addr) in reconnectable {
                                // A live connection already runs the sync
                                // engine for this peer — leave it alone.
                                let connected = node
                                    .conns
                                    .lock()
                                    .map(|conns| conns.contains_key(&node_id))
                                    .unwrap_or(false);
                                if connected {
                                    continue;
                                }
                                let online = node
                                    .connect_to_peer(&endpoint_addr, &local_name)
                                    .await
                                    .is_ok();
                                // Presence must reflect this attempt, so a peer
                                // that dropped stops showing as online.
                                if let Ok(conn) = rusqlite::Connection::open(&reconnect_db_path) {
                                    peers::set_peer_online_by_node_id(&conn, &node_id, online).ok();
                                }
                            }
                            tokio::time::sleep(RECONNECT_INTERVAL).await;
                        }
                    });
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cmd_user_exists,
            commands::cmd_create_user,
            commands::cmd_login,
            commands::cmd_list_users,
            commands::cmd_update_user,
            commands::cmd_generate_pairing_code,
            commands::cmd_consume_pairing_code,
            commands::cmd_add_peer,
            commands::cmd_reconnect_peers,
            commands::cmd_list_peers,
            commands::cmd_remove_peer,
            commands::cmd_rename_peer,
            commands::cmd_set_permission,
            commands::cmd_get_peer_permissions,
            commands::cmd_rename_file,
            commands::cmd_ensure_local_file,
            commands::cmd_semantic_search,
            commands::cmd_ai_nl_sql,
            commands::cmd_reindex_embeddings,
            commands::cmd_create_share,
            commands::cmd_list_shares,
            commands::cmd_delete_share,
            commands::cmd_list_files,
            commands::cmd_refresh_share,
            commands::cmd_set_selective_sync,
            commands::cmd_acquire_lock,
            commands::cmd_release_lock,
            commands::cmd_get_lock,
            commands::cmd_list_active_locks,
            commands::cmd_list_activity,
            commands::cmd_dashboard_stats,
            commands::cmd_get_setting,
            commands::cmd_set_setting,
            commands::cmd_excel_open,
            commands::cmd_excel_get_sheet,
            commands::cmd_excel_set_cell,
            commands::cmd_excel_detect_form,
            commands::cmd_excel_get_record,
            commands::cmd_excel_save_record,
            commands::cmd_sql_execute,
            commands::cmd_sql_execute_script,
            commands::cmd_sql_import_dump,
            commands::cmd_sql_query_file,
            commands::cmd_sql_attach_file,
            commands::cmd_sql_generate_edit,
            commands::cmd_list_versions,
            commands::cmd_restore_version,
            commands::cmd_list_conflicts,
            commands::cmd_resolve_conflict,
            commands::cmd_search,
            commands::cmd_index_file,
            commands::cmd_get_node_id,
            commands::cmd_get_node_ticket,
            // File history & favourites
            commands::cmd_record_file_open,
            commands::cmd_get_file_history,
            commands::cmd_add_favourite,
            commands::cmd_remove_favourite,
            commands::cmd_get_favourites,
            commands::cmd_read_file_base64,
            commands::cmd_read_file_text,
            commands::cmd_write_file_bytes,
            // AI
            commands::cmd_ai_status,
            commands::cmd_ai_download_models,
            commands::cmd_ai_remove_models,
            commands::cmd_ai_explain_conflict,
            commands::cmd_ai_activity_summary,
            commands::cmd_ai_data_insights,
            commands::cmd_ai_chat_document,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
