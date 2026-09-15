/// Lightweight version history: on every save, snapshot the file content
/// with a SHA-256 hash (dedup by content — same content costs nothing extra).
use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub file_path: String,
    pub hash: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub actor: String,
    pub snapshot_path: String, // path in the versions store dir
}

pub fn ensure_versions_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS file_versions (
            id            TEXT PRIMARY KEY,
            file_path     TEXT NOT NULL,
            hash          TEXT NOT NULL,
            size_bytes     INTEGER NOT NULL,
            created_at    TEXT NOT NULL,
            actor         TEXT NOT NULL,
            snapshot_path TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_versions_path ON file_versions(file_path, created_at DESC);
    ",
    )?;
    Ok(())
}

/// Snapshot a file. If the content hash already exists for this path, skip (dedup).
pub fn snapshot_file(
    conn: &Connection,
    versions_dir: &Path,
    file_path: &Path,
    actor: &str,
) -> Result<Option<Version>> {
    let bytes = std::fs::read(file_path)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let size_bytes = bytes.len() as u64;

    // Check for duplicate content for this file
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM file_versions WHERE file_path = ?1 AND hash = ?2 ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![file_path.to_string_lossy().as_ref(), hash],
        |row| row.get(0),
    ).ok();
    if existing.is_some() {
        return Ok(None); // No change, skip snapshot
    }

    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    std::fs::create_dir_all(versions_dir)?;
    let snapshot_path = versions_dir.join(format!("{}.bin", id));
    std::fs::write(&snapshot_path, &bytes)?;

    let fp = file_path.to_string_lossy().to_string();
    let sp = snapshot_path.to_string_lossy().to_string();

    conn.execute(
        "INSERT INTO file_versions (id, file_path, hash, size_bytes, created_at, actor, snapshot_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, fp, hash, size_bytes as i64, created_at, actor, sp],
    )?;

    Ok(Some(Version {
        id,
        file_path: fp,
        hash,
        size_bytes,
        created_at,
        actor: actor.to_string(),
        snapshot_path: sp,
    }))
}

/// List all versions for a file, newest first
pub fn list_versions(conn: &Connection, file_path: &str) -> Result<Vec<Version>> {
    let mut stmt = conn.prepare(
        "SELECT id, file_path, hash, size_bytes, created_at, actor, snapshot_path
         FROM file_versions WHERE file_path = ?1 ORDER BY created_at DESC",
    )?;
    let versions = stmt
        .query_map(rusqlite::params![file_path], |row| {
            Ok(Version {
                id: row.get(0)?,
                file_path: row.get(1)?,
                hash: row.get(2)?,
                size_bytes: row.get::<_, i64>(3)? as u64,
                created_at: row.get(4)?,
                actor: row.get(5)?,
                snapshot_path: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(versions)
}

/// Restore a version: copy snapshot bytes back to the original path
pub fn restore_version(conn: &Connection, version_id: &str) -> Result<String> {
    let (file_path, snapshot_path): (String, String) = conn.query_row(
        "SELECT file_path, snapshot_path FROM file_versions WHERE id = ?1",
        rusqlite::params![version_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let bytes = std::fs::read(&snapshot_path)?;
    std::fs::write(&file_path, bytes)?;
    Ok(file_path)
}
