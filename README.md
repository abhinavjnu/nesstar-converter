# 📊 Nesstar Converter

[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-58%20passing-brightgreen.svg)](#running-tests)
[![CI](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavjnu/nesstar-converter/actions)

**Convert proprietary Nesstar binary files into researcher-friendly formats — CSV, Parquet, Excel, Stata, and more.**

India's National Sample Survey (NSS) microdata is distributed by the Ministry of Statistics (MOSPI) in a proprietary Nesstar binary format. Until now, researchers needed Nesstar Explorer — a discontinued, Windows-only application — just to read the data. This tool eliminates that dependency entirely. The Nesstar format was reverse-engineered from binary analysis and validated with **100% cell-level accuracy** against official Nesstar Explorer text exports across 11+ survey rounds.

---

## Quick Start

### Install

```bash
pip install nesstar-converter
```

Or from source:

```bash
git clone https://github.com/abhinavjnu/nesstar-converter.git
cd nesstar-converter
pip install -e .
```

### Basic Usage

```bash
# Inspect a Nesstar file
python nesstar_converter.py info myfile.Nesstar

# Convert to CSV
python nesstar_converter.py convert myfile.Nesstar ./output --formats csv

# Convert to multiple formats at once
python nesstar_converter.py convert myfile.Nesstar ./output --formats csv,excel,stata
```

That's it. The `ddi.xml` metadata file is auto-detected from the same directory as the `.Nesstar` file.

---

## Features

- **8 output formats** — Parquet, CSV, TSV, Excel, Stata, JSON, JSON Lines, fixed-width
- **Zero-dependency data access** — no Nesstar Explorer, no Windows, no GUI needed
- **100% validated accuracy** — cell-for-cell match against official Nesstar Explorer exports
- **Auto-detection** — finds the companion `ddi.xml` automatically
- **Memory-mapped I/O** — handles large survey files (hundreds of MB) without loading everything into RAM
- **Batch conversion** — convert all rounds of a survey in one command
- **Built-in validation** — compare your converted output against Nesstar Explorer text exports
- **Variable labels preserved** — Excel and Stata outputs carry human-readable column labels
- **Leading zeros preserved** — Stata `.dta` output keeps string identifiers intact (state codes, sample codes)
- **Single-file architecture** — one Python file, no complex package structure, easy to audit

---

## Supported Formats

| Format    | Extension   | Best For                                      |
|-----------|-------------|-----------------------------------------------|
| `parquet` | `.parquet`  | Large datasets, fast analytics (pandas, R, DuckDB) |
| `csv`     | `.csv`      | Excel, Google Sheets, R, Stata — universal    |
| `tsv`     | `.tsv`      | Tab-separated; traditional survey data format |
| `excel`   | `.xlsx`     | Non-technical users; includes variable labels |
| `stata`   | `.dta`      | Stata users; preserves leading zeros (v117)   |
| `json`    | `.json`     | Web applications, APIs                        |
| `jsonl`   | `.jsonl`    | Streaming pipelines, line-by-line processing  |
| `fwf`     | `.txt`      | Fixed-width text; preserves original column widths |

**Recommendation:** Use `parquet` for analysis (smallest files, fastest reads). Use `csv` if you just need to open the data in a spreadsheet.

---

## Detailed Usage

### Inspect a file

See what's inside a Nesstar file before converting — block names, record counts, variable types:

```bash
python nesstar_converter.py info myfile.Nesstar
```

Sample output:

```
═══════════════════════════════════════════════════════════
  NESSTAR FILE INFO
═══════════════════════════════════════════════════════════
  Nesstar file : myfile.Nesstar
  File size    : 142.3 MB (142,312,448 bytes)
  Magic        : NESSTART ✓

  Blocks       : 9
  Total records: 3,421,607
  Total vars   : 187

  Block Details:
    F1  Person_Records                       1,234,567 records   32 vars
    F2  Household_Records                      456,789 records   28 vars
    ...
```

### Convert

```bash
# Single format
python nesstar_converter.py convert myfile.Nesstar ./output --formats csv

# Multiple formats
python nesstar_converter.py convert myfile.Nesstar ./output --formats csv,excel,stata

# All 8 formats
python nesstar_converter.py convert myfile.Nesstar ./output --formats all

# Explicit DDI path (if auto-detection doesn't work)
python nesstar_converter.py convert myfile.Nesstar ddi.xml ./output --formats csv

# With year label and quiet mode
python nesstar_converter.py convert myfile.Nesstar ./output --formats parquet --year 2023-24 --quiet
```

Output files are organized by block name:

```
output/
├── Person_Records.csv
├── Person_Records.parquet
├── Household_Records.csv
├── Household_Records.parquet
└── ...
```

### Validate against Nesstar Explorer exports

If you have text exports from Nesstar Explorer, you can verify the converter's output cell-by-cell:

```bash
python nesstar_converter.py validate ./parquet_dir ./export_dir
```

The validator compares every cell in every row using multiset matching — row order doesn't matter, but every value must match exactly.

### Batch convert

Convert all Nesstar files for an entire survey at once:

```bash
python nesstar_converter.py batch --survey hces --formats parquet,csv
```

### List supported formats

```bash
python nesstar_converter.py formats
```

---

## How It Works

The Nesstar binary format is undocumented. This converter was built by reverse-engineering the binary structure through systematic analysis. Here's what happens when you run a conversion:

1. **DDI metadata parsing** — Reads the companion `ddi.xml` file (DDI Codebook standard) to discover block definitions, variable names, data types, value ranges, and expected record counts.

2. **Memory-mapped file access** — The `.Nesstar` binary is memory-mapped for efficient random access without loading the entire file into RAM.

3. **Metadata slot discovery** — Scans the binary for 160-byte metadata slot signatures. Each slot describes one variable: its encoding type, character width, and name (stored as UTF-16-LE).

4. **DDI ↔ binary matching** — Aligns DDI variable definitions with discovered metadata slots using name matching and positional heuristics.

5. **Column-major decoding** — Data is stored column-major: all N values for variable 0, then all N values for variable 1, and so on. Three encoding types are decoded:

   | Encoding | Storage | Description |
   |----------|---------|-------------|
   | `char`   | Fixed-width ASCII bytes | String data (state codes, identifiers) |
   | `offset` | Variable-width integers | Integers with range compression (min/max from DDI) |
   | `double` | 8 bytes IEEE 754 | Floating-point values |

6. **Validation** — Each extracted block is checked against DDI expectations (row count, column count, column names) before writing output files.

### File structure

Every MOSPI Nesstar distribution contains two files in the same directory:

| File | Contents |
|------|----------|
| `*.Nesstar` | Binary data (starts with `NESSTART` magic bytes) |
| `ddi.xml` | DDI Codebook XML — variable names, types, labels, value ranges, record counts |

The converter auto-detects `ddi.xml` from the same directory as the `.Nesstar` file.

---

## Validation Results

The converter has been validated against ground-truth Nesstar Explorer text exports with zero mismatches:

| Survey | Round | Blocks | Rows | Cells (approx.) | Mismatches |
|--------|-------|--------|------|------------------|------------|
| EUS (Employment-Unemployment) | 38th (1983) | 9/9 | 3.4M | ~978M | **0** |
| HCES (Household Consumption) | 38th (1983) | 7/7 | — | — | **0** |
| HCES | 45th (1989-90) | 10/10 | — | — | **0** |
| HCES | 66th (2009-10) | 10/11* | — | — | **0** |
| **Total validated** | | **36+ blocks** | **23.4M+ rows** | **~billions** | **0** |

\* One HCES 66th block has a known DDI metadata discrepancy (missing from DDI). All blocks present in DDI extract with zero errors.

The test suite contains **58 automated tests** (unit + integration) covering DDI parsing, binary decoding, metadata matching, format output, CLI behavior, and edge cases. CI runs on Python 3.10–3.13.

---

## For Non-Technical Users

Never used a command line before? This step-by-step guide will walk you through converting your Nesstar files to CSV, which opens directly in Microsoft Excel or Google Sheets.

### Step 1: Install Python

1. Go to [python.org/downloads](https://www.python.org/downloads/)
2. Download Python 3.10 or newer (click the big yellow button)
3. **Important:** During installation, check the box that says **"Add Python to PATH"**
4. Click "Install Now" and wait for it to finish

### Step 2: Open a terminal

- **Windows:** Press `Win + R`, type `cmd`, press Enter
- **Mac:** Open Spotlight (`Cmd + Space`), type `Terminal`, press Enter
- **Linux:** Press `Ctrl + Alt + T`

### Step 3: Install Nesstar Converter

Type this into your terminal and press Enter:

```bash
pip install nesstar-converter
```

You should see some progress bars and then a success message.

### Step 4: Find your Nesstar files

Your MOSPI data download should contain two files in the same folder:

- A file ending in `.Nesstar` (this is your data)
- A file called `ddi.xml` (this is the metadata)

Note the full path to the `.Nesstar` file. For example:
- Windows: `C:\Users\YourName\Downloads\NSS_data\mysurvey.Nesstar`
- Mac/Linux: `/home/yourname/Downloads/NSS_data/mysurvey.Nesstar`

### Step 5: Convert to CSV

In your terminal, type:

```bash
python nesstar_converter.py convert "path/to/mysurvey.Nesstar" ./my_output --formats csv
```

Replace `path/to/mysurvey.Nesstar` with the actual path to your file. The tool will automatically find the `ddi.xml` in the same folder.

### Step 6: Find your output

Look in the `my_output` folder (created in your current directory). You'll find one `.csv` file for each data block in the survey — for example, `Person_Records.csv`, `Household_Records.csv`, etc.

Double-click any `.csv` file to open it in Excel or upload it to Google Sheets.

### Which format should I choose?

| You want to... | Use this |
|----------------|----------|
| Open in Excel or Google Sheets | `csv` |
| Use in Stata | `stata` |
| Use in R or Python for analysis | `parquet` |
| Share with someone who uses Excel | `excel` |
| Not sure | `csv` — it works everywhere |

---

## API Usage

Use `nesstar_converter` as a Python library in your own scripts:

```python
from nesstar_converter import convert_nesstar, parse_ddi, show_info

# Inspect a file (prints to stdout)
show_info('myfile.Nesstar', 'ddi.xml')

# Convert to multiple formats
report = convert_nesstar(
    'myfile.Nesstar', 'ddi.xml', './output',
    formats=['csv', 'parquet', 'stata'],
    year='2023-24'
)

# Access the conversion report
for block_name, info in report['blocks'].items():
    print(f"{block_name}: {info['rows']} rows, {info['columns']} cols")
```

### Parse DDI metadata only

```python
from nesstar_converter import parse_ddi

blocks = parse_ddi('ddi.xml')
for fid, block in blocks.items():
    print(f"{block['name']}: {block['nrecs']} records, {len(block['ddi_vars'])} variables")
    for var in block['ddi_vars'][:5]:
        print(f"  {var['name']:20s}  {var['type']:10s}  {var['label']}")
```

### Key functions

| Function | Description |
|----------|-------------|
| `convert_nesstar(nesstar, ddi, outdir, formats, year)` | Convert a Nesstar file; returns extraction report dict |
| `parse_ddi(ddi_path)` | Parse DDI XML; returns block definitions with variable specs |
| `show_info(nesstar, ddi)` | Print file inspection report to stdout |
| `validate_against_export(parquet_dir, export_dir)` | Cell-level validation against Nesstar Explorer exports |
| `batch_convert(survey, formats)` | Find and convert all Nesstar files for a survey |

---

## Running Tests

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Run the full test suite
pytest tests/ -v

# Run unit tests only (no real data files required)
pytest tests/ -v -m "not integration and not slow"
```

The test suite includes:
- **Unit tests** — DDI parsing, binary width computation, slot reading, encoding/decoding, metadata matching, format writers, CLI argument handling
- **Integration tests** — full pipeline extraction against real Nesstar files (auto-skipped if data files are not present)

---

## Surveys Known to Work

This tool works with any NSS survey distributed through the Nesstar system. Tested surveys include:

| Survey | Full Name |
|--------|-----------|
| **EUS** | Employment-Unemployment Survey |
| **HCES** | Household Consumer Expenditure Survey |
| **ASI** | Annual Survey of Industries |
| **SAS** | Social Consumption (various rounds) |
| **TUS** | Time Use Survey |

If you successfully convert another survey, please open an issue or PR to add it to this list.

---

## Requirements

- Python 3.10+
- [pandas](https://pandas.pydata.org/)
- [pyarrow](https://arrow.apache.org/docs/python/) (for Parquet support)
- [numpy](https://numpy.org/)
- [tqdm](https://tqdm.github.io/) (progress bars; optional but recommended)
- [openpyxl](https://openpyxl.readthedocs.io/) (for Excel output)

All dependencies are installed automatically with `pip install nesstar-converter`.

---

## Contributing

Contributions are welcome! Here are some ways to help:

- **Test with new surveys** — Try converting Nesstar files from surveys not yet on the tested list and report results
- **Report bugs** — Open an issue with the error message and (if possible) the DDI file
- **Improve documentation** — Especially for surveys you've worked with
- **Add features** — See open issues for ideas

### Development setup

```bash
git clone https://github.com/abhinavjnu/nesstar-converter.git
cd nesstar-converter
pip install -e ".[dev]"
pytest tests/ -v
```

Please ensure all tests pass before submitting a pull request.

---

## License

[MIT](LICENSE) — use it freely for research, teaching, and commercial work.

---

## Acknowledgments

This tool was built to support open access to India's public survey microdata. The Nesstar binary format is undocumented; the decoding logic was developed through careful binary analysis and validated against official Nesstar Explorer exports.

If this tool saves you time in your research, consider citing it or starring the repository.
