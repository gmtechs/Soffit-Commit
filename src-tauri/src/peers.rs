use crate::models::{Peer, PermissionLevel, SharePermission};
use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

pub fn add_peer(
    conn: &Connection,
    node_id: &str,
    display_name: &str,
    public_key: &str,
) -> Result<Peer> {
    let id = Uuid::new_v4().to_string();
    let last_seen = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO peers (id, node_id, display_name, public_key, trust_status, last_seen, is_online)
         VALUES (?1, ?2, ?3, ?4, 'trusted', ?5, 1)
         ON CONFLICT(node_id) DO UPDATE SET
           display_name = excluded.display_name,
           public_key = excluded.public_key,
           last_seen = excluded.last_seen,
           is_online = 1",
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

/// Start every app session pessimistically; a peer becomes online only after
/// the authenticated Hello/HelloAck handshake completes.
pub fn mark_all_offline(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE peers SET is_online = 0", [])?;
    Ok(())
}

pub fn set_peer_online_by_node_id(conn: &Connection, node_id: &str, online: bool) -> Result<()> {
    let last_seen = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE peers SET is_online = ?1, last_seen = ?2 WHERE node_id = ?3",
        rusqlite::params![online as i32, last_seen, node_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refreshing_a_peer_preserves_its_endpoint_address() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE peers (id TEXT PRIMARY KEY, node_id TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL, public_key TEXT NOT NULL, trust_status TEXT NOT NULL, last_seen TEXT, is_online INTEGER NOT NULL, endpoint_addr TEXT);").unwrap();
        add_peer(&conn, "node-a", "First", "key-a").unwrap();
        conn.execute(
            "UPDATE peers SET endpoint_addr = 'ticket' WHERE node_id = 'node-a'",
            [],
        )
        .unwrap();
        add_peer(&conn, "node-a", "Renamed", "key-b").unwrap();
        let row: (String, String, String) = conn.query_row("SELECT display_name, public_key, endpoint_addr FROM peers WHERE node_id = 'node-a'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(row, ("Renamed".into(), "key-b".into(), "ticket".into()));
    }
}

pub fn list_peers(conn: &Connection) -> Result<Vec<Peer>> {
    let mut stmt = conn.prepare(
        "SELECT id, node_id, display_name, public_key, trust_status, last_seen, is_online FROM peers ORDER BY display_name"
    )?;
    let peers = stmt
        .query_map([], |row| {
            Ok(Peer {
                id: row.get(0)?,
                node_id: row.get(1)?,
                display_name: row.get(2)?,
                public_key: row.get(3)?,
                trust_status: row.get(4)?,
                last_seen: row.get(5)?,
                is_online: row.get::<_, i32>(6)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(peers)
}

pub fn remove_peer(conn: &Connection, peer_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM share_permissions WHERE peer_id = ?1",
        rusqlite::params![peer_id],
    )?;
    conn.execute(
        "DELETE FROM peers WHERE id = ?1",
        rusqlite::params![peer_id],
    )?;
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

    Ok(SharePermission {
        id,
        share_id: share_id.to_string(),
        peer_id: peer_id.to_string(),
        level,
        updated_at,
    })
}

pub fn get_permission(conn: &Connection, share_id: &str, peer_id: &str) -> Result<PermissionLevel> {
    let level: String = conn
        .query_row(
            "SELECT level FROM share_permissions WHERE share_id = ?1 AND peer_id = ?2",
            rusqlite::params![share_id, peer_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "none".to_string());
    Ok(PermissionLevel::from(level.as_str()))
}

/// Permission level for a peer identified by its authenticated iroh node ID.
/// The sync engine uses this — the node_id is the transport-level identity,
/// so permission checks never depend on unauthenticated data.
pub fn get_permission_by_node(conn: &Connection, share_id: &str, node_id: &str) -> PermissionLevel {
    let level: String = conn
        .query_row(
            "SELECT sp.level FROM share_permissions sp
             JOIN peers p ON p.id = sp.peer_id
             WHERE sp.share_id = ?1 AND p.node_id = ?2",
            rusqlite::params![share_id, node_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "none".to_string());
    PermissionLevel::from(level.as_str())
}

/// (peer display_name, endpoint_addr JSON) for every peer that has a stored
/// address AND holds a non-`none` permission on the given share. Used to
/// re-trigger a push after a share is created or refreshed.
pub fn list_granted_peer_addrs(conn: &Connection, share_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT p.display_name, p.endpoint_addr
         FROM share_permissions sp
         JOIN peers p ON p.id = sp.peer_id
         WHERE sp.share_id = ?1 AND p.endpoint_addr IS NOT NULL AND p.endpoint_addr != ''
           AND sp.level != 'none'",
    )?;
    let peers = stmt
        .query_map(rusqlite::params![share_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(peers)
}

/// All permission rows for one peer — used by the Peers screen to preselect
/// each share's access dropdown.
pub fn list_permissions_for_peer(
    conn: &Connection,
    peer_id: &str,
) -> Result<Vec<SharePermission>> {
    let mut stmt = conn.prepare(
        "SELECT id, share_id, peer_id, level, updated_at FROM share_permissions WHERE peer_id = ?1",
    )?;
    let perms = stmt
        .query_map(rusqlite::params![peer_id], |row| {
            let level: String = row.get(3)?;
            Ok(SharePermission {
                id: row.get(0)?,
                share_id: row.get(1)?,
                peer_id: row.get(2)?,
                level: PermissionLevel::from(level.as_str()),
                updated_at: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(perms)
}
