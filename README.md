# Nesstar Converter

[![PyPI](https://img.shields.io/pypi/v/nesstar-converter)](https://pypi.org/project/nesstar-converter/)
[![PyPI - Downloads](https://img.shields.io/pypi/dm/nesstar-converter?color=blue&label=PyPI%20downloads%2Fmonth)](https://pypistats.org/packages/nesstar-converter)
[![Downloads](https://static.pepy.tech/badge/nesstar-converter)](https://pepy.tech/project/nesstar-converter)
[![GitHub Release](https://img.shields.io/github/v/release/abhinavjnu/nesstar-converter)](https://github.com/abhinavjnu/nesstar-converter/releases/latest)
[![GitHub all releases](https://img.shields.io/github/downloads/abhinavjnu/nesstar-converter/total?color=orange&label=release%20downloads)](https://github.com/abhinavjnu/nesstar-converter/releases)
[![Analytics](https://img.shields.io/badge/📈_Usage-Analytics-blueviolet)](ANALYTICS.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![CI](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml)

**`nesstar-converter`** is a high-performance parser, streaming engine, and native desktop app for reading locked proprietary `.Nesstar` survey files (used widely by India's MoSPI/NSS, European Social Survey, and national data archives). It reverse-engineers the binary format without requiring legacy Windows software, converting multi-gigabyte survey datasets directly into **Apache Parquet, Stata (.dta), SPSS (.sav), CSV, and Excel**.

---

## ⚡ Quick Start

### 1. 🖥️ Desktop App (GUI) — *No Terminal Required*

Download the standalone native application (~16 MB, zero dependencies):

| Platform | Download | Instructions |
|---|---|---|
| 🪟 **Windows** | [NesstarConverter-Windows.zip](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Unzip and run `NesstarConverter.exe` |
| 🐧 **Linux** | [nesstar-converter.deb](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Install `.deb` package or run standalone binary |
| 🍎 **macOS** | [NesstarConverter-macOS.zip](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Drag `NesstarConverter.app` to Applications |

---

### 2. ⚡ High-Speed Rust CLI — *For Data Pipelines*

```bash
# Convert to Parquet, CSV, Stata (.dta), SPSS (.sav), or TSV (.txt)
nesstar-cli convert survey.Nesstar ddi.xml ./output/dataset.parquet
```

*Infers output format directly from destination file extension.*

---

### 3. 🐍 Python Library & CLI — *For Pandas, Polars & Jupyter*

```bash
pip install -U nesstar-converter
```

```python
from nesstar_converter import convert_nesstar, show_info

# Inspect metadata & data blocks
show_info("survey.Nesstar", "ddi.xml")

# Convert to Parquet and CSV
convert_nesstar("survey.Nesstar", "ddi.xml", "./output", formats=["parquet", "csv"])
```

**Python CLI:**
```bash
nesstar-converter convert survey.Nesstar ddi.xml ./output --formats parquet,csv,stata
```

---

## 📦 Supported Formats

| Format | Extension | Primary Use Case |
|---|---|---|
| **Apache Parquet** | `.parquet` | Python, R, DuckDB, Polars, long-term analytical storage |
| **Stata** | `.dta` | Econometric modeling in Stata (preserves leading zeros & types) |
| **SPSS** | `.sav` | Social science analysis in SPSS / PSPP |
| **CSV / TSV** | `.csv` / `.tsv` | Universal tabular interchange, Excel, Google Sheets |
| **Excel** | `.xlsx` | Spreadsheet review with formatted headers |
| **JSON / JSONL** | `.json` / `.jsonl` | Web applications and streaming data pipelines |

---

## 🌟 Key Features

- **No Windows Executable Required**: 100% native reverse-engineered parser running seamlessly on Linux, macOS, and Windows.
- **Large Dataset Support (>4 GB)**: Supports 48-bit index offsets for massive longitudinal surveys (e.g. NSS 68th Round Consumer Expenditure).
- **Streaming & Low Memory**: Memory-mapped zero-copy decoding maintains a flat ~16 MB RAM footprint regardless of dataset size.
- **Cell-Level Rigor**: Differential validation against official Nesstar Explorer exports on **30,000,000+ survey cells** with 0.00% mismatch.

---

<!-- ANALYTICS:START -->
## 📊 Analytics & Usage Stats

| Metric | Count | Description |
|---|---|---|
| **PyPI Total Downloads (Clean)** | **806** | Direct pip installs (excluding automated bots) |
| **PyPI Total Downloads (Gross)** | **2,560** | All recorded package pulls |
| **Monthly PyPI Installs** | **43** | Downloads in the last 30 days |
| **Desktop App Releases** | **7** | Native GUI & CLI desktop binary downloads |

> 📈 *View the full breakdown by OS (Linux 56%, macOS 27%, Windows 17%), Python versions, and traffic history in **[ANALYTICS.md](ANALYTICS.md)**.*
<!-- ANALYTICS:END -->

---

## 📚 Documentation & Research

- **Technical Specifications & Binary Layout**: [`docs/TECHNICAL.md`](docs/TECHNICAL.md)
- **Global Institutional Coverage**: [`docs/global-coverage.md`](docs/global-coverage.md)
- **Academic Citation**: Please cite via [`CITATION.cff`](CITATION.cff) or [`paper/paper.md`](paper/paper.md).

## License

[MIT](LICENSE) © Abhinav Kumar
