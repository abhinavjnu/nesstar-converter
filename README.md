# Nesstar Converter

[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![PyPI](https://img.shields.io/pypi/v/nesstar-converter)](https://pypi.org/project/nesstar-converter/)
[![CI](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavjnu/nesstar-converter/actions/workflows/ci.yml)

**Pure-Python conversion for legacy Nesstar survey files - no `NesstarExporter.exe`, no Windows-only GUI, no dependency on discontinued desktop tooling.**

Nesstar was once a common dissemination format across social-science archives, national data services, and statistical agencies worldwide. The legacy ecosystem persists, but the original tooling is fragmented: many servers are gone, much documentation is outdated, and the surviving migration tools still depend on a proprietary Windows executable.

`nesstar-converter` takes the opposite approach. It reverse-engineers the binary format directly in Python and writes open outputs such as Parquet, CSV, Excel, Stata, and JSON on Linux, macOS, and Windows.

This project started with India's MoSPI survey archives, but the underlying problem is global. The wider Nesstar ecosystem touched the UK Data Archive, the European Social Survey, Statistics Canada / ODESI, GESIS, SSJDA in Japan, and the IHSN / World Bank metadata workflow. See [`docs/global-coverage.md`](docs/global-coverage.md) for the evidence-backed map.

---

## Why this exists

- **Zero `.exe` dependency** - no `NesstarExporter.exe`, no batch wrappers, no Wine
- **Cross-platform** - works anywhere Python 3.10+ works
- **Reverse-engineered binary parser** - reads `.Nesstar` files directly
- **Open output formats** - Parquet, CSV, TSV, Excel, Stata, JSON, JSONL, fixed-width text
- **Validation-first** - compares converted output against official Nesstar Explorer exports
- **Non-technical friendly** - one CLI, clear commands, sensible defaults

---

## `nesstar-converter` vs `ihsn/nesstar-exporter`

The IHSN tool is useful if you already have the official Windows exporter binary and want to automate that workflow. It is **not** a replacement for the binary itself.

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

## Who uses Nesstar?

Nesstar was not just an India-specific format. It was part of a broader international archive ecosystem.

| Institution / repository | Country / region | What we verified |
|---|---|---|
| **NSD / Sikt** | Norway | Original Nesstar developer and ESS host |
| **UK Data Archive / UK Data Service** | United Kingdom | Co-developer and former Nesstar WebView operator |
| **European Social Survey** | Pan-European | Disseminated through Nesstar from 2004 |
| **Statistics Canada / ODESI** | Canada | Licensed the full Nesstar suite; former WebView instance |
| **GESIS ZACAT** | Germany | Former Nesstar WebView catalog |
| **Sciences Po / CDSP** | France | Publicly documented migration away from Nesstar |
| **SSJDA / CSRDA** | Japan | Publicly documented Nesstar deployment |
| **IHSN / World Bank ecosystem** | Global | Still distributes Nesstar Publisher and maintains migration tooling |
| **India MoSPI / NSO** | India | Active distributor of `.Nesstar` survey files |
| **DataFirst / Stats SA** | South Africa | Important related archive / testing target, but evidence is legacy or mixed |

For the full institution table, confidence levels, and source links, see [`docs/global-coverage.md`](docs/global-coverage.md).

---

## Supported formats

| Format | Extension | Best for |
|---|---|---|
| `parquet` | `.parquet` | Analytics, DuckDB, pandas, R, long-term storage |
| `csv` | `.csv` | Universal spreadsheet compatibility |
| `tsv` | `.tsv` | Tab-separated workflows and legacy survey tooling |
| `excel` | `.xlsx` | Non-technical users |
| `stata` | `.dta` | Stata users, with leading zeros preserved |
| `json` | `.json` | Web apps and structured interchange |
| `jsonl` | `.jsonl` | Streaming and line-oriented pipelines |
| `fwf` | `.txt` | Fixed-width text output |

---

## Quick start

### Install from PyPI

```bash
python -m pip install nesstar-converter
```

### Install from source

```bash
git clone https://github.com/abhinavjnu/nesstar-converter.git
cd nesstar-converter
python -m pip install -e ".[dev]"
```

### Inspect a file

```bash
nesstar-converter info path/to/file.Nesstar path/to/ddi.xml
```

### Convert to open formats

```bash
nesstar-converter convert path/to/file.Nesstar path/to/ddi.xml ./output --formats csv,parquet,stata
```

### Validate against official text exports

```bash
nesstar-converter validate ./output ./exported_text
```

If the companion `ddi.xml` sits beside the `.Nesstar` file, you can omit it and the tool will auto-detect it.

---

## Validation and coverage

This repository distinguishes between:

1. **Cell-level validation** - converted output matched official Nesstar Explorer exports row-for-row and value-for-value.
2. **Structure-level verification** - official export files matched published file counts and variable counts, but the raw package lacked the companion DDI XML required for full binary re-validation.

| Survey / corpus | Years / rounds | Verification level | Result |
|---|---|---|---|
| **EUS** | 38th Round (1983) | Cell-level | 9/9 blocks, 3.4M rows, zero mismatches against official exports |
| **HCES** | 38th (1983), 45th (1989-90), 66th (2009-10) | Cell-level | 27/28 blocks, 23.4M+ rows, zero mismatches for blocks present in DDI |
| **PLFS** | 2017-18 to 2022-23 | Structure-level | 24/24 official export files matched NADA data-dictionary row/column counts; one 2017-18 revisit export includes a trailing blank tab column |

**PLFS note:** the local PLFS raw ZIPs contain `.Nesstar` files, official text exports, and the legacy Nesstar Explorer installer, but not the companion DDI XML needed for full binary decoding in the current open parser. That means PLFS is confirmed as a real Nesstar distribution corpus, but its current evidence in this repo is structural rather than full cell-level re-validation.

---

## For non-technical users

If your goal is simply "turn this old survey file into something Excel can open", the shortest path is:

```bash
python -m pip install nesstar-converter
nesstar-converter convert path/to/file.Nesstar path/to/ddi.xml ./output --formats csv
```

Then open the generated `.csv` files in Excel, LibreOffice, Google Sheets, Stata, R, or Python.

If you are unsure which format to choose:

| You want to... | Use |
|---|---|
| Open the data in Excel | `csv` |
| Work in Stata | `stata` |
| Analyze in Python / R / DuckDB | `parquet` |
| Preserve a text-like interchange format | `tsv` or `fwf` |

---

## Python API

```python
from nesstar_converter import convert_nesstar, show_info

show_info("survey.Nesstar", "ddi.xml")

report = convert_nesstar(
    "survey.Nesstar",
    "ddi.xml",
    "./output",
    formats=["csv", "parquet"],
    year="2022-23",
)
```

Key functions:

| Function | Purpose |
|---|---|
| `convert_nesstar(...)` | Convert one `.Nesstar` file to one or more formats |
| `parse_ddi(...)` | Parse DDI XML block and variable metadata |
| `show_info(...)` | Inspect a file before conversion |
| `validate_against_export(...)` | Compare converted output to official text exports |
| `batch_convert(...)` | Convert a survey corpus in batch mode |

---

## Limitations

- **Full decoding currently expects DDI metadata.** If a distributor ships only the `.Nesstar` binary and omits the companion DDI XML, the current parser cannot yet do full open extraction on its own.
- **This is a data-conversion tool, not an RDF packager.** If your goal is specifically DDI / RDF export via the official legacy toolchain, the IHSN wrapper may still be useful - but it still requires `NesstarExporter.exe`.
- **Legacy ecosystems are inconsistent.** Different institutions used different Nesstar-era conventions, so community test cases from outside India are especially valuable.

---

## Documentation

- [`docs/TECHNICAL.md`](docs/TECHNICAL.md) - binary format notes and implementation details
- [`docs/global-coverage.md`](docs/global-coverage.md) - institutions, countries, archives, and source links

---

## Testing

```bash
python -m pip install -e ".[dev]"
pytest tests/ -v
```

CI runs unit tests on Python 3.10-3.13 and checks formatting with Ruff.

---

## Contributing

Good contributions for this project:

- Test the converter on non-MoSPI Nesstar files
- Report datasets that still circulate as `.Nesstar` / `.NSDstat`
- Share evidence of legacy Nesstar repositories or migrations
- Improve metadata recovery for archives that omit `ddi.xml`

Community testing requests are tracked in the issue tracker, including:

- Stats SA GHS
- UK Data Archive legacy Nesstar packages
- World Bank / IHSN LSMS-style Nesstar corpora

---

## Citation

If you use this tool in research, please cite it using [`CITATION.cff`](CITATION.cff).

---

## License

[MIT](LICENSE)
