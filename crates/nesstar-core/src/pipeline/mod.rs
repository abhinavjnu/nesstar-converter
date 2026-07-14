//! Streaming conversion orchestration.

use std::{
    fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{
    ddi::parse_ddi,
    decode::{DecodeError, decode_metadata_batches, decode_resource_batches},
    formats::csv::CsvOutput,
    layout::{metadata_scan::discover_metadata_layout, resource_index::discover_resource_layout},
    source::ReadOnlySource,
};

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("conversion failed: {0}")]
    Failed(String),
    #[error("output already exists: {0}")]
    OutputExists(PathBuf),
}

pub fn convert_csv(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    batch_size: usize,
    keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    let output_path = output_path.as_ref();
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi(ddi_path).map_err(|error| PipelineError::Failed(error.to_string()))?;
    let source = ReadOnlySource::open(source_path)
        .map_err(|error| PipelineError::Failed(error.to_string()))?;
    let block = metadata
        .blocks
        .first()
        .ok_or_else(|| PipelineError::Failed("DDI has no blocks".into()))?;
    let partial = partial_path(output_path);
    if let Some(parent) = partial.parent() {
        fs::create_dir_all(parent).map_err(|error| PipelineError::Failed(error.to_string()))?;
    }
    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let headers = layout
            .columns
            .iter()
            .map(|column| column.variable.name.clone())
            .collect::<Vec<_>>();
        let mut output = CsvOutput::create(&partial, &headers)
            .map_err(|error| PipelineError::Failed(error.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, keep_going, |batch| {
            output.write_batch(&batch).map_err(DecodeError::Writer)
        })
        .and_then(|_| output.finish().map_err(DecodeError::Writer))
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|error| PipelineError::Failed(error.to_string()))?;
        let headers = layout
            .columns_in_ddi_order()
            .into_iter()
            .map(|column| column.variable.name.clone())
            .collect::<Vec<_>>();
        let mut output = CsvOutput::create(&partial, &headers)
            .map_err(|error| PipelineError::Failed(error.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, keep_going, |batch| {
            output.write_batch(&batch).map_err(DecodeError::Writer)
        })
        .and_then(|_| output.finish().map_err(DecodeError::Writer))
    };
    match result {
        Ok(()) => fs::rename(&partial, output_path)
            .map_err(|error| PipelineError::Failed(error.to_string())),
        Err(error) => {
            let _ = fs::remove_file(&partial);
            Err(PipelineError::Failed(error.to_string()))
        }
    }
}

fn partial_path(output: &Path) -> PathBuf {
    let mut value = output.as_os_str().to_os_string();
    value.push(".partial");
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name)
    }

    #[test]
    fn converts_resource_fixture_to_csv_without_partial_file() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-{}", std::process::id()));
        let output = directory.join("resource.csv");
        let _ = fs::remove_dir_all(&directory);
        convert_csv(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        let actual = fs::read_to_string(&output).unwrap();
        assert!(actual.starts_with("ASCII,UTF8,NIBBLE"));
        assert!(actual.contains("A,café,0,100"));
        assert!(!PathBuf::from(format!("{}.partial", output.display())).exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
