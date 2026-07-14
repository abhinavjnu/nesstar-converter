use nesstar_core::{
    ddi::parse_ddi,
    decode::{DecodeError, decode_metadata_batches, decode_resource_batches},
    formats::csv::CsvOutput,
    layout::{metadata_scan::discover_metadata_layout, resource_index::discover_resource_layout},
    source::ReadOnlySource,
};
use std::{
    env, fs,
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("Usage: cargo run --bin convert_all <input.Nesstar> <ddi.xml> <output_dir>");
        return ExitCode::from(2);
    }
    let source_path = &args[0];
    let ddi_path = &args[1];
    let output_dir = Path::new(&args[2]);

    if let Err(err) = fs::create_dir_all(output_dir) {
        eprintln!("Failed to create output directory: {err}");
        return ExitCode::from(1);
    }

    let metadata = match parse_ddi(ddi_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to parse DDI: {e}");
            return ExitCode::from(1);
        }
    };

    let source = match ReadOnlySource::open(source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open source Nesstar: {e}");
            return ExitCode::from(1);
        }
    };

    println!("Total blocks found in DDI: {}", metadata.blocks.len());

    for block in &metadata.blocks {
        let safe_name = block
            .name
            .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
        let output_path = output_dir.join(format!("{}.csv", safe_name));
        println!(
            "Converting block {} ({}) to {}",
            block.file_id,
            block.name,
            output_path.display()
        );

        let result = if let Ok(layout) = discover_resource_layout(source.bytes(), block) {
            println!("  Using resource index layout");
            let headers = layout
                .columns
                .iter()
                .map(|column| column.variable.name.clone())
                .collect::<Vec<_>>();
            let mut output = match CsvOutput::create(&output_path, &headers) {
                Ok(out) => out,
                Err(e) => {
                    eprintln!("    Failed to create CSV output: {e}");
                    continue;
                }
            };
            decode_resource_batches(
                &source,
                &layout,
                10_000,
                || true,
                |batch| output.write_batch(&batch).map_err(DecodeError::Writer),
            )
            .and_then(|_| output.finish().map_err(DecodeError::Writer))
        } else {
            println!("  Using metadata scan layout");
            let layout = match discover_metadata_layout(source.bytes(), block) {
                Ok(lay) => lay,
                Err(e) => {
                    eprintln!("    Failed to discover metadata layout: {e}");
                    continue;
                }
            };
            let headers = layout
                .columns_in_ddi_order()
                .into_iter()
                .map(|column| column.variable.name.clone())
                .collect::<Vec<_>>();
            let mut output = match CsvOutput::create(&output_path, &headers) {
                Ok(out) => out,
                Err(e) => {
                    eprintln!("    Failed to create CSV output: {e}");
                    continue;
                }
            };
            decode_metadata_batches(
                &source,
                &layout,
                10_000,
                || true,
                |batch| output.write_batch(&batch).map_err(DecodeError::Writer),
            )
            .and_then(|_| output.finish().map_err(DecodeError::Writer))
        };

        match result {
            Ok(()) => println!("  Successfully converted block {}", block.name),
            Err(e) => {
                eprintln!("  Failed to convert block {}: {}", block.name, e);
            }
        }
    }

    ExitCode::SUCCESS
}
