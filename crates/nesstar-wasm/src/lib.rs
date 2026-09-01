use serde::{Deserialize, Serialize};
use std::io::Cursor;
use wasm_bindgen::prelude::*;

use nesstar_core::{
    ddi::parse_ddi_reader,
    decode::{DecodeError, decode_metadata_batches, decode_resource_batches},
    formats::{dta::DtaOutput, fwf::FixedWidthOutput, parquet::ParquetOutput, spss::SpssOutput},
    layout::{metadata_scan::discover_metadata_layout, resource_index::discover_resource_layout},
    model::{BlockDefinition, CellValue, VariableDefinition},
    source::ReadOnlySource,
};

#[derive(Serialize, Deserialize)]
pub struct DatasetInfo {
    pub name: String,
    pub row_count: u64,
    pub column_count: usize,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub label: String,
    pub declared_type: String,
    pub width: u32,
}

#[derive(Serialize, Deserialize)]
pub struct PreviewData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
    pub total_cols: usize,
}

#[wasm_bindgen]
pub fn get_dataset_info(_nesstar_bytes: &[u8], ddi_xml: &str) -> Result<JsValue, JsValue> {
    let metadata = parse_ddi_reader(ddi_xml.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("DDI Error: {e}")))?;

    let block = metadata
        .blocks
        .first()
        .ok_or_else(|| JsValue::from_str("DDI XML contains no data blocks"))?;

    let cols = block
        .variables
        .iter()
        .map(|v| ColumnInfo {
            name: v.name.clone(),
            label: v.label.clone(),
            declared_type: format!("{:?}", v.declared_type),
            width: v.ddi_width,
        })
        .collect();

    let info = DatasetInfo {
        name: block.name.clone(),
        row_count: block.row_count,
        column_count: block.variables.len(),
        columns: cols,
    };

    serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn preview_nesstar(
    nesstar_bytes: &[u8],
    ddi_xml: &str,
    limit: usize,
) -> Result<JsValue, JsValue> {
    let metadata = parse_ddi_reader(ddi_xml.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("DDI Error: {e}")))?;
    let block = metadata
        .blocks
        .first()
        .ok_or_else(|| JsValue::from_str("DDI XML contains no data blocks"))?;

    let source = ReadOnlySource::from_bytes(nesstar_bytes.to_vec())
        .map_err(|e| JsValue::from_str(&format!("Binary Error: {e}")))?;

    let headers: Vec<String>;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let max_rows = if limit == 0 { 50 } else { limit };

    if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        headers = layout
            .columns
            .iter()
            .map(|c| c.variable.name.clone())
            .collect();
        let _ = decode_resource_batches(
            &source,
            &layout,
            max_rows,
            || true,
            |batch| {
                for row_idx in 0..batch.row_count.min(max_rows - rows.len()) {
                    let mut row = Vec::with_capacity(batch.columns.len());
                    for col in &batch.columns {
                        match &col.values[row_idx] {
                            CellValue::Missing => row.push(String::new()),
                            CellValue::Text(s) => row.push(s.clone()),
                        }
                    }
                    rows.push(row);
                    if rows.len() >= max_rows {
                        break;
                    }
                }
                Ok(())
            },
        );
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| JsValue::from_str(&format!("Layout Error: {e}")))?;
        headers = layout
            .columns_in_ddi_order()
            .into_iter()
            .map(|c| c.variable.name.clone())
            .collect();
        let _ = decode_metadata_batches(
            &source,
            &layout,
            max_rows,
            || true,
            |batch| {
                for row_idx in 0..batch.row_count.min(max_rows - rows.len()) {
                    let mut row = Vec::with_capacity(batch.columns.len());
                    for col in &batch.columns {
                        match &col.values[row_idx] {
                            CellValue::Missing => row.push(String::new()),
                            CellValue::Text(s) => row.push(s.clone()),
                        }
                    }
                    rows.push(row);
                    if rows.len() >= max_rows {
                        break;
                    }
                }
                Ok(())
            },
        );
    }

    let total_cols = headers.len();
    let preview = PreviewData {
        headers,
        rows,
        total_rows: block.row_count,
        total_cols,
    };

    serde_wasm_bindgen::to_value(&preview).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn convert_nesstar(
    nesstar_bytes: &[u8],
    ddi_xml: &str,
    format: &str,
) -> Result<js_sys::Uint8Array, JsValue> {
    let metadata = parse_ddi_reader(ddi_xml.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("DDI Error: {e}")))?;
    let block = metadata
        .blocks
        .first()
        .ok_or_else(|| JsValue::from_str("DDI XML contains no data blocks"))?;

    let source = ReadOnlySource::from_bytes(nesstar_bytes.to_vec())
        .map_err(|e| JsValue::from_str(&format!("Binary Error: {e}")))?;

    let is_resource = discover_resource_layout(source.bytes(), block);
    let (headers, variables): (Vec<String>, Vec<VariableDefinition>) = match &is_resource {
        Ok(layout) => (
            layout
                .columns
                .iter()
                .map(|c| c.variable.name.clone())
                .collect(),
            layout.columns.iter().map(|c| c.variable.clone()).collect(),
        ),
        Err(_) => {
            let layout = discover_metadata_layout(source.bytes(), block)
                .map_err(|e| JsValue::from_str(&format!("Layout Error: {e}")))?;
            (
                layout
                    .columns_in_ddi_order()
                    .into_iter()
                    .map(|c| c.variable.name.clone())
                    .collect(),
                layout
                    .columns_in_ddi_order()
                    .into_iter()
                    .map(|c| c.variable.clone())
                    .collect(),
            )
        }
    };

    let fmt = format.trim().to_lowercase();
    let result = convert_to_format_bytes(
        &source,
        block,
        &headers,
        &variables,
        is_resource.is_ok(),
        &fmt,
    )?;

    let array = js_sys::Uint8Array::new_with_length(result.len() as u32);
    array.copy_from(&result);
    Ok(array)
}

fn convert_to_format_bytes(
    source: &ReadOnlySource,
    block: &BlockDefinition,
    headers: &[String],
    variables: &[VariableDefinition],
    is_resource: bool,
    format: &str,
) -> Result<Vec<u8>, JsValue> {
    match format {
        "parquet" => {
            let buffer = Cursor::new(Vec::new());
            let mut writer = ParquetOutput::from_writer(buffer, variables)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| writer.write_batch(&batch).map_err(DecodeError::Writer),
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| writer.write_batch(&batch).map_err(DecodeError::Writer),
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }

            let cursor = writer.finish().map_err(|e| JsValue::from_str(&e))?;
            Ok(cursor.into_inner())
        }
        "dta" | "stata" => {
            let cursor = Cursor::new(Vec::new());
            let mut writer = DtaOutput::from_writer(cursor, headers, variables)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        writer
                            .write_batch(&batch, variables)
                            .map_err(DecodeError::Writer)
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        writer
                            .write_batch(&batch, variables)
                            .map_err(DecodeError::Writer)
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }

            let cursor = writer
                .finish(headers, variables)
                .map_err(|e| JsValue::from_str(&e))?;
            Ok(cursor.into_inner())
        }
        "sav" | "spss" => {
            let cursor = Cursor::new(Vec::new());
            let mut writer = SpssOutput::from_writer(cursor, variables)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| writer.write_batch(&batch).map_err(DecodeError::Writer),
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| writer.write_batch(&batch).map_err(DecodeError::Writer),
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }

            let cursor = writer
                .finish(variables)
                .map_err(|e| JsValue::from_str(&e))?;
            Ok(cursor.into_inner())
        }
        "fwf" => {
            let ddi_widths: Vec<u32> = variables.iter().map(|v| v.ddi_width).collect();
            let mut fwf_bytes = Vec::new();
            let mut writer = FixedWidthOutput::from_writer(&mut fwf_bytes, headers, &ddi_widths)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        writer
                            .write_batch(&batch)
                            .map_err(|e| DecodeError::Writer(e.to_string()))
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        writer
                            .write_batch(&batch)
                            .map_err(|e| DecodeError::Writer(e.to_string()))
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }
            writer
                .finish()
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            Ok(fwf_bytes)
        }
        "jsonl" => {
            let mut output_bytes = Vec::new();
            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        for row_idx in 0..batch.row_count {
                            let mut obj = serde_json::Map::new();
                            for (c_idx, col) in batch.columns.iter().enumerate() {
                                match &col.values[row_idx] {
                                    CellValue::Missing => {
                                        obj.insert(headers[c_idx].clone(), serde_json::Value::Null);
                                    }
                                    CellValue::Text(s) => {
                                        obj.insert(
                                            headers[c_idx].clone(),
                                            serde_json::Value::String(s.clone()),
                                        );
                                    }
                                }
                            }
                            if let Ok(line) = serde_json::to_string(&obj) {
                                output_bytes.extend_from_slice(line.as_bytes());
                                output_bytes.push(b'\n');
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        for row_idx in 0..batch.row_count {
                            let mut obj = serde_json::Map::new();
                            for (c_idx, col) in batch.columns.iter().enumerate() {
                                match &col.values[row_idx] {
                                    CellValue::Missing => {
                                        obj.insert(headers[c_idx].clone(), serde_json::Value::Null);
                                    }
                                    CellValue::Text(s) => {
                                        obj.insert(
                                            headers[c_idx].clone(),
                                            serde_json::Value::String(s.clone()),
                                        );
                                    }
                                }
                            }
                            if let Ok(line) = serde_json::to_string(&obj) {
                                output_bytes.extend_from_slice(line.as_bytes());
                                output_bytes.push(b'\n');
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }
            Ok(output_bytes)
        }
        "json" => {
            let mut output_bytes = Vec::new();
            output_bytes.extend_from_slice(b"[\n");
            let mut first_record = true;

            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        for row_idx in 0..batch.row_count {
                            if !first_record {
                                output_bytes.extend_from_slice(b",\n");
                            }
                            first_record = false;
                            let mut obj = serde_json::Map::new();
                            for (c_idx, col) in batch.columns.iter().enumerate() {
                                match &col.values[row_idx] {
                                    CellValue::Missing => {
                                        obj.insert(headers[c_idx].clone(), serde_json::Value::Null);
                                    }
                                    CellValue::Text(s) => {
                                        obj.insert(
                                            headers[c_idx].clone(),
                                            serde_json::Value::String(s.clone()),
                                        );
                                    }
                                }
                            }
                            if let Ok(line) = serde_json::to_string(&obj) {
                                output_bytes.extend_from_slice(line.as_bytes());
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        for row_idx in 0..batch.row_count {
                            if !first_record {
                                output_bytes.extend_from_slice(b",\n");
                            }
                            first_record = false;
                            let mut obj = serde_json::Map::new();
                            for (c_idx, col) in batch.columns.iter().enumerate() {
                                match &col.values[row_idx] {
                                    CellValue::Missing => {
                                        obj.insert(headers[c_idx].clone(), serde_json::Value::Null);
                                    }
                                    CellValue::Text(s) => {
                                        obj.insert(
                                            headers[c_idx].clone(),
                                            serde_json::Value::String(s.clone()),
                                        );
                                    }
                                }
                            }
                            if let Ok(line) = serde_json::to_string(&obj) {
                                output_bytes.extend_from_slice(line.as_bytes());
                            }
                        }
                        Ok(())
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }
            output_bytes.extend_from_slice(b"\n]\n");
            Ok(output_bytes)
        }
        _ => {
            let is_tsv = format.eq_ignore_ascii_case("tsv") || format.eq_ignore_ascii_case("txt");
            let sep = if is_tsv { b'\t' } else { b',' };
            let mut output_bytes = Vec::new();

            for (idx, h) in headers.iter().enumerate() {
                if idx > 0 {
                    output_bytes.push(sep);
                }
                output_bytes.extend_from_slice(h.as_bytes());
            }
            output_bytes.push(b'\n');

            if is_resource {
                let layout = discover_resource_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_resource_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        for row_idx in 0..batch.row_count {
                            for (c_idx, col) in batch.columns.iter().enumerate() {
                                if c_idx > 0 {
                                    output_bytes.push(sep);
                                }
                                match &col.values[row_idx] {
                                    CellValue::Missing => {}
                                    CellValue::Text(s) => {
                                        if !is_tsv
                                            && (s.contains(',')
                                                || s.contains('"')
                                                || s.contains('\n'))
                                        {
                                            output_bytes.push(b'"');
                                            output_bytes.extend_from_slice(
                                                s.replace('"', "\"\"").as_bytes(),
                                            );
                                            output_bytes.push(b'"');
                                        } else {
                                            output_bytes.extend_from_slice(s.as_bytes());
                                        }
                                    }
                                }
                            }
                            output_bytes.push(b'\n');
                        }
                        Ok(())
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            } else {
                let layout = discover_metadata_layout(source.bytes(), block)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                decode_metadata_batches(
                    source,
                    &layout,
                    5_000,
                    || true,
                    |batch| {
                        for row_idx in 0..batch.row_count {
                            for (c_idx, col) in batch.columns.iter().enumerate() {
                                if c_idx > 0 {
                                    output_bytes.push(sep);
                                }
                                match &col.values[row_idx] {
                                    CellValue::Missing => {}
                                    CellValue::Text(s) => {
                                        if !is_tsv
                                            && (s.contains(',')
                                                || s.contains('"')
                                                || s.contains('\n'))
                                        {
                                            output_bytes.push(b'"');
                                            output_bytes.extend_from_slice(
                                                s.replace('"', "\"\"").as_bytes(),
                                            );
                                            output_bytes.push(b'"');
                                        } else {
                                            output_bytes.extend_from_slice(s.as_bytes());
                                        }
                                    }
                                }
                            }
                            output_bytes.push(b'\n');
                        }
                        Ok(())
                    },
                )
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }
            Ok(output_bytes)
        }
    }
}
