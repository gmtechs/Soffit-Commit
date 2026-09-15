/// ALPN strings and message types for our custom iroh protocol
use serde::{Deserialize, Serialize};

/// ALPN identifier for the Soffit Commit sync protocol
pub const SOFFIT_ALPN: &[u8] = b"soffit-commit/sync/1";

/// One entry in a share's file index, exchanged before a transfer so each
/// side can compute which files the other side is missing or has changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub relative_path: String,
    pub size_bytes: i64,
    pub content_hash: String,
    pub modified_at: Option<String>,
}

/// Wire messages sent over iroh QUIC streams
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncMessage {
    /// Announce our identity and display name after connecting
    Hello {
        node_id: String,
        display_name: String,
        endpoint_addr: String,
    },
    /// Confirms that the remote device authenticated and recorded a Hello.
    /// A pairing command does not report success until this is received.
    HelloAck,
    /// Request a file by its share_id + relative_path
    RequestFile { share_id: String, path: String },
    /// Permission check response
    PermissionDenied { reason: String },
    /// Share owner → peer: grants visibility of a share. The receiving device
    /// creates a local share record (is_owner = 0) and a folder to receive into.
    ShareGrant {
        share_id: String,
        display_name: String,
        selective_sync: bool,
    },
    /// Ask the peer for their local file index of a share (so the owner can
    /// compute which files still need to be transferred).
    FileIndexRequest { share_id: String },
    /// Fire-and-forget hint that a share folder changed on the sender's side,
    /// so the receiving peer re-syncs immediately instead of waiting for its
    /// next poll tick (realtime sync, sync.rs SYNC_INTERVAL stays a fallback).
    LocalChange { share_id: String },
    /// Response carrying the peer's local file index (possibly empty).
    FileIndex {
        share_id: String,
        files: Vec<IndexedFile>,
    },
    /// File metadata. The header is followed, in the same uni stream, by an
    /// 8-byte big-endian payload length and then the raw bytes starting at
    /// `resume_from` (0 for a full transfer — modules §4: transfers resume
    /// instead of restarting from zero).
    FileInfo {
        share_id: String,
        path: String,
        size: u64,
        hash: String,
        #[serde(default)]
        resume_from: u64,
    },
    /// Ask a sender-side peer how many bytes of a file we already hold, so an
    /// interrupted transfer resumes instead of restarting (modules §4).
    ResumeQuery {
        share_id: String,
        path: String,
        hash: String,
    },
    /// Response: the receiver already holds `have_bytes` of exactly that
    /// content version (0 when nothing usable is on disk).
    ResumeInfo {
        share_id: String,
        path: String,
        have_bytes: u64,
    },
    /// A device was unpaired by `remover_node_id`. Delivered over gossip so
    /// the removed device learns it was cut off and both sides forget the
    /// pairing (modules §1).
    PeerRemoved {
        remover_node_id: String,
        removed_node_id: String,
    },
    /// Lock acquired notification broadcast via gossip
    LockAcquired {
        file_path: String,
        peer_name: String,
    },
    /// Lock released
    LockReleased { file_path: String },
    /// Permission changed
    PermissionChanged {
        share_id: String,
        peer_id: String,
        level: String,
    },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire compatibility guard: the serde "type" tags and field names of
    /// every sync message must stay stable across versions, otherwise paired
    /// devices running different builds could no longer understand each other.
    #[test]
    fn sync_messages_round_trip_with_stable_tags() {
        let cases: Vec<(SyncMessage, &str)> = vec![
            (
                SyncMessage::Hello {
                    node_id: "n1".into(),
                    display_name: "Alice".into(),
                    endpoint_addr: "{}".into(),
                },
                r#"{"type":"hello","node_id":"n1","display_name":"Alice","endpoint_addr":"{}"}"#,
            ),
            (SyncMessage::HelloAck, r#"{"type":"hello_ack"}"#),
            (
                SyncMessage::ShareGrant {
                    share_id: "s1".into(),
                    display_name: "Docs".into(),
                    selective_sync: false,
                },
                r#"{"type":"share_grant","share_id":"s1","display_name":"Docs","selective_sync":false}"#,
            ),
            (
                SyncMessage::FileIndexRequest { share_id: "s1".into() },
                r#"{"type":"file_index_request","share_id":"s1"}"#,
            ),
            (
                SyncMessage::LocalChange { share_id: "s1".into() },
                r#"{"type":"local_change","share_id":"s1"}"#,
            ),
            (
                SyncMessage::FileIndex {
                    share_id: "s1".into(),
                    files: vec![IndexedFile {
                        relative_path: "a.txt".into(),
                        size_bytes: 3,
                        content_hash: "abc".into(),
                        modified_at: None,
                    }],
                },
                r#"{"type":"file_index","share_id":"s1","files":[{"relative_path":"a.txt","size_bytes":3,"content_hash":"abc","modified_at":null}]}"#,
            ),
            (
                SyncMessage::FileInfo {
                    share_id: "s1".into(),
                    path: "a.txt".into(),
                    size: 3,
                    hash: "abc".into(),
                    resume_from: 0,
                },
                r#"{"type":"file_info","share_id":"s1","path":"a.txt","size":3,"hash":"abc","resume_from":0}"#,
            ),
            (
                SyncMessage::ResumeQuery {
                    share_id: "s1".into(),
                    path: "a.txt".into(),
                    hash: "abc".into(),
                },
                r#"{"type":"resume_query","share_id":"s1","path":"a.txt","hash":"abc"}"#,
            ),
            (
                SyncMessage::ResumeInfo {
                    share_id: "s1".into(),
                    path: "a.txt".into(),
                    have_bytes: 12,
                },
                r#"{"type":"resume_info","share_id":"s1","path":"a.txt","have_bytes":12}"#,
            ),
            (
                SyncMessage::PeerRemoved {
                    remover_node_id: "n1".into(),
                    removed_node_id: "n2".into(),
                },
                r#"{"type":"peer_removed","remover_node_id":"n1","removed_node_id":"n2"}"#,
            ),
        ];
        for (msg, wire) in cases {
            let encoded = serde_json::to_string(&msg).unwrap();
            assert_eq!(encoded, wire, "wire format changed for {msg:?}");
            let decoded: SyncMessage = serde_json::from_str(&encoded).unwrap();
            assert_eq!(serde_json::to_string(&decoded).unwrap(), wire);
        }
    }
}
