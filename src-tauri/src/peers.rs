use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;
use crate::models::{Peer, PermissionLevel, SharePermission};

pub fn add_peer(conn: &Connection, node_id: &str, display_name: &str, public_key: &str) -> Result<Peer> {
    let id = Uuid::new_v4().to_string();
    let last_seen = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO peers (id, node_id, display_name, public_key, trust_status, last_seen, is_online)
         VALUES (?1, ?2, ?3, ?4, 'trusted', ?5, 1)",
        rusqlite::params![id, node_id, display_name, public_key, last_seen],
    )?;

    Ok(Peer {
        id,
        node_id: node_id.to_string(),
        display_name: display_name.to_string(),
        public_key: public_key.to_string(),
        trust_status: "trusted".to_string(),
        last_seen: Some(last_seen),
        is_online: true,
    })
}

pub fn list_peers(conn: &Connection) -> Result<Vec<Peer>> {
    let mut stmt = conn.prepare(
        "SELECT id, node_id, display_name, public_key, trust_status, last_seen, is_online FROM peers ORDER BY display_name"
    )?;
    let peers = stmt.query_map([], |row| {
        Ok(Peer {
            id: row.get(0)?,
            node_id: row.get(1)?,
            display_name: row.get(2)?,
            public_key: row.get(3)?,
            trust_status: row.get(4)?,
            last_seen: row.get(5)?,
            is_online: row.get::<_, i32>(6)? != 0,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(peers)
}

pub fn remove_peer(conn: &Connection, peer_id: &str) -> Result<()> {
    conn.execute("DELETE FROM share_permissions WHERE peer_id = ?1", rusqlite::params![peer_id])?;
    conn.execute("DELETE FROM peers WHERE id = ?1", rusqlite::params![peer_id])?;
    Ok(())
}

pub fn rename_peer(conn: &Connection, peer_id: &str, new_name: &str) -> Result<()> {
    conn.execute(
        "UPDATE peers SET display_name = ?1 WHERE id = ?2",
        rusqlite::params![new_name, peer_id],
    )?;
    Ok(())
}

pub fn set_peer_online(conn: &Connection, peer_id: &str, online: bool) -> Result<()> {
    let last_seen = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE peers SET is_online = ?1, last_seen = ?2 WHERE id = ?3",
        rusqlite::params![online as i32, last_seen, peer_id],
    )?;
    Ok(())
}

pub fn set_permission(
    conn: &Connection,
    share_id: &str,
    peer_id: &str,
    level: PermissionLevel,
) -> Result<SharePermission> {
    let id = Uuid::new_v4().to_string();
    let updated_at = Utc::now().to_rfc3339();
    let level_str = level.to_string();

    conn.execute(
        "INSERT INTO share_permissions (id, share_id, peer_id, level, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(share_id, peer_id) DO UPDATE SET level = ?4, updated_at = ?5",
        rusqlite::params![id, share_id, peer_id, level_str, updated_at],
    )?;

    Ok(SharePermission { id, share_id: share_id.to_string(), peer_id: peer_id.to_string(), level, updated_at })
}

pub fn get_permission(conn: &Connection, share_id: &str, peer_id: &str) -> Result<PermissionLevel> {
    let level: String = conn.query_row(
        "SELECT level FROM share_permissions WHERE share_id = ?1 AND peer_id = ?2",
        rusqlite::params![share_id, peer_id],
        |row| row.get(0),
    ).unwrap_or_else(|_| "none".to_string());
    Ok(PermissionLevel::from(level.as_str()))
}
