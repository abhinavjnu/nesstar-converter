use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use crate::{NesstarError, decode::RecordBatch, model::CellValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonMode {
    Array,
    Lines,
}

pub struct JsonOutput {
    writer: BufWriter<File>,
    headers: Vec<String>,
    mode: JsonMode,
    first: bool,
}

impl JsonOutput {
    pub fn create(path: &Path, headers: &[String], mode: JsonMode) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|error| {
            NesstarError::Unsupported(format!("cannot create JSON {}: {error}", path.display()))
        })?;
        let mut writer = BufWriter::new(file);
        if mode == JsonMode::Array {
            writer.write_all(b"[").map_err(io_error)?;
        }
        Ok(Self {
            writer,
            headers: headers.to_vec(),
            mode,
            first: true,
        })
    }
    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), NesstarError> {
        if batch.columns.len() != self.headers.len() {
            return Err(NesstarError::Unsupported(
                "JSON batch schema differs from header".into(),
            ));
        }
        for row in 0..batch.row_count {
            if self.mode == JsonMode::Array && !self.first {
                self.writer.write_all(b",").map_err(io_error)?;
            }
            self.first = false;
            self.writer.write_all(b"{").map_err(io_error)?;
            for (index, column) in batch.columns.iter().enumerate() {
                if index != 0 {
                    self.writer.write_all(b",").map_err(io_error)?;
                }
                write_json_string(&mut self.writer, &self.headers[index])?;
                self.writer.write_all(b":").map_err(io_error)?;
                let value = match &column.values[row] {
                    CellValue::Missing => "",
                    CellValue::Text(value) => value,
                };
                write_json_string(&mut self.writer, value)?;
            }
            self.writer.write_all(b"}").map_err(io_error)?;
            if self.mode == JsonMode::Lines {
                self.writer.write_all(b"\n").map_err(io_error)?;
            }
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<(), NesstarError> {
        if self.mode == JsonMode::Array {
            self.writer.write_all(b"]\n").map_err(io_error)?;
        }
        self.writer.flush().map_err(io_error)
    }
}

fn write_json_string(writer: &mut impl Write, value: &str) -> Result<(), NesstarError> {
    writer.write_all(b"\"").map_err(io_error)?;
    for character in value.chars() {
        match character {
            '"' => writer.write_all(b"\\\"").map_err(io_error)?,
            '\\' => writer.write_all(b"\\\\").map_err(io_error)?,
            '\n' => writer.write_all(b"\\n").map_err(io_error)?,
            '\r' => writer.write_all(b"\\r").map_err(io_error)?,
            '\t' => writer.write_all(b"\\t").map_err(io_error)?,
            character if character <= '\u{1f}' => {
                write!(writer, "\\u{:04x}", character as u32).map_err(io_error)?
            }
            character => write!(writer, "{character}").map_err(io_error)?,
        }
    }
    writer.write_all(b"\"").map_err(io_error)
}
fn io_error(error: std::io::Error) -> NesstarError {
    NesstarError::Unsupported(format!("JSON write failed: {error}"))
}
