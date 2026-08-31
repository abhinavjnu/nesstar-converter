# Nesstar Converter

[![PyPI](https://img.shields.io/pypi/v/nesstar-converter)](https://pypi.org/project/nesstar-converter/)
[![PyPI - Downloads](https://img.shields.io/pypi/dm/nesstar-converter?color=blue&label=PyPI%20downloads%2Fmonth)](https://pypistats.org/packages/nesstar-converter)
[![Downloads](https://static.pepy.tech/badge/nesstar-converter)](https://pepy.tech/project/nesstar-converter)
[![GitHub Release](https://img.shields.io/github/v/release/abhinavjnu/nesstar-converter)](https://github.com/abhinavjnu/nesstar-converter/releases/latest)
[![GitHub all releases](https://img.shields.io/github/downloads/abhinavjnu/nesstar-converter/total?color=orange&label=release%20downloads)](https://github.com/abhinavjnu/nesstar-converter/releases)
[![Analytics](https://img.shields.io/badge/📈_Usage-Analytics-blueviolet)](ANALYTICS.md)
[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![CI](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml)

## 3 Ways to Use Nesstar Converter

Choose the best workflow for your needs:

### 1. 🖥️ Desktop App (GUI) — *For Researchers & Non-Coders*

> **No terminal or Python required** — download, open, select your `.Nesstar` and `ddi.xml` files, and export.

| Platform | Download | Installation |
|---|---|---|
| 🪟 **Windows** | [NesstarConverter-Windows.zip](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Unzip and run `NesstarConverter.exe` |
| 🐧 **Linux** | [nesstar-converter.deb](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Install `.deb` package or run `.tar.gz` standalone binary |
| 🍎 **macOS** | [NesstarConverter-macOS.zip](https://github.com/abhinavjnu/nesstar-converter/releases/latest) | Unzip and drag `NesstarConverter.app` to Applications |

*Built in native Rust with [eframe](https://github.com/emilk/egui). Lightweight (~16 MB), zero runtime dependencies, dark/light theme, real-time progress bars.*

---

### 2. ⚡ High-Speed Rust CLI — *For Data Engineers & Batch Pipelines*

> High-throughput streaming binary decoder with native Apache Arrow & Parquet compression.

```bash
# Convert to Parquet, CSV, Stata (.dta), SPSS (.sav), or TSV (.txt)
nesstar-cli convert survey.Nesstar ddi.xml ./output/dataset.parquet
```

*Infers output format from file extension (`.parquet`, `.csv`, `.dta`, `.sav`, `.txt`).*

---

### 3. 🐍 Python Library & CLI — *For Data Scientists & Analysts*

> Pure-Python library and CLI for Jupyter notebooks, Pandas, and Polars workflows.

```bash
pip install -U nesstar-converter
```

**Python CLI:**
```bash
# Convert to multiple formats simultaneously
nesstar-converter convert survey.Nesstar ddi.xml ./output --formats parquet,csv,stata

# Inspect metadata & data blocks
nesstar-converter info survey.Nesstar ddi.xml

# Validate extraction cell-for-cell against official exports
nesstar-converter validate ./output ./official_text_exports/
```

**Python API:**
```python
from nesstar_converter import convert_nesstar, show_info

# Inspect dataset
show_info("survey.Nesstar", "ddi.xml")

# Convert to Parquet & CSV
convert_nesstar("survey.Nesstar", "ddi.xml", "./output", formats=["parquet", "csv"])
```

---

## ⚡ Performance & Benchmarks

| Feature / Benchmark | Legacy Nesstar Explorer (1999–2014) | `ihsn/nesstar-exporter` (Wrapper) | `nesstar-converter` (Rust / Python) |
|---|---|---|---|
| **Underlying Engine** | 32-bit proprietary Windows binary | Python shelling out to `.exe` | **Native Rust & pure-Python parser** |
| **Cross-Platform** | ❌ Windows only (needs Wine on Linux) | ❌ Requires Windows `.exe` | ✅ **Native Linux, macOS & Windows** |
| **Parquet / Arrow Output** | ❌ No | ❌ No | ✅ **Yes (Streaming compressed Parquet)** |
| **Memory Footprint** | ~500 MB+ (crashes on >4GB files) | ~500 MB+ | ✅ **Streaming chunked (~16 MB RAM)** |
| **Files > 4 GB (e.g. NSS 68)** | ❌ 32-bit integer overflow crash | ❌ Crashes with `.exe` | ✅ **Supported (48-bit index offsets)** |
| **Automation & CI/CD** | ❌ Manual GUI clicks | ⚠️ Subprocess scripts | ✅ **Native CLI, Python API, Rust Crate** |

---

## Supported formats

| Format | Extension | Best for |
|---|---|---|
| `parquet` | `.parquet` | Python, R, DuckDB, long-term storage |
| `csv` | `.csv` | Excel, LibreOffice, Google Sheets |
| `tsv` | `.tsv` | Tab-separated workflows |
| `excel` | `.xlsx` | Non-technical users who just want a spreadsheet |
| `stata` | `.dta` | Stata, with leading zeros preserved |
| `json` | `.json` | Web apps, structured interchange |
| `jsonl` | `.jsonl` | Streaming pipelines |
| `fwf` | `.txt` | Fixed-width text |

---

## `nesstar-converter` vs `ihsn/nesstar-exporter`

The IHSN tool wraps the official Windows binary. It is not a replacement for it — you still need the `.exe`.

| Dimension | `ihsn/nesstar-exporter` | `nesstar-converter` |
|---|---|---|
| Core approach | Python wrapper around `NesstarExporter.exe` | Pure-Python binary parser |
| Requires `NesstarExporter.exe` | **Yes** | **No** |
| OS model | Windows-oriented workflow | Linux / macOS / Windows |
| Reads binary directly | No | Yes |
| Reverse-engineered format support | No | Yes |
| Parquet output | No | Yes |
| RDF / DDI export via official tool | Yes | No |
| Validation against text exports | No built-in validation layer | Yes |
| Install model | Repo scripts + external exe path | Standard Python package / console script |

**Evidence:** the IHSN repo's own README, `config.json`, `src/config.py`, and `src/exporter.py` all require a path to `NesstarExporter.exe` and shell out to it with `subprocess.run(...)`.

---

## Who uses Nesstar

| Institution / repository | Country / region | Status |
|---|---|---|
| **NSD / Sikt** | Norway | Original Nesstar developer and ESS host |
| **UK Data Archive / UK Data Service** | United Kingdom | Co-developer and former Nesstar WebView operator |
| **European Social Survey** | Pan-European | Disseminated through Nesstar from 2004 |
| **Statistics Canada / ODESI** | Canada | Licensed the full Nesstar suite |
| **GESIS ZACAT** | Germany | Former Nesstar WebView catalog |
| **Sciences Po / CDSP** | France | Documented migration away from Nesstar |
| **SSJDA / CSRDA** | Japan | Documented Nesstar deployment |
| **IHSN / World Bank** | Global | Still distributes Nesstar Publisher and migration tooling |
| **India MoSPI / NSO** | India | Active distributor of `.Nesstar` survey files |
| **DataFirst / Stats SA** | South Africa | Legacy archive and testing target |

Full evidence and source links: [`docs/global-coverage.md`](docs/global-coverage.md).

---

## Validation coverage

Validation distinguishes **cell-level** (row-for-row, value-for-value match against official exports) from **structure-level** (file counts and variable counts confirmed, but companion DDI XML was not shipped by the distributor for full binary re-validation).

| Survey | Years / rounds | Level | Result |
|---|---|---|---|
| **EUS** | 38th Round (1983) | Cell-level | 9/9 blocks, 3.4M rows, zero mismatches |
| **HCES** | 38th, 45th, 66th | Cell-level | 27/28 blocks, 23.4M+ rows, zero mismatches |
| **PLFS** | 2017-18 to 2022-23 | Structure-level | 24/24 exports matched NADA dictionary row/column counts |

PLFS raw packages include `.Nesstar` files but omit the companion DDI XML, so current evidence is structural. Cell-level re-validation awaits DDI availability.

---

## Python API

```python
from nesstar_converter import convert_nesstar, show_info

show_info("survey.Nesstar", "ddi.xml")
convert_nesstar("survey.Nesstar", "ddi.xml", "./output", formats=["csv", "parquet"])
```

---

## Limitations

- **Expects DDI metadata.** Without the companion DDI XML, the parser cannot yet do full extraction from the binary alone.
- **Data conversion, not RDF packaging.** For DDI/RDF export via the official legacy toolchain, the IHSN wrapper exists — but still requires `NesstarExporter.exe`.
- **Legacy ecosystems vary.** Different institutions used different Nesstar-era conventions; community test cases from outside India are especially valuable.

---

## Contributing

- Test on non-Indian Nesstar files and report results
- Share evidence of `.Nesstar` / `.NSDstat` datasets still in circulation
- Help improve metadata recovery for archives that omit DDI XML

Docs: [`docs/TECHNICAL.md`](docs/TECHNICAL.md) · [`docs/global-coverage.md`](docs/global-coverage.md)

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

## Citation

If you use this in research, please cite via [`CITATION.cff`](CITATION.cff).

## License

[MIT](LICENSE)
