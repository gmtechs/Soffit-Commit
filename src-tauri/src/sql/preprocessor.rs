/// MySQL → SQLite preprocessor.
/// Converts a phpMyAdmin/MySQL dump to SQLite-compatible SQL.
use regex::Regex;

pub fn preprocess_mysql_dump(sql: &str) -> String {
    let lines: Vec<&str> = sql.lines().collect();
    let mut out_lines: Vec<String> = Vec::new();

    for line in &lines {
        let trimmed = line.trim();

        // Skip blank lines
        if trimmed.is_empty() { continue; }

        // Skip pure comment lines
        if trimmed.starts_with("--") { continue; }

        // Skip versioned comments /*!... */
        if trimmed.starts_with("/*!") { continue; }

        // Skip MySQL session directives
        if is_mysql_directive(trimmed) { continue; }

        // Skip KEY/INDEX lines inside CREATE TABLE (they cause syntax errors in SQLite)
        // But keep PRIMARY KEY lines
        let upper = trimmed.to_uppercase();
        if (upper.starts_with("KEY ") || upper.starts_with("UNIQUE KEY ") || upper.starts_with("INDEX "))
            && !upper.starts_with("PRIMARY KEY") {
            continue;
        }

        // Translate the line
        let translated = translate_line(line);
        let t = translated.trim().to_string();
        if !t.is_empty() {
            out_lines.push(t);
        }
    }

    // Join and clean up trailing commas before closing parens
    // e.g. "  `col` TEXT,\n)" should become "  `col` TEXT\n)"
    let joined = out_lines.join("\n");

    // Remove trailing comma before closing paren of CREATE TABLE
    let trailing_comma = Regex::new(r",(\s*\n\s*\))").unwrap();
    let cleaned = trailing_comma.replace_all(&joined, "$1");

    cleaned.into_owned()
}

fn is_mysql_directive(line: &str) -> bool {
    let upper = line.to_uppercase();
    upper.starts_with("SET SQL_MODE")
        || upper.starts_with("SET TIME_ZONE")
        || upper.starts_with("SET NAMES")
        || upper.starts_with("SET CHARACTER_SET")
        || upper.starts_with("SET @OLD_")
        || upper.starts_with("SET @")
        || upper.starts_with("UNLOCK TABLES")
        || upper.starts_with("LOCK TABLES")
        || upper.starts_with("USE ")
        || upper.starts_with("START TRANSACTION")
        || upper.trim_end_matches(';') == "COMMIT"
        // MySQL-only ALTER TABLE variants SQLite doesn't support
        || (upper.starts_with("ALTER TABLE") && (upper.contains("ADD PRIMARY KEY")
            || upper.contains("ADD UNIQUE KEY")
            || upper.contains("ADD KEY")
            || upper.contains("ADD INDEX")
            || upper.contains("MODIFY ")
            || upper.contains("CHANGE ")
            || upper.contains("DROP INDEX")
            || upper.contains("DROP KEY")))
}

fn translate_line(line: &str) -> String {
    // Backtick → double-quote, track string context
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => { in_single = !in_single; out.push(ch); }
            '"'  if !in_single => { in_double = !in_double; out.push(ch); }
            '`'  if !in_single && !in_double => {
                let mut ident = String::new();
                for c in chars.by_ref() { if c == '`' { break; } ident.push(c); }
                out.push('"'); out.push_str(&ident); out.push('"');
            }
            _ => out.push(ch),
        }
    }

    translate_ddl_types(&out)
}

fn translate_ddl_types(sql: &str) -> String {
    let int_n     = Regex::new(r"(?i)\b(?:tiny|small|medium|big)?int\s*\(\s*\d+\s*\)").unwrap();
    let varchar   = Regex::new(r"(?i)\b(?:var)?char\s*\(\s*\d+\s*\)").unwrap();
    let floatp    = Regex::new(r"(?i)\b(?:double|float|decimal\s*\(\s*\d+\s*,\s*\d+\s*\))").unwrap();
    let longtext  = Regex::new(r"(?i)\b(?:long|medium|tiny)text\b").unwrap();
    let ts        = Regex::new(r"(?i)\b(?:datetime|timestamp)\b").unwrap();
    let engine    = Regex::new(r"(?i)\s*ENGINE\s*=\s*\w+").unwrap();
    let charset   = Regex::new(r"(?i)\s*(?:DEFAULT\s+)?(?:CHARSET|CHARACTER\s+SET)\s*=?\s*[\w-]+").unwrap();
    let collate   = Regex::new(r"(?i)\s*COLLATE\s*=?\s*[\w-]+").unwrap();
    let ai_opt    = Regex::new(r"(?i)\s*AUTO_INCREMENT\s*=\s*\d+").unwrap();
    let ai        = Regex::new(r"(?i)\s*\bAUTO_INCREMENT\b").unwrap();
    let unsigned  = Regex::new(r"(?i)\s*\bUNSIGNED\b").unwrap();
    // MySQL functions not supported in SQLite
    let cur_ts    = Regex::new(r"(?i)\bCURRENT_TIMESTAMP\s*\(\s*\)").unwrap();
    let now_fn    = Regex::new(r"(?i)\bNOW\s*\(\s*\)").unwrap();
    let sysdate   = Regex::new(r"(?i)\bSYSDATE\s*\(\s*\)").unwrap();
    // MySQL ENUM/SET → TEXT
    let enum_set  = Regex::new(r"(?i)\b(?:ENUM|SET)\s*\([^)]*\)").unwrap();
    // ON UPDATE CURRENT_TIMESTAMP clause
    let on_update = Regex::new(r"(?i)\s*ON\s+UPDATE\s+\S+").unwrap();

    let s = int_n.replace_all(sql, "INTEGER");
    let s = varchar.replace_all(&s, "TEXT");
    let s = floatp.replace_all(&s, "REAL");
    let s = longtext.replace_all(&s, "TEXT");
    let s = enum_set.replace_all(&s, "TEXT");
    let s = ts.replace_all(&s, "TEXT");
    let s = engine.replace_all(&s, "");
    let s = charset.replace_all(&s, "");
    let s = collate.replace_all(&s, "");
    let s = on_update.replace_all(&s, "");
    let s = ai_opt.replace_all(&s, "");
    let s = ai.replace_all(&s, "");
    let s = unsigned.replace_all(&s, "");
    // Fix timestamp functions — must come after type replacements
    let s = cur_ts.replace_all(&s, "CURRENT_TIMESTAMP");
    let s = now_fn.replace_all(&s, "CURRENT_TIMESTAMP");
    let s = sysdate.replace_all(&s, "CURRENT_TIMESTAMP");

    s.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_dump_roundtrip() {
        let dump = r#"
SET SQL_MODE = "NO_AUTO_VALUE_ON_ZERO";
START TRANSACTION;
CREATE TABLE `messages` (
  `id` int(11) NOT NULL,
  `sender` varchar(255) DEFAULT NULL,
  `content` text DEFAULT NULL,
  `created_at` timestamp NOT NULL DEFAULT current_timestamp(),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT INTO `messages` (`id`, `sender`, `content`) VALUES (1, 'Alice', 'Hello');
ALTER TABLE `messages` ADD PRIMARY KEY (`id`);
ALTER TABLE `messages` MODIFY `id` int(11) NOT NULL AUTO_INCREMENT;
COMMIT;
"#;
        let out = preprocess_mysql_dump(dump);
        assert!(!out.contains("ENGINE"), "no ENGINE");
        assert!(!out.contains("START TRANSACTION"), "no START TRANSACTION");
        assert!(!out.contains("ADD PRIMARY KEY"), "no ADD PRIMARY KEY");
        assert!(!out.contains("MODIFY"), "no MODIFY");
        assert!(!out.contains("current_timestamp()"), "no current_timestamp()");
        assert!(out.contains("CURRENT_TIMESTAMP"), "has CURRENT_TIMESTAMP");
        assert!(out.contains("CREATE TABLE"), "has CREATE TABLE");
        assert!(out.contains("INSERT INTO"), "has INSERT");
        assert!(out.contains("\"messages\""), "backticks translated");
        assert!(!out.contains(",\n)"), "no trailing comma before )");
    }
}
