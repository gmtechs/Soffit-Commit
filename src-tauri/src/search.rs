/// Full-text search using SQLite FTS5 — zero extra compile cost,
/// already bundled with rusqlite.
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub share_id: String,
    pub score: f32,
    pub snippet: Option<String>,
}

pub fn ensure_fts_table(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE VIRTUAL TABLE IF NOT EXISTS file_fts USING fts5(
            share_id UNINDEXED,
            file_path UNINDEXED,
            file_name,
            content,
            tokenize='porter unicode61'
        );
    ")?;
    Ok(())
}

/// Index or re-index a file
pub fn index_file(conn: &Connection, share_id: &str, rel_path: &str, abs_path: &Path) -> Result<()> {
    // Remove stale entry
    conn.execute("DELETE FROM file_fts WHERE file_path = ?1", rusqlite::params![rel_path])?;

    let file_name = abs_path.file_name()
        .and_then(|n| n.to_str()).unwrap_or(rel_path);
    let content = read_text_content(abs_path).unwrap_or_default();

    conn.execute(
        "INSERT INTO file_fts(share_id, file_path, file_name, content) VALUES(?1,?2,?3,?4)",
        rusqlite::params![share_id, rel_path, file_name, content],
    )?;
    Ok(())
}

/// Search file names and content
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let escaped = query.replace('"', "\"\"");
    let fts_query = format!("\"{}\"", escaped);

    let mut stmt = conn.prepare(
        "SELECT share_id, file_path,
                bm25(file_fts) as score,
                snippet(file_fts, 3, '<b>', '</b>', '...', 20)
         FROM file_fts
         WHERE file_fts MATCH ?1
         ORDER BY bm25(file_fts)
         LIMIT ?2"
    )?;

    let results = stmt.query_map(
        rusqlite::params![fts_query, limit as i64],
        |row| Ok(SearchResult {
            share_id:  row.get(0)?,
            file_path: row.get(1)?,
            score:     row.get::<_, f64>(2)? as f32,
            snippet:   row.get(3)?,
        }),
    )?.filter_map(|r| r.ok()).collect();

    Ok(results)
}

/// Remove all indexed entries for a share
pub fn remove_share(conn: &Connection, share_id: &str) -> Result<()> {
    conn.execute("DELETE FROM file_fts WHERE share_id = ?1", rusqlite::params![share_id])?;
    Ok(())
}

fn read_text_content(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let text_exts = ["txt","md","rs","ts","js","py","sql","json","toml","yaml","yml","csv","html","css"];
    if text_exts.contains(&ext.as_str()) {
        // Cap at 500KB to avoid bloating the FTS index
        let bytes = std::fs::read(path).ok()?;
        let capped = &bytes[..bytes.len().min(500_000)];
        String::from_utf8(capped.to_vec()).ok()
    } else {
        None
    }
}
