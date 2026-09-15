use super::engine::{get_sheet_data, set_cell, CellValue};
/// Auto-detect header rows, data ranges, and provide per-record form access.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormLayout {
    pub sheet_index: usize,
    pub header_row: u32,     // 1-based row where headers are found
    pub data_start_row: u32, // first data row (header_row + 1)
    pub data_end_row: u32,   // last row with data
    pub col_start: u32,      // first column (1-based)
    pub col_end: u32,        // last column (1-based)
    pub headers: Vec<String>,
    pub total_records: u32,
    pub detectable: bool, // false = free-form layout, Form mode disabled
    pub disable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormRecord {
    pub record_index: u32, // 0-based among data rows
    pub row: u32,          // actual sheet row (1-based)
    pub fields: Vec<FormField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub col: u32,
    pub header: String,
    pub value: CellValue,
    pub detected_type: FieldType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Text,
    Number,
    Date,
    Bool,
}

/// Auto-detect the form layout for a sheet.
pub fn detect_form_layout(path: &Path, sheet_index: usize) -> Result<FormLayout> {
    let data = get_sheet_data(path, sheet_index)?;

    if data.rows.is_empty() || data.max_col == 0 {
        return Ok(FormLayout {
            sheet_index,
            header_row: 0,
            data_start_row: 0,
            data_end_row: 0,
            col_start: 0,
            col_end: 0,
            headers: vec![],
            total_records: 0,
            detectable: false,
            disable_reason: Some("Sheet is empty".to_string()),
        });
    }

    // Scan rows to find the header row: the first row where most cells are non-empty text
    let header_row_idx = find_header_row(&data.rows);

    if header_row_idx.is_none() {
        return Ok(FormLayout {
            sheet_index,
            header_row: 0,
            data_start_row: 0,
            data_end_row: 0,
            col_start: 0,
            col_end: 0,
            headers: vec![],
            total_records: 0,
            detectable: false,
            disable_reason: Some(
                "No detectable header row found. This may be a free-form layout.".to_string(),
            ),
        });
    }

    let header_row_idx = header_row_idx.unwrap();
    let header_row = &data.rows[header_row_idx];

    // Find the contiguous non-empty column span in the header row
    let (col_start, col_end) = find_contiguous_cols(header_row);
    if col_start > col_end {
        return Ok(FormLayout {
            sheet_index,
            header_row: 0,
            data_start_row: 0,
            data_end_row: 0,
            col_start: 0,
            col_end: 0,
            headers: vec![],
            total_records: 0,
            detectable: false,
            disable_reason: Some("Headers are not in a contiguous range.".to_string()),
        });
    }

    let headers: Vec<String> = (col_start..=col_end)
        .map(|c| cell_to_string(&header_row[(c - 1) as usize]))
        .collect();

    let data_start_row = (header_row_idx + 2) as u32; // +1 for 1-based, +1 for first data row
    let mut data_end_row = data_start_row - 1;

    // Find last non-empty data row
    for (i, row) in data.rows.iter().enumerate().skip(header_row_idx + 1) {
        let has_data = row
            .iter()
            .skip((col_start - 1) as usize)
            .take((col_end - col_start + 1) as usize)
            .any(|c| !matches!(c, CellValue::Empty));
        if has_data {
            data_end_row = (i + 1) as u32;
        }
    }

    let total_records = if data_end_row >= data_start_row {
        data_end_row - data_start_row + 1
    } else {
        0
    };

    Ok(FormLayout {
        sheet_index,
        header_row: (header_row_idx + 1) as u32,
        data_start_row,
        data_end_row,
        col_start,
        col_end,
        headers,
        total_records,
        detectable: true,
        disable_reason: None,
    })
}

/// Get one record by 0-based record index
pub fn get_record(path: &Path, layout: &FormLayout, record_index: u32) -> Result<FormRecord> {
    if !layout.detectable {
        return Err(anyhow!("Form mode is not available for this sheet"));
    }
    if record_index >= layout.total_records {
        return Err(anyhow!(
            "Record index {} out of range (total: {})",
            record_index,
            layout.total_records
        ));
    }

    let row = layout.data_start_row + record_index;
    let data = get_sheet_data(path, layout.sheet_index)?;
    let row_data = data
        .rows
        .get((row - 1) as usize)
        .ok_or_else(|| anyhow!("Row {} not found", row))?;

    let mut fields = Vec::new();
    for (i, header) in layout.headers.iter().enumerate() {
        let col = layout.col_start + i as u32;
        let value = row_data
            .get((col - 1) as usize)
            .cloned()
            .unwrap_or(CellValue::Empty);
        let detected_type = detect_field_type(&value, header);
        fields.push(FormField {
            col,
            header: header.clone(),
            value,
            detected_type,
        });
    }

    Ok(FormRecord {
        record_index,
        row,
        fields,
    })
}

/// Save a record: validate then write only changed cells
pub fn save_record(
    path: &Path,
    layout: &FormLayout,
    record_index: u32,
    updates: Vec<(u32, String)>, // (col, new_value)
) -> Result<Vec<String>> {
    if !layout.detectable {
        return Err(anyhow!("Form mode is not available for this sheet"));
    }

    let row = layout.data_start_row + record_index;
    let mut errors: Vec<String> = Vec::new();

    // Validate before writing
    for (col, value) in &updates {
        let header_idx = (col - layout.col_start) as usize;
        if let Some(header) = layout.headers.get(header_idx) {
            if let Some(err) = validate_field(header, value, *col) {
                errors.push(err);
            }
        }
    }
    if !errors.is_empty() {
        return Ok(errors); // Return errors without writing
    }

    // Write each changed cell
    for (col, value) in updates {
        set_cell(path, layout.sheet_index, row, col, &value)?;
    }
    Ok(vec![]) // empty = success
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn find_header_row(rows: &[Vec<CellValue>]) -> Option<usize> {
    for (i, row) in rows.iter().enumerate().take(20) {
        let text_count = row
            .iter()
            .filter(|c| matches!(c, CellValue::Text(_)))
            .count();
        let non_empty = row
            .iter()
            .filter(|c| !matches!(c, CellValue::Empty))
            .count();
        // Header row: mostly text, at least 2 columns
        if non_empty >= 2 && text_count as f32 / non_empty as f32 >= 0.7 {
            return Some(i);
        }
    }
    None
}

fn find_contiguous_cols(row: &[CellValue]) -> (u32, u32) {
    let first = row.iter().position(|c| !matches!(c, CellValue::Empty));
    if first.is_none() {
        return (1, 0);
    }
    let first = first.unwrap();
    // Find contiguous block from first
    let mut last = first;
    for (i, c) in row.iter().enumerate().skip(first) {
        if !matches!(c, CellValue::Empty) {
            last = i;
        }
    }
    ((first + 1) as u32, (last + 1) as u32)
}

fn cell_to_string(c: &CellValue) -> String {
    match c {
        CellValue::Text(s) => s.clone(),
        CellValue::Number(n) => n.to_string(),
        CellValue::Bool(b) => b.to_string(),
        CellValue::Empty => String::new(),
    }
}

fn detect_field_type(val: &CellValue, header: &str) -> FieldType {
    let h = header.to_lowercase();
    if h.contains("date") || h.contains("time") || h.contains("day") {
        return FieldType::Date;
    }
    match val {
        CellValue::Number(_) => FieldType::Number,
        CellValue::Bool(_) => FieldType::Bool,
        _ => FieldType::Text,
    }
}

fn validate_field(header: &str, value: &str, _col: u32) -> Option<String> {
    let h = header.to_lowercase();
    if h.contains("email") && !value.is_empty() && !value.contains('@') {
        return Some(format!("{}: invalid email address", header));
    }
    if (h.contains("age")
        || h.contains("qty")
        || h.contains("quantity")
        || h.contains("count")
        || h.contains("number"))
        && !value.is_empty()
        && value.parse::<f64>().is_err()
    {
        return Some(format!("{}: must be a number", header));
    }
    None
}
