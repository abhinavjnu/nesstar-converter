//! Stata DTA v118 writer (Stata 14+).
//!
//! Writes the binary DTA format described at:
//! <https://www.stata.com/help.cgi?dta>
//! The implementation is write-only and covers the minimal subset needed to
//! produce a file that Stata and pandas can read.

use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{
    NesstarError,
    decode::RecordBatch,
    model::{CellValue, DeclaredType, VariableDefinition},
};

/// Maximum length of a Stata variable name (129 bytes in v118).
const STATA_NAME_LEN: usize = 129;
/// Maximum length of a Stata variable label (321 bytes in v118).
const STATA_LABEL_LEN: usize = 321;
/// Maximum length of a string variable (2045 bytes in v118).
const STATA_STR_MAX: usize = 2045;

/// Stata system-missing value for numeric variables.
const STATA_SYSMIS: f64 = f64::from_bits(0x7fe0_0000_0000_0000);

/// DTA type codes (v118).
#[derive(Clone, Copy)]
enum DtaType {
    /// 8-byte double
    Double,
    /// fixed-width string of `n` bytes (type code = 32768 - n, for n <= 2045)
    Str(u16),
}

impl DtaType {
    fn from_ddi(var: &VariableDefinition) -> Self {
        match var.declared_type {
            DeclaredType::Numeric | DeclaredType::Other(_) => Self::Double,
            DeclaredType::Character => {
                // use declared DDI width, capped at STATA_STR_MAX
                let width = (var.ddi_width as usize).clamp(1, STATA_STR_MAX);
                Self::Str(width as u16)
            }
        }
    }

    /// DTA v118 type code written to the typelist.
    fn code(self) -> u16 {
        match self {
            Self::Double => 65526,
            Self::Str(n) => n,
        }
    }
}

pub struct DtaOutput<W: Write + Seek = BufWriter<File>> {
    writer: W,
    types: Vec<DtaType>,
    nvar: u32,
    nobs: u64,
    /// Accumulated rows (as raw observation bytes).
    rows: Vec<u8>,
}

fn truncate_pad_utf8(s: &str, max_bytes: usize) -> Vec<u8> {
    let mut bytes = s.as_bytes().to_vec();
    bytes.truncate(max_bytes);
    bytes.resize(max_bytes, 0);
    bytes
}

impl DtaOutput<BufWriter<File>> {
    pub fn create(
        path: &Path,
        headers: &[String],
        variables: &[VariableDefinition],
    ) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|e| {
            NesstarError::Unsupported(format!("cannot create DTA {}: {e}", path.display()))
        })?;
        Self::from_writer(BufWriter::new(file), headers, variables)
    }
}

impl<W: Write + Seek> DtaOutput<W> {
    pub fn from_writer(
        writer: W,
        headers: &[String],
        variables: &[VariableDefinition],
    ) -> Result<Self, NesstarError> {
        let types: Vec<DtaType> = variables.iter().map(DtaType::from_ddi).collect();
        let nvar = headers.len() as u32;
        Ok(Self {
            writer,
            types,
            nvar,
            nobs: 0,
            rows: Vec::new(),
        })
    }

    pub fn write_batch(
        &mut self,
        batch: &RecordBatch,
        _variables: &[VariableDefinition],
    ) -> Result<(), String> {
        for row in 0..batch.row_count {
            self.nobs += 1;
            for (col_idx, column) in batch.columns.iter().enumerate() {
                let dtype = self.types[col_idx];
                let cell = &column.values[row];
                match dtype {
                    DtaType::Double => {
                        let val: f64 = match cell {
                            CellValue::Missing => STATA_SYSMIS,
                            CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(STATA_SYSMIS),
                        };
                        self.rows.extend_from_slice(&val.to_le_bytes());
                    }
                    DtaType::Str(width) => {
                        let s = match cell {
                            CellValue::Missing => "",
                            CellValue::Text(s) => s.as_str(),
                        };
                        let bytes = truncate_pad_utf8(s, width as usize);
                        self.rows.extend_from_slice(&bytes);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        _headers: &[String],
        variables: &[VariableDefinition],
    ) -> Result<W, String> {
        let w = &mut self.writer;

        macro_rules! tag {
            ($t:expr) => {
                w.write_all($t.as_bytes()).map_err(|e| e.to_string())?
            };
        }
        macro_rules! u8v {
            ($v:expr) => {
                w.write_all(&[$v]).map_err(|e| e.to_string())?
            };
        }
        macro_rules! u16le {
            ($v:expr) => {
                w.write_all(&($v as u16).to_le_bytes())
                    .map_err(|e| e.to_string())?
            };
        }
        macro_rules! u64le {
            ($v:expr) => {
                w.write_all(&($v as u64).to_le_bytes())
                    .map_err(|e| e.to_string())?
            };
        }
        macro_rules! i8le {
            ($v:expr) => {
                w.write_all(&($v as i8).to_le_bytes())
                    .map_err(|e| e.to_string())?
            };
        }

        // --- Header -----------------------------------------------------------
        tag!("<stata_dta><header><release>118</release><byteorder>LSF</byteorder>");
        tag!("<K>");
        u16le!(self.nvar);
        tag!("</K><N>");
        u64le!(self.nobs);
        tag!("</N><label>");
        let ds_label_bytes = truncate_pad_utf8("Nesstar Export", 80);
        u8v!(ds_label_bytes.len().min(80) as u8);
        w.write_all(&ds_label_bytes[..ds_label_bytes.len().min(80)])
            .map_err(|e| e.to_string())?;
        tag!("</label><timestamp>");
        let ts_str = "01 Jan 2024 00:00";
        u8v!(ts_str.len() as u8);
        tag!(ts_str);
        tag!("</timestamp></header>");

        // --- <map> (placeholder offsets, patched at the end) -----------------
        let map_pos = w.stream_position().map_err(|e| e.to_string())?;
        tag!("<map>");
        let mut map_offsets = [0u64; 14];
        for _ in 0..14 {
            u64le!(0u64);
        }
        tag!("</map>");

        macro_rules! record_pos {
            ($idx:expr) => {
                map_offsets[$idx] = w.stream_position().map_err(|e| e.to_string())?;
            };
        }

        // --- <variable_types> ------------------------------------------------
        record_pos!(1);
        tag!("<variable_types>");
        for dtype in &self.types {
            u16le!(dtype.code());
        }
        tag!("</variable_types>");

        // --- <varnames> (129 bytes each, null-terminated) --------------------
        record_pos!(2);
        tag!("<varnames>");
        for var in variables {
            let mut buf = vec![0u8; STATA_NAME_LEN];
            let name_bytes = var.name.as_bytes();
            let len = name_bytes.len().min(STATA_NAME_LEN - 1);
            buf[..len].copy_from_slice(&name_bytes[..len]);
            w.write_all(&buf).map_err(|e| e.to_string())?;
        }
        tag!("</varnames>");

        // --- <sortlist> (2 bytes per var * (K+1), all 0 = unsorted) ----------
        record_pos!(3);
        tag!("<sortlist>");
        for _ in 0..=(self.nvar as usize) {
            u16le!(0u16);
        }
        tag!("</sortlist>");

        // --- <formats> (57 bytes each) ---------------------------------------
        record_pos!(4);
        tag!("<formats>");
        for (i, var) in variables.iter().enumerate() {
            let mut buf = vec![0u8; 57];
            let fmt_bytes = match self.types[i] {
                DtaType::Double => b"%9.0g".as_slice(),
                DtaType::Str(_) => {
                    let w_str = format!("%{}s", var.ddi_width.clamp(1, STATA_STR_MAX as u32));
                    let len = w_str.len().min(56);
                    buf[..len].copy_from_slice(&w_str.as_bytes()[..len]);
                    w.write_all(&buf).map_err(|e| e.to_string())?;
                    continue;
                }
            };
            let len = fmt_bytes.len().min(56);
            buf[..len].copy_from_slice(&fmt_bytes[..len]);
            w.write_all(&buf).map_err(|e| e.to_string())?;
        }
        tag!("</formats>");

        // --- <value_label_names> (33 bytes each, blank = no labels) ----------
        record_pos!(6);
        tag!("<value_label_names>");
        let blank_name = vec![0u8; STATA_NAME_LEN];
        for _ in 0..self.nvar {
            w.write_all(&blank_name).map_err(|e| e.to_string())?;
        }
        tag!("</value_label_names>");

        // --- <variable_labels> (81 bytes each) --------------------------------
        record_pos!(7);
        tag!("<variable_labels>");
        for var in variables {
            let mut buf = vec![0u8; STATA_LABEL_LEN];
            let src = var.label.as_bytes();
            let len = src.len().min(STATA_LABEL_LEN - 1);
            buf[..len].copy_from_slice(&src[..len]);
            w.write_all(&buf).map_err(|e| e.to_string())?;
        }
        tag!("</variable_labels>");

        // --- <characteristics> (empty) ----------------------------------------
        record_pos!(8);
        tag!("<characteristics></characteristics>");

        // --- <data> -----------------------------------------------------------
        record_pos!(9);
        tag!("<data>");
        w.write_all(&self.rows).map_err(|e| e.to_string())?;
        tag!("</data>");

        // --- <strls> (empty) --------------------------------------------------
        record_pos!(10);
        tag!("<strls></strls>");

        // --- <value_labels> (empty) -------------------------------------------
        record_pos!(11);
        tag!("<value_labels></value_labels>");

        // --- end --------------------------------------------------------------
        record_pos!(12);
        tag!("</stata_dta>");

        w.flush().map_err(|e| e.to_string())?;

        // Patch the map offsets
        w.seek(SeekFrom::Start(map_pos + 5))
            .map_err(|e| e.to_string())?;
        for val in &map_offsets {
            w.write_all(&val.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        w.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;

        w.flush().map_err(|e| e.to_string())?;

        // suppress unused-variable warnings
        i8le!(0u8);

        Ok(self.writer)
    }
}
