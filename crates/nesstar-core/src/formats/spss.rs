//! SPSS SAV writer.
//!
//! Writes uncompressed SPSS system files (.sav) compliant with the SPSS
//! file format specification. The output is readable by IBM SPSS Statistics,
//! PSPP, and pyreadstat / pandas.

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use crate::{
    NesstarError,
    decode::RecordBatch,
    model::{CellValue, DeclaredType, VariableDefinition},
};

/// SPSS system-missing value for numeric variables.
const SPSS_SYSMIS: f64 = -1.797_693_134_862_315_7e308; // LOWEST

/// Maximum variable name length in SPSS (8 ASCII chars).
const SPSS_NAME_LEN: usize = 8;

/// All numeric variables are stored as 8-byte IEEE-754 double.
/// String variables are stored as fixed-width blocks of 8 bytes each.

#[derive(Clone)]
enum SpssType {
    Numeric,
    /// Number of bytes (padded to multiple of 8).
    Str { width: usize, n_segments: usize },
}

impl SpssType {
    fn from_ddi(var: &VariableDefinition) -> Self {
        match var.declared_type {
            DeclaredType::Numeric | DeclaredType::Other(_) => Self::Numeric,
            DeclaredType::Character => {
                let width = (var.ddi_width as usize).clamp(1, 255);
                let n_segments = width.div_ceil(8);
                Self::Str { width, n_segments }
            }
        }
    }

    /// SPSS type code: 0 = numeric, n = string width.
    fn code(&self) -> i32 {
        match self {
            Self::Numeric => 0,
            Self::Str { width, .. } => *width as i32,
        }
    }

    /// Number of 8-byte cells this variable occupies in each observation.
    fn n_cells(&self) -> usize {
        match self {
            Self::Numeric => 1,
            Self::Str { n_segments, .. } => *n_segments,
        }
    }
}

fn truncate_name(name: &str) -> [u8; SPSS_NAME_LEN] {
    let mut buf = [b' '; SPSS_NAME_LEN];
    for (i, b) in name.bytes().take(SPSS_NAME_LEN).enumerate() {
        buf[i] = b.to_ascii_uppercase();
    }
    buf
}

fn pad8(s: &str, total_bytes: usize) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.truncate(total_bytes);
    v.resize(total_bytes, b' ');
    v
}

pub struct SpssOutput<W: Write = BufWriter<File>> {
    writer: W,
    types: Vec<SpssType>,
    nobs: u32,
    rows: Vec<u8>,
}

impl SpssOutput<BufWriter<File>> {
    pub fn create(path: &Path, variables: &[VariableDefinition]) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|e| {
            NesstarError::Unsupported(format!("cannot create SAV {}: {e}", path.display()))
        })?;
        Self::from_writer(BufWriter::new(file), variables)
    }
}

impl<W: Write> SpssOutput<W> {
    pub fn from_writer(writer: W, variables: &[VariableDefinition]) -> Result<Self, NesstarError> {
        let types: Vec<SpssType> = variables.iter().map(SpssType::from_ddi).collect();
        Ok(Self {
            writer,
            types,
            nobs: 0,
            rows: Vec::new(),
        })
    }

    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), String> {
        for row in 0..batch.row_count {
            self.nobs += 1;
            for (col_idx, column) in batch.columns.iter().enumerate() {
                let stype = &self.types[col_idx];
                let cell = &column.values[row];
                match stype {
                    SpssType::Numeric => {
                        let val: f64 = match cell {
                            CellValue::Missing => SPSS_SYSMIS,
                            CellValue::Text(s) => s.trim().parse::<f64>().unwrap_or(SPSS_SYSMIS),
                        };
                        self.rows.extend_from_slice(&val.to_le_bytes());
                    }
                    SpssType::Str { n_segments, .. } => {
                        let s = match cell {
                            CellValue::Missing => "",
                            CellValue::Text(s) => s.as_str(),
                        };
                        let padded = pad8(s, n_segments * 8);
                        self.rows.extend_from_slice(&padded);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        variables: &[VariableDefinition],
    ) -> Result<W, String> {
        let w = &mut self.writer;

        macro_rules! i32le {
            ($v:expr) => {
                w.write_all(&($v as i32).to_le_bytes())
                    .map_err(|e| e.to_string())?
            };
        }
        macro_rules! f64le {
            ($v:expr) => {
                w.write_all(&($v as f64).to_le_bytes())
                    .map_err(|e| e.to_string())?
            };
        }
        macro_rules! w_all {
            ($b:expr) => {
                w.write_all($b).map_err(|e| e.to_string())?
            };
        }

        // Total number of 8-byte slots per observation across all vars
        let nominal_case_size: usize = self.types.iter().map(SpssType::n_cells).sum();

        // ----------------------------------------------------------------
        // Record 1: General Header (176 bytes)
        // ----------------------------------------------------------------
        w_all!(b"$FL2"); // magic: 4 bytes
        let prod = format!("@(#) SPSS DATA FILE NesstarConverter");
        let mut prod_buf = [b' '; 60];
        let p_bytes = prod.as_bytes();
        prod_buf[..p_bytes.len().min(60)].copy_from_slice(&p_bytes[..p_bytes.len().min(60)]);
        w_all!(&prod_buf);

        i32le!(2i32); // layout_code: 2 = normal format
        i32le!(nominal_case_size as i32); // nominal_case_size
        i32le!(0i32); // compressed: 0 = uncompressed
        i32le!(0i32); // weight_index: 0 = unweighted
        i32le!(self.nobs as i32); // n_cases (-1 if unknown, we know nobs)
        f64le!(100.0f64); // bias: compression bias (standard 100.0)

        let creation_date = b"01 Jan 24";
        let creation_time = b"00:00:00";
        w_all!(creation_date);
        w_all!(creation_time);

        let mut flbl = [b' '; 64];
        let ds_bytes = b"Nesstar Export";
        flbl[..ds_bytes.len().min(64)].copy_from_slice(&ds_bytes[..ds_bytes.len().min(64)]);
        w_all!(&flbl);

        // Padding: 3 bytes zeros
        w_all!(&[0u8, 0u8, 0u8]);

        // ----------------------------------------------------------------
        // Record 2: Variable Records (one per variable + continuation records for strings)
        // ----------------------------------------------------------------
        for (i, var) in variables.iter().enumerate() {
            let stype = &self.types[i];

            i32le!(2i32); // rec_type: 2
            i32le!(stype.code()); // type: 0=numeric, >0=string width
            let has_label = !var.label.is_empty();
            i32le!(if has_label { 1i32 } else { 0i32 });
            i32le!(0i32); // n_missing_values: 0
            // print format: type 5 = A (string), type 1 = F (float) — packed as bytes
            let print_fmt: i32 = match stype {
                SpssType::Numeric => (1 << 16) | (8 << 8) | 2, // F8.2
                SpssType::Str { width, .. } => {
                    let w = (*width).min(255) as i32;
                    (5 << 16) | (w << 8) // A<width>
                }
            };
            i32le!(print_fmt);
            i32le!(print_fmt); // write format same as print
            // variable name: 8 bytes
            w_all!(&truncate_name(&var.name));

            // string continuation cells
            if let SpssType::Str { n_segments, .. } = stype {
                if has_label {
                    let lbl_bytes = var.label.as_bytes();
                    let lbl_len = lbl_bytes.len().min(120);
                    i32le!(lbl_len as i32);
                    let pad_len = (4 - (lbl_len % 4)) % 4;
                    w_all!(&lbl_bytes[..lbl_len]);
                    w_all!(&vec![0u8; pad_len]);
                }

                for _ in 1..*n_segments {
                    i32le!(2i32);
                    i32le!(-1i32); // type -1: string continuation
                    i32le!(0i32);
                    i32le!(0i32);
                    i32le!(0i32);
                    i32le!(0i32);
                    w_all!(&[b' '; 8]);
                }
            } else if has_label {
                let lbl_bytes = var.label.as_bytes();
                let lbl_len = lbl_bytes.len().min(120);
                i32le!(lbl_len as i32);
                let pad_len = (4 - (lbl_len % 4)) % 4;
                w_all!(&lbl_bytes[..lbl_len]);
                w_all!(&vec![0u8; pad_len]);
            }
        }

        // ----------------------------------------------------------------
        // Record 7 subtype 3: Machine integer info
        // ----------------------------------------------------------------
        i32le!(7i32);
        i32le!(3i32);
        i32le!(4i32);
        i32le!(8i32);
        i32le!(1i32);
        i32le!(2i32);
        i32le!(1i32);
        i32le!(1i32);
        i32le!(65001i32);
        i32le!(0i32);
        i32le!(0i32);
        i32le!(0i32);

        // ----------------------------------------------------------------
        // Record 7 subtype 4: Machine floating-point info
        // ----------------------------------------------------------------
        i32le!(7i32);
        i32le!(4i32);
        i32le!(8i32);
        i32le!(3i32);
        f64le!(SPSS_SYSMIS);
        f64le!(1.7976931348623157e308f64);
        f64le!(-1.7976931348623157e308f64);

        // ----------------------------------------------------------------
        // Record 7 subtype 20: Character encoding (UTF-8)
        // ----------------------------------------------------------------
        {
            let enc = b"UTF-8";
            i32le!(7i32);
            i32le!(20i32);
            i32le!(1i32);
            i32le!(enc.len() as i32);
            w_all!(enc);
        }

        // ----------------------------------------------------------------
        // Record 7 subtype 13: long variable name map
        // ----------------------------------------------------------------
        {
            let mut name_map = String::new();
            for var in variables.iter() {
                if !name_map.is_empty() {
                    name_map.push('\t');
                }
                let short = var.name.to_ascii_uppercase();
                let short = if short.len() > 8 { &short[..8] } else { &short };
                name_map.push_str(short);
                name_map.push('=');
                name_map.push_str(&var.name);
            }
            let map_bytes = name_map.as_bytes();
            i32le!(7i32);
            i32le!(13i32);
            i32le!(1i32);
            i32le!(map_bytes.len() as i32);
            w_all!(map_bytes);
        }

        // ----------------------------------------------------------------
        // Record 999: End of dictionary
        // ----------------------------------------------------------------
        i32le!(999i32);
        i32le!(0i32);

        // ----------------------------------------------------------------
        // Data records (uncompressed)
        // ----------------------------------------------------------------
        w_all!(&self.rows);

        w.flush().map_err(|e| e.to_string())?;
        Ok(self.writer)
    }
}
