use crate::{NesstarError, decode::RecordBatch, model::CellValue};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct FixedWidthOutput {
    writer: BufWriter<File>,
    widths: Vec<usize>,
}
impl FixedWidthOutput {
    pub fn create(
        path: &Path,
        headers: &[String],
        ddi_widths: &[u32],
    ) -> Result<Self, NesstarError> {
        if headers.len() != ddi_widths.len() {
            return Err(NesstarError::Unsupported(
                "FWF header and width count differ".into(),
            ));
        }
        let widths = headers
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
        let file = File::create(path).map_err(|error| {
            NesstarError::Unsupported(format!("cannot create FWF {}: {error}", path.display()))
        })?;
        let mut output = Self {
            writer: BufWriter::new(file),
            widths,
        };
        output.write_line(headers.iter().map(String::as_str))?;
        Ok(output)
    }
    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), NesstarError> {
        for row in 0..batch.row_count {
            self.write_line(
                batch
                    .columns
                    .iter()
                    .map(|column| match &column.values[row] {
                        CellValue::Missing => "",
                        CellValue::Text(value) => value,
                    }),
            )?;
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<(), NesstarError> {
        self.writer.flush().map_err(io_error)
    }
    fn write_line<'a>(
        &mut self,
        values: impl Iterator<Item = &'a str>,
    ) -> Result<(), NesstarError> {
        let mut line = String::new();
        for (value, width) in values.zip(&self.widths) {
            line.push_str(&format!("{value:<width$}", width = *width));
        }
        writeln!(self.writer, "{}", line.trim_end()).map_err(io_error)
    }
}
fn io_error(error: std::io::Error) -> NesstarError {
    NesstarError::Unsupported(format!("FWF write failed: {error}"))
}
