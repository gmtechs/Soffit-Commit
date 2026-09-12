use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub node_id: String,
    pub display_name: String,
    pub public_key: String,
    pub trust_status: String,
    pub last_seen: Option<String>,
    pub is_online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Share {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub is_owner: bool,
    pub selective_sync: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePermission {
    pub id: String,
    pub share_id: String,
    pub peer_id: String,
    pub level: PermissionLevel,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    None,
    View,
    Edit,
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionLevel::None => write!(f, "none"),
            PermissionLevel::View => write!(f, "view"),
            PermissionLevel::Edit => write!(f, "edit"),
        }
    }
}

impl From<&str> for PermissionLevel {
    fn from(s: &str) -> Self {
        match s {
            "view" => PermissionLevel::View,
            "edit" => PermissionLevel::Edit,
            _ => PermissionLevel::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lock {
    pub id: String,
    pub file_path: String,
    pub held_by_peer_id: String,
    pub held_by_name: String,
    pub acquired_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: String,
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIndex {
    pub id: String,
    pub share_id: String,
    pub relative_path: String,
    pub size_bytes: i64,
    pub modified_at: Option<String>,
    pub content_hash: Option<String>,
    pub file_kind: FileKind,
    pub sync_status: SyncStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Excel,
    Sql,
    Csv,
    Sqlite,
    Parquet,
    Image,
    Pdf,
    Text,
    Generic,
}

impl FileKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "xlsx" | "xls" => FileKind::Excel,
            "sql" => FileKind::Sql,
            "csv" => FileKind::Csv,
            "sqlite" | "db" => FileKind::Sqlite,
            "parquet" => FileKind::Parquet,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => FileKind::Image,
            "pdf" => FileKind::Pdf,
            "txt" | "md" | "rs" | "ts" | "js" | "py" | "json" | "toml" | "yaml" | "yml" => FileKind::Text,
            _ => FileKind::Generic,
        }
    }
}

impl std::fmt::Display for FileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FileKind::Excel => "excel",
            FileKind::Sql => "sql",
            FileKind::Csv => "csv",
            FileKind::Sqlite => "sqlite",
            FileKind::Parquet => "parquet",
            FileKind::Image => "image",
            FileKind::Pdf => "pdf",
            FileKind::Text => "text",
            FileKind::Generic => "generic",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Synced,
    Syncing,
    Conflict,
    Locked,
    Pending,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SyncStatus::Synced => "synced",
            SyncStatus::Syncing => "syncing",
            SyncStatus::Conflict => "conflict",
            SyncStatus::Locked => "locked",
            SyncStatus::Pending => "pending",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub storage_used_bytes: i64,
    pub files_synced: i64,
    pub files_edited_this_month: i64,
    pub conflicts_resolved: i64,
    pub sync_health_percent: f64,
    pub file_type_breakdown: FileTypeBreakdown,
    pub online_peers: i64,
    pub total_peers: i64,
    // Real deltas vs last week
    pub storage_delta_pct: i64,
    pub files_synced_delta_pct: i64,
    pub files_edited_delta_pct: i64,
    pub conflicts_delta_pct: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileTypeBreakdown {
    pub excel: i64,
    pub sql: i64,
    pub other: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairingCode {
    pub id: String,
    pub code: String,
    pub created_at: String,
    pub expires_at: String,
}
