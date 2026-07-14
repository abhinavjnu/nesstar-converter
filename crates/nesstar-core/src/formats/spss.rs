//! SPSS System File (.sav) writer.
//!
//! Implements a minimal write-only subset of the SPSS System File format
//! described in the PSPP documentation:
//! <https://www.gnu.org/software/pspp/pspp-dev/html_node/System-File-Format.html>
//!
//! The output can be read by SPSS, pyreadstat, and other compatible tools.

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
const SPSS_SYSMIS: f64 = -1.797693134862315708145274237317e+308; // LOWEST

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
                let width = (var.ddi_width as usize).min(255).max(1);
                let n_segments = (width + 7) / 8;
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

pub struct SpssOutput {
    writer: BufWriter<File>,
    types: Vec<SpssType>,
    nobs: u32,
    rows: Vec<u8>,
}

impl SpssOutput {
    pub fn create(
        path: &Path,
        variables: &[VariableDefinition],
    ) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|e| {
            NesstarError::Unsupported(format!("cannot create SAV {}: {e}", path.display()))
        })?;
        let types: Vec<SpssType> = variables.iter().map(SpssType::from_ddi).collect();
        Ok(Self {
            writer: BufWriter::new(file),
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
                            CellValue::Text(s) => {
                                s.trim().parse::<f64>().unwrap_or(SPSS_SYSMIS)
                            }
                        };
                        self.rows.extend_from_slice(&val.to_le_bytes());
                    }
                    SpssType::Str { width, n_segments } => {
                        let s = match cell {
                            CellValue::Missing => "",
                            CellValue::Text(s) => s.as_str(),
                        };
                        let bytes = pad8(s, n_segments * 8);
                        let _ = width; // width encoded in pad8 via n_segments
                        self.rows.extend_from_slice(&bytes);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn finish(mut self, variables: &[VariableDefinition]) -> Result<(), String> {
        let w = &mut self.writer;

        macro_rules! w_all {
            ($b:expr) => {
                w.write_all($b).map_err(|e| e.to_string())?
            };
        }
        macro_rules! i32le {
            ($v:expr) => {
                w_all!(&($v as i32).to_le_bytes())
            };
        }
        macro_rules! f64le {
            ($v:expr) => {
                w_all!(&($v as f64).to_le_bytes())
            };
        }

        // ----------------------------------------------------------------
        // Record 1: File Header
        // ----------------------------------------------------------------
        // Signature: 4 bytes
        w_all!(b"$FL2");
        // Product name: 60 bytes, space-padded
        let mut prod = b"@(#) SPSS DATA FILE".to_vec();
        prod.resize(60, b' ');
        w_all!(&prod);
        // layout code = 2 (little-endian)
        i32le!(2i32);
        // number of "observation variables" (each string segment counts)
        let obs_vars: i32 = self
            .types
            .iter()
            .map(|t| t.n_cells() as i32)
            .sum();
        i32le!(obs_vars);
        // compression: 0 = none
        i32le!(0i32);
        // weight variable index: 0 = none
        i32le!(0i32);
        // number of cases: -1 = unknown (we'll patch below if needed, but -1 is accepted)
        let n_cases_offset = 60 + 4 + 4 + 4 + 4; // byte offset of this field from start
        let _ = n_cases_offset;
        i32le!(self.nobs as i32);
        // bias: 100.0 for uncompressed, but 0 here
        f64le!(100.0f64);
        // creation date: 9 bytes "01 Jan 01"
        w_all!(b"01 Jan 01");
        // creation time: 8 bytes "00:00:00"
        w_all!(b"00:00:00");
        // file label: 64 bytes, space-padded
        let mut label = vec![b' '; 64];
        label[0] = b'N';
        label[1] = b'e';
        label[2] = b's';
        label[3] = b's';
        label[4] = b't';
        label[5] = b'a';
        label[6] = b'r';
        w_all!(&label);
        // 3 bytes padding
        w_all!(&[0u8, 0, 0]);

        // ----------------------------------------------------------------
        // Record 2: Variable Records (one per cell, including string continuation)
        // ----------------------------------------------------------------
        for (idx, var) in variables.iter().enumerate() {
            let stype = &self.types[idx];

            // First cell for this variable
            i32le!(2i32); // record type 2
            i32le!(stype.code());
            i32le!(0i32); // has_var_label: we'll add labels below
            i32le!(0i32); // n_missing_values: 0
            // print format: type 5 = A (string), type 1 = F (float) — packed as bytes
            let print_fmt: i32 = match stype {
                SpssType::Numeric => (1 << 16) | (8 << 8) | 2, // F8.2
                SpssType::Str { width, .. } => {
                    let w = (*width).min(255) as i32;
                    (5 << 16) | (w << 8) | 0 // A<width>
                }
            };
            i32le!(print_fmt);
            i32le!(print_fmt); // write format same as print
            // variable name: 8 bytes
            w_all!(&truncate_name(&var.name));

            // string continuation cells
            if let SpssType::Str { n_segments, .. } = stype {
                for _ in 1..*n_segments {
                    i32le!(2i32); // record type 2
                    i32le!(-1i32); // type -1 = string continuation
                    i32le!(0i32);
                    i32le!(0i32);
                    i32le!(0i32);
                    i32le!(0i32);
                    w_all!(b"        "); // 8 blank bytes for continuation name
                }
            }
        }

        // ----------------------------------------------------------------
        // Record 3: Value Labels (none — empty)
        // ----------------------------------------------------------------

        // ----------------------------------------------------------------
        // Record 6: Documents (none)
        // ----------------------------------------------------------------

        // ----------------------------------------------------------------
        // Record 7: Machine-specific info (minimal)
        // ----------------------------------------------------------------
        i32le!(7i32); // record type 7
        i32le!(3i32); // subtype 3: machine integer info
        i32le!(4i32); // data element size: 4 bytes
        i32le!(8i32); // 8 elements
        i32le!(20i32); // version major
        i32le!(0i32); // version minor
        i32le!(0i32); // version revision
        i32le!(-1i32); // machine code
        i32le!(1i32); // floating-point representation: 1 = IEEE 754
        i32le!(1i32); // compression code: 1 = bytecode (must match reference)
        i32le!(2i32); // endianness: 2 = little-endian
        i32le!(65001i32); // character code: 65001 = UTF-8

        // ----------------------------------------------------------------
        // Record 7 subtype 4: machine floating-point info
        // ----------------------------------------------------------------
        i32le!(7i32);
        i32le!(4i32); // subtype 4
        i32le!(8i32); // element size: 8
        i32le!(3i32); // 3 elements
        f64le!(SPSS_SYSMIS);           // SYSMIS
        f64le!(f64::MAX);              // HIGHEST
        {
            // LOWEST: one ULP above -f64::MAX (matches SPSS convention)
            let lowest_bits: u64 = (-f64::MAX).to_bits() - 1;
            let lowest = f64::from_bits(lowest_bits);
            f64le!(lowest);
        }

        // ----------------------------------------------------------------
        // Record 7 subtype 13: long variable name map
        // ----------------------------------------------------------------
        {
            // Build "SHORT=long\tSHORT2=long2\t..." mapping
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
            i32le!(13i32); // subtype 13
            i32le!(1i32);  // element size: 1 byte
            i32le!(map_bytes.len() as i32);
            w_all!(map_bytes);
        }

        // ----------------------------------------------------------------
        // Record 999: End of dictionary
        // ----------------------------------------------------------------
        i32le!(999i32);
        i32le!(0i32); // filler

        // ----------------------------------------------------------------
        // Data records (uncompressed)
        // ----------------------------------------------------------------
        w_all!(&self.rows);

        w.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}
