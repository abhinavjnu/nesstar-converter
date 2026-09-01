use super::csv::DelimitedOutput;
use crate::{NesstarError, decode::RecordBatch};
use std::path::Path;

pub struct TsvOutput(DelimitedOutput);

impl TsvOutput {
    pub fn create(path: &Path, headers: &[String]) -> Result<Self, NesstarError> {
        DelimitedOutput::create(path, headers, b'\t').map(Self)
    }
    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<(), String> {
        self.0.write_batch(batch)
    }
    pub fn finish(self) -> Result<(), String> {
        self.0.finish()
    }
}
