# Original User Request

## Initial Request — 2026-07-14T05:39:40Z

Audit, test, and verify the new Rust-based Nesstar converter (`nesstar-core` and `nesstar-cli`) for functional correctness and output equivalence compared to the reference Python implementation.

Working directory: /Users/abhishekmaurya/nesstar-convertor
Integrity mode: development

## Requirements

### R1. Output Equivalence Validation
Conduct a differential testing audit to verify that the Rust parser produces identical output datasets to the reference Python parser when run on actual MoSPI survey data (specifically the extracted PLFS 2017-18 dataset at `/Users/abhishekmaurya/Documents/MOSPI/plfs_unitdata/14_DDI-IND-CSO-PLFS-2017-18/PLFS_Data_2017-18/DDI-IND-CSO-PLFS-2017-18.Nesstar` with companion XML `DDI-IND-CSO-PLFS-2017-18.xml`).

### R2. Code Integrity & Test Executions
Audit the codebase in `crates/nesstar-core` and `crates/nesstar-cli` for edge-case bugs, resource-indexed format compatibility issues, and parse/handling correctness. Ensure the existing 14 tests and any new tests run successfully.

## Verification Resources
* Reference Python implementation: `nesstar_converter.py`
* Test survey data: `/Users/abhishekmaurya/Documents/MOSPI/plfs_unitdata/14_DDI-IND-CSO-PLFS-2017-18/`

## Acceptance Criteria

### Test Validation
- [ ] All unit/integration tests in the workspace (`cargo test`) pass successfully.
- [ ] A verification script validates cell-by-cell equivalence between Python-converted files and Rust-converted files for the test dataset.

### Code Quality & Correctness
- [ ] Zero build warnings/errors under the standard compiler profiles.
- [ ] Rust parser handles all missing values and data formats compatibly with the Python reference parser.

## Follow-up — 2026-07-14T07:05:34Z

Integrate the Rust-based Nesstar converter (`nesstar-core` and `nesstar-cli`) as the primary conversion engine for the local `mospi_agent` data pipeline and perform a robustness and compatibility audit against multiple local and public datasets.

Working directory: /Users/abhishekmaurya/nesstar-convertor
Integrity mode: demo

## Requirements

### R1. Integrate Rust Converter in `mospi_agent` Pipeline
Modify the local dataset build script `/Users/abhishekmaurya/Documents/MOSPI/build_plfs_agent_ready.py` to use the release-built Rust converter CLI (`/Users/abhishekmaurya/nesstar-convertor/target/release/nesstar-cli`) instead of the Python converter module. 
* Since the Rust CLI outputs CSV, format the conversion step to first write a temporary CSV file via `nesstar-cli`, and then convert that CSV to Parquet using the existing `write_parquet_from_csv` helper function.
* Clean up all temporary CSV files after conversion.

### R2. Robustness Validation on Local MoSPI Datasets
Execute the updated `build_plfs_agent_ready.py` pipeline to convert all available PLFS datasets under `/Users/abhishekmaurya/Documents/MOSPI/plfs_unitdata/`. Validate that:
* The conversion completes without errors or crashes.
* The DuckDB analytics database `/Users/abhishekmaurya/Documents/MOSPI/mospi_agent_data/analytics.duckdb` can be successfully rebuilt using `python3 -m mospi_agent build-index`.
* Re-run query benchmarks to ensure DuckDB answers recipes correctly.

### R3. IHSN/NADA Registry Validation
Identify publicly accessible sample Nesstar datasets or DDI XML metadata files from public NADA/IHSN registry catalogs that do not require credentials. Programmatically download these sample files, parse them using `nesstar-cli`, and verify that the parser successfully handles diverse layouts, enums, formats, and schemas without crashing.

## Acceptance Criteria

### Integration & Execution
- [ ] The `build_plfs_agent_ready.py` script is modified to invoke `nesstar-cli` for conversion and successfully outputs Parquet files.
- [ ] Running the data build pipeline completes cleanly with no errors, producing a fully functioning Parquet database under `plfs_agent_ready/datasets/`.

### Analytics & Queries
- [ ] The DuckDB index is rebuilt successfully (`build-index`).
- [ ] A sample test run query `python3 -m mospi_agent query "neet across years"` prints valid statistics.

### Registry Validation
- [ ] At least two public sample Nesstar datasets from NADA/IHSN are successfully downloaded and parsed by the Rust converter CLI, logging zero crashes.

## Follow-up — 2026-07-14T13:36:30Z

Extend the Rust-based Nesstar converter (`crates/nesstar-core`, `crates/nesstar-cli`, and `crates/nesstar-gui`) to support export to multiple statistical and data formats directly from the pure Rust binaries: CSV, TXT (Tab-separated), Parquet (.parquet), Stata (.dta), and SPSS (.sav).

Working directory: /Users/abhishekmaurya/nesstar-convertor
Integrity mode: demo

## Requirements

### R1. Support Multi-Format Core Output Writers
Add implementation files inside `crates/nesstar-core/src/formats/` for the following output streams:
* **TXT (Tab-separated)**: Reuse the existing `csv` writer configured with `\t` as the field delimiter.
* **Parquet**: Integrate the `parquet` and `arrow` crates with minimal features (to prevent binary bloat). Map DDI `DeclaredType` definitions (`Numeric` -> `Float64`, `Character` -> `Utf8`) and write records as Arrow columns.
* **Stata DTA (v118)**: Write a lightweight, native Stata DTA v118 stream writer. Handle metadata variables (descriptors, types, and labels) and record outputs. Map DDI `Numeric` variables to double-precision values and missing values to Stata's missing system values.
* **SPSS SAV**: Write a lightweight, native SPSS System File (.sav) format stream writer. Map DDI metadata variables (variable labels, value labels, and formats) and write records accordingly.

### R2. Auto-Detect Format in `nesstar-cli`
Modify the CLI application `crates/nesstar-cli` to:
* Use a single generic command: `nesstar-cli convert <input.Nesstar> <ddi.xml> <output_file>`
* Automatically detect the target format based on the `<output_file>` extension: `.csv` (CSV), `.txt` (TXT), `.parquet` (Parquet), `.dta` (Stata), or `.sav` (SPSS).

### R3. Implement Format Selection in Pure Rust GUI
Update the GUI application `crates/nesstar-gui` (built with `eframe` / `egui`) to:
* Provide a clean format selector (e.g., ComboBox or Radio Buttons) containing: CSV, TXT, Parquet, Stata (.dta), and SPSS (.sav).
* Automatically adjust the output save file filter extension based on the selected format.
* Invoke the worker using the updated format-aware conversion pipeline.

## Acceptance Criteria

### Compiling and Binary Size
- [ ] The workspace compiles successfully with `cargo build --release`.
- [ ] The compiled Rust GUI application `target/release/nesstar-gui` remains lightweight (file size under 15 MB).

### Format Parity & Correctness
- [ ] Writing to `.txt` produces valid tab-separated values.
- [ ] Writing to `.parquet` produces a valid Parquet file that can be queried and loaded by DuckDB or Python `pandas.read_parquet`.
- [ ] Writing to `.dta` produces a valid Stata DTA file loadable by Python `pandas.read_stata`.
- [ ] Writing to `.sav` produces a valid SPSS SAV file loadable by Python `pyreadstat`.

### CLI & GUI Integration
- [ ] Running `cargo run --release --bin nesstar-cli -- convert input.Nesstar ddi.xml output.parquet` writes a valid Parquet file.
- [ ] Launching the Rust GUI application displays the format selection UI, and selecting any format executes the conversion successfully to the chosen file path.
