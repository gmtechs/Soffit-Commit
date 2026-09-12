/// GBNF grammars for constrained structured output.

/// JSON grammar that forces output: {"sql": "...", "explanation": "..."}
pub const NL_TO_SQL_GRAMMAR: &str = r#"
root   ::= "{" ws "\"sql\"" ws ":" ws string ws "," ws "\"explanation\"" ws ":" ws string ws "}"
string ::= "\"" ( [^"\\] | "\\" ["\\/bfnrt] )* "\""
ws     ::= [ \t\n]*
"#;

/// Validate that a string matches the NL→SQL JSON shape.
pub fn validate_nl_sql_output(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let sql = v["sql"].as_str()?.to_string();
        let explanation = v["explanation"].as_str()?.to_string();
        if !sql.trim().is_empty() {
            return Some((sql, explanation));
        }
    }
    None
}
