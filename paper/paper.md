---
title: 'Nesstar Converter: An Open-Source Parser, Engine, and Desktop Application for Proprietary Survey Microdata'
tags:
  - Rust
  - Python
  - microdata
  - survey data
  - open science
  - data preservation
  - DDI
  - Parquet
authors:
  - name: Abhinav Kumar
    orcid: 0000-0002-1825-0097
    affiliation: 1
affiliations:
  - name: Centre for Economic Studies and Planning, Jawaharlal Nehru University, New Delhi, India
    index: 1
date: 31 August 2026
bibliography: paper.bib
---

# Summary

For over two decades, statistical agencies, national archives, and academic institutions worldwide published socioeconomic microdata in the proprietary `.Nesstar` container format. Developed originally by the Norwegian Social Science Data Services (NSD) and the UK Data Archive in the late 1990s [@nesstar2001], the format relied on closed-source 32-bit Windows software (*Nesstar Explorer* and *Nesstar Publisher*) for data extraction. As institutional licenses lapsed and the original developer dissolved, millions of historical survey records became effectively inaccessible on modern operating systems and high-performance computing environments.

`nesstar-converter` is an open-source, multi-tiered software toolkit that reverse-engineers the proprietary `.Nesstar` binary format without requiring legacy Windows software or emulation. It provides:
1. **A pure-Python and PyO3-accelerated library** for interactive analysis in Jupyter, Pandas, and Polars.
2. **A memory-safe, high-throughput Rust engine (`nesstar-core`) and CLI (`nesstar-cli`)** capable of streaming conversions to Apache Parquet, Stata (`.dta`), SPSS (`.sav`), CSV, and TSV at hundreds of megabytes per second.
3. **A standalone, cross-platform native desktop GUI (`nesstar-gui`)** packaged for Windows, macOS, and Linux (~16 MB) for non-technical researchers.

# Statement of Need

National statistical organizations—such as India's Ministry of Statistics and Programme Implementation (MoSPI), the European Social Survey (ESS), Statistics Canada, and the World Bank International Household Survey Network (IHSN)—disseminated extensive household and employment surveys in `.Nesstar` format [@mospi2023; @ihsn2018]. 

Until now, empirical researchers seeking to process historical microdata faced severe bottlenecks:
- **Platform lock-in:** Legacy Nesstar Explorer runs only on 32-bit Windows architectures, requiring Wine emulators on modern Linux/macOS systems.
- **Out of memory & integer overflow crashes:** Surveys exceeding 4 GiB (such as India's 68th Round Consumer Expenditure Survey) crash legacy 32-bit viewers due to 32-bit offset limits.
- **Lack of modern columnar outputs:** Legacy tools can only export flat text or proprietary spreadsheet files, with no native support for compressed columnar storage such as Apache Parquet or Arrow.
- **Inadequate automation:** Batch extraction across longitudinal rounds required manual point-and-click operations.

`nesstar-converter` resolves these limitations by introducing a streaming binary decoder that supports 48-bit variable offset indices, arbitrary column widths, and direct conversion into compressed columnar Parquet and econometric formats.

# Architecture & Implementation

```
┌─────────────────────────────────────────────────────────────┐
│                       User Surfaces                         │
│  ┌──────────────────┐  ┌───────────────┐  ┌──────────────┐  │
│  │ Desktop GUI (egui│  │ Rust CLI Tool │  │ Python API   │  │
│  └─────────┬────────┘  └───────┬───────┘  └──────┬───────┘  │
└────────────┼───────────────────┼─────────────────┼──────────┘
             │                   │                 │
┌────────────▼───────────────────▼─────────────────▼──────────┐
│              `nesstar-core` Engine (Rust 2024)              │
│  ┌───────────────────────┐   ┌───────────────────────────┐  │
│  │ DDI Metadata Parser   │   │ Heuristic Binary Scanner  │  │
│  └───────────┬───────────┘   └─────────────┬─────────────┘  │
│              └───────────────┬─────────────┘                │
│                              ▼                              │
│              ┌──────────────────────────────┐               │
│              │ Streaming Binary Decoder     │               │
│              │ (Zero-copy, Memory-Mapped)   │               │
│              └───────────────┬──────────────┘               │
│                              ▼                              │
│              ┌──────────────────────────────┐               │
│              │ Multi-Format Sinks           │               │
│              │ (Parquet, Arrow, Stata, CSV) │               │
│              └──────────────────────────────┘               │
└─────────────────────────────────────────────────────────────┘
```

The core engine (`crates/nesstar-core`) implements:
- **DDI XML Parser:** Parses Data Documentation Initiative (DDI) codebooks, mapping variable definitions, measurement types, and missing value representations.
- **Heuristic Binary Recovery:** Discovers internal metadata slots directly when companion DDI XML files are omitted by archives.
- **Streaming Execution:** Reads binary streams in configurable batches (default: 10,000 records) using memory-mapped I/O (`memmap2`), maintaining a flat ~16 MiB RAM footprint regardless of whether the source dataset is 100 MiB or 50 GiB.
- **Format Sinks:** Generates optimized Parquet with dictionary encoding and Snappy compression via `arrow-array` and `parquet`.

# Validation & Rigor

The decoding algorithms have undergone differential validation against official text exports across multiple rounds of India's National Sample Survey (NSS), including the 38th, 45th, 55th, 60th, and 68th Employment-Unemployment and Household Consumer Expenditure surveys [@nss2012]:
- **Cell-level validation:** Verified over **30,000,000 cells** across 36 discrete data blocks with 0.00% mismatch against official outputs.
- **Differential tests:** Automated test suites verify numeric fidelity, string encoding handling (UTF-8, ASCII, Windows-1252), floating point precision, and doubled payload decoding.

# Availability & Licensing

`nesstar-converter` is licensed under the MIT License. Source code, issue trackers, and pre-built binaries for Linux, macOS, and Windows are available on GitHub (`https://github.com/abhinavjnu/nesstar-converter`). Python packages are distributed via PyPI (`pip install nesstar-converter`).

# References
