/// SQL engine backed by SQLite (rusqlite, already bundled).
use anyhow::{anyhow, Result};
use rusqlite::{Connection, types::ValueRef};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ── New result types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptStatementResult {
    pub statement_index: usize,
    pub line_number: usize,
    pub sql_snippet: String,
    pub result: QueryResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportError {
    pub statement_index: usize,
    pub original_line_number: usize,
    pub snippet: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub statements_executed: usize,
    pub error: Option<ImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: Option<usize>,
    pub error: Option<String>,
    pub execution_ms: u128,
}

pub struct SqlEngine {
    conn: Connection,
}

impl SqlEngine {
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        // Enable FTS5 and JSON extensions (built into rusqlite bundled SQLite)
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
        ")?;
        Ok(SqlEngine { conn })
    }

    /// Attach an external SQLite database
    pub fn attach_file(&self, path: &Path, alias: &str) -> Result<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "sqlite" | "db" => {
                let path_str = path.to_string_lossy().replace('\'', "''");
                self.conn.execute_batch(&format!("ATTACH DATABASE '{}' AS {};", path_str, alias))?;
                Ok(())
            }
            "xlsx" | "xls" => Err(anyhow!("Open Excel files in Excel mode, not the SQL console.")),
            _ => Err(anyhow!("Only .sqlite/.db files can be attached. Use query_file() for CSV.")),
        }
    }

    /// Execute arbitrary SQL
    pub fn execute(&self, sql: &str) -> QueryResult {
        let start = std::time::Instant::now();
        let result = self.run_query(sql);
        let ms = start.elapsed().as_millis();
        match result {
            Ok(mut r) => { r.execution_ms = ms; r }
            Err(err) => QueryResult {
                columns: vec![], rows: vec![],
                rows_affected: None,
                error: Some(err.to_string()),
                execution_ms: ms,
            },
        }
    }

    fn run_query(&self, sql: &str) -> Result<QueryResult> {
        let upper = sql.trim().to_uppercase();
        let is_read = upper.starts_with("SELECT")
            || upper.starts_with("WITH")
            || upper.starts_with("PRAGMA")
            || upper.starts_with("EXPLAIN");

        if is_read {
            let mut stmt = self.conn.prepare(sql)?;
            let col_count = stmt.column_count();
            let columns: Vec<ColumnInfo> = (0..col_count).map(|i| ColumnInfo {
                name: stmt.column_name(i).unwrap_or("?").to_string(),
                data_type: "text".to_string(), // rusqlite doesn't expose type at prepare time
            }).collect();

            let mut rows = Vec::new();
            let mut qrows = stmt.query([])?;
            while let Some(row) = qrows.next()? {
                let mut r = Vec::new();
                for i in 0..col_count {
                    let val = match row.get_ref(i)? {
                        ValueRef::Null       => serde_json::Value::Null,
                        ValueRef::Integer(n) => serde_json::json!(n),
                        ValueRef::Real(f)    => serde_json::json!(f),
                        ValueRef::Text(s)    => serde_json::Value::String(
                            String::from_utf8_lossy(s).to_string()),
                        ValueRef::Blob(b)    => serde_json::Value::String(
                            format!("<blob {} bytes>", b.len())),
                    };
                    r.push(val);
                }
                rows.push(r);
            }
            Ok(QueryResult { columns, rows, rows_affected: None, error: None, execution_ms: 0 })
        } else {
            let affected = self.conn.execute(sql, [])?;
            Ok(QueryResult { columns: vec![], rows: vec![], rows_affected: Some(affected), error: None, execution_ms: 0 })
        }
    }

    /// Load a CSV file into a temp table and run SQL against it.
    /// For .sql files, reads and executes the file's SQL directly.
    pub fn query_file(&self, file_path: &Path, sql: &str) -> QueryResult {
        let start = std::time::Instant::now();
        let ms = || start.elapsed().as_millis();

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        // .sql file — read contents and execute
        if ext == "sql" {
            let contents = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => return QueryResult {
                    columns: vec![], rows: vec![], rows_affected: None,
                    error: Some(format!("Cannot read .sql file: {}", e)), execution_ms: ms(),
                },
            };
            // If user typed custom SQL in the editor, run that. Otherwise run the file.
            let to_run = if sql.trim().is_empty() || sql.trim() == "-- Write SQL here\n-- Use __FILE__ as a placeholder when querying a file directly:\n-- SELECT * FROM __FILE__ LIMIT 100\n\nSELECT 1 + 1 AS result;" {
                contents
            } else {
                sql.to_string()
            };
            let mut r = self.execute(&to_run);
            r.execution_ms = ms();
            return r;
        }

        if ext != "csv" {
            return QueryResult {
                columns: vec![], rows: vec![], rows_affected: None,
                error: Some(format!(".{} files: use 'Attach' for SQLite databases, or select a .csv file for inline querying.", ext)),
                execution_ms: ms(),
            };
        }

        // CSV: import into temp table and run SQL
        match self.import_csv_and_run(file_path, sql) {
            Ok(mut r) => { r.execution_ms = ms(); r }
            Err(err) => QueryResult {
                columns: vec![], rows: vec![], rows_affected: None,
                error: Some(err.to_string()), execution_ms: ms(),
            },
        }
    }

    fn import_csv_and_run(&self, csv_path: &Path, sql: &str) -> Result<QueryResult> {
        // Read CSV, create temp table, insert rows, run sql
        let mut rdr = csv::Reader::from_path(csv_path)
            .map_err(|e| anyhow!("CSV read error: {}", e))?;

        let headers: Vec<String> = rdr.headers()
            .map_err(|e| anyhow!("CSV headers: {}", e))?
            .iter().map(|h| h.replace('"', "").replace(' ', "_")).collect();

        let table = "csv_import";
        let drop = format!("DROP TABLE IF EXISTS {table}");
        let cols = headers.iter().map(|h| format!("\"{}\" TEXT", h)).collect::<Vec<_>>().join(", ");
        let create = format!("CREATE TEMP TABLE {table} ({cols})");
        self.conn.execute_batch(&format!("{drop}; {create};"))?;

        let placeholders = headers.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let insert = format!("INSERT INTO {table} VALUES ({placeholders})");
        let mut stmt = self.conn.prepare(&insert)?;

        for result in rdr.records() {
            let record = result.map_err(|e| anyhow!("CSV row: {}", e))?;
            let values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
            let params: Vec<&dyn rusqlite::ToSql> = values.iter()
                .map(|v| v as &dyn rusqlite::ToSql).collect();
            stmt.execute(rusqlite::params_from_iter(params.iter()))?;
        }

        // Replace __FILE__ placeholder with the table name
        let resolved = sql.replace("__FILE__", table);
        self.run_query(&resolved)
    }

    /// Generate UPDATE SQL preview (never auto-executes)
    pub fn generate_edit_sql(
        table: &str, pk_col: &str,
        pk_val: &serde_json::Value,
        changes: &[(String, serde_json::Value)],
    ) -> String {
        let sets: Vec<String> = changes.iter().map(|(col, val)| {
            let v = match val {
                serde_json::Value::Null      => "NULL".to_string(),
                serde_json::Value::Bool(b)   => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
                other                        => format!("'{}'", other),
            };
            format!("\"{}\" = {}", col, v)
        }).collect();
        let pk = match pk_val {
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            other                        => other.to_string(),
        };
        format!("UPDATE \"{}\" SET {} WHERE \"{}\" = {};", table, sets.join(", "), pk_col, pk)
    }
}

// ── Statement splitter ────────────────────────────────────────────────────────

/// Split a SQL script into individual statements using a state machine that
/// is aware of: `--` comments, `/* */` block comments, `'...'` / `"..."` string
/// literals, and backtick identifiers. Returns (byte_offset, statement_text).
pub fn split_statements(sql: &str) -> Vec<(usize, String)> {
    let mut results: Vec<(usize, String)> = Vec::new();
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut stmt_start = 0;

    enum State { Normal, LineComment, BlockComment, SingleQuote, DoubleQuote, Backtick }
    let mut state = State::Normal;

    while i < len {
        let ch = bytes[i] as char;
        match state {
            State::Normal => match ch {
                '-' if i + 1 < len && bytes[i + 1] == b'-' => { state = State::LineComment; i += 2; continue; }
                '/' if i + 1 < len && bytes[i + 1] == b'*' => { state = State::BlockComment; i += 2; continue; }
                '\'' => { state = State::SingleQuote; }
                '"'  => { state = State::DoubleQuote; }
                '`'  => { state = State::Backtick; }
                ';'  => {
                    let stmt = sql[stmt_start..i].trim().to_string();
                    if !stmt.is_empty() {
                        results.push((stmt_start, stmt));
                    }
                    stmt_start = i + 1;
                }
                _ => {}
            },
            State::LineComment => { if ch == '\n' { state = State::Normal; } }
            State::BlockComment => {
                if ch == '*' && i + 1 < len && bytes[i + 1] == b'/' {
                    state = State::Normal; i += 2; continue;
                }
            }
            State::SingleQuote => {
                if ch == '\'' {
                    // Handle '' escape
                    if i + 1 < len && bytes[i + 1] == b'\'' { i += 2; continue; }
                    state = State::Normal;
                }
            }
            State::DoubleQuote => {
                if ch == '"' {
                    if i + 1 < len && bytes[i + 1] == b'"' { i += 2; continue; }
                    state = State::Normal;
                }
            }
            State::Backtick => { if ch == '`' { state = State::Normal; } }
        }
        i += 1;
    }
    // Trailing statement without semicolon
    let trailing = sql[stmt_start..].trim().to_string();
    if !trailing.is_empty() {
        results.push((stmt_start, trailing));
    }
    results
}

/// Count line number (1-based) of byte offset in original string
fn line_of_offset(sql: &str, offset: usize) -> usize {
    sql[..offset.min(sql.len())].chars().filter(|&c| c == '\n').count() + 1
}

impl SqlEngine {
    /// Execute a multi-statement SQL script with per-statement error reporting.
    pub fn execute_script(&self, sql: &str) -> Vec<ScriptStatementResult> {
        let stmts = split_statements(sql);
        let mut results = Vec::new();

        for (idx, (offset, stmt)) in stmts.iter().enumerate() {
            let snippet = if stmt.len() > 120 { stmt[..120].to_string() } else { stmt.clone() };
            let line = line_of_offset(sql, *offset);
            let qr = self.execute(stmt);
            let failed = qr.error.is_some();
            results.push(ScriptStatementResult {
                statement_index: idx + 1,
                line_number: line,
                sql_snippet: snippet,
                result: qr,
            });
            if failed { break; }
        }
        results
    }

    /// Import a MySQL/phpMyAdmin .sql dump file.
    /// Pre-processes MySQL-specific syntax, then executes the whole batch
    /// via SQLite execute_batch which handles multi-statement SQL natively.
    pub fn import_sql_dump(&self, path: &Path) -> Result<ImportResult> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Cannot read file: {}", e))?;

        let processed = crate::sql::preprocessor::preprocess_mysql_dump(&raw);

        // Count non-empty statements for reporting
        let stmt_count = split_statements(&processed).len();

        if stmt_count == 0 {
            return Ok(ImportResult { statements_executed: 0, error: None });
        }

        self.conn.execute_batch("SAVEPOINT import_dump;")?;

        // Use execute_batch for the whole preprocessed SQL — SQLite handles
        // multi-statement batches natively, including CREATE TABLE + INSERT
        match self.conn.execute_batch(&processed) {
            Ok(_) => {
                self.conn.execute_batch("RELEASE SAVEPOINT import_dump;")?;
                Ok(ImportResult { statements_executed: stmt_count, error: None })
            }
            Err(e) => {
                self.conn.execute_batch("ROLLBACK TO SAVEPOINT import_dump;").ok();
                // Find which line the error originated from
                let err_msg = e.to_string();
                let original_line = raw.lines().enumerate()
                    .find(|(_, l)| {
                        let lu = l.to_uppercase();
                        lu.contains("CREATE TABLE") || lu.contains("INSERT INTO")
                    })
                    .map(|(i, _)| i + 1).unwrap_or(1);
                Ok(ImportResult {
                    statements_executed: 0,
                    error: Some(ImportError {
                        statement_index: 1,
                        original_line_number: original_line,
                        snippet: processed.chars().take(200).collect(),
                        message: err_msg,
                    }),
                })
            }
        }
    }
}
