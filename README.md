# Nesstar Converter

[![PyPI](https://img.shields.io/pypi/v/nesstar-converter)](https://pypi.org/project/nesstar-converter/)
[![PyPI - Downloads](https://img.shields.io/pypi/dm/nesstar-converter?color=blue&label=PyPI%20downloads%2Fmonth)](https://pypistats.org/packages/nesstar-converter)
[![Downloads](https://static.pepy.tech/badge/nesstar-converter)](https://pepy.tech/project/nesstar-converter)
[![GitHub Release](https://img.shields.io/github/v/release/abhinavjnu/nesstar-converter)](https://github.com/abhinavjnu/nesstar-converter/releases/latest)
[![GitHub all releases](https://img.shields.io/github/downloads/abhinavjnu/nesstar-converter/total?color=orange&label=release%20downloads)](https://github.com/abhinavjnu/nesstar-converter/releases)
[![Analytics](https://img.shields.io/badge/Usage_Analytics-blueviolet)](ANALYTICS.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![CI](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml)

`nesstar-converter` is an open-source parser, streaming engine, and cross-platform desktop application for reading proprietary `.Nesstar` microdata containers (disseminated by India's Ministry of Statistics and Programme Implementation, the European Social Survey, and national data archives). It reverse-engineers the binary format without requiring legacy Windows software, converting multi-gigabyte survey datasets directly into Apache Parquet, Stata (`.dta`), SPSS (`.sav`), CSV, and JSON Lines.

---

## Quick Start

### 1. Web Application (WebAssembly)

Run conversions directly inside modern web browsers with 100% client-side privacy:

[Launch Web Converter (abhinavjnu.github.io/nesstar-converter)](https://abhinavjnu.github.io/nesstar-converter/)

*Zero installation. No data or files are transmitted to any server; execution runs entirely in local browser memory.*

---

### 2. Desktop Application

Standalone native desktop application (~16 MB, zero external runtime dependencies):

| Platform | Package | Installation |
|---|---|---|
| Linux (Universal AppImage) | [NesstarConverter-x86_64.AppImage](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | `chmod +x` and run on any Linux distribution (Ubuntu, Fedora, Arch) |
| Linux (Debian / Ubuntu) | [nesstar-converter.deb](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Install via `sudo dpkg -i nesstar-converter_*.deb` |
| Windows (x86_64) | [NesstarConverter-Windows.zip](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Extract archive and run `NesstarConverter.exe` |
| macOS (Universal) | [NesstarConverter-macOS.zip](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Move `NesstarConverter.app` to Applications |

---

### 3. Command Line Interface

High-throughput streaming conversion tool for batch data engineering pipelines:

```bash
# Convert to Parquet, Stata (.dta), SPSS (.sav), CSV, TSV, or JSON Lines (.jsonl)
nesstar-cli convert survey.Nesstar ddi.xml ./output/dataset.parquet
```

*Output format is automatically inferred from the target file extension.*

---

### 4. Python Library

Python package for integration with Pandas, Polars, PyArrow, and Jupyter analysis environments:

```bash
pip install -U nesstar-converter
```

```python
from nesstar_converter import convert_nesstar, show_info

# Display survey structure and metadata blocks
show_info("survey.Nesstar", "ddi.xml")

# Convert to Parquet and CSV
convert_nesstar("survey.Nesstar", "ddi.xml", "./output", formats=["parquet", "csv"])
```

**CLI Usage:**
```bash
nesstar-converter convert survey.Nesstar ddi.xml ./output --formats parquet,csv,stata
```

---

## Supported Formats

| Format | Extension | Intended Use Case |
|---|---|---|
| Apache Parquet | `.parquet` | High-performance analytical storage, DuckDB, Polars, Pandas |
| Stata Data File | `.dta` | Econometric estimation in Stata (preserves leading zeros and variable types) |
| SPSS System File | `.sav` | Statistical analysis in SPSS and PSPP |
| Delimited Text | `.csv` / `.tsv` | Tabular data interchange across spreadsheets and databases |
| JSON Lines | `.jsonl` | Record-oriented streaming ingestion, cloud data warehouses, AI pipelines |
| Fixed-Width Text | `.fwf` | Column-aligned text for legacy archive compatibility |

---

## Core Capabilities

- **Native Binary Parser**: Independent reverse-engineered binary reader; eliminates dependency on discontinued 32-bit Windows executables.
- **Large Dataset Support (>4 GB)**: Implements 48-bit pointer offsets, resolving legacy 32-bit integer overflow limitations on large longitudinal surveys.
- **Constant Memory Streaming**: Memory-mapped streaming decoders maintain a flat ~16 MB RAM footprint across multi-gigabyte files.
- **Differential Validation**: Verified against official Nesstar Explorer exports across 30,000,000+ data cells with 0.00% discrepancy.

---

<!-- ANALYTICS:START -->
## Usage Statistics

| Metric | Count | Description |
|---|---|---|
| **PyPI Total Downloads (Clean)** | **806** | Direct package installations (excluding mirror indexing bots) |
| **PyPI Total Downloads (Gross)** | **2,560** | Total recorded package pulls |
| **Monthly PyPI Installs** | **43** | Installations within the last 30 days |
| **Desktop Releases Downloaded** | **7** | Standalone GUI and CLI binary distributions |

*Detailed breakdowns by operating system, Python version, and longitudinal traffic history are documented in [ANALYTICS.md](ANALYTICS.md).*
<!-- ANALYTICS:END -->

---

## Documentation and Citation

- **Technical Reference & Specification**: [`docs/TECHNICAL.md`](docs/TECHNICAL.md)
- **Institutional Archive Coverage**: [`docs/global-coverage.md`](docs/global-coverage.md)
- **Academic Citation**: Please reference [`CITATION.cff`](CITATION.cff) or the preprint in [`paper/paper.md`](paper/paper.md).

## License

MIT License. Copyright (c) Abhinav Kumar.
