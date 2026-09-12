use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;
use std::path::Path;
use crate::models::{Share, FileIndex, FileKind, SyncStatus};

pub fn create_share(conn: &Connection, path: &str, display_name: &str) -> Result<Share> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO shares (id, path, display_name, is_owner, selective_sync, created_at) VALUES (?1, ?2, ?3, 1, 0, ?4)",
        rusqlite::params![id, path, display_name, created_at],
    )?;

    index_share_files(conn, &id, path)?;

    Ok(Share { id, path: path.to_string(), display_name: display_name.to_string(), is_owner: true, selective_sync: false, created_at })
}

pub fn list_shares(conn: &Connection) -> Result<Vec<Share>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, display_name, is_owner, selective_sync, created_at FROM shares ORDER BY display_name"
    )?;
    let shares = stmt.query_map([], |row| {
        Ok(Share {
            id: row.get(0)?,
            path: row.get(1)?,
            display_name: row.get(2)?,
            is_owner: row.get::<_, i32>(3)? != 0,
            selective_sync: row.get::<_, i32>(4)? != 0,
            created_at: row.get(5)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(shares)
}

pub fn set_selective_sync(conn: &Connection, share_id: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE shares SET selective_sync = ?1 WHERE id = ?2",
        rusqlite::params![enabled as i32, share_id],
    )?;
    Ok(())
}

pub fn delete_share(conn: &Connection, share_id: &str) -> Result<()> {
    conn.execute("DELETE FROM share_permissions WHERE share_id = ?1", rusqlite::params![share_id])?;
    conn.execute("DELETE FROM file_index WHERE share_id = ?1", rusqlite::params![share_id])?;
    conn.execute("DELETE FROM shares WHERE id = ?1", rusqlite::params![share_id])?;
    Ok(())
}

pub fn list_files_in_share(conn: &Connection, share_id: &str) -> Result<Vec<FileIndex>> {
    let mut stmt = conn.prepare(
        "SELECT id, share_id, relative_path, size_bytes, modified_at, content_hash, file_kind, sync_status
         FROM file_index WHERE share_id = ?1 ORDER BY relative_path"
    )?;
    let files = stmt.query_map(rusqlite::params![share_id], |row| {
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
    })?.filter_map(|r| r.ok()).collect();
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
            let metadata = std::fs::metadata(&path).ok();
            let size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
            let modified = metadata.and_then(|m| m.modified().ok()).map(|t| {
                let dt: chrono::DateTime<Utc> = t.into();
                dt.to_rfc3339()
            });
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let kind = FileKind::from_extension(ext);

            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO file_index (id, share_id, relative_path, size_bytes, modified_at, file_kind, sync_status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'synced')
                 ON CONFLICT(share_id, relative_path) DO UPDATE SET size_bytes = ?4, modified_at = ?5, file_kind = ?6",
                rusqlite::params![id, share_id, relative_str, size, modified, kind.to_string()],
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
