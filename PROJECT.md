# Project: Nesstar Rust Converter Verification

## Architecture
- `crates/nesstar-core`: Core Rust parser, DDI reader, byte decoding, and formatting library.
- `crates/nesstar-cli`: CLI executable wrapping the core conversion pipeline.
- `nesstar_converter.py`: Reference Python parser and converter serving as correctness oracle.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | WP-B0 | Explore codebase, compile workspace, and run existing unit/pytest suites | None | DONE (Rust check ok, 14 unit tests pass, python pytest ok) |
| 2 | WP-E1 | Run differential testing audit on PLFS 2017-18 dataset, build verification script | WP-B0 | DONE (Rust correct, Python bug found in 3 columns) |
| 3 | WP-E2 | Code audit on `nesstar-core` and `nesstar-cli` for edge cases and correctness | WP-B0 | DONE (Audit clean, reviewer found 2 issues, challenger verified 4 blocks) |
| 4 | WP-E3 | Fix any bugs, discrepancies, or warnings discovered in WP-E1/WP-E2 | WP-E1, WP-E2 | DONE (Float rendering and unchecked addition bugs fixed and tested) |
| 5 | WP-E4 | Final qualification (run all tests, parity validation, benchmarks) | WP-E3 | DONE (All tests pass, binary size 583 KB, parity 100% verified correct) |

## Interface Contracts
- Rust CLI signature: `nesstar-cli convert <input.Nesstar> <ddi.xml> <output.csv>`
- Python CLI signature: `python nesstar_converter.py convert <input.Nesstar> <ddi.xml> <output.csv> --formats csv`

## Code Layout
- `crates/nesstar-core/` - Core library
- `crates/nesstar-cli/` - CLI wrapper
- `nesstar_converter.py` - Reference Python converter
- `tests/` - Python test suite for validation
- `fixtures/` - Test fixtures
