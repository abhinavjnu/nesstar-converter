# Project: Nesstar Converter

## Architecture
- `crates/nesstar-core`: Core Rust parser, DDI reader, byte decoding, and formatting library.
- `crates/nesstar-cli`: CLI executable wrapping the core conversion pipeline.
- `crates/nesstar-gui`: Native desktop GUI built with eframe/egui.
- `nesstar_converter.py`: Reference Python parser and converter.
- `gui/`: Legacy PySide6 desktop app (superseded by `nesstar-gui`).

## Milestones
| # | Name | Scope | Status |
|---|------|-------|--------|
| 1 | WP-B0 | Explore codebase, compile workspace, run test suites | DONE |
| 2 | WP-E1 | Differential testing audit on PLFS 2017-18 | DONE |
| 3 | WP-E2 | Code audit on nesstar-core and nesstar-cli | DONE |
| 4 | WP-E3 | Fix bugs discovered in WP-E1/WP-E2 | DONE |
| 5 | WP-E4 | Final qualification — all tests pass, 100% parity | DONE |
| 6 | v1.0.5 | Rust rewrite, native GUI, multi-format output | DONE |
| 7 | v1.0.6 | GUI redesign, Linux build fix | DONE |
| 8 | v1.0.7 | Windows support, .deb packaging, cross-platform CI | DONE |

## Interface Contracts
- Rust CLI: `nesstar-cli convert <input.Nesstar> <ddi.xml> <output.csv|.parquet|.dta|.txt>`
- Rust GUI: `NesstarConverter` (standalone desktop app)
- Python CLI: `nesstar-converter convert <input.Nesstar> <ddi.xml> ./output --formats csv,parquet,stata`

## Code Layout
- `crates/nesstar-core/` — Core Rust library
- `crates/nesstar-cli/` — Rust CLI wrapper
- `crates/nesstar-gui/` — Rust desktop GUI (eframe/egui)
- `gui/` — Legacy PySide6 GUI (superseded)
- `nesstar_converter.py` — Reference Python converter
- `tests/` — Python test suite
- `fixtures/` — Test fixtures
- `tools/` — Build scripts (build_deb.sh)
- `docs/` — Technical docs, ADRs, spikes, migration plan
- `.github/workflows/` — CI/CD (Python tests, Rust builds, PyPI publish)
