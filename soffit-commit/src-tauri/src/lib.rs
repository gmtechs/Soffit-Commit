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

use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tauri::Manager;
use network::IrohNode;
use sql::engine::SqlEngine;

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

            // SQL engine (SQLite in-memory)
            let sql_engine = SqlEngine::new().expect("failed to init SQL engine");

            // Snapshot dir
            let versions_dir = data_dir.join("versions");
            std::fs::create_dir_all(&versions_dir).ok();

            // iroh node
            let key_path = data_dir.join("identity.key");
            let (iroh_tx, iroh_rx) = std::sync::mpsc::channel::<Arc<IrohNode>>();
            let rt_clone = rt_handle.clone();

            std::thread::spawn(move || {
                rt_clone.block_on(async move {
                    match network::identity::load_or_create(&key_path) {
                        Ok(key) => match IrohNode::start(key).await {
                            Ok(node) => { iroh_tx.send(node).ok(); }
                            Err(e)   => eprintln!("[iroh] start failed: {e}"),
                        },
                        Err(e) => eprintln!("[iroh] identity error: {e}"),
                    }
                });
            });

            let iroh_node = iroh_rx.recv_timeout(std::time::Duration::from_secs(5)).ok();

            // Network event → OS notification bridge
            if let Some(ref node) = iroh_node {
                let mut rx = node.event_tx.subscribe();
                let app_handle = app.handle().clone();
                let rt_notif = rt_handle.clone();
                std::thread::spawn(move || {
                    rt_notif.block_on(async move {
                        while let Ok(event) = rx.recv().await {
                            use network::NetworkEvent::*;
                            match event {
                                PeerOnline { display_name, .. } =>
                                    notifications::notify(&app_handle, "Peer online",
                                        &format!("{display_name} is now online")),
                                LockAcquired { file_path, peer_name } =>
                                    notifications::notify(&app_handle, "File locked",
                                        &format!("{peer_name} locked {}",
                                            file_path.split('/').last().unwrap_or(&file_path))),
                                LockReleased { file_path } =>
                                    notifications::notify(&app_handle, "Lock released",
                                        &format!("{} is now available",
                                            file_path.split('/').last().unwrap_or(&file_path))),
                                SyncError { path, error } =>
                                    notifications::notify(&app_handle, "Sync error",
                                        &format!("{}: {error}",
                                            path.split('/').last().unwrap_or(&path))),
                                _ => {}
                            }
                        }
                    });
                });
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                iroh_node: Mutex::new(iroh_node),
                sql_engine: Mutex::new(sql_engine),
                versions_dir: Mutex::new(versions_dir),
                rt: rt_handle,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cmd_user_exists,
            commands::cmd_create_user,
            commands::cmd_login,
            commands::cmd_generate_pairing_code,
            commands::cmd_consume_pairing_code,
            commands::cmd_add_peer,
            commands::cmd_reconnect_peers,
            commands::cmd_list_peers,
            commands::cmd_remove_peer,
            commands::cmd_rename_peer,
            commands::cmd_set_permission,
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
