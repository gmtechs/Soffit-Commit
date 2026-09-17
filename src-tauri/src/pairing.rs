use crate::models::PairingCode;
/// Pairing codes.
///
/// There are two forms, for two situations:
///
/// 1. **Short code** — `K7F2-QP3M`, 8 characters. This is what a person reads
///    out or types. It carries no address, so it is resolved on the local
///    network by `network::rendezvous` (see that module for why a short code
///    cannot be self-contained).
/// 2. **Portable code** — the device's node id in Crockford base32, 52
///    characters shown in blocks of four. It is the device's identity, not a
///    secret, and it needs no address lookup: iroh's DNS discovery finds the
///    device through its relay, so this form works across networks while
///    staying human-readable — the same alphabet treatment as the short code,
///    just longer because it carries the device's actual identity.
use anyhow::Result;
use base64::Engine as _;
use chrono::{Duration, Utc};
use rand::Rng;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Short-code alphabet, Crockford-style: no `0/O`, `1/I/L` or `U`, so a code
/// can be read aloud or copied by hand without ambiguity.
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 8 characters from a 30-symbol alphabet ≈ 39 bits — with a 15-minute expiry
/// and a local-network-only reach that is far more than enough.
pub const SHORT_CODE_LEN: usize = 8;
pub const CODE_TTL_MINUTES: i64 = 15;

fn random_short_code() -> String {
    let mut rng = rand::thread_rng();
    (0..SHORT_CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// `K7F2QP3M` → `K7F2-QP3M`. Grouping in fours is what makes a code easy to
/// read out loud.
pub fn display_code(code: &str) -> String {
    let cleaned: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    cleaned
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
        .to_ascii_uppercase()
}

/// The short-code portion of a stored or portable code.
pub fn short_code_of(code: &str) -> String {
    code.split(':').next().unwrap_or(code).trim().to_string()
}

/// True when `code` is a well-formed short code.
pub fn is_short_code(code: &str) -> bool {
    let normalized = crate::network::rendezvous::normalize(code);
    normalized.len() == SHORT_CODE_LEN
        && normalized.bytes().all(|b| CODE_ALPHABET.contains(&b))
}

/// Portable-code alphabet: full Crockford base32. Unlike the short code it
/// carries real data (the device's node id), so all 32 symbols are used; the
/// ambiguous look-alikes (`O`→`0`, `I`/`L`→`1`) are folded back on input.
const PORTABLE_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 32 bytes of node id = 256 bits = 52 base32 symbols: 13 blocks of four.
const PORTABLE_LEN: usize = 52;

/// Clean user input for the portable form: separators out, uppercase, and the
/// classic mis-reads folded back onto alphabet symbols.
pub fn normalize_portable(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

/// True when `code` is a well-formed portable code (a node id in base32).
pub fn is_portable_code(code: &str) -> bool {
    let normalized = normalize_portable(code);
    normalized.len() == PORTABLE_LEN
        && normalized.bytes().all(|b| PORTABLE_ALPHABET.contains(&b))
}

/// Encode raw bytes as Crockford base32 (no padding characters; trailing
/// bits are zero).
fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &byte in bytes {
        buf = (buf << 8) | byte as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(PORTABLE_ALPHABET[((buf >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(PORTABLE_ALPHABET[((buf << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

/// Decode Crockford base32 back to raw bytes. `None` for characters outside
/// the alphabet.
fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for c in text.chars() {
        let value = PORTABLE_ALPHABET.iter().position(|&a| a == c as u8)? as u32;
        buf = (buf << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(bytes)
}

/// The portable code for a device: its node id in Crockford base32. 52
/// characters that group into blocks of four — the same human treatment as
/// the short code, one identity instead of an address dump.
pub fn portable_code_for_node(node_id: &str) -> Result<String> {
    use std::str::FromStr;
    let key = iroh::PublicKey::from_str(node_id)
        .map_err(|_| anyhow::anyhow!("invalid node id"))?;
    Ok(base32_encode(key.as_bytes()))
}

/// The node id a portable code stands for, as 64 lowercase hex characters.
fn node_id_from_portable(portable: &str) -> Result<String> {
    let bytes = base32_decode(&normalize_portable(portable))
        .filter(|b| b.len() >= 32)
        .ok_or_else(|| anyhow::anyhow!("The connection code is malformed"))?;
    let arr: [u8; 32] = bytes[..32]
        .try_into()
        .expect("at least 32 bytes are decoded");
    iroh::PublicKey::from_bytes(&arr)
        .map(|key| key.to_string())
        .map_err(|_| anyhow::anyhow!("The connection code is malformed"))
}

/// The portable portion of a stored pairing code: everything after the short
/// prefix, or the whole value when no address was available at mint time.
pub fn portable_of(stored_code: &str) -> String {
    match stored_code.split_once(':') {
        Some((_, portable)) => portable.trim().to_string(),
        None => stored_code.trim().to_string(),
    }
}

/// Legacy ticket (the first format): the peer's full `EndpointAddr` plus an
/// expiry, base64-encoded. Still decoded so codes minted by older builds keep
/// pairing; new codes carry only the node id instead.
#[derive(Debug, Serialize, Deserialize)]
struct PairingTicket {
    version: u8,
    endpoint_addr: String,
    expires_at: String,
}

/// Mint a new short code and remember it, so this device can recognise its own
/// code later (and reject someone typing it back in on the wrong device).
pub fn generate_pairing_code(
    conn: &Connection,
    endpoint_addr_json: Option<&str>,
) -> Result<PairingCode> {
    // Collisions are astronomically unlikely, but the column is UNIQUE, so
    // retry rather than surface a constraint error to the user.
    let mut short = random_short_code();
    for _ in 0..8 {
        let taken: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pairing_codes WHERE code LIKE ?1",
            rusqlite::params![format!("{short}%")],
            |row| row.get(0),
        )?;
        if taken == 0 {
            break;
        }
        short = random_short_code();
    }

    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + Duration::minutes(CODE_TTL_MINUTES)).to_rfc3339();

    let full_code = match endpoint_addr_json.filter(|s| !s.is_empty()) {
        Some(addr) => {
            match endpoint_node_id(addr).and_then(|node| portable_code_for_node(&node)) {
                // Current form: just the node id in readable base32. The
                // address itself is not carried — iroh's discovery resolves
                // the device through its relay, and a node id cannot expire.
                Ok(portable) => format!("{short}:{portable}"),
                // A device whose address cannot be reduced to a node id keeps
                // the legacy ticket so cross-network pairing still works.
                Err(_) => {
                    let ticket = PairingTicket {
                        version: 1,
                        endpoint_addr: addr.to_string(),
                        expires_at: expires_at.clone(),
                    };
                    let encoded = serde_json::to_vec(&ticket)?;
                    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(encoded);
                    format!("{short}:{b64}")
                }
            }
        }
        None => short.clone(),
    };

    conn.execute(
        "INSERT INTO pairing_codes (id, code, created_at, expires_at, used) VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![id, full_code, created_at, expires_at],
    )?;

    Ok(PairingCode {
        id,
        code: full_code,
        created_at,
        expires_at,
    })
}

/// Decode a full cross-device code and return the peer's address.
///
/// Two payload formats are understood: the current one (the peer's node id in
/// Crockford base32, which discovery resolves through its relay) and the
/// legacy base64 ticket carrying a full `EndpointAddr`.
fn decode_remote_ticket(code: &str) -> Result<String> {
    let (_, encoded) = code
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("Enter the full connection code from the other device"))?;
    let encoded = encoded.trim();

    if is_portable_code(encoded) {
        let node_id = node_id_from_portable(encoded)?;
        use std::str::FromStr;
        let key = iroh::PublicKey::from_str(&node_id)
            .map_err(|_| anyhow::anyhow!("The connection code is malformed"))?;
        return Ok(serde_json::to_string(&iroh::EndpointAddr::from(key))?);
    }

    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(encoded))
        .map_err(|_| anyhow::anyhow!("The connection code is malformed"))?;

    if let Ok(ticket) = serde_json::from_slice::<PairingTicket>(&bytes) {
        if ticket.version != 1 || ticket.endpoint_addr.trim().is_empty() {
            return Err(anyhow::anyhow!("The connection code is invalid"));
        }
        let expires_at = chrono::DateTime::parse_from_rfc3339(&ticket.expires_at)
            .map_err(|_| anyhow::anyhow!("The connection code has an invalid expiry"))?;
        if expires_at <= Utc::now() {
            return Err(anyhow::anyhow!(
                "This connection code has expired. Generate a new one."
            ));
        }
        return Ok(ticket.endpoint_addr);
    }

    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("The connection code is malformed"))
}

/// Validate a pairing code that a user typed.
///
/// Returns either an `EndpointAddr` JSON string (already-resolved ticket) or a
/// bare 8-character short code for the caller to resolve on the local network.
/// It never dials anything itself, so the DB lock is held only briefly.
pub fn consume_pairing_code(conn: &Connection, code: &str) -> Result<Option<String>> {
    let code = code.trim();
    if code.is_empty() {
        return Err(anyhow::anyhow!(
            "Enter the code shown on the other device."
        ));
    }

    // 1. Portable ticket. It embeds the peer's address, so no lookup is needed
    //    and it works even when the two devices are on different networks.
    if code.contains(':') {
        return decode_remote_ticket(code).map(Some);
    }

    // 2. Short code — the human-friendly path.
    if is_short_code(code) {
        let short = crate::network::rendezvous::normalize(code);
        // If we minted this code ourselves, the peer is the one who must type
        // it. Typing it back on the device that issued it is a user error, and
        // saying so is far clearer than dialling ourselves.
        let ours: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pairing_codes WHERE code LIKE ?1 AND expires_at > ?2",
            rusqlite::params![format!("{short}%"), Utc::now().to_rfc3339()],
            |row| row.get(0),
        )?;
        if ours > 0 {
            return Err(anyhow::anyhow!(
                "{} is this device's own code — type it on the other device instead.",
                display_code(&short)
            ));
        }
        return Ok(Some(short));
    }

    // 3. Portable code — the other device's node id in base32. iroh's own
    //    discovery resolves the device through its relay, so this works even
    //    when the two devices are on different networks.
    let portable = normalize_portable(code);
    if is_portable_code(&portable) {
        let ours: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pairing_codes WHERE code LIKE ?1",
            rusqlite::params![format!("%:{portable}")],
            |row| row.get(0),
        )?;
        if ours > 0 {
            return Err(anyhow::anyhow!(
                "That is this device's own code — type the code the other device shows."
            ));
        }
        return Ok(Some(node_id_from_portable(&portable)?));
    }

    // 4. A full node id (64 hex characters) from an older build. It carries no
    //    address, but iroh's own discovery can still resolve it, so it stays
    //    accepted instead of being rejected outright.
    let hex: String = code.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 64 {
        return Ok(Some(hex.to_ascii_lowercase()));
    }

    Err(anyhow::anyhow!(
        "That code does not look right. Codes are 8 characters, like K7F2-QP3M, or the longer code shown under \"Not on the same network?\"."
    ))
}

/// Get the cryptographically authenticated iroh endpoint ID from a ticket.
pub fn endpoint_node_id(endpoint_addr_json: &str) -> Result<String> {
    let addr: iroh::EndpointAddr = serde_json::from_str(endpoint_addr_json)
        .map_err(|e| anyhow::anyhow!("Invalid endpoint address in connection code: {e}"))?;
    Ok(addr.id.to_string())
}

/// Store the refreshed endpoint addr for a peer (called after each successful connection)
pub fn update_peer_endpoint(
    conn: &Connection,
    node_id: &str,
    endpoint_addr_json: &str,
) -> Result<()> {
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
    let peers = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE pairing_codes (id TEXT PRIMARY KEY, code TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL, expires_at TEXT NOT NULL, used INTEGER NOT NULL DEFAULT 0);"
        ).unwrap();
        conn
    }

    #[test]
    fn a_full_ticket_is_consumed_without_a_local_database_row() {
        let issuer = db();
        let code = generate_pairing_code(&issuer, Some(r#"{"id":"example","addrs":[]}"#)).unwrap();
        let receiver = db();
        let result = consume_pairing_code(&receiver, &code.code).unwrap();
        assert_eq!(result.unwrap(), r#"{"id":"example","addrs":[]}"#.to_string());
    }

    #[test]
    fn short_code_is_accepted_for_local_resolution() {
        let receiver = db();
        // No ticket, no local row: the address is resolved over the LAN by the
        // rendezvous module, so validation must accept the code itself.
        let result = consume_pairing_code(&receiver, "K7F2-QP3M").unwrap();
        assert_eq!(result.unwrap(), "K7F2QP3M");
    }

    #[test]
    fn own_short_code_is_rejected_with_a_clear_message() {
        let conn = db();
        let code = generate_pairing_code(&conn, None).unwrap();
        let err = consume_pairing_code(&conn, &code.code).unwrap_err().to_string();
        assert!(err.contains("this device's own code"), "got: {err}");
    }

    #[test]
    fn expired_portable_ticket_is_rejected() {
        let ticket = PairingTicket {
            version: 1,
            endpoint_addr: "{}".into(),
            expires_at: (Utc::now() - Duration::minutes(1)).to_rfc3339(),
        };
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&ticket).unwrap());
        assert!(consume_pairing_code(&db(), &format!("ABCD-EFGH:{encoded}")).is_err());
    }

    #[test]
    fn malformed_code_is_rejected() {
        let conn = db();
        // Right length, but contains characters outside the alphabet.
        assert!(consume_pairing_code(&conn, "ABCDEFGI").is_err());
        assert!(consume_pairing_code(&conn, "").is_err());
    }

    #[test]
    fn display_code_groups_in_fours() {
        assert_eq!(display_code("k7f2qp3m"), "K7F2-QP3M");
        assert_eq!(display_code("K7F2QP3M"), "K7F2-QP3M");
    }

    #[test]
    fn short_code_of_strips_the_ticket() {
        assert_eq!(short_code_of("K7F2QP3M:eyJ2ZXJzaW9uIjoxfQ"), "K7F2QP3M");
        assert_eq!(short_code_of("K7F2QP3M"), "K7F2QP3M");
    }

    fn test_node_id() -> String {
        iroh::SecretKey::generate().public().to_string()
    }

    #[test]
    fn portable_code_is_readable_and_resolves_the_node_id() {
        let node_id = test_node_id();
        let issuer = db();
        let receiver = db();
        let code = generate_pairing_code(
            &issuer,
            Some(&format!(r#"{{"id":"{node_id}","addrs":[]}}"#)),
        )
        .unwrap();

        let portable = portable_of(&code.code);
        assert_eq!(portable.len(), 52, "a node id is exactly 52 base32 symbols");
        assert!(is_portable_code(&portable));
        // Human treatment: blocks of four, one alphabet, no base64 punctuation.
        let shown = display_code(&portable);
        assert_eq!(shown.matches('-').count(), 12);
        assert!(!portable.contains(':'));
        assert!(!portable.contains('_'));
        assert!(!portable.contains('='));

        // Typed back in lowercase with the dashes — still resolves.
        let typed = shown.to_lowercase();
        let resolved = consume_pairing_code(&receiver, &typed).unwrap().unwrap();
        assert_eq!(resolved, node_id, "a typed portable code yields the node id");

        // The colon form (as stored, pasted, or scanned) resolves too.
        let resolved = consume_pairing_code(&receiver, &code.code).unwrap().unwrap();
        assert!(resolved.contains(&node_id), "got: {resolved}");
    }

    #[test]
    fn typing_your_own_portable_code_is_rejected() {
        let node_id = test_node_id();
        let conn = db();
        let code = generate_pairing_code(
            &conn,
            Some(&format!(r#"{{"id":"{node_id}","addrs":[]}}"#)),
        )
        .unwrap();
        let err = consume_pairing_code(&conn, &portable_of(&code.code))
            .unwrap_err()
            .to_string();
        assert!(err.contains("own code"), "got: {err}");
    }

    #[test]
    fn legacy_ticket_from_an_older_build_still_pairs() {
        let node_id = test_node_id();
        let ticket = PairingTicket {
            version: 1,
            endpoint_addr: format!(r#"{{"id":"{node_id}","addrs":[]}}"#),
            expires_at: (Utc::now() + Duration::minutes(5)).to_rfc3339(),
        };
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&ticket).unwrap());
        let result = consume_pairing_code(&db(), &format!("ABCD-EFGH:{encoded}"))
            .unwrap()
            .unwrap();
        assert!(result.contains(&node_id), "got: {result}");
    }
}
