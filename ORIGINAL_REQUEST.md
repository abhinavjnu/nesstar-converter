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
