use crate::models::{FileIndex, FileKind, Share, SyncStatus};
use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn create_share(conn: &Connection, path: &str, display_name: &str) -> Result<Share> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO shares (id, path, display_name, is_owner, selective_sync, created_at) VALUES (?1, ?2, ?3, 1, 0, ?4)",
        rusqlite::params![id, path, display_name, created_at],
    )?;

    index_share_files(conn, &id, path)?;

    Ok(Share {
        id,
        path: path.to_string(),
        display_name: display_name.to_string(),
        is_owner: true,
        selective_sync: false,
        created_at,
    })
}

pub fn list_shares(conn: &Connection) -> Result<Vec<Share>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, display_name, is_owner, selective_sync, created_at FROM shares ORDER BY display_name"
    )?;
    let shares = stmt
        .query_map([], |row| {
            Ok(Share {
                id: row.get(0)?,
                path: row.get(1)?,
                display_name: row.get(2)?,
                is_owner: row.get::<_, i32>(3)? != 0,
                selective_sync: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(shares)
}

/// Shares this device owns. Only the owner pushes a share's contents to the
/// peers that have been granted a permission on it.
pub fn list_owned_shares(conn: &Connection) -> Result<Vec<Share>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, display_name, is_owner, selective_sync, created_at
         FROM shares WHERE is_owner = 1 ORDER BY display_name"
    )?;
    let shares = stmt
        .query_map([], |row| {
            Ok(Share {
                id: row.get(0)?,
                path: row.get(1)?,
                display_name: row.get(2)?,
                is_owner: row.get::<_, i32>(3)? != 0,
                selective_sync: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(shares)
}

/// Upsert a share record that maps to a peer's share (is_owner = 0) and
/// return the local folder that receives the synced files.
/// The folder lives under `<app_data_dir>/shared/<share_id>` so every remote
/// share is stored separately and never touches the owner's paths.
/// `owner_node_id` records which device granted the share so its files can be
/// pulled on demand (pull-mode selective sync).
pub fn ensure_receiver_share(
    conn: &Connection,
    shared_root: &Path,
    share_id: &str,
    display_name: &str,
    selective_sync: bool,
    owner_node_id: &str,
) -> Result<PathBuf> {
    let path = shared_root.join(share_id);
    let created_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO shares (id, path, display_name, is_owner, selective_sync, created_at, owner_node_id)
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           display_name = excluded.display_name,
           selective_sync = ?4,
           owner_node_id = ?6",
        rusqlite::params![
            share_id,
            path.to_string_lossy(),
            display_name,
            selective_sync as i32,
            created_at,
            owner_node_id
        ],
    )?;
    std::fs::create_dir_all(&path).ok();
    Ok(path)
}

/// Record metadata-only knowledge of a file on a received share (selective
/// sync) without clobbering a more advanced local state, e.g. a file that is
/// already fully synced.
pub fn upsert_pending_file_entry(
    conn: &Connection,
    share_id: &str,
    relative_path: &str,
    size_bytes: i64,
    modified_at: Option<String>,
    content_hash: Option<&str>,
    file_kind: FileKind,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO file_index
            (id, share_id, relative_path, size_bytes, modified_at, content_hash, file_kind, sync_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
        rusqlite::params![
            id,
            share_id,
            relative_path,
            size_bytes,
            modified_at,
            content_hash,
            file_kind.to_string()
        ],
    )?;
    Ok(())
}

/// Create or update a single file_index row after a verified transfer.
pub fn upsert_file_entry(
    conn: &Connection,
    share_id: &str,
    relative_path: &str,
    size_bytes: i64,
    modified_at: Option<String>,
    content_hash: &str,
    file_kind: FileKind,
    sync_status: SyncStatus,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO file_index (id, share_id, relative_path, size_bytes, modified_at, content_hash, file_kind, sync_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(share_id, relative_path) DO UPDATE SET
           size_bytes = ?4, modified_at = ?5, content_hash = ?6,
           file_kind = ?7, sync_status = ?8",
        rusqlite::params![
            id,
            share_id,
            relative_path,
            size_bytes,
            modified_at,
            content_hash,
            file_kind.to_string(),
            sync_status.to_string()
        ],
    )?;
    Ok(())
}

/// Flip the sync status of one file row (e.g. pending → synced after a
/// verified transfer, or → conflict when hashes diverge).
pub fn set_file_sync_status(
    conn: &Connection,
    share_id: &str,
    relative_path: &str,
    sync_status: SyncStatus,
) -> Result<()> {
    conn.execute(
        "UPDATE file_index SET sync_status = ?1 WHERE share_id = ?2 AND relative_path = ?3",
        rusqlite::params![sync_status.to_string(), share_id, relative_path],
    )?;
    Ok(())
}

/// Remember the content hash this device last accepted from a peer for a file
/// (verified transfer install or "already identical" acknowledgement). A local
/// file whose hash still equals this marker is an untouched replica and may be
/// replaced by a newer verified copy; once the user edits the file the hashes
/// diverge, so the next remote change is reported as a conflict instead of
/// silently discarding the edit.
pub fn set_file_synced_hash(
    conn: &Connection,
    share_id: &str,
    relative_path: &str,
    synced_hash: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE file_index SET synced_hash = ?1 WHERE share_id = ?2 AND relative_path = ?3",
        rusqlite::params![synced_hash, share_id, relative_path],
    )?;
    Ok(())
}

pub fn set_selective_sync(conn: &Connection, share_id: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE shares SET selective_sync = ?1 WHERE id = ?2",
        rusqlite::params![enabled as i32, share_id],
    )?;
    Ok(())
}

pub fn delete_share(conn: &Connection, share_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM share_permissions WHERE share_id = ?1",
        rusqlite::params![share_id],
    )?;
    conn.execute(
        "DELETE FROM file_index WHERE share_id = ?1",
        rusqlite::params![share_id],
    )?;
    conn.execute(
        "DELETE FROM shares WHERE id = ?1",
        rusqlite::params![share_id],
    )?;
    Ok(())
}

pub fn list_files_in_share(conn: &Connection, share_id: &str) -> Result<Vec<FileIndex>> {
    let mut stmt = conn.prepare(
        "SELECT id, share_id, relative_path, size_bytes, modified_at, content_hash, file_kind, sync_status
         FROM file_index WHERE share_id = ?1 ORDER BY relative_path"
    )?;
    let files = stmt
        .query_map(rusqlite::params![share_id], |row| {
            let kind_str: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            Ok(FileIndex {
                id: row.get(0)?,
                share_id: row.get(1)?,
                relative_path: row.get(2)?,
                size_bytes: row.get(3)?,
                modified_at: row.get(4)?,
                content_hash: row.get(5)?,
                file_kind: parse_file_kind(&kind_str),
                sync_status: parse_sync_status(&status_str),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(files)
}

pub fn index_share_files(conn: &Connection, share_id: &str, root_path: &str) -> Result<()> {
    let root = Path::new(root_path);
    if !root.exists() {
        return Ok(());
    }
    index_dir(conn, share_id, root, root)?;
    Ok(())
}

fn index_dir(conn: &Connection, share_id: &str, root: &Path, dir: &Path) -> Result<()> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let relative_str = relative.to_string_lossy().to_string();

        if path.is_dir() {
            index_dir(conn, share_id, root, &path)?;
        } else {
            // Never index in-progress transfer temp files.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".soffit-part") {
                continue;
            }
            let metadata = std::fs::metadata(&path).ok();
            let size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
            let modified = metadata.and_then(|m| m.modified().ok()).map(|t| {
                let dt: chrono::DateTime<Utc> = t.into();
                dt.to_rfc3339()
            });
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let kind = FileKind::from_extension(ext);
            // Reuse the stored content hash when the file is unchanged (same
            // size and mtime) so frequent background re-sync cycles only stat
            // files; otherwise recompute so the sync diff always reflects the
            // real content. Unreadable files keep a NULL hash and are skipped.
            let (prev_size, prev_modified, prev_hash): (Option<i64>, Option<String>, Option<String>) =
                conn.query_row(
                    "SELECT size_bytes, modified_at, content_hash FROM file_index
                     WHERE share_id = ?1 AND relative_path = ?2",
                    rusqlite::params![share_id, relative_str],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap_or((None, None, None));
            let content_hash = if prev_hash.is_some()
                && prev_size == Some(size)
                && prev_modified == modified
            {
                prev_hash
            } else {
                crate::conflicts::hash_file(&path).ok()
            };

            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO file_index (id, share_id, relative_path, size_bytes, modified_at, content_hash, file_kind, sync_status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'synced')
                 ON CONFLICT(share_id, relative_path) DO UPDATE SET size_bytes = ?4, modified_at = ?5, content_hash = ?6, file_kind = ?7",
                rusqlite::params![id, share_id, relative_str, size, modified, content_hash, kind.to_string()],
            )?;
        }
    }
    Ok(())
}

pub fn parse_file_kind(s: &str) -> FileKind {
    match s {
        "excel" => FileKind::Excel,
        "sql" => FileKind::Sql,
        "csv" => FileKind::Csv,
        "sqlite" => FileKind::Sqlite,
        "parquet" => FileKind::Parquet,
        "image" => FileKind::Image,
        "pdf" => FileKind::Pdf,
        "text" => FileKind::Text,
        _ => FileKind::Generic,
    }
}

pub fn parse_sync_status(s: &str) -> SyncStatus {
    match s {
        "syncing" => SyncStatus::Syncing,
        "conflict" => SyncStatus::Conflict,
        "locked" => SyncStatus::Locked,
        "pending" => SyncStatus::Pending,
        _ => SyncStatus::Synced,
    }
}
