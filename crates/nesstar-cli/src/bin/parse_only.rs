use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        eprintln!("Usage: cargo run --bin parse_only <ddi.xml>");
        return ExitCode::from(2);
    }
    match nesstar_core::ddi::parse_ddi(&args[0]) {
        Ok(metadata) => {
            println!("Successfully parsed DDI XML: {}", args[0]);
            println!("Found {} blocks", metadata.blocks.len());
            for block in metadata.blocks {
                println!(
                    "  Block: ID={}, Name={}, Variables={}",
                    block.file_id,
                    block.name,
                    block.variables.len()
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("DDI Parsing failed: {error}");
            ExitCode::from(1)
        }
    }
}
