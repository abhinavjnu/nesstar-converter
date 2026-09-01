use crate::{NesstarError, decode::RecordBatch, model::CellValue};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct FixedWidthOutput<W: Write = BufWriter<File>> {
    writer: W,
    widths: Vec<usize>,
}

impl FixedWidthOutput<BufWriter<File>> {
    pub fn create(
        path: &Path,
        headers: &[String],
        ddi_widths: &[u32],
    ) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|error| {
            NesstarError::Unsupported(format!("cannot create FWF {}: {error}", path.display()))
        })?;
        Self::from_writer(BufWriter::new(file), headers, ddi_widths)
    }
}

impl<W: Write> FixedWidthOutput<W> {
    pub fn from_writer(
        mut writer: W,
        headers: &[String],
        ddi_widths: &[u32],
    ) -> Result<Self, NesstarError> {
        if headers.len() != ddi_widths.len() {
            return Err(NesstarError::Unsupported(
                "FWF header and width count differ".into(),
            ));
        }
        let widths: Vec<usize> = headers
            .iter()
            .zip(ddi_widths)
            .map(|(name, width)| {
                usize::try_from(*width)
                    .unwrap_or(usize::MAX)
                    .max(10)
                    .max(name.chars().count() + 1)
                    + 1
            })
            .collect();

        let mut line = String::new();
        for (value, width) in headers.iter().zip(&widths) {
            line.push_str(&format!("{value:<width$}", width = *width));
        }
        writeln!(writer, "{}", line.trim_end()).map_err(io_error)?;

        Ok(Self { writer, widths })
    }

    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), NesstarError> {
        for row in 0..batch.row_count {
            let mut line = String::new();
            for (col, width) in batch.columns.iter().zip(&self.widths) {
                let value = match &col.values[row] {
                    CellValue::Missing => "",
                    CellValue::Text(val) => val.as_str(),
                };
                line.push_str(&format!("{value:<width$}", width = *width));
            }
            writeln!(self.writer, "{}", line.trim_end()).map_err(io_error)?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), NesstarError> {
        self.writer.flush().map_err(io_error)
    }
}

fn io_error(error: std::io::Error) -> NesstarError {
    NesstarError::Unsupported(format!("FWF write failed: {error}"))
}
