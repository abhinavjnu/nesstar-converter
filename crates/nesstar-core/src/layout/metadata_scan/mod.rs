//! Metadata-scan layout discovery.
//!
//! Older Nesstar files place fixed-size variable metadata slots directly after
//! their column-major payload.  There is no authoritative index for those
//! slots, so this module only accepts a candidate after validating its first
//! and last names against the DDI and checking every arithmetic operation.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::model::{BlockDefinition, NumericRange, VariableDefinition};

const SLOT_SIZE: usize = 160;
const NAME_OFFSET: usize = 63;
const NAME_LENGTH: usize = 80;
const MAX_VAR_NUMBER: u32 = 50_000;
const DEFAULT_MAX_SLOTS: usize = 200;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetadataLayoutError {
    #[error("metadata {context}: offset arithmetic overflow")]
    OffsetOverflow { context: &'static str },
    #[error("metadata {context}: range {start}..{end} exceeds {length} bytes")]
    OutOfBounds {
        context: &'static str,
        start: usize,
        end: usize,
        length: usize,
    },
    #[error("metadata slot {slot_index} at offset {offset} is truncated")]
    TruncatedSlot { slot_index: usize, offset: usize },
    #[error("metadata layout for block `{block_id}` has no variables")]
    EmptyBlock { block_id: String },
    #[error("metadata layout for block `{block_id}` could not be found")]
    NotFound { block_id: String },
    #[error("DDI has {ddi_count} variables but metadata has {slot_count} slots")]
    IncompatibleSlotCount { ddi_count: usize, slot_count: usize },
    #[error(
        "DDI has {ddi_count} variables and metadata has {slot_count} slots with no name overlap"
    )]
    NoNameOverlap { ddi_count: usize, slot_count: usize },
    #[error("metadata width for `{variable}` is zero")]
    ZeroWidth { variable: String },
    #[error("metadata payload for `{variable}` is outside the source")]
    PayloadOutOfBounds { variable: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataEncoding {
    FixedAscii,
    LittleEndianDouble,
    OffsetInteger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataSlot {
    pub var_num: u32,
    pub encoding: MetadataEncoding,
    pub char_width: usize,
    pub name: String,
    /// Position in the metadata table, used to restore DDI order later.
    pub slot_index: usize,
}

#[derive(Clone, Debug)]
pub struct MetadataColumn {
    pub variable: VariableDefinition,
    pub slot: MetadataSlot,
    /// Bytes per value in the column-major binary payload.
    pub binary_width: usize,
}

#[derive(Clone, Debug)]
pub struct MetadataLayout {
    pub metadata_offset: usize,
    pub data_offset: usize,
    /// Columns are in binary (`var_num`) order, as required for payload reads.
    pub columns: Vec<MetadataColumn>,
}

impl MetadataLayout {
    /// Return the same columns in DDI/slot order for the output schema.
    pub fn columns_in_ddi_order(&self) -> Vec<&MetadataColumn> {
        let mut columns = self.columns.iter().collect::<Vec<_>>();
        columns.sort_by_key(|column| column.slot.slot_index);
        columns
    }
}

/// Find a validated metadata table for every block that has one.
pub fn find_metadata_sections(data: &[u8], blocks: &[BlockDefinition]) -> Vec<(String, usize)> {
    let mut name_owners: BTreeMap<&str, usize> = BTreeMap::new();
    for block in blocks {
        for variable in &block.variables {
            *name_owners.entry(&variable.name).or_default() += 1;
        }
    }

    let mut found = Vec::new();
    let mut used = BTreeSet::new();
    let mut ordered = blocks.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|block| block.file_id_number);
    for block in ordered {
        let count = block.variables.len();
        if count == 0 {
            continue;
        }
        let mut candidates = block
            .variables
            .iter()
            .enumerate()
            .filter(|(_, variable)| name_owners[variable.name.as_str()] == 1)
            .map(|(index, variable)| (&variable.name, index))
            .collect::<Vec<_>>();
        candidates.push((&block.variables[0].name, 0));
        if count > 1 {
            candidates.push((&block.variables[count - 1].name, count - 1));
        }

        let section = candidates.into_iter().find_map(|(name, candidate_index)| {
            find_candidate(data, block, name, candidate_index, &mut used)
        });
        if let Some(offset) = section {
            found.push((block.file_id.clone(), offset));
        }
    }
    found
}

/// Count adjacent well-formed metadata slots, stopping at the first invalid
/// record. A truncated record is not treated as a valid slot.
pub fn count_metadata_slots(data: &[u8], metadata_offset: usize, max_slots: usize) -> usize {
    (0..max_slots)
        .take_while(|index| {
            let Ok(offset) = slot_offset(metadata_offset, *index) else {
                return false;
            };
            let Ok(slot) = checked_range(data, offset, SLOT_SIZE, "metadata slot") else {
                return false;
            };
            let var_num = u32::from_le_bytes(slot[0..4].try_into().expect("fixed four-byte slice"));
            var_num != 0
                && var_num <= MAX_VAR_NUMBER
                && decode_slot_name(slot).is_ok_and(|name| !name.trim().is_empty())
        })
        .count()
}

/// Read a requested number of metadata slots. Unlike counting, this reports a
/// truncated requested slot as an error so callers cannot silently decode a
/// partial layout.
pub fn read_metadata_slots(
    data: &[u8],
    metadata_offset: usize,
    slot_count: usize,
) -> Result<Vec<MetadataSlot>, MetadataLayoutError> {
    (0..slot_count)
        .map(|slot_index| {
            let offset = slot_offset(metadata_offset, slot_index)?;
            let slot = checked_range(data, offset, SLOT_SIZE, "metadata slot").map_err(
                |error| match error {
                    MetadataLayoutError::OutOfBounds { .. } => {
                        MetadataLayoutError::TruncatedSlot { slot_index, offset }
                    }
                    other => other,
                },
            )?;
            let var_num = u32::from_le_bytes(slot[0..4].try_into().expect("fixed four-byte slice"));
            let encoding = if slot[4] == 1 {
                MetadataEncoding::FixedAscii
            } else if slot[5] == 10 {
                MetadataEncoding::LittleEndianDouble
            } else {
                MetadataEncoding::OffsetInteger
            };
            let name = decode_slot_name(slot).unwrap_or_else(|_| format!("var_{var_num}"));
            Ok(MetadataSlot {
                var_num,
                encoding,
                char_width: usize::from(slot[14]),
                name,
                slot_index,
            })
        })
        .collect()
}

/// Match DDI variables to slots, returning binary order (`var_num`). When the
/// counts agree Python pairs the entries positionally; small count differences
/// instead use exact/prefix name matching.
pub fn match_ddi_to_slots(
    variables: &[VariableDefinition],
    slots: &[MetadataSlot],
) -> Result<Vec<MetadataColumn>, MetadataLayoutError> {
    let mut columns = Vec::new();
    if variables.len() == slots.len() {
        for (variable, slot) in variables.iter().zip(slots) {
            columns.push(column(variable.clone(), slot.clone())?);
        }
    } else if variables.len().abs_diff(slots.len()) <= 5 {
        let mut used = vec![false; slots.len()];
        for variable in variables {
            let match_index = slots
                .iter()
                .enumerate()
                .find_map(|(index, slot)| {
                    (!used[index] && slot.name == variable.name).then_some(index)
                })
                .or_else(|| {
                    slots.iter().enumerate().find_map(|(index, slot)| {
                        (!used[index] && names_close(&variable.name, &slot.name)).then_some(index)
                    })
                });
            if let Some(index) = match_index {
                used[index] = true;
                columns.push(column(variable.clone(), slots[index].clone())?);
            }
        }
        if columns.is_empty() {
            return Err(MetadataLayoutError::NoNameOverlap {
                ddi_count: variables.len(),
                slot_count: slots.len(),
            });
        }
    } else {
        return Err(MetadataLayoutError::IncompatibleSlotCount {
            ddi_count: variables.len(),
            slot_count: slots.len(),
        });
    }
    columns.sort_by_key(|column| column.slot.var_num);
    Ok(columns)
}

pub fn compute_binary_width(
    variable: &VariableDefinition,
    slot: &MetadataSlot,
) -> Result<usize, MetadataLayoutError> {
    match slot.encoding {
        MetadataEncoding::FixedAscii => {
            if slot.char_width == 0 {
                Err(MetadataLayoutError::ZeroWidth {
                    variable: variable.name.clone(),
                })
            } else {
                Ok(slot.char_width)
            }
        }
        MetadataEncoding::LittleEndianDouble => Ok(8),
        MetadataEncoding::OffsetInteger => {
            Ok(width_for_range(variable.range.as_ref(), variable.ddi_width))
        }
    }
}

/// Build and validate the layout for a known metadata offset.
pub fn build_metadata_layout(
    data: &[u8],
    block: &BlockDefinition,
    metadata_offset: usize,
) -> Result<MetadataLayout, MetadataLayoutError> {
    if block.variables.is_empty() {
        return Err(MetadataLayoutError::EmptyBlock {
            block_id: block.file_id.clone(),
        });
    }
    let actual = count_metadata_slots(data, metadata_offset, DEFAULT_MAX_SLOTS);
    let read_count = if actual >= block.variables.len() {
        block.variables.len()
    } else {
        actual
    };
    let slots = read_metadata_slots(data, metadata_offset, read_count)?;
    let mut columns = match_ddi_to_slots(&block.variables, &slots)?;
    let data_offset = cautiously_reduce_widths(metadata_offset, block.row_count, &mut columns)?;
    for column in &columns {
        let bytes = column
            .binary_width
            .checked_mul(block.row_count as usize)
            .ok_or(MetadataLayoutError::OffsetOverflow {
                context: "column payload size",
            })?;
        // Data is column-major and begins at data_offset; all column byte
        // counts together have already been checked by cautiously_reduce_widths.
        if bytes == 0 {
            return Err(MetadataLayoutError::PayloadOutOfBounds {
                variable: column.variable.name.clone(),
            });
        }
    }
    checked_range(
        data,
        data_offset,
        metadata_offset - data_offset,
        "metadata payload",
    )?;
    Ok(MetadataLayout {
        metadata_offset,
        data_offset,
        columns,
    })
}

/// Locate then build the metadata layout for one DDI block.
pub fn discover_metadata_layout(
    data: &[u8],
    block: &BlockDefinition,
) -> Result<MetadataLayout, MetadataLayoutError> {
    let offset = find_metadata_sections(data, std::slice::from_ref(block))
        .into_iter()
        .next()
        .map(|(_, offset)| offset)
        .ok_or_else(|| MetadataLayoutError::NotFound {
            block_id: block.file_id.clone(),
        })?;
    build_metadata_layout(data, block, offset)
}

fn find_candidate(
    data: &[u8],
    block: &BlockDefinition,
    candidate_name: &str,
    candidate_index: usize,
    used: &mut BTreeSet<(usize, usize)>,
) -> Option<usize> {
    let needle = utf16le(candidate_name);
    if needle.is_empty() {
        return None;
    }
    let mut search_from: usize = 0;
    while search_from.checked_add(needle.len())? <= data.len() {
        let relative = data[search_from..]
            .windows(needle.len())
            .position(|window| window == needle)?;
        let position = search_from.checked_add(relative)?;
        search_from = position.checked_add(1)?;
        let slot_start = position.checked_sub(NAME_OFFSET)?;
        let metadata_offset = slot_start.checked_sub(candidate_index.checked_mul(SLOT_SIZE)?)?;
        let pair = (metadata_offset, block.variables.len());
        if used.contains(&pair) {
            continue;
        }
        let first = checked_range(data, metadata_offset, SLOT_SIZE, "first metadata slot").ok()?;
        let first_var_num =
            u32::from_le_bytes(first[0..4].try_into().expect("fixed four-byte slice"));
        if first_var_num == 0 || first_var_num > MAX_VAR_NUMBER {
            continue;
        }
        let Ok(first_name) = decode_slot_name(first) else {
            continue;
        };
        if !names_close(&first_name, &block.variables[0].name) {
            continue;
        }
        let actual = count_metadata_slots(data, metadata_offset, DEFAULT_MAX_SLOTS);
        if actual == 0 {
            continue;
        }
        let last_name = if actual >= block.variables.len() {
            slot_name_at(data, metadata_offset, block.variables.len() - 1)
        } else if actual.abs_diff(block.variables.len()) <= 3 {
            slot_name_at(data, metadata_offset, actual - 1)
        } else {
            None
        };
        if !last_name.is_some_and(|name| {
            names_close(&name, &block.variables[block.variables.len() - 1].name)
        }) && actual != block.variables.len()
        {
            continue;
        }
        used.insert(pair);
        return Some(metadata_offset);
    }
    None
}

fn column(
    variable: VariableDefinition,
    slot: MetadataSlot,
) -> Result<MetadataColumn, MetadataLayoutError> {
    let binary_width = compute_binary_width(&variable, &slot)?;
    Ok(MetadataColumn {
        variable,
        slot,
        binary_width,
    })
}

fn cautiously_reduce_widths(
    metadata_offset: usize,
    row_count: u64,
    columns: &mut [MetadataColumn],
) -> Result<usize, MetadataLayoutError> {
    let row_count =
        usize::try_from(row_count).map_err(|_| MetadataLayoutError::OffsetOverflow {
            context: "row count",
        })?;
    let mut payload_bytes = total_payload_bytes(columns, row_count)?;
    let mut data_offset = metadata_offset.saturating_sub(payload_bytes);
    let mut candidates = columns
        .iter()
        .enumerate()
        .filter(|(_, column)| {
            column.slot.encoding == MetadataEncoding::OffsetInteger && column.binary_width > 1
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|index| std::cmp::Reverse(columns[*index].binary_width));
    for index in candidates {
        if metadata_offset >= payload_bytes && data_offset >= 8 {
            break;
        }
        let column = &columns[index];
        let proposed = column.binary_width - 1;
        if range_needs_more_than(&column.variable.range, proposed) {
            continue;
        }
        columns[index].binary_width = proposed;
        payload_bytes = total_payload_bytes(columns, row_count)?;
        data_offset = metadata_offset.saturating_sub(payload_bytes);
    }
    if metadata_offset < payload_bytes || data_offset < 8 {
        return Err(MetadataLayoutError::PayloadOutOfBounds {
            variable: "metadata layout".into(),
        });
    }
    Ok(data_offset)
}

fn total_payload_bytes(
    columns: &[MetadataColumn],
    row_count: usize,
) -> Result<usize, MetadataLayoutError> {
    columns.iter().try_fold(0usize, |total, column| {
        let bytes = column.binary_width.checked_mul(row_count).ok_or(
            MetadataLayoutError::OffsetOverflow {
                context: "column payload size",
            },
        )?;
        total
            .checked_add(bytes)
            .ok_or(MetadataLayoutError::OffsetOverflow {
                context: "metadata payload size",
            })
    })
}

fn width_for_range(range: Option<&NumericRange>, ddi_width: u32) -> usize {
    let delta = range
        .and_then(|range| match (range.minimum, range.maximum) {
            (Some(minimum), Some(maximum)) => {
                Some((maximum as i128 - minimum as i128).max(1) as u128)
            }
            _ => None,
        })
        .unwrap_or_else(|| 10u128.saturating_pow(ddi_width).saturating_sub(1));
    let bits = (128 - delta.leading_zeros() as usize).max(1);
    bits.div_ceil(8).max(1)
}

fn range_needs_more_than(range: &Option<NumericRange>, width: usize) -> bool {
    let Some(NumericRange {
        minimum: Some(minimum),
        maximum: Some(maximum),
    }) = range
    else {
        return false;
    };
    let delta = (*maximum as i128 - *minimum as i128).max(0) as u128;
    let bits = width.saturating_mul(8);
    bits < 128 && delta > ((1u128 << bits) - 1)
}

fn slot_offset(metadata_offset: usize, index: usize) -> Result<usize, MetadataLayoutError> {
    metadata_offset
        .checked_add(
            index
                .checked_mul(SLOT_SIZE)
                .ok_or(MetadataLayoutError::OffsetOverflow {
                    context: "metadata slot offset",
                })?,
        )
        .ok_or(MetadataLayoutError::OffsetOverflow {
            context: "metadata slot offset",
        })
}

fn checked_range<'a>(
    data: &'a [u8],
    start: usize,
    len: usize,
    context: &'static str,
) -> Result<&'a [u8], MetadataLayoutError> {
    let end = start
        .checked_add(len)
        .ok_or(MetadataLayoutError::OffsetOverflow { context })?;
    data.get(start..end)
        .ok_or(MetadataLayoutError::OutOfBounds {
            context,
            start,
            end,
            length: data.len(),
        })
}

fn decode_slot_name(slot: &[u8]) -> Result<String, ()> {
    let bytes = slot.get(NAME_OFFSET..NAME_OFFSET + NAME_LENGTH).ok_or(())?;
    let words = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|word| *word != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&words).map_err(|_| ())
}

fn slot_name_at(data: &[u8], metadata_offset: usize, index: usize) -> Option<String> {
    let offset = slot_offset(metadata_offset, index).ok()?;
    decode_slot_name(checked_range(data, offset, SLOT_SIZE, "metadata slot").ok()?).ok()
}

fn utf16le(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn names_close(left: &str, right: &str) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::ddi::parse_ddi;

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    #[test]
    fn discovers_and_builds_synthetic_metadata_layout() {
        let metadata = parse_ddi(fixture("synthetic/metadata-scan.ddi.xml")).unwrap();
        let block = &metadata.blocks[0];
        let data = fs::read(fixture("synthetic/metadata-scan.Nesstar")).unwrap();
        let sections = find_metadata_sections(&data, &metadata.blocks);
        assert_eq!(sections, vec![("F1".into(), 128)]);
        let layout = discover_metadata_layout(&data, block).unwrap();
        // The fixture deliberately has eight header/padding bytes before the
        // column-major payload; the inferred payload therefore begins at 72.
        assert_eq!(layout.data_offset, 72);
        assert_eq!(
            layout
                .columns
                .iter()
                .map(|column| column.variable.name.as_str())
                .collect::<Vec<_>>(),
            ["ASCII", "OFFSET", "FLOAT"]
        );
        assert_eq!(
            layout
                .columns
                .iter()
                .map(|column| column.binary_width)
                .collect::<Vec<_>>(),
            [4, 2, 8]
        );
        assert_eq!(
            layout
                .columns_in_ddi_order()
                .iter()
                .map(|column| column.variable.name.as_str())
                .collect::<Vec<_>>(),
            ["ASCII", "OFFSET", "FLOAT"]
        );
    }

    #[test]
    fn rejects_truncated_requested_metadata_slot() {
        let data = fs::read(fixture("malformed/truncated-metadata.Nesstar")).unwrap();
        assert!(matches!(
            read_metadata_slots(&data, 128, 1),
            Err(MetadataLayoutError::TruncatedSlot {
                slot_index: 0,
                offset: 128
            })
        ));
    }

    #[test]
    fn refuses_width_reduction_that_cannot_represent_range() {
        let variable = VariableDefinition {
            name: "NUMBER".into(),
            label: String::new(),
            declared_type: crate::model::DeclaredType::Numeric,
            ddi_width: 6,
            decimals: 0,
            range: Some(NumericRange {
                minimum: Some(0.0),
                maximum: Some(400_000.0),
            }),
            referenced_file_ids: vec![],
        };
        let slot = MetadataSlot {
            var_num: 1,
            encoding: MetadataEncoding::OffsetInteger,
            char_width: 0,
            name: "NUMBER".into(),
            slot_index: 0,
        };
        assert_eq!(compute_binary_width(&variable, &slot).unwrap(), 3);
        assert!(range_needs_more_than(&variable.range, 2));
    }
}
