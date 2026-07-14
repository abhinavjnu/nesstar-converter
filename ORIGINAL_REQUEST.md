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
