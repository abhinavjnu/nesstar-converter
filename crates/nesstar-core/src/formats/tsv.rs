use std::{fs::File, path::Path};

use csv::{Terminator, Writer, WriterBuilder};

use crate::{NesstarError, decode::RecordBatch, model::CellValue};

pub struct TsvOutput {
    writer: Writer<File>,
}

impl TsvOutput {
    pub fn create(path: &Path, headers: &[String]) -> Result<Self, NesstarError> {
        let file = File::create(path).map_err(|error| {
            NesstarError::Unsupported(format!("cannot create TSV {}: {error}", path.display()))
        })?;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .delimiter(b'\t')
            .terminator(Terminator::Any(b'\n'))
            .from_writer(file);
        writer.write_record(headers).map_err(|error| {
            NesstarError::Unsupported(format!("cannot write TSV header: {error}"))
        })?;
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
                            CellValue::Missing => "".to_owned(),
                            CellValue::Text(value) => value.clone(),
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
