use crate::{NesstarError, decode::RecordBatch, model::CellValue};
use csv::{Terminator, Writer, WriterBuilder};
use std::{fs::File, path::Path};

pub struct DelimitedOutput {
    writer: Writer<File>,
}

impl DelimitedOutput {
    pub fn create(path: &Path, headers: &[String], delimiter: u8) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|error| {
            NesstarError::Unsupported(format!("cannot create file {}: {error}", path.display()))
        })?;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .delimiter(delimiter)
            .terminator(Terminator::Any(b'\n'))
            .from_writer(file);
        writer
            .write_record(headers)
            .map_err(|error| NesstarError::Unsupported(format!("cannot write header: {error}")))?;
        Ok(Self { writer })
    }

    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), String> {
        for row in 0..batch.row_count {
            self.writer
                .write_record(
                    batch
                        .columns
                        .iter()
                        .map(|column| match &column.values[row] {
                            CellValue::Missing => "",
                            CellValue::Text(value) => value.as_str(),
                        }),
                )
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), String> {
        self.writer.flush().map_err(|error| error.to_string())
    }
}

pub struct CsvOutput(DelimitedOutput);

impl CsvOutput {
    pub fn create(path: &Path, headers: &[String]) -> Result<Self, NesstarError> {
        DelimitedOutput::create(path, headers, b',').map(Self)
    }
    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), String> {
        self.0.write_batch(batch)
    }
    pub fn finish(self) -> Result<(), String> {
        self.0.finish()
    }
}
