# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - 2025-04-14

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
