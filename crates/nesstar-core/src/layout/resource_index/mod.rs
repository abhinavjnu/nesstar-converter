//! Resource-index layout discovery.
//!
//! Modern Nesstar containers use a trailing record index.  The index is the
//! source of truth for payload spans; DDI dimensions are used only to match a
//! descriptor to its logical block and to classify a column for decoding.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::model::{BlockDefinition, VariableDefinition};

const RESOURCE_INDEX_OFFSET_FIELD: usize = 0x25;
const DATASET_COUNT_FIELD: usize = 0x2b;
const DESCRIPTOR_RECORD_SIZE_FIELD: usize = 0x2d;
const DESCRIPTOR_TABLE_RECORD_ID_FIELD: usize = 0x2f;
const RESOURCE_INDEX_RECORD_SIZE: usize = 15;
const MIN_DIRECTORY_ENTRY_SIZE: usize = 160;
const MIN_DESCRIPTOR_SIZE: usize = 26;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResourceLayoutError {
    #[error("resource index {context}: offset arithmetic overflow")]
    OffsetOverflow { context: &'static str },
    #[error("resource index {context}: range {start}..{end} exceeds {length} bytes")]
    OutOfBounds {
        context: &'static str,
        start: usize,
        end: usize,
        length: usize,
    },
    #[error("resource index has no usable records")]
    EmptyIndex,
    #[error("resource index references missing record {record_id}")]
    MissingRecord { record_id: u32 },
    #[error("resource descriptor record is invalid: {0}")]
    InvalidDescriptor(String),
    #[error("resource directory entry for `{name}` is invalid: {reason}")]
    InvalidDirectoryEntry { name: String, reason: String },
    #[error("resource layout for block `{block_id}` was not found")]
    NotFound { block_id: String },
    #[error("resource layout for block `{block_id}` has no DDI variable overlap")]
    NoDdiOverlap { block_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRecord {
    pub record_id: u32,
    pub offset: usize,
    pub length: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceVariable {
    pub entry_index: u32,
    pub name: String,
    pub payload_record_id: u32,
    pub payload_offset: usize,
    pub payload_length: usize,
    pub effective_payload_length: usize,
    /// True only when two adjacent payload halves are byte-for-byte equal.
    pub duplicate_payload: bool,
    pub width_value: usize,
    pub label_resource_id: u32,
    pub category_resource_id: u16,
    pub object_id: u32,
    pub mode_code: u8,
    pub value_format_code: u8,
    pub value_offset_i64: i64,
}

#[derive(Clone, Debug)]
pub struct ResourceLayout {
    pub block_id: String,
    pub dataset_number: u32,
    pub row_count: u64,
    pub variable_count: usize,
    pub directory_offset: usize,
    pub entry_size: usize,
    /// Columns are in DDI order and only include variables with directory entries.
    pub columns: Vec<ResourceColumn>,
}

#[derive(Clone, Debug)]
pub struct ResourceColumn {
    pub variable: VariableDefinition,
    pub resource: ResourceVariable,
}

/// Read the trailing record-id index with checked offsets and lengths.
pub fn parse_resource_index(
    data: &[u8],
) -> Result<BTreeMap<u32, ResourceRecord>, ResourceLayoutError> {
    let index_offset = usize::try_from(u48_le(data, RESOURCE_INDEX_OFFSET_FIELD, "offset field")?)
        .map_err(|_| ResourceLayoutError::OffsetOverflow {
            context: "index offset",
        })?;
    if index_offset == 0 {
        return Err(ResourceLayoutError::EmptyIndex);
    }
    let record_count =
        usize::try_from(u32_le(data, index_offset, "record count")?).map_err(|_| {
            ResourceLayoutError::OffsetOverflow {
                context: "record count",
            }
        })?;
    if record_count == 0 {
        return Err(ResourceLayoutError::EmptyIndex);
    }
    let records_offset =
        index_offset
            .checked_add(4)
            .ok_or(ResourceLayoutError::OffsetOverflow {
                context: "records offset",
            })?;
    let records_length = record_count.checked_mul(RESOURCE_INDEX_RECORD_SIZE).ok_or(
        ResourceLayoutError::OffsetOverflow {
            context: "records length",
        },
    )?;
    checked_range(data, records_offset, records_length, "records")?;

    let mut records = BTreeMap::new();
    for index in 0..record_count {
        let offset = records_offset
            .checked_add(index.checked_mul(RESOURCE_INDEX_RECORD_SIZE).ok_or(
                ResourceLayoutError::OffsetOverflow {
                    context: "record offset",
                },
            )?)
            .ok_or(ResourceLayoutError::OffsetOverflow {
                context: "record offset",
            })?;
        let record_id = u32_le(data, offset, "record id")?;
        let payload_offset = usize::try_from(u48_le(data, offset + 4, "record payload offset")?)
            .map_err(|_| ResourceLayoutError::OffsetOverflow {
                context: "record payload offset",
            })?;
        let length =
            usize::try_from(u32_le(data, offset + 10, "record length")?).map_err(|_| {
                ResourceLayoutError::OffsetOverflow {
                    context: "record length",
                }
            })?;
        checked_range(data, payload_offset, length, "record payload")?;
        records.insert(
            record_id,
            ResourceRecord {
                record_id,
                offset: payload_offset,
                length,
            },
        );
    }
    if records.is_empty() {
        Err(ResourceLayoutError::EmptyIndex)
    } else {
        Ok(records)
    }
}

/// Parse every descriptor that can be reached through a valid resource index.
pub fn parse_resource_layouts(
    data: &[u8],
    blocks: &[BlockDefinition],
) -> Result<Vec<ResourceLayout>, ResourceLayoutError> {
    let records = parse_resource_index(data)?;
    let dataset_count = usize::from(*data.get(DATASET_COUNT_FIELD).ok_or(
        ResourceLayoutError::OutOfBounds {
            context: "dataset count",
            start: DATASET_COUNT_FIELD,
            end: DATASET_COUNT_FIELD + 1,
            length: data.len(),
        },
    )?);
    let descriptor_size = usize::from(u16_le(
        data,
        DESCRIPTOR_RECORD_SIZE_FIELD,
        "descriptor size",
    )?);
    let descriptor_id = u32_le(
        data,
        DESCRIPTOR_TABLE_RECORD_ID_FIELD,
        "descriptor record id",
    )?;
    if dataset_count == 0 || descriptor_size == 0 {
        return Err(ResourceLayoutError::InvalidDescriptor(
            "missing dataset count or descriptor size".into(),
        ));
    }
    if descriptor_size < MIN_DESCRIPTOR_SIZE {
        return Err(ResourceLayoutError::InvalidDescriptor(format!(
            "descriptor size {descriptor_size} is below {MIN_DESCRIPTOR_SIZE}"
        )));
    }
    let descriptor_record =
        records
            .get(&descriptor_id)
            .ok_or(ResourceLayoutError::MissingRecord {
                record_id: descriptor_id,
            })?;
    let descriptor_bytes =
        dataset_count
            .checked_mul(descriptor_size)
            .ok_or(ResourceLayoutError::OffsetOverflow {
                context: "descriptor table size",
            })?;
    if descriptor_bytes > descriptor_record.length {
        return Err(ResourceLayoutError::InvalidDescriptor(
            "descriptor table exceeds its indexed record".into(),
        ));
    }
    checked_range(
        data,
        descriptor_record.offset,
        descriptor_bytes,
        "descriptor table",
    )?;

    let mut descriptors = Vec::new();
    for index in 0..dataset_count {
        let offset = descriptor_record
            .offset
            .checked_add(index.checked_mul(descriptor_size).ok_or(
                ResourceLayoutError::OffsetOverflow {
                    context: "descriptor offset",
                },
            )?)
            .ok_or(ResourceLayoutError::OffsetOverflow {
                context: "descriptor offset",
            })?;
        let variable_count =
            usize::try_from(u32_le(data, offset + 4, "descriptor variable count")?).map_err(
                |_| ResourceLayoutError::OffsetOverflow {
                    context: "descriptor variable count",
                },
            )?;
        let row_count = u64::from(u32_le(data, offset + 8, "descriptor row count")?);
        let entry_size = usize::from(u16_le(data, offset + 20, "directory entry size")?);
        let directory_id = u32_le(data, offset + 22, "directory record id")?;
        if variable_count == 0 || row_count == 0 {
            continue;
        }
        if entry_size < MIN_DIRECTORY_ENTRY_SIZE {
            return Err(ResourceLayoutError::InvalidDescriptor(format!(
                "directory entry size {entry_size} is below {MIN_DIRECTORY_ENTRY_SIZE}"
            )));
        }
        let directory = records
            .get(&directory_id)
            .ok_or(ResourceLayoutError::MissingRecord {
                record_id: directory_id,
            })?;
        let directory_bytes =
            variable_count
                .checked_mul(entry_size)
                .ok_or(ResourceLayoutError::OffsetOverflow {
                    context: "directory size",
                })?;
        if directory_bytes > directory.length {
            return Err(ResourceLayoutError::InvalidDescriptor(
                "directory exceeds its indexed record".into(),
            ));
        }
        let variables = parse_variable_directory(
            data,
            &records,
            directory,
            variable_count,
            entry_size,
            row_count,
        )?;
        descriptors.push(ParsedDescriptor {
            dataset_number: u32_le(data, offset, "dataset number")?,
            row_count,
            variable_count,
            directory_offset: directory.offset,
            entry_size,
            variables,
        });
    }

    let mut unused = (0..descriptors.len()).collect::<BTreeSet<_>>();
    let mut layouts = Vec::new();
    let mut ordered = blocks.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|block| block.file_id_number);
    for block in ordered {
        let ddi_names = block
            .variables
            .iter()
            .map(|variable| variable.name.as_str())
            .collect::<BTreeSet<_>>();
        let best = unused
            .iter()
            .copied()
            .max_by_key(|index| descriptor_score(&descriptors[*index], block, &ddi_names));
        let Some(index) = best else {
            continue;
        };
        let descriptor = &descriptors[index];
        let by_name = descriptor
            .variables
            .iter()
            .map(|variable| (variable.name.as_str(), variable))
            .collect::<BTreeMap<_, _>>();
        if ddi_names
            .intersection(&by_name.keys().copied().collect())
            .next()
            .is_none()
        {
            continue;
        }
        let columns = block
            .variables
            .iter()
            .filter_map(|variable| {
                by_name
                    .get(variable.name.as_str())
                    .map(|resource| ResourceColumn {
                        variable: variable.clone(),
                        resource: (*resource).clone(),
                    })
            })
            .collect::<Vec<_>>();
        unused.remove(&index);
        layouts.push(ResourceLayout {
            block_id: block.file_id.clone(),
            dataset_number: descriptor.dataset_number,
            row_count: descriptor.row_count,
            variable_count: descriptor.variable_count,
            directory_offset: descriptor.directory_offset,
            entry_size: descriptor.entry_size,
            columns,
        });
    }
    Ok(layouts)
}

pub fn discover_resource_layout(
    data: &[u8],
    block: &BlockDefinition,
) -> Result<ResourceLayout, ResourceLayoutError> {
    parse_resource_layouts(data, std::slice::from_ref(block))?
        .into_iter()
        .next()
        .ok_or_else(|| ResourceLayoutError::NotFound {
            block_id: block.file_id.clone(),
        })
}

#[derive(Clone, Debug)]
struct ParsedDescriptor {
    dataset_number: u32,
    row_count: u64,
    variable_count: usize,
    directory_offset: usize,
    entry_size: usize,
    variables: Vec<ResourceVariable>,
}

fn parse_variable_directory(
    data: &[u8],
    records: &BTreeMap<u32, ResourceRecord>,
    directory: &ResourceRecord,
    variable_count: usize,
    entry_size: usize,
    row_count: u64,
) -> Result<Vec<ResourceVariable>, ResourceLayoutError> {
    let mut variables = Vec::new();
    for index in 0..variable_count {
        let offset = directory
            .offset
            .checked_add(index.checked_mul(entry_size).ok_or(
                ResourceLayoutError::OffsetOverflow {
                    context: "directory entry offset",
                },
            )?)
            .ok_or(ResourceLayoutError::OffsetOverflow {
                context: "directory entry offset",
            })?;
        let entry = checked_range(data, offset, entry_size, "directory entry")?;
        let name = decode_name(entry).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let payload_record_id = u32_le(entry, 15, "directory payload record id")?;
        let payload =
            records
                .get(&payload_record_id)
                .ok_or(ResourceLayoutError::MissingRecord {
                    record_id: payload_record_id,
                })?;
        checked_range(data, payload.offset, payload.length, "column payload")?;
        let format = entry[5];
        let mode = entry[159];
        let width = usize::from(entry[149]);
        let duplicate_payload = is_duplicate_payload(
            data,
            payload,
            compact_payload_size(format, row_count),
            mode,
            width,
            row_count,
        );
        let effective_payload_length = if duplicate_payload {
            payload.length / 2
        } else {
            payload.length
        };
        variables.push(ResourceVariable {
            entry_index: u32_le(entry, 0, "directory entry index")?,
            name,
            payload_record_id,
            payload_offset: payload.offset,
            payload_length: payload.length,
            effective_payload_length,
            duplicate_payload,
            width_value: width,
            label_resource_id: u32_le(entry, 127, "label resource id")?,
            category_resource_id: u16_le(entry, 131, "category resource id")?,
            object_id: u32_le(entry, 155, "object id")?,
            mode_code: mode,
            value_format_code: format,
            value_offset_i64: i64::from_le_bytes(
                entry[6..14].try_into().expect("fixed eight-byte slice"),
            ),
        });
    }
    Ok(variables)
}

fn descriptor_score(
    descriptor: &ParsedDescriptor,
    block: &BlockDefinition,
    names: &BTreeSet<&str>,
) -> usize {
    let descriptor_names = descriptor
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut score = names.intersection(&descriptor_names).count();
    if descriptor.row_count == block.row_count {
        score += 10_000;
    }
    if descriptor.variable_count == block.variables.len() {
        score += 1_000;
    }
    score
}

fn compact_payload_size(format: u8, rows: u64) -> Option<usize> {
    let rows = usize::try_from(rows).ok()?;
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

fn is_duplicate_payload(
    data: &[u8],
    payload: &ResourceRecord,
    compact_size: Option<usize>,
    mode: u8,
    width: usize,
    rows: u64,
) -> bool {
    let expected_text = usize::try_from(rows)
        .ok()
        .and_then(|rows| width.checked_mul(rows));
    let expected = compact_size.filter(|_| mode == 5).or(expected_text);
    let Some(expected) = expected else {
        return false;
    };
    if payload.length != expected.saturating_mul(2) {
        return false;
    }
    let Ok(bytes) = checked_range(data, payload.offset, payload.length, "duplicate payload") else {
        return false;
    };
    bytes[..expected] == bytes[expected..]
}

fn u16_le(data: &[u8], offset: usize, context: &'static str) -> Result<u16, ResourceLayoutError> {
    Ok(u16::from_le_bytes(
        checked_range(data, offset, 2, context)?
            .try_into()
            .expect("fixed two-byte slice"),
    ))
}
fn u32_le(data: &[u8], offset: usize, context: &'static str) -> Result<u32, ResourceLayoutError> {
    Ok(u32::from_le_bytes(
        checked_range(data, offset, 4, context)?
            .try_into()
            .expect("fixed four-byte slice"),
    ))
}
fn u48_le(data: &[u8], offset: usize, context: &'static str) -> Result<u64, ResourceLayoutError> {
    let bytes = checked_range(data, offset, 6, context)?;
    Ok(u64::from(bytes[0])
        | (u64::from(bytes[1]) << 8)
        | (u64::from(bytes[2]) << 16)
        | (u64::from(bytes[3]) << 24)
        | (u64::from(bytes[4]) << 32)
        | (u64::from(bytes[5]) << 40))
}
fn checked_range<'a>(
    data: &'a [u8],
    start: usize,
    length: usize,
    context: &'static str,
) -> Result<&'a [u8], ResourceLayoutError> {
    let end = start
        .checked_add(length)
        .ok_or(ResourceLayoutError::OffsetOverflow { context })?;
    data.get(start..end)
        .ok_or(ResourceLayoutError::OutOfBounds {
            context,
            start,
            end,
            length: data.len(),
        })
}
fn decode_name(entry: &[u8]) -> Result<String, ()> {
    let bytes = entry.get(63..127).ok_or(())?;
    let words = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&words).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ddi::parse_ddi;
    use std::{fs, path::PathBuf};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    #[test]
    fn parses_resource_fixture_ranges_and_format_codes() {
        let metadata = parse_ddi(fixture("synthetic/resource-index.ddi.xml")).unwrap();
        let data = fs::read(fixture("synthetic/resource-index.Nesstar")).unwrap();
        let layout = discover_resource_layout(&data, &metadata.blocks[0]).unwrap();
        assert_eq!(layout.row_count, 5);
        assert_eq!(layout.columns.len(), 10);
        assert_eq!(
            layout
                .columns
                .iter()
                .map(|column| column.variable.name.as_str())
                .collect::<Vec<_>>(),
            [
                "ASCII", "UTF8", "NIBBLE", "U8", "U16", "U24", "U32", "U40", "CDOUBLE", "RAWBYTE"
            ]
        );
        assert_eq!(
            layout
                .columns
                .iter()
                .map(|column| column.resource.value_format_code)
                .collect::<Vec<_>>(),
            [0, 0, 2, 3, 4, 5, 6, 7, 10, 0]
        );
        assert_eq!(layout.columns[2].resource.value_offset_i64, -1);
        assert_eq!(layout.columns[3].resource.value_offset_i64, 100);
        assert!(
            layout
                .columns
                .iter()
                .all(
                    |column| column.resource.payload_offset + column.resource.payload_length
                        <= data.len()
                )
        );
    }

    #[test]
    fn rejects_truncated_resource_index() {
        let data = fs::read(fixture("malformed/truncated-resource.Nesstar")).unwrap();
        assert!(matches!(
            parse_resource_index(&data),
            Err(ResourceLayoutError::OutOfBounds {
                context: "record count",
                ..
            })
        ));
    }

    #[test]
    fn duplicate_payload_requires_matching_halves() {
        let record = ResourceRecord {
            record_id: 1,
            offset: 0,
            length: 4,
        };
        assert!(is_duplicate_payload(b"abab", &record, Some(2), 5, 0, 4));
        assert!(!is_duplicate_payload(b"abcd", &record, Some(2), 5, 0, 4));
    }
}
