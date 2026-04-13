# Nesstar Converter

[![PyPI](https://img.shields.io/pypi/v/nesstar-converter)](https://pypi.org/project/nesstar-converter/)
[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![CI](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml)

Researchers, statistical agencies, and national archives spent years building survey datasets that document the economic and social life of entire populations — often with public money. Those datasets ended up locked in `.Nesstar`, a proprietary binary format whose only reader was a discontinued Windows desktop application. The company folded. The servers went offline. The licenses expired. But the data didn't stop mattering. **`nesstar-converter`** is a pure-Python binary parser that reads the format directly — no `.exe`, no Windows, no institutional subscription — and writes Parquet, CSV, Stata, Excel, and more. Your data. Open formats. Any platform.

---

## What this does

- **Reverse-engineered binary parser** — reads `.Nesstar` files directly in Python, no proprietary executable involved
- **Writes open formats** — Parquet, CSV, TSV, Excel, Stata, JSON, JSONL, fixed-width text
- **Validates against official exports** — cell-level comparison with Nesstar Explorer output
- **Runs everywhere** — Linux, macOS, Windows; Python 3.10+

---

## Quick start

**Install:**
```bash
pip install nesstar-converter
```

**Convert:**
```bash
nesstar-converter convert survey.Nesstar ddi.xml ./output --formats csv,parquet,stata
```

**Validate:**
```bash
nesstar-converter validate ./output ./exported_text
```

The DDI XML is auto-detected if it sits beside the `.Nesstar` file.

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

## Citation

If you use this in research, please cite via [`CITATION.cff`](CITATION.cff).

## License

[MIT](LICENSE)
