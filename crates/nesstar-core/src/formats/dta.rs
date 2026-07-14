//! Stata DTA v118 writer (Stata 14+).
//!
//! Writes the binary DTA format described at:
//! <https://www.stata.com/help.cgi?dta>
//! The implementation is write-only and covers the minimal subset needed to
//! produce a file that Stata and pandas can read.

use std::{
    fs::File,
    io::{BufWriter, Write, Seek, SeekFrom},
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
                let width = (var.ddi_width as usize).min(STATA_STR_MAX).max(1);
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

pub struct DtaOutput {
    writer: BufWriter<File>,
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

impl DtaOutput {
    pub fn create(
        path: &Path,
        headers: &[String],
        variables: &[VariableDefinition],
    ) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|e| {
            NesstarError::Unsupported(format!("cannot create DTA {}: {e}", path.display()))
        })?;
        let types: Vec<DtaType> = variables.iter().map(DtaType::from_ddi).collect();
        let nvar = headers.len() as u32;
        Ok(Self {
            writer: BufWriter::new(file),
            types,
            nvar,
            nobs: 0,
            rows: Vec::new(),
        })
    }

    pub fn write_batch(
        &mut self,
        batch: &RecordBatch,
        variables: &[VariableDefinition],
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
                            CellValue::Text(s) => {
                                s.trim().parse::<f64>().unwrap_or(STATA_SYSMIS)
                            }
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
                let _ = variables; // suppress unused warning
            }
        }
        Ok(())
    }

    /// Finalize and write the full DTA v118 binary file.
    pub fn finish(
        mut self,
        headers: &[String],
        variables: &[VariableDefinition],
    ) -> Result<(), String> {
        let w = &mut self.writer;

        let mut map_offsets = [0u64; 14];
        let map_pos = 158u64;
        map_offsets[1] = map_pos;

        macro_rules! record_pos {
            ($idx:expr) => {
                w.flush().map_err(|e| e.to_string())?;
                map_offsets[$idx] = w.stream_position().map_err(|e| e.to_string())?;
            };
        }

        macro_rules! tag {
            ($s:expr) => {
                w.write_all($s.as_bytes()).map_err(|e| e.to_string())?
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
                w.write_all(&[$v as u8]).map_err(|e| e.to_string())?
            };
        }

        // --- <stata_dta> header ------------------------------------------------
        tag!("<stata_dta>");
        tag!("<header>");
        tag!("<release>118</release>");
        tag!("<byteorder>LSF</byteorder>"); // little-endian
        tag!("<K>");
        u16le!(self.nvar);
        tag!("</K>");
        tag!("<N>");
        u64le!(self.nobs);
        tag!("</N>");
        // dataset label (up to 80 bytes, starts with u16 length)
        tag!("<label>");
        let empty_label = [0u8, 0]; // u16 length = 0, no label string follows
        w.write_all(&empty_label).map_err(|e| e.to_string())?;
        tag!("</label>");
        // timestamp (18 bytes: 0x11 followed by 17 spaces)
        tag!("<timestamp>");
        let ts = b"\x11                 "; // 0x11 prefix + 17 spaces
        w.write_all(ts).map_err(|e| e.to_string())?;
        tag!("</timestamp>");
        tag!("</header>");

        // --- <map> (14 × u64 offsets, finalised at the end) -------------------
        tag!("<map>");
        let map_placeholder = vec![0u8; 14 * 8];
        w.write_all(&map_placeholder).map_err(|e| e.to_string())?;
        tag!("</map>");

        // --- <variable_types> ------------------------------------------------
        record_pos!(2);
        tag!("<variable_types>");
        for dtype in &self.types {
            u16le!(dtype.code());
        }
        tag!("</variable_types>");

        // --- <varnames> (each name = 33 bytes, null-terminated) ---------------
        record_pos!(3);
        tag!("<varnames>");
        for name in headers {
            let mut buf = vec![0u8; STATA_NAME_LEN];
            let src = name.as_bytes();
            let len = src.len().min(STATA_NAME_LEN - 1);
            buf[..len].copy_from_slice(&src[..len]);
            w.write_all(&buf).map_err(|e| e.to_string())?;
        }
        tag!("</varnames>");

        // --- <sortlist> (nvar+1 × u16, all zero = unsorted) ------------------
        record_pos!(4);
        tag!("<sortlist>");
        let sortlist = vec![0u8; (self.nvar as usize + 1) * 2];
        w.write_all(&sortlist).map_err(|e| e.to_string())?;
        tag!("</sortlist>");

        // --- <formats> (57 bytes each) ----------------------------------------
        record_pos!(5);
        tag!("<formats>");
        for dtype in &self.types {
            let mut buf = vec![0u8; 57];
            let fmt: &[u8] = match dtype {
                DtaType::Double => b"%10.0g",
                DtaType::Str(n) => {
                    let s = format!("%{}s", n);
                    let bytes = s.as_bytes();
                    let len = bytes.len().min(56);
                    buf[..len].copy_from_slice(&bytes[..len]);
                    w.write_all(&buf).map_err(|e| e.to_string())?;
                    continue;
                }
            };
            let len = fmt.len().min(56);
            buf[..len].copy_from_slice(&fmt[..len]);
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
        w.seek(SeekFrom::Start(map_pos + 5)).map_err(|e| e.to_string())?;
        for val in &map_offsets {
            w.write_all(&val.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        w.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;

        w.flush().map_err(|e| e.to_string())?;

        // suppress unused-variable warnings
        let _ = i8le!(0u8);

        Ok(())
    }
}
