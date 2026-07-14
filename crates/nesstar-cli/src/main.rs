use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 || args.first().map(String::as_str) != Some("convert") {
        eprintln!(
            "Usage: nesstar-cli convert <input.Nesstar> <ddi.xml> <output>\n\
             Output format is inferred from the extension:\n\
             .csv (CSV)  .txt (Tab-separated)  .parquet (Parquet)  .dta (Stata)  .sav (SPSS)"
        );
        return ExitCode::from(2);
    }
    match nesstar_core::pipeline::convert(&args[1], &args[2], &args[3], 10_000, || true) {
        Ok(()) => {
            println!("Wrote {}", args[3]);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Conversion failed: {error}");
            ExitCode::from(1)
        }
    }
}
