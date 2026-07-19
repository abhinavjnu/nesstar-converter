# Changelog

All notable changes to this project will be documented in this file.

## [1.0.7] - 2026-07-15

### Added
- Windows build support — CI now compiles and releases `NesstarConverter.exe` for Windows
- Native Debian packaging (`.deb`) for one-click Linux installation
- Local `.deb` build script (`tools/build_deb.sh`)

### Changed
- Release workflow now produces artifacts for all three platforms (macOS, Linux, Windows)

## [1.0.6] - 2026-07-14

### Changed
- Redesigned GUI with brand sidebar, builder credits (Abhinav), and sponsor links

### Fixed
- Use default features for `rfd` crate to prevent Linux build panic

## [1.0.5] - 2026-07-14

### Added
- Complete Rust rewrite of the Nesstar converter with `nesstar-core` library
- Native eframe/egui desktop GUI (`nesstar-gui`) — no Python runtime required
- Native multi-format output: CSV, tab-separated text, Parquet, and Stata DTA in pure Rust
- MoSPI pipeline integration and public DDI registry validation
- Test fixtures validation for Rust converter correctness

### Changed
- CI build workflow updated to compile optimized Rust desktop apps instead of Python/PyInstaller bundles
- Binary size reduced from ~250 MB (PySide6) to ~16 MB (Rust/eframe)

## [1.0.4] - 2026-06-16

### Fixed
- Parse 48-bit little-endian resource-index offsets for containers larger than 4 GiB; u32-only reads silently dropped the index and fell back to lossy metadata scanning ([#4](https://github.com/abhinavjnu/nesstar-converter/issues/4), thanks [@adamlooney](https://github.com/adamlooney))
- Detect doubled column payloads (size = 2× true width) and decode the first copy instead of gluing consecutive values ([#5](https://github.com/abhinavjnu/nesstar-converter/issues/5), thanks [@adamlooney](https://github.com/adamlooney))

### Added
- Synthetic unit tests for 48-bit resource-index offsets and doubled payload handling

## [1.0.3] - 2026-05-11

### Added
- Resource-indexed Nesstar extraction path that reads dataset descriptors, variable directories, and exact payload offsets from trailing resource records
- Compact numeric decoding support for nibble-packed, uint8/16/24/32/40, and float64 resource payloads
- Integration regression test for PLFS resource-indexed files against official text exports when local fixtures are available

### Changed
- Conversion pipeline now prefers resource-index extraction and falls back to metadata-adjacent scanning only when needed
- Improved resource text decoding for NUL-terminated string slots and stricter payload-size validation
- Updated technical format documentation for resource-indexed layout and compact encodings

## [1.0.2] - 2026-04-15

### Changed
- Rewrote README and package description to lead with the human problem — public knowledge locked in a discontinued proprietary binary — and carry an open-source/academic solidarity angle
- Trimmed README from ~1,100 to ~870 words while preserving all technical tables

## [1.0.1] - 2026-04-13

### Added
- Global Nesstar ecosystem documentation in `docs/global-coverage.md`
- `CITATION.cff` metadata for academic citation
- GitHub Actions workflow for building and publishing release artifacts to PyPI via trusted publishing

### Changed
- Reframed the README around the global legacy-format problem, not just India/MOSPI
- Added an evidence-backed comparison against `ihsn/nesstar-exporter`
- Added PLFS structure-verification coverage based on NADA data-dictionary counts and official export files

## [1.0.0] - 2026-04-13

### Added
- Initial release of nesstar-converter
- Convert Nesstar binary files to 8 formats: Parquet, CSV, TSV, Excel, Stata, JSON, JSONL, Fixed-Width
- DDI XML metadata parsing with automatic variable type detection
- Auto-detection of ddi.xml from the same directory as .Nesstar file
- Memory-mapped binary reading for efficient large file handling
- Three encoding types: char (ASCII), offset (range-compressed integers), double (IEEE 754)
- Built-in validation against DDI expectations (row count, column count, column names)
- Validation command to compare output against Nesstar Explorer text exports
- Batch conversion mode for processing entire survey directories
- Excel output with variable labels and metadata sheet
- Stata output preserving leading zeros as string columns
- Progress bar with tqdm (graceful fallback without it)
- Comprehensive test suite (58 tests)
- CLI with info, convert, validate, batch, and formats commands
- Proper exit codes for scripting (0=success, 1=conversion error, 2=usage error)

### Validated Against
- EUS 38th Round (1983): 9/9 blocks, 3,445,585 rows — zero mismatches
- HCES 38th, 45th, 66th Rounds: 27/28 blocks, 23.4M rows
- Cross-survey compatibility verified across EUS and HCES
