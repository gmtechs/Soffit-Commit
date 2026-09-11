use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    create_tables(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn create_tables(conn: &Connection) -> Result<()> {    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS users (
            id          TEXT PRIMARY KEY,
            username    TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS peers (
            id          TEXT PRIMARY KEY,
            node_id     TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            public_key  TEXT NOT NULL,
            trust_status TEXT NOT NULL DEFAULT 'trusted',
            last_seen   TEXT,
            is_online   INTEGER NOT NULL DEFAULT 0,
            endpoint_addr TEXT
        );

        CREATE TABLE IF NOT EXISTS shares (
            id          TEXT PRIMARY KEY,
            path        TEXT NOT NULL,
            display_name TEXT NOT NULL,
            is_owner    INTEGER NOT NULL DEFAULT 1,
            selective_sync INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS share_permissions (
            id          TEXT PRIMARY KEY,
            share_id    TEXT NOT NULL,
            peer_id     TEXT NOT NULL,
            level       TEXT NOT NULL DEFAULT 'none',
            updated_at  TEXT NOT NULL,
            FOREIGN KEY(share_id) REFERENCES shares(id),
            FOREIGN KEY(peer_id) REFERENCES peers(id),
            UNIQUE(share_id, peer_id)
        );

        CREATE TABLE IF NOT EXISTS locks (
            id          TEXT PRIMARY KEY,
            file_path   TEXT NOT NULL UNIQUE,
            held_by_peer_id TEXT NOT NULL,
            held_by_name TEXT NOT NULL,
            acquired_at TEXT NOT NULL,
            expires_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS activity_log (
            id          TEXT PRIMARY KEY,
            timestamp   TEXT NOT NULL,
            actor       TEXT NOT NULL,
            action      TEXT NOT NULL,
            target      TEXT NOT NULL,
            metadata    TEXT
        );

        CREATE TABLE IF NOT EXISTS file_index (
            id          TEXT PRIMARY KEY,
            share_id    TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            size_bytes  INTEGER NOT NULL DEFAULT 0,
            modified_at TEXT,
            content_hash TEXT,
            file_kind   TEXT NOT NULL DEFAULT 'generic',
            sync_status TEXT NOT NULL DEFAULT 'synced',
            UNIQUE(share_id, relative_path)
        );

        CREATE TABLE IF NOT EXISTS pairing_codes (
            id          TEXT PRIMARY KEY,
            code        TEXT NOT NULL UNIQUE,
            created_at  TEXT NOT NULL,
            expires_at  TEXT NOT NULL,
            used        INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key         TEXT PRIMARY KEY,
            value       TEXT NOT NULL
        );
    ")?;
    Ok(())
}

/// Additive migrations — safe to run on existing databases.
/// Each ALTER TABLE uses a try/ignore pattern since SQLite has no
/// "ADD COLUMN IF NOT EXISTS" before version 3.37.
fn run_migrations(conn: &Connection) -> Result<()> {
    // shares: add selective_sync if missing
    conn.execute_batch(
        "ALTER TABLE shares ADD COLUMN selective_sync INTEGER NOT NULL DEFAULT 0;"
    ).ok(); // .ok() = ignore error if column already exists

    // peers: add endpoint_addr if missing
    conn.execute_batch(
        "ALTER TABLE peers ADD COLUMN endpoint_addr TEXT;"
    ).ok();

    // file_index: add content_hash if missing (older schema may not have it)
    conn.execute_batch(
        "ALTER TABLE file_index ADD COLUMN content_hash TEXT;"
    ).ok();

    Ok(())
}

pub fn ensure_extra_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS file_history (
            id          TEXT PRIMARY KEY,
            file_path   TEXT NOT NULL,
            opened_at   TEXT NOT NULL,
            file_kind   TEXT NOT NULL DEFAULT 'generic'
        );
        CREATE INDEX IF NOT EXISTS idx_history_path ON file_history(file_path);
        CREATE INDEX IF NOT EXISTS idx_history_time ON file_history(opened_at DESC);

        CREATE TABLE IF NOT EXISTS file_favourites (
            id          TEXT PRIMARY KEY,
            file_path   TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            file_kind   TEXT NOT NULL DEFAULT 'generic',
            added_at    TEXT NOT NULL
        );
    ")?;
    Ok(())
}
