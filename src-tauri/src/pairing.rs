/// Pairing: short human-readable code that embeds the iroh EndpointAddr.
/// Format: XXXX-XXXX:<base64-encoded EndpointAddr JSON>
/// The short code part is for display; the full string is what gets exchanged.
use anyhow::Result;
use chrono::{Duration, Utc};
use rand::Rng;
use base32::Alphabet;
use rusqlite::Connection;
use uuid::Uuid;
use base64::Engine as _;
use crate::models::PairingCode;

/// Generate a pairing code that embeds the endpoint addr for direct dialing.
/// `endpoint_addr_json` is the serialized iroh EndpointAddr (or empty string
/// if iroh hasn't started yet — the short code still works for manual entry).
pub fn generate_pairing_code(conn: &Connection, endpoint_addr_json: Option<&str>) -> Result<PairingCode> {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..5).map(|_| rng.gen::<u8>()).collect();
    let encoded = base32::encode(Alphabet::RFC4648 { padding: false }, &bytes);
    let short = format!("{}-{}", &encoded[..4], &encoded[4..8]);

    // Full code = SHORT:base64(endpoint_addr_json) so the other side can dial directly
    let full_code = if let Some(addr) = endpoint_addr_json.filter(|s| !s.is_empty()) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(addr.as_bytes());
        format!("{}:{}", short, b64)
    } else {
        short.clone()
    };

    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + Duration::minutes(15)).to_rfc3339();

    conn.execute(
        "INSERT INTO pairing_codes (id, code, created_at, expires_at, used) VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![id, full_code, created_at, expires_at],
    )?;

    Ok(PairingCode { id, code: full_code, created_at, expires_at })
}

/// Parse a pairing code entered by the user.
/// Returns (short_display_code, Option<endpoint_addr_json>)
pub fn parse_pairing_code(code: &str) -> (String, Option<String>) {
    let code = code.trim();
    if let Some(colon_pos) = code.find(':') {
        let short = code[..colon_pos].to_string();
        let b64 = &code[colon_pos + 1..];
        let addr = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .ok()
            .and_then(|b| String::from_utf8(b).ok());
        (short, addr)
    } else {
        (code.to_string(), None)
    }
}

/// Validate and consume a pairing code. Returns the embedded endpoint addr if present.
pub fn consume_pairing_code(conn: &Connection, code: &str) -> Result<Option<String>> {
    let now = Utc::now().to_rfc3339();
    let (_, addr) = parse_pairing_code(code);

    // Match on the full code string
    let rows = conn.execute(
        "UPDATE pairing_codes SET used = 1 WHERE code = ?1 AND used = 0 AND expires_at > ?2",
        rusqlite::params![code, now],
    )?;

    if rows == 0 {
        // Also try matching just the short prefix (user may have typed only XXXX-XXXX)
        let short = code.split(':').next().unwrap_or(code);
        let full: Option<String> = conn.query_row(
            "SELECT code FROM pairing_codes WHERE code LIKE ?1 AND used = 0 AND expires_at > ?2 LIMIT 1",
            rusqlite::params![format!("{}%", short), now],
            |row| row.get(0),
        ).ok();

        if let Some(ref full_code) = full {
            conn.execute(
                "UPDATE pairing_codes SET used = 1 WHERE code = ?1",
                rusqlite::params![full_code],
            )?;
            let (_, embedded) = parse_pairing_code(full_code);
            return Ok(embedded);
        }
        return Err(anyhow::anyhow!("Code not found, already used, or expired"));
    }

    Ok(addr)
}

/// Store the refreshed endpoint addr for a peer (called after each successful connection)
pub fn update_peer_endpoint(conn: &Connection, node_id: &str, endpoint_addr_json: &str) -> Result<()> {
    let last_seen = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE peers SET endpoint_addr = ?1, last_seen = ?2, is_online = 1 WHERE node_id = ?3",
        rusqlite::params![endpoint_addr_json, last_seen, node_id],
    )?;
    Ok(())
}

/// Get all peers that have a stored endpoint addr (for auto-reconnect on startup)
pub fn get_reconnectable_peers(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, endpoint_addr FROM peers WHERE endpoint_addr IS NOT NULL AND endpoint_addr != ''"
    )?;
    let peers = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?.filter_map(|r| r.ok()).collect();
    Ok(peers)
}
