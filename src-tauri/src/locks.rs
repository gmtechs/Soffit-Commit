use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::Connection;
use uuid::Uuid;
use crate::models::Lock;

const DEFAULT_LOCK_TIMEOUT_MINUTES: i64 = 15;

pub fn acquire_lock(conn: &Connection, file_path: &str, peer_id: &str, peer_name: &str) -> Result<Lock> {
    // Check if already locked
    if let Some(existing) = get_lock(conn, file_path)? {
        if !is_lock_expired(&existing) {
            return Err(anyhow::anyhow!("File is locked by {}", existing.held_by_name));
        }
        // Expired lock — release it first
        release_lock_internal(conn, file_path)?;
    }

    let id = Uuid::new_v4().to_string();
    let acquired_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + Duration::minutes(DEFAULT_LOCK_TIMEOUT_MINUTES)).to_rfc3339();

    conn.execute(
        "INSERT INTO locks (id, file_path, held_by_peer_id, held_by_name, acquired_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, file_path, peer_id, peer_name, acquired_at, expires_at],
    )?;

    Ok(Lock { id, file_path: file_path.to_string(), held_by_peer_id: peer_id.to_string(), held_by_name: peer_name.to_string(), acquired_at, expires_at })
}

pub fn release_lock(conn: &Connection, file_path: &str, peer_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM locks WHERE file_path = ?1 AND held_by_peer_id = ?2",
        rusqlite::params![file_path, peer_id],
    )?;
    Ok(rows > 0)
}

fn release_lock_internal(conn: &Connection, file_path: &str) -> Result<()> {
    conn.execute("DELETE FROM locks WHERE file_path = ?1", rusqlite::params![file_path])?;
    Ok(())
}

pub fn get_lock(conn: &Connection, file_path: &str) -> Result<Option<Lock>> {
    let result = conn.query_row(
        "SELECT id, file_path, held_by_peer_id, held_by_name, acquired_at, expires_at FROM locks WHERE file_path = ?1",
        rusqlite::params![file_path],
        |row| Ok(Lock {
            id: row.get(0)?,
            file_path: row.get(1)?,
            held_by_peer_id: row.get(2)?,
            held_by_name: row.get(3)?,
            acquired_at: row.get(4)?,
            expires_at: row.get(5)?,
        }),
    );
    match result {
        Ok(lock) => Ok(Some(lock)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn list_active_locks(conn: &Connection) -> Result<Vec<Lock>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT id, file_path, held_by_peer_id, held_by_name, acquired_at, expires_at
         FROM locks WHERE expires_at > ?1 ORDER BY acquired_at DESC"
    )?;
    let locks = stmt.query_map(rusqlite::params![now], |row| {
        Ok(Lock {
            id: row.get(0)?,
            file_path: row.get(1)?,
            held_by_peer_id: row.get(2)?,
            held_by_name: row.get(3)?,
            acquired_at: row.get(4)?,
            expires_at: row.get(5)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(locks)
}

pub fn cleanup_expired_locks(conn: &Connection) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute("DELETE FROM locks WHERE expires_at <= ?1", rusqlite::params![now])?;
    Ok(rows)
}

fn is_lock_expired(lock: &Lock) -> bool {
    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&lock.expires_at) {
        expires < Utc::now()
    } else {
        false
    }
}
