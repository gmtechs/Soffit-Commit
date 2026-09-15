/// Conflict detection: compare content hashes of a file between two versions.
/// When two offline edits diverge on reconnect, this is how we catch it.
use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    KeepMine,
    KeepTheirs,
    KeepBoth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub id: String,
    pub file_path: String,
    pub local_hash: String,
    pub remote_hash: String,
    pub remote_peer_id: String,
    pub detected_at: String,
    pub resolved: bool,
    pub resolution: Option<String>,
}

pub fn ensure_conflicts_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS conflicts (
            id              TEXT PRIMARY KEY,
            file_path       TEXT NOT NULL,
            local_hash      TEXT NOT NULL,
            remote_hash     TEXT NOT NULL,
            remote_peer_id  TEXT NOT NULL,
            detected_at     TEXT NOT NULL,
            resolved        INTEGER NOT NULL DEFAULT 0,
            resolution      TEXT
        );
    ",
    )?;
    Ok(())
}

/// Hash a file's bytes
pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// Record a conflict when diverged hashes are detected on reconnect
pub fn record_conflict(
    conn: &Connection,
    file_path: &str,
    local_hash: &str,
    remote_hash: &str,
    remote_peer_id: &str,
) -> Result<Conflict> {
    let id = Uuid::new_v4().to_string();
    let detected_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO conflicts (id, file_path, local_hash, remote_hash, remote_peer_id, detected_at, resolved)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        rusqlite::params![id, file_path, local_hash, remote_hash, remote_peer_id, detected_at],
    )?;
    Ok(Conflict {
        id,
        file_path: file_path.to_string(),
        local_hash: local_hash.to_string(),
        remote_hash: remote_hash.to_string(),
        remote_peer_id: remote_peer_id.to_string(),
        detected_at,
        resolved: false,
        resolution: None,
    })
}

/// List unresolved conflicts
pub fn list_conflicts(conn: &Connection) -> Result<Vec<Conflict>> {
    let mut stmt = conn.prepare(
        "SELECT id, file_path, local_hash, remote_hash, remote_peer_id, detected_at, resolved, resolution
         FROM conflicts WHERE resolved = 0 ORDER BY detected_at DESC"
    )?;
    let items = stmt
        .query_map([], |row| {
            Ok(Conflict {
                id: row.get(0)?,
                file_path: row.get(1)?,
                local_hash: row.get(2)?,
                remote_hash: row.get(3)?,
                remote_peer_id: row.get(4)?,
                detected_at: row.get(5)?,
                resolved: row.get::<_, i32>(6)? != 0,
                resolution: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

/// Resolve a conflict
pub fn resolve_conflict(
    conn: &Connection,
    conflict_id: &str,
    resolution: ConflictResolution,
    local_path: &Path,
    their_bytes: Option<&[u8]>,
) -> Result<()> {
    let res_str = match resolution {
        ConflictResolution::KeepMine => "keep_mine",
        ConflictResolution::KeepTheirs => "keep_theirs",
        ConflictResolution::KeepBoth => "keep_both",
    };

    match resolution {
        ConflictResolution::KeepMine => {
            // Nothing to do — local file is already the "mine" version
        }
        ConflictResolution::KeepTheirs => {
            if let Some(bytes) = their_bytes {
                std::fs::write(local_path, bytes)?;
            }
        }
        ConflictResolution::KeepBoth => {
            if let Some(bytes) = their_bytes {
                // Write theirs with a .conflict suffix
                let conflict_path = local_path.with_extension(format!(
                    "{}.conflict",
                    local_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                ));
                std::fs::write(conflict_path, bytes)?;
            }
        }
    }

    conn.execute(
        "UPDATE conflicts SET resolved = 1, resolution = ?1 WHERE id = ?2",
        rusqlite::params![res_str, conflict_id],
    )?;
    Ok(())
}
