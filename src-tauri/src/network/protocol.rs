/// ALPN strings and message types for our custom iroh protocol
use serde::{Deserialize, Serialize};

/// ALPN identifier for the Soffit Commit sync protocol
pub const SOFFIT_ALPN: &[u8] = b"soffit-commit/sync/1";

/// Wire messages sent over iroh QUIC streams
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncMessage {
    /// Announce our identity and display name after connecting
    Hello { node_id: String, display_name: String },
    /// Request a file by its share_id + relative_path
    RequestFile { share_id: String, path: String },
    /// Permission check response
    PermissionDenied { reason: String },
    /// File metadata before bytes follow on next stream
    FileInfo { share_id: String, path: String, size: u64, hash: String },
    /// Lock acquired notification broadcast via gossip
    LockAcquired { file_path: String, peer_name: String },
    /// Lock released
    LockReleased { file_path: String },
    /// Permission changed
    PermissionChanged { share_id: String, peer_id: String, level: String },
    /// Presence ping
    Ping,
    /// Presence pong
    Pong { display_name: String },
}

/// Gossip topic IDs — derived from a fixed namespace
pub fn presence_topic() -> [u8; 32] {
    let mut b = [0u8; 32];
    let src = b"soffit-commit:presence:v1";
    b[..src.len()].copy_from_slice(src);
    b
}
