//! Bounded, checked decoding for both Nesstar layout variants.

use thiserror::Error;

use crate::{
    layout::{
        metadata_scan::{MetadataColumn, MetadataEncoding, MetadataLayout},
        resource_index::{ResourceColumn, ResourceLayout},
    },
    model::{CellValue, DeclaredType},
    source::{ReadOnlySource, SourceError},
};

const DOUBLE_MISSING_SENTINEL: f64 = f64::MAX * 0.99;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error("decode cancelled")]
    Cancelled,
    #[error("decode `{variable}`: unsupported compact format code {format_code}")]
    UnsupportedFormat { variable: String, format_code: u8 },
    #[error("decode `{variable}`: invalid payload length {length} for {rows} rows")]
    InvalidPayload {
        variable: String,
        length: usize,
        rows: usize,
    },
    #[error("decode `{variable}`: invalid value width {width}")]
    InvalidWidth { variable: String, width: usize },
    #[error("decode arithmetic overflow while calculating {context}")]
    Overflow { context: &'static str },
    #[error("output writer failed: {0}")]
    Writer(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnBatch {
    pub variable_name: String,
    pub values: Vec<CellValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordBatch {
    pub start_row: u64,
    pub row_count: usize,
    /// Columns retain the stable DDI schema order.
    pub columns: Vec<ColumnBatch>,
}

/// Decode metadata-scan data in bounded row batches. `keep_going` is checked
/// immediately before every batch, including the first one.
pub fn decode_metadata_batches(
    source: &ReadOnlySource,
    layout: &MetadataLayout,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
    mut on_batch: impl FnMut(RecordBatch) -> Result<(), DecodeError>,
) -> Result<(), DecodeError> {
    let rows = rows_from_metadata(layout)?;
    let positions = metadata_positions(layout)?;
    decode_batches(
        rows,
        batch_size,
        &mut keep_going,
        &mut on_batch,
        |start, count| {
            layout
                .columns_in_ddi_order()
                .into_iter()
                .map(|column| {
                    let offset = *positions
                        .iter()
                        .find_map(|(candidate, offset)| {
                            (candidate.slot.var_num == column.slot.var_num).then_some(offset)
                        })
                        .expect("every metadata column has a position");
                    decode_metadata_column(source, column, offset, start, count)
                })
                .collect()
        },
    )
}

/// Decode resource-indexed data in bounded row batches.
pub fn decode_resource_batches(
    source: &ReadOnlySource,
    layout: &ResourceLayout,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
    mut on_batch: impl FnMut(RecordBatch) -> Result<(), DecodeError>,
) -> Result<(), DecodeError> {
    let rows = usize::try_from(layout.row_count).map_err(|_| DecodeError::Overflow {
        context: "resource row count",
    })?;
    decode_batches(
        rows,
        batch_size,
        &mut keep_going,
        &mut on_batch,
        |start, count| {
            layout
                .columns
                .iter()
                .map(|column| decode_resource_column(source, column, rows, start, count))
                .collect()
        },
    )
}

fn decode_batches(
    rows: usize,
    batch_size: usize,
    keep_going: &mut impl FnMut() -> bool,
    on_batch: &mut impl FnMut(RecordBatch) -> Result<(), DecodeError>,
    mut decode_batch: impl FnMut(usize, usize) -> Result<Vec<ColumnBatch>, DecodeError>,
) -> Result<(), DecodeError> {
    let batch_size = batch_size.max(1);
    for start in (0..rows).step_by(batch_size) {
        if !keep_going() {
            return Err(DecodeError::Cancelled);
        }
        let count = (rows - start).min(batch_size);
        on_batch(RecordBatch {
            start_row: start as u64,
            row_count: count,
            columns: decode_batch(start, count)?,
        })?;
    }
    Ok(())
}

fn rows_from_metadata(layout: &MetadataLayout) -> Result<usize, DecodeError> {
    let payload = layout
        .metadata_offset
        .checked_sub(layout.data_offset)
        .ok_or(DecodeError::Overflow {
            context: "metadata payload range",
        })?;
    let width = layout.columns.iter().try_fold(0usize, |total, column| {
        total
            .checked_add(column.binary_width)
            .ok_or(DecodeError::Overflow {
                context: "metadata row width",
            })
    })?;
    if width == 0 || payload % width != 0 {
        return Err(DecodeError::InvalidPayload {
            variable: "metadata layout".into(),
            length: payload,
            rows: 0,
        });
    }
    Ok(payload / width)
}

fn metadata_positions(
    layout: &MetadataLayout,
) -> Result<Vec<(&MetadataColumn, usize)>, DecodeError> {
    let mut current = layout.data_offset;
    let rows = rows_from_metadata(layout)?;
    layout
        .columns
        .iter()
        .map(|column| {
            let position = current;
            let size = column
                .binary_width
                .checked_mul(rows)
                .ok_or(DecodeError::Overflow {
                    context: "metadata column size",
                })?;
            current = current.checked_add(size).ok_or(DecodeError::Overflow {
                context: "metadata column offset",
            })?;
            Ok((column, position))
        })
        .collect()
}

fn decode_metadata_column(
    source: &ReadOnlySource,
    column: &MetadataColumn,
    payload_offset: usize,
    start: usize,
    count: usize,
) -> Result<ColumnBatch, DecodeError> {
    let width = column.binary_width;
    let values = (start..start + count)
        .map(|row| {
            let offset = payload_offset
                .checked_add(row.checked_mul(width).ok_or(DecodeError::Overflow {
                    context: "metadata value offset",
                })?)
                .ok_or(DecodeError::Overflow {
                    context: "metadata value offset",
                })?;
            let bytes = source.slice(offset, width, "metadata value")?;
            let value = match column.slot.encoding {
                MetadataEncoding::FixedAscii => text_value(bytes, false),
                MetadataEncoding::LittleEndianDouble => float_value(
                    f64::from_le_bytes(bytes.try_into().map_err(|_| {
                        DecodeError::InvalidWidth {
                            variable: column.variable.name.clone(),
                            width,
                        }
                    })?),
                    column.variable.decimals,
                ),
                MetadataEncoding::OffsetInteger => {
                    offset_value(bytes, range_minimum(column.variable.range.as_ref()))
                }
            };
            Ok(value)
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
    Ok(ColumnBatch {
        variable_name: column.variable.name.clone(),
        values,
    })
}

fn decode_resource_column(
    source: &ReadOnlySource,
    column: &ResourceColumn,
    rows: usize,
    start: usize,
    count: usize,
) -> Result<ColumnBatch, DecodeError> {
    let resource = &column.resource;
    let length = resource.effective_payload_length;
    let bytes = source.slice(resource.payload_offset, length, "resource payload")?;
    let compact = compact_size(resource.value_format_code, rows);
    let numeric = column.variable.declared_type == DeclaredType::Numeric;
    let compact_numeric = compact == Some(length) && (resource.mode_code == 5 || numeric);
    let values = if compact_numeric {
        decode_compact(
            bytes,
            resource.value_format_code,
            start,
            count,
            &column.variable.name,
            column.variable.decimals,
            resource.value_offset_i64,
            resource.mode_code == 5,
        )?
    } else {
        let width = text_width(resource.width_value, length, rows, &column.variable.name)?;
        (start..start + count)
            .map(|row| {
                let offset = row.checked_mul(width).ok_or(DecodeError::Overflow {
                    context: "resource text value offset",
                })?;
                let value = if numeric && looks_like_raw_byte_numeric(bytes, rows, width) {
                    let raw = *bytes.get(row).ok_or(DecodeError::InvalidPayload {
                        variable: column.variable.name.clone(),
                        length,
                        rows,
                    })?;
                    if raw == u8::MAX {
                        CellValue::Missing
                    } else {
                        CellValue::Text(raw.to_string())
                    }
                } else {
                    let value =
                        bytes
                            .get(offset..offset + width)
                            .ok_or(DecodeError::InvalidPayload {
                                variable: column.variable.name.clone(),
                                length,
                                rows,
                            })?;
                    text_value(value, resource.mode_code == 1)
                };
                Ok(value)
            })
            .collect::<Result<Vec<_>, DecodeError>>()?
    };
    Ok(ColumnBatch {
        variable_name: column.variable.name.clone(),
        values,
    })
}

fn text_width(
    declared: usize,
    length: usize,
    rows: usize,
    variable: &str,
) -> Result<usize, DecodeError> {
    let expected = declared.checked_mul(rows).ok_or(DecodeError::Overflow {
        context: "declared text payload size",
    })?;
    let width = if declared != 0 && expected == length {
        declared
    } else if rows != 0 && length.is_multiple_of(rows) {
        length / rows
    } else {
        return Err(DecodeError::InvalidPayload {
            variable: variable.into(),
            length,
            rows,
        });
    };
    if width == 0 {
        Err(DecodeError::InvalidWidth {
            variable: variable.into(),
            width,
        })
    } else {
        Ok(width)
    }
}

fn compact_size(format: u8, rows: usize) -> Option<usize> {
    match format {
        2 => Some(rows.div_ceil(2)),
        3 => Some(rows),
        4 => rows.checked_mul(2),
        5 => rows.checked_mul(3),
        6 => rows.checked_mul(4),
        7 => rows.checked_mul(5),
        10 => rows.checked_mul(8),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)] // Decoder inputs directly mirror resource-directory metadata.
fn decode_compact(
    bytes: &[u8],
    format: u8,
    start: usize,
    count: usize,
    variable: &str,
    decimals: u16,
    additive_offset: i64,
    apply_offset: bool,
) -> Result<Vec<CellValue>, DecodeError> {
    (start..start + count)
        .map(|row| match format {
            2 => {
                let byte = *bytes.get(row / 2).ok_or(DecodeError::InvalidPayload {
                    variable: variable.into(),
                    length: bytes.len(),
                    rows: row + 1,
                })?;
                let value = if row % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                Ok(if value == 0x0f {
                    CellValue::Missing
                } else {
                    integer_value(i64::from(value), additive_offset, apply_offset)
                })
            }
            3..=7 => {
                let width = usize::from(format - 2);
                let offset = row.checked_mul(width).ok_or(DecodeError::Overflow {
                    context: "compact value offset",
                })?;
                let raw = unsigned_le(bytes.get(offset..offset + width).ok_or(
                    DecodeError::InvalidPayload {
                        variable: variable.into(),
                        length: bytes.len(),
                        rows: row + 1,
                    },
                )?);
                let missing = (1u64 << (width * 8)) - 1;
                Ok(if raw == missing {
                    CellValue::Missing
                } else {
                    integer_value(raw as i64, additive_offset, apply_offset)
                })
            }
            10 => {
                let offset = row.checked_mul(8).ok_or(DecodeError::Overflow {
                    context: "compact double offset",
                })?;
                let value = f64::from_le_bytes(
                    bytes
                        .get(offset..offset + 8)
                        .ok_or(DecodeError::InvalidPayload {
                            variable: variable.into(),
                            length: bytes.len(),
                            rows: row + 1,
                        })?
                        .try_into()
                        .expect("fixed double slice"),
                );
                Ok(float_value(value, decimals))
            }
            _ => Err(DecodeError::UnsupportedFormat {
                variable: variable.into(),
                format_code: format,
            }),
        })
        .collect()
}

fn unsigned_le(bytes: &[u8]) -> u64 {
    bytes.iter().enumerate().fold(0u64, |value, (index, byte)| {
        value | (u64::from(*byte) << (index * 8))
    })
}
fn range_minimum(range: Option<&crate::model::NumericRange>) -> i64 {
    range.and_then(|range| range.minimum).unwrap_or(0.0) as i64
}
fn integer_value(raw: i64, offset: i64, apply_offset: bool) -> CellValue {
    let to_add = if apply_offset { offset } else { 0 };
    if let Some(sum) = raw.checked_add(to_add) {
        CellValue::Text(sum.to_string())
    } else {
        CellValue::Missing
    }
}
fn text_value(bytes: &[u8], nul_terminated: bool) -> CellValue {
    let bytes = if nul_terminated {
        &bytes[..bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len())]
    } else {
        bytes
    };
    let value = String::from_utf8_lossy(bytes)
        .replace('\0', " ")
        .trim()
        .to_owned();
    if value.is_empty() {
        CellValue::Missing
    } else {
        CellValue::Text(value)
    }
}
fn offset_value(bytes: &[u8], minimum: i64) -> CellValue {
    if bytes.iter().all(|byte| *byte == u8::MAX) {
        CellValue::Missing
    } else {
        let raw = unsigned_le(bytes);
        match i64::try_from(raw) {
            Ok(raw_i64) => match raw_i64.checked_add(minimum) {
                Some(sum) => CellValue::Text(sum.to_string()),
                None => CellValue::Missing,
            },
            Err(_) => CellValue::Missing,
        }
    }
}
fn float_value(value: f64, decimals: u16) -> CellValue {
    if !value.is_finite() || value >= DOUBLE_MISSING_SENTINEL {
        return CellValue::Missing;
    }
    if (-9223372036854775808.0..9223372036854775808.0).contains(&value) && value.fract() == 0.0 {
        return CellValue::Text((value as i64).to_string());
    }
    let formatted = format!(
        "{value:.precision$}",
        precision = usize::from(decimals).max(6)
    );
    CellValue::Text(
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned(),
    )
}
fn looks_like_raw_byte_numeric(bytes: &[u8], rows: usize, width: usize) -> bool {
    if width != 1 || bytes.len() < rows || rows == 0 {
        return false;
    }
    let sample = &bytes[..rows.min(1000)];
    sample
        .iter()
        .filter(|byte| **byte < 32 && !matches!(**byte, 0 | 9 | 10 | 13))
        .count()
        * 4
        > sample.len()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::{
        ddi::parse_ddi,
        layout::{
            metadata_scan::discover_metadata_layout, resource_index::discover_resource_layout,
        },
    };

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn expected_tsv(name: &str) -> Vec<Vec<String>> {
        fs::read_to_string(fixture(name))
            .unwrap()
            .lines()
            .skip(1)
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect()
    }

    fn collect_rows(batches: &mut Vec<Vec<String>>, batch: RecordBatch) {
        for row in 0..batch.row_count {
            batches.push(
                batch
                    .columns
                    .iter()
                    .map(|column| match &column.values[row] {
                        CellValue::Missing => String::new(),
                        CellValue::Text(value) => value.clone(),
                    })
                    .collect(),
            );
        }
    }

    #[test]
    fn metadata_fixture_matches_expected_at_small_and_large_batch_sizes() {
        let metadata = parse_ddi(fixture("synthetic/metadata-scan.ddi.xml")).unwrap();
        let source = ReadOnlySource::open(fixture("synthetic/metadata-scan.Nesstar")).unwrap();
        let layout = discover_metadata_layout(source.bytes(), &metadata.blocks[0]).unwrap();
        for batch_size in [1, 64] {
            let mut rows = Vec::new();
            decode_metadata_batches(
                &source,
                &layout,
                batch_size,
                || true,
                |batch| {
                    collect_rows(&mut rows, batch);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(rows, expected_tsv("expected/metadata-scan.tsv"));
        }
    }

    #[test]
    fn resource_fixture_matches_expected_at_small_and_large_batch_sizes() {
        let metadata = parse_ddi(fixture("synthetic/resource-index.ddi.xml")).unwrap();
        let source = ReadOnlySource::open(fixture("synthetic/resource-index.Nesstar")).unwrap();
        let layout = discover_resource_layout(source.bytes(), &metadata.blocks[0]).unwrap();
        for batch_size in [1, 64] {
            let mut rows = Vec::new();
            decode_resource_batches(
                &source,
                &layout,
                batch_size,
                || true,
                |batch| {
                    collect_rows(&mut rows, batch);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(rows, expected_tsv("expected/resource-index.tsv"));
        }
    }

    #[test]
    fn cancellation_is_checked_between_resource_batches() {
        let metadata = parse_ddi(fixture("synthetic/resource-index.ddi.xml")).unwrap();
        let source = ReadOnlySource::open(fixture("synthetic/resource-index.Nesstar")).unwrap();
        let layout = discover_resource_layout(source.bytes(), &metadata.blocks[0]).unwrap();
        let mut calls = 0;
        let result = decode_resource_batches(
            &source,
            &layout,
            2,
            || {
                calls += 1;
                calls == 1
            },
            |_| Ok(()),
        );
        assert!(matches!(result, Err(DecodeError::Cancelled)));
        assert_eq!(calls, 2);
    }

    #[test]
    fn malformed_payload_range_returns_an_error() {
        let metadata = parse_ddi(fixture("synthetic/resource-index.ddi.xml")).unwrap();
        let source = ReadOnlySource::open(fixture("synthetic/resource-index.Nesstar")).unwrap();
        let mut layout = discover_resource_layout(source.bytes(), &metadata.blocks[0]).unwrap();
        layout.columns[0].resource.payload_offset = source.len();
        let result = decode_resource_batches(&source, &layout, 1, || true, |_| Ok(()));
        assert!(matches!(
            result,
            Err(DecodeError::Source(SourceError::OutOfBounds { .. }))
        ));
    }

    #[test]
    fn test_float_value() {
        // Negative values
        assert_eq!(
            float_value(-12.34, 2),
            CellValue::Text("-12.34".to_string())
        );
        assert_eq!(float_value(-5.0, 2), CellValue::Text("-5".to_string()));

        // Zero
        assert_eq!(float_value(0.0, 2), CellValue::Text("0".to_string()));
        assert_eq!(float_value(-0.0, 2), CellValue::Text("0".to_string()));

        // Fractional values
        assert_eq!(
            float_value(0.1234567, 4),
            CellValue::Text("0.123457".to_string())
        );
        assert_eq!(
            float_value(0.123456789, 8),
            CellValue::Text("0.12345679".to_string())
        );
        assert_eq!(float_value(1.5, 0), CellValue::Text("1.5".to_string()));

        // NaN
        assert_eq!(float_value(f64::NAN, 2), CellValue::Missing);

        // Infinity
        assert_eq!(float_value(f64::INFINITY, 2), CellValue::Missing);
        assert_eq!(float_value(f64::NEG_INFINITY, 2), CellValue::Missing);

        // Missing sentinel
        assert_eq!(float_value(DOUBLE_MISSING_SENTINEL, 2), CellValue::Missing);
        assert_eq!(
            float_value(DOUBLE_MISSING_SENTINEL + 1.0, 2),
            CellValue::Missing
        );

        // Large float values that do not fit in i64 (like 1e20 and 1.23e20)
        assert_eq!(
            float_value(1e20, 2),
            CellValue::Text("100000000000000000000".to_string())
        );
        assert_eq!(
            float_value(1.23e20, 2),
            CellValue::Text("123000000000000000000".to_string())
        );

        // Check edge boundaries of i64.
        assert_eq!(
            float_value(9007199254740990.0, 2),
            CellValue::Text("9007199254740990".to_string())
        );
        assert_eq!(
            float_value(-9007199254740990.0, 2),
            CellValue::Text("-9007199254740990".to_string())
        );
        assert_eq!(
            float_value(9223372036854774784.0, 2),
            CellValue::Text("9223372036854774784".to_string())
        );
        assert_eq!(
            float_value(-9223372036854774784.0, 2),
            CellValue::Text("-9223372036854774784".to_string())
        );
    }

    #[test]
    fn test_checked_additions() {
        // integer_value
        // Safe addition
        assert_eq!(
            integer_value(10, 5, true),
            CellValue::Text("15".to_string())
        );
        assert_eq!(
            integer_value(10, 5, false),
            CellValue::Text("10".to_string())
        );

        // Overflow addition
        assert_eq!(integer_value(i64::MAX, 1, true), CellValue::Missing);
        assert_eq!(integer_value(i64::MIN, -1, true), CellValue::Missing);

        // offset_value
        // Safe offset addition
        assert_eq!(
            offset_value(&[1, 0, 0, 0, 0, 0, 0, 0], 10),
            CellValue::Text("11".to_string())
        );

        // Missing sentinel (all bytes u8::MAX)
        assert_eq!(offset_value(&[255, 255, 255, 255], 10), CellValue::Missing);

        // Value too large for i64 (try_from failure)
        assert_eq!(
            offset_value(&[0, 0, 0, 0, 0, 0, 0, 128], 10),
            CellValue::Missing
        );

        // Overflow addition
        assert_eq!(
            offset_value(&[255, 255, 255, 255, 255, 255, 255, 127], 1),
            CellValue::Missing
        );
    }
}
