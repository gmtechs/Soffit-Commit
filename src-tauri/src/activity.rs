use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;
use crate::models::ActivityEntry;

pub fn log_activity(conn: &Connection, actor: &str, action: &str, target: &str, metadata: Option<&str>) -> Result<ActivityEntry> {
    let id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO activity_log (id, timestamp, actor, action, target, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, timestamp, actor, action, target, metadata],
    )?;

    Ok(ActivityEntry {
        id,
        timestamp,
        actor: actor.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        metadata: metadata.map(String::from),
    })
}

pub fn list_activity(conn: &Connection, limit: usize) -> Result<Vec<ActivityEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, actor, action, target, metadata FROM activity_log ORDER BY timestamp DESC LIMIT ?1"
    )?;
    let entries = stmt.query_map(rusqlite::params![limit as i64], |row| {
        Ok(ActivityEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            actor: row.get(2)?,
            action: row.get(3)?,
            target: row.get(4)?,
            metadata: row.get(5)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(entries)
}
