//! Streaming conversion orchestration.

use std::{
    fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{
    ddi::parse_ddi_auto,
    decode::{DecodeError, decode_metadata_batches, decode_resource_batches},
    formats::{
        csv::CsvOutput,
        dta::DtaOutput,
        fwf::FixedWidthOutput,
        json::{JsonMode, JsonOutput},
        spss::SpssOutput,
        tsv::TsvOutput,
    },
    layout::{metadata_scan::discover_metadata_layout, resource_index::discover_resource_layout},
    model::VariableDefinition,
    source::ReadOnlySource,
};

/// Supported output formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Csv,
    Tsv,
    Parquet,
    Dta,
    Spss,
    Json,
    Jsonl,
    Fwf,
}

impl OutputFormat {
    /// Infer format from a file extension. Falls back to CSV.
    pub fn from_extension(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("txt") | Some("tsv") => Self::Tsv,
            Some("parquet") => Self::Parquet,
            Some("dta") => Self::Dta,
            Some("sav") => Self::Spss,
            Some("json") => Self::Json,
            Some("jsonl") | Some("ndjson") => Self::Jsonl,
            Some("fwf") => Self::Fwf,
            _ => Self::Csv,
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Tsv => "TSV (Tab-separated)",
            Self::Parquet => "Parquet",
            Self::Dta => "Stata (.dta)",
            Self::Spss => "SPSS (.sav)",
            Self::Json => "JSON (.json)",
            Self::Jsonl => "JSON Lines (.jsonl)",
            Self::Fwf => "Fixed-Width (.fwf)",
        }
    }

    /// Default file extension (without leading dot).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Parquet => "parquet",
            Self::Dta => "dta",
            Self::Spss => "sav",
            Self::Json => "json",
            Self::Jsonl => "jsonl",
            Self::Fwf => "fwf",
        }
    }
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("conversion failed: {0}")]
    Failed(String),
    #[error("output already exists: {0}")]
    OutputExists(PathBuf),
}

fn sanitize_name(name: &str) -> String {
    let mut safe = String::new();
    let mut last_was_under = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            safe.push(c.to_ascii_lowercase());
            last_was_under = false;
        } else if !last_was_under {
            safe.push('_');
            last_was_under = true;
        }
    }
    safe.trim_matches('_').to_string()
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
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|error| PipelineError::Failed(error.to_string()))?;
    let source = ReadOnlySource::open(source_path)
        .map_err(|error| PipelineError::Failed(error.to_string()))?;
    let block = metadata
        .blocks
        .iter()
        .find(|b| {
            let output_stem = output_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            sanitize_name(&b.name) == sanitize_name(output_stem)
        })
        .or_else(|| metadata.blocks.first())
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

/// Convert a Nesstar dataset to any supported format, detected from the output
/// file extension. Supported extensions: `.csv`, `.tsv`, `.parquet`, `.dta`, `.sav`, `.json`, `.jsonl`, `.fwf`.
pub fn convert(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    batch_size: usize,
    keep_going: impl FnMut() -> bool + Clone,
) -> Result<(), PipelineError> {
    let output_path = output_path.as_ref();
    let fmt = OutputFormat::from_extension(output_path);

    match fmt {
        OutputFormat::Csv => {
            convert_csv(source_path, ddi_path, output_path, batch_size, keep_going)
        }
        OutputFormat::Tsv => {
            convert_with_tsv(source_path, ddi_path, output_path, batch_size, keep_going)
        }
        OutputFormat::Dta => {
            convert_with_dta(source_path, ddi_path, output_path, batch_size, keep_going)
        }
        OutputFormat::Spss => {
            convert_with_spss(source_path, ddi_path, output_path, batch_size, keep_going)
        }
        OutputFormat::Json => convert_with_json(
            source_path,
            ddi_path,
            output_path,
            JsonMode::Array,
            batch_size,
            keep_going,
        ),
        OutputFormat::Jsonl => convert_with_json(
            source_path,
            ddi_path,
            output_path,
            JsonMode::Lines,
            batch_size,
            keep_going,
        ),
        OutputFormat::Fwf => {
            convert_with_fwf(source_path, ddi_path, output_path, batch_size, keep_going)
        }
        OutputFormat::Parquet => {
            #[cfg(feature = "parquet")]
            {
                convert_with_parquet(source_path, ddi_path, output_path, batch_size, keep_going)
            }
            #[cfg(not(feature = "parquet"))]
            {
                Err(PipelineError::Failed(
                    "Parquet support not compiled in (enable the `parquet` feature)".into(),
                ))
            }
        }
    }
}

fn convert_with_tsv(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: &Path,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|e| PipelineError::Failed(e.to_string()))?;
    let source =
        ReadOnlySource::open(source_path).map_err(|e| PipelineError::Failed(e.to_string()))?;
    let block = pick_block(&metadata, output_path)?;
    let partial = partial_path(output_path);
    ensure_parent(&partial)?;

    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let headers = col_headers_resource(&layout);
        let mut out = TsvOutput::create(&partial, &headers)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b).map_err(DecodeError::Writer)
        })
        .and_then(|_| out.finish().map_err(DecodeError::Writer))
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        let headers = col_headers_metadata(&layout);
        let mut out = TsvOutput::create(&partial, &headers)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b).map_err(DecodeError::Writer)
        })
        .and_then(|_| out.finish().map_err(DecodeError::Writer))
    };
    finalize(result, &partial, output_path)
}

fn convert_with_dta(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: &Path,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|e| PipelineError::Failed(e.to_string()))?;
    let source =
        ReadOnlySource::open(source_path).map_err(|e| PipelineError::Failed(e.to_string()))?;
    let block = pick_block(&metadata, output_path)?;
    let partial = partial_path(output_path);
    ensure_parent(&partial)?;

    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let headers = col_headers_resource(&layout);
        let vars: Vec<VariableDefinition> =
            layout.columns.iter().map(|c| c.variable.clone()).collect();
        let mut out = DtaOutput::create(&partial, &headers, &vars)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b, &vars).map_err(DecodeError::Writer)
        })
        .and_then(|_| {
            out.finish(&headers, &vars)
                .map(|_| ())
                .map_err(DecodeError::Writer)
        })
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        let ordered = layout.columns_in_ddi_order();
        let headers: Vec<String> = ordered.iter().map(|c| c.variable.name.clone()).collect();
        let vars: Vec<VariableDefinition> = ordered.iter().map(|c| c.variable.clone()).collect();
        let mut out = DtaOutput::create(&partial, &headers, &vars)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b, &vars).map_err(DecodeError::Writer)
        })
        .and_then(|_| {
            out.finish(&headers, &vars)
                .map(|_| ())
                .map_err(DecodeError::Writer)
        })
    };
    finalize(result, &partial, output_path)
}

fn convert_with_spss(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: &Path,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|e| PipelineError::Failed(e.to_string()))?;
    let source =
        ReadOnlySource::open(source_path).map_err(|e| PipelineError::Failed(e.to_string()))?;
    let block = pick_block(&metadata, output_path)?;
    let partial = partial_path(output_path);
    ensure_parent(&partial)?;

    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let vars: Vec<VariableDefinition> =
            layout.columns.iter().map(|c| c.variable.clone()).collect();
        let mut out = SpssOutput::create(&partial, &vars)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b).map_err(DecodeError::Writer)
        })
        .and_then(|_| out.finish(&vars).map(|_| ()).map_err(DecodeError::Writer))
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        let ordered = layout.columns_in_ddi_order();
        let vars: Vec<VariableDefinition> = ordered.iter().map(|c| c.variable.clone()).collect();
        let mut out = SpssOutput::create(&partial, &vars)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b).map_err(DecodeError::Writer)
        })
        .and_then(|_| out.finish(&vars).map(|_| ()).map_err(DecodeError::Writer))
    };
    finalize(result, &partial, output_path)
}

fn convert_with_json(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: &Path,
    mode: JsonMode,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|e| PipelineError::Failed(e.to_string()))?;
    let source =
        ReadOnlySource::open(source_path).map_err(|e| PipelineError::Failed(e.to_string()))?;
    let block = pick_block(&metadata, output_path)?;
    let partial = partial_path(output_path);
    ensure_parent(&partial)?;

    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let headers = col_headers_resource(&layout);
        let mut out = JsonOutput::create(&partial, &headers, mode)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b)
                .map_err(|e| DecodeError::Writer(e.to_string()))
        })
        .and_then(|_| out.finish().map_err(|e| DecodeError::Writer(e.to_string())))
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        let headers = col_headers_metadata(&layout);
        let mut out = JsonOutput::create(&partial, &headers, mode)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b)
                .map_err(|e| DecodeError::Writer(e.to_string()))
        })
        .and_then(|_| out.finish().map_err(|e| DecodeError::Writer(e.to_string())))
    };
    finalize(result, &partial, output_path)
}

fn convert_with_fwf(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: &Path,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|e| PipelineError::Failed(e.to_string()))?;
    let source =
        ReadOnlySource::open(source_path).map_err(|e| PipelineError::Failed(e.to_string()))?;
    let block = pick_block(&metadata, output_path)?;
    let partial = partial_path(output_path);
    ensure_parent(&partial)?;

    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let headers = col_headers_resource(&layout);
        let ddi_widths: Vec<u32> = layout
            .columns
            .iter()
            .map(|c| c.variable.ddi_width)
            .collect();
        let mut out = FixedWidthOutput::create(&partial, &headers, &ddi_widths)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b)
                .map_err(|e| DecodeError::Writer(e.to_string()))
        })
        .and_then(|_| out.finish().map_err(|e| DecodeError::Writer(e.to_string())))
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        let headers = col_headers_metadata(&layout);
        let ddi_widths: Vec<u32> = layout
            .columns_in_ddi_order()
            .into_iter()
            .map(|c| c.variable.ddi_width)
            .collect();
        let mut out = FixedWidthOutput::create(&partial, &headers, &ddi_widths)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b)
                .map_err(|e| DecodeError::Writer(e.to_string()))
        })
        .and_then(|_| out.finish().map_err(|e| DecodeError::Writer(e.to_string())))
    };
    finalize(result, &partial, output_path)
}

#[cfg(feature = "parquet")]
fn convert_with_parquet(
    source_path: impl AsRef<Path>,
    ddi_path: impl AsRef<Path>,
    output_path: &Path,
    batch_size: usize,
    mut keep_going: impl FnMut() -> bool,
) -> Result<(), PipelineError> {
    use crate::formats::parquet::ParquetOutput;
    if output_path.exists() {
        return Err(PipelineError::OutputExists(output_path.into()));
    }
    let metadata = parse_ddi_auto(&ddi_path, &source_path)
        .map_err(|e| PipelineError::Failed(e.to_string()))?;
    let source =
        ReadOnlySource::open(source_path).map_err(|e| PipelineError::Failed(e.to_string()))?;
    let block = pick_block(&metadata, output_path)?;
    let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
        let vars: Vec<VariableDefinition> =
            layout.columns.iter().map(|c| c.variable.clone()).collect();
        let mut out = ParquetOutput::create(output_path, &vars)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_resource_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b).map_err(DecodeError::Writer)
        })
        .and_then(|_| out.finish().map(|_| ()).map_err(DecodeError::Writer))
    } else {
        let layout = discover_metadata_layout(source.bytes(), block)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        let ordered = layout.columns_in_ddi_order();
        let vars: Vec<VariableDefinition> = ordered.iter().map(|c| c.variable.clone()).collect();
        let mut out = ParquetOutput::create(output_path, &vars)
            .map_err(|e| PipelineError::Failed(e.to_string()))?;
        decode_metadata_batches(&source, &layout, batch_size, &mut keep_going, |b| {
            out.write_batch(&b).map_err(DecodeError::Writer)
        })
        .and_then(|_| out.finish().map(|_| ()).map_err(DecodeError::Writer))
    };
    result.map_err(|e| PipelineError::Failed(e.to_string()))
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn pick_block<'m>(
    metadata: &'m crate::model::SurveyMetadata,
    output_path: &Path,
) -> Result<&'m crate::model::BlockDefinition, PipelineError> {
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let stem_clean = sanitize_name(stem);
    let stem_upper = stem.to_ascii_uppercase();

    metadata
        .blocks
        .iter()
        .find(|b| {
            sanitize_name(&b.name) == stem_clean
                || sanitize_name(&b.file_id) == stem_clean
                || stem_upper.contains(&b.file_id.to_ascii_uppercase())
        })
        .or_else(|| metadata.blocks.first())
        .ok_or_else(|| PipelineError::Failed("DDI has no blocks".into()))
}

fn ensure_parent(path: &Path) -> Result<(), PipelineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PipelineError::Failed(e.to_string()))?;
    }
    Ok(())
}

fn col_headers_resource(layout: &crate::layout::resource_index::ResourceLayout) -> Vec<String> {
    layout
        .columns
        .iter()
        .map(|c| c.variable.name.clone())
        .collect()
}

fn col_headers_metadata(layout: &crate::layout::metadata_scan::MetadataLayout) -> Vec<String> {
    layout
        .columns_in_ddi_order()
        .into_iter()
        .map(|c| c.variable.name.clone())
        .collect()
}

fn finalize(
    result: Result<(), DecodeError>,
    partial: &Path,
    output_path: &Path,
) -> Result<(), PipelineError> {
    match result {
        Ok(()) => {
            fs::rename(partial, output_path).map_err(|e| PipelineError::Failed(e.to_string()))
        }
        Err(e) => {
            let _ = fs::remove_file(partial);
            Err(PipelineError::Failed(e.to_string()))
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

    #[test]
    fn converts_resource_fixture_to_tsv() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-tsv-{}", std::process::id()));
        let output = directory.join("resource.txt");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        let actual = fs::read_to_string(&output).unwrap();
        assert!(actual.starts_with("ASCII\tUTF8\tNIBBLE"));
        assert!(actual.contains("A\tcafé\t0\t100"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_resource_fixture_to_json() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-json-{}", std::process::id()));
        let output = directory.join("resource.json");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        let actual = fs::read_to_string(&output).unwrap();
        assert!(actual.starts_with('['));
        assert!(actual.ends_with("]\n"));
        assert!(actual.contains("\"ASCII\":\"A\""));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_resource_fixture_to_jsonl() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-jsonl-{}", std::process::id()));
        let output = directory.join("resource.jsonl");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        let actual = fs::read_to_string(&output).unwrap();
        assert!(actual.contains("{\"ASCII\":\"A\""));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_resource_fixture_to_fwf() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-fwf-{}", std::process::id()));
        let output = directory.join("resource.fwf");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        let actual = fs::read_to_string(&output).unwrap();
        assert!(actual.starts_with("ASCII"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_resource_fixture_to_parquet() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-pq-{}", std::process::id()));
        let output = directory.join("resource.parquet");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        assert!(output.exists());
        assert!(fs::metadata(&output).unwrap().len() > 100);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_resource_fixture_to_dta() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-dta-{}", std::process::id()));
        let output = directory.join("resource.dta");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        assert!(output.exists());
        let bytes = fs::read(&output).unwrap();
        assert!(bytes.starts_with(b"<stata_dta>"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_resource_fixture_to_spss() {
        let directory =
            std::env::temp_dir().join(format!("nesstar-pipeline-sav-{}", std::process::id()));
        let output = directory.join("resource.sav");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        convert(
            fixture("resource-index.Nesstar"),
            fixture("resource-index.ddi.xml"),
            &output,
            2,
            || true,
        )
        .unwrap();
        assert!(output.exists());
        let bytes = fs::read(&output).unwrap();
        assert!(bytes.starts_with(b"$FL2"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("hh_per_fv_2017-18"), "hh_per_fv_2017_18");
        assert_eq!(sanitize_name("Some (Name) - 1.2"), "some_name_1_2");
        assert_eq!(sanitize_name("___test___"), "test");
        assert_eq!(sanitize_name("abc"), "abc");
    }
}
