use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetInfo {
    pub index: usize,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CellValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeCell {
    /// 0-based row/col start and span
    pub r: u32,
    pub c: u32,
    pub rs: u32, // row span
    pub cs: u32, // col span
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetData {
    pub sheet_name: String,
    pub rows: Vec<Vec<CellValue>>,
    pub max_row: u32,
    pub max_col: u32,
    /// Merged cell regions (0-based indices)
    pub merge_cells: Vec<MergeCell>,
    /// Column widths in pixels, keyed by 0-based col index (only non-default included)
    pub col_widths: Vec<(u32, f64)>,
    /// Row heights in pixels, keyed by 0-based row index (only non-default included)
    pub row_heights: Vec<(u32, f64)>,
}

pub fn open_workbook(path: &Path) -> Result<Vec<SheetInfo>> {
    let book = umya_spreadsheet::reader::xlsx::read(path)
        .map_err(|e| anyhow!("Failed to open: {:?}", e))?;
    let sheets = book.sheet_collection()
        .iter()
        .enumerate()
        .map(|(i, s)| SheetInfo { index: i, name: s.name().to_string() })
        .collect();
    Ok(sheets)
}

pub fn get_sheet_data(path: &Path, sheet_index: usize) -> Result<SheetData> {
    let book = umya_spreadsheet::reader::xlsx::read(path)
        .map_err(|e| anyhow!("Failed to open: {:?}", e))?;

    let sheet = book.sheet(sheet_index)
        .map_err(|e| anyhow!("Sheet {} not found: {:?}", sheet_index, e))?;

    let sheet_name = sheet.name().to_string();
    let (max_col, max_row) = sheet.highest_column_and_row();

    // ── Cell values ──────────────────────────────────────────────────────────
    // Limit to a sensible max to avoid IPC blowup on huge sheets
    let row_limit = max_row.min(2000);
    let col_limit = max_col.min(500);

    let mut rows: Vec<Vec<CellValue>> = Vec::with_capacity(row_limit as usize);
    for row_idx in 1..=row_limit {
        let mut row: Vec<CellValue> = Vec::with_capacity(col_limit as usize);
        for col_idx in 1..=col_limit {
            let val = match sheet.cell((col_idx, row_idx)) {
                None => CellValue::Empty,
                Some(c) => {
                    let raw = c.value().to_string();
                    if raw.is_empty() {
                        CellValue::Empty
                    } else if let Some(n) = c.value_number() {
                        CellValue::Number(n)
                    } else if raw.starts_with('=') {
                        let fmt = c.formatted_value();
                        if fmt.is_empty() {
                            CellValue::Text(raw)
                        } else {
                            // Try parsing as number (common for formula results)
                            if let Ok(n) = fmt.replace(',', "").parse::<f64>() {
                                CellValue::Number(n)
                            } else {
                                CellValue::Text(fmt.to_string())
                            }
                        }
                    } else if raw.eq_ignore_ascii_case("true") {
                        CellValue::Bool(true)
                    } else if raw.eq_ignore_ascii_case("false") {
                        CellValue::Bool(false)
                    } else if let Ok(n) = raw.parse::<f64>() {
                        CellValue::Number(n)
                    } else {
                        CellValue::Text(raw)
                    }
                }
            };
            row.push(val);
        }
        rows.push(row);
    }

    // ── Merged cells ─────────────────────────────────────────────────────────
    let merge_cells: Vec<MergeCell> = sheet
        .merge_cells()
        .iter()
        .filter_map(|mc| {
            let range = mc.range();
            parse_merge_range(&range)
        })
        .collect();

    // ── Column widths (character units → approximate pixels) ─────────────────
    let col_widths: Vec<(u32, f64)> = sheet
        .column_dimensions()
        .iter()
        .filter_map(|cd| {
            let idx = cd.col_num(); // 1-based
            if idx == 0 || idx > col_limit { return None; }
            let w = cd.width();
            if w > 0.0 {
                Some((idx - 1, w * 7.0))
            } else {
                None
            }
        })
        .collect();

    // ── Row heights (points → pixels: pt × 4/3) ──────────────────────────────
    let row_heights: Vec<(u32, f64)> = sheet
        .row_dimensions()
        .iter()
        .filter_map(|rd| {
            let r = rd.row_num(); // 1-based
            if r == 0 || r > row_limit { return None; }
            let h = rd.height();
            if h > 0.0 {
                Some((r - 1, h * 1.333))
            } else {
                None
            }
        })
        .collect();

    Ok(SheetData {
        sheet_name,
        rows,
        max_row: row_limit,
        max_col: col_limit,
        merge_cells,
        col_widths,
        row_heights,
    })
}

pub fn set_cell(path: &Path, sheet_index: usize, row: u32, col: u32, value: &str) -> Result<()> {
    let mut book = umya_spreadsheet::reader::xlsx::read(path)
        .map_err(|e| anyhow!("Failed to open: {:?}", e))?;

    {
        let sheet = book.sheet_mut(sheet_index)
            .map_err(|e| anyhow!("Sheet {} not found: {:?}", sheet_index, e))?;
        sheet.cell_mut((col, row)).set_value(value);
    }

    umya_spreadsheet::writer::xlsx::write(&book, path)
        .map_err(|e| anyhow!("Failed to save: {:?}", e))?;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a merge range like "A1:C3" into a MergeCell (0-based)
fn parse_merge_range(range: &str) -> Option<MergeCell> {
    let parts: Vec<&str> = range.split(':').collect();
    if parts.len() != 2 { return None; }
    let (sc, sr) = parse_cell_ref(parts[0])?;
    let (ec, er) = parse_cell_ref(parts[1])?;
    Some(MergeCell {
        r: sr.saturating_sub(1),
        c: sc.saturating_sub(1),
        rs: er.saturating_sub(sr) + 1,
        cs: ec.saturating_sub(sc) + 1,
    })
}

/// "B3" → (col 2, row 3) — 1-based
fn parse_cell_ref(s: &str) -> Option<(u32, u32)> {
    let s = s.trim();
    let col_part: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let row_part: String = s.chars().skip_while(|c| c.is_ascii_alphabetic()).collect();
    let col = column_label_to_index(&col_part) as u32;
    let row: u32 = row_part.parse().ok()?;
    if col == 0 || row == 0 { return None; }
    Some((col, row))
}

/// "A" → 1, "Z" → 26, "AA" → 27
fn column_label_to_index(label: &str) -> usize {
    label.chars().fold(0usize, |acc, c| {
        acc * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1)
    })
}
