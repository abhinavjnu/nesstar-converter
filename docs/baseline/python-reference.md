# WP-B0 Python Reference Baseline

**Recorded:** 2026-07-13  
**Host:** macOS 26.5.1 (Build 25F80), ARM64  
**Plan:** [`../RUST_MIGRATION_PLAN.md`](../RUST_MIGRATION_PLAN.md)

## Repository state

The working tree was already dirty when the Rust migration plan began. No files listed below were reverted or committed.

```text
 M pyproject.toml
?? .DS_Store
?? .github/workflows/build-gui.yml
?? docs/RUST_MIGRATION_PLAN.md
?? gui/
?? handoff.md
```

Interpret these paths as existing user/prototype work. Migration agents must not delete or reset them.

## Python environment

System Python and isolated verification environment both reported Python 3.14.6.

The `.verify_env` environment contained:

```text
altgraph==0.17.5
cramjam==2.11.0
et_xmlfile==2.0.0
fastparquet==2026.5.0
fsspec==2026.6.0
iniconfig==2.3.0
macholib==1.16.4
nesstar-converter==1.0.4
numpy==2.5.1
openpyxl==3.1.5
packaging==26.2
pandas==3.0.3
pip==26.1.2
pluggy==1.6.0
pyarrow==25.0.0
Pygments==2.20.0
pyinstaller==6.21.0
pyinstaller-hooks-contrib==2026.6
PySide6_Essentials==6.11.1
pytest==9.1.1
python-dateutil==2.9.0.post0
setuptools==83.0.0
shiboken6==6.11.1
six==1.17.0
tqdm==4.68.4
```

`fastparquet`, `cramjam`, and `fsspec` were installed during an exploratory compatibility check after the recorded PyInstaller bundle was built. The current application still uses and bundles PyArrow; those three packages are not evidence of application support or an approved dependency change.

## Test baseline

Command:

```bash
.verify_env/bin/python -m pytest -q -rs
```

Result:

```text
48 passed, 23 skipped in 1.48s
```

Skip causes:

- Real EUS Nesstar/DDI data was unavailable.
- Official text exports were unavailable.
- PLFS resource-indexed data and exports were unavailable.

The existing test defaults reference Linux paths under `/media/abhinav/Data`, which do not exist on this macOS host. `NESSTAR_TEST_DATA` was not set.

Therefore this baseline proves only the available unit and synthetic behavior. It does **not** prove end-to-end parity against real survey data on this host.

## Real-data availability

The following default test inputs were checked and were absent:

```text
/media/abhinav/Data/MOSPI/data/eus/1983/Nss38_10_new format/survey0/data/NSS_38_SCH_10_EMP_UNEMP.Nesstar
/media/abhinav/Data/MOSPI/data/eus/1983/Nss38_10_new format/survey0/data/ddi.xml
/media/abhinav/Data/Datasets/PLFS/DDI-IND-CSO-PLFS-2023-24.Nesstar
```

No real-data hashes could be recorded. WP-F0 must create redistributable synthetic fixtures. Later real-data parity remains a protected/local qualification requirement.

## Packaged application baseline

The optimized Python/PySide6 application built successfully before this baseline.

```text
231M  dist/Nesstar Converter.app
231M  dist/NesstarConverter
```

Largest packaged components:

```text
43.1 MiB  pyarrow/libarrow.2500.dylib
22.3 MiB  pyarrow/libarrow_flight.2500.dylib
14.5 MiB  pyarrow/libarrow_compute.2500.dylib
8.7 MiB   PySide6/QtWidgets.abi3.so
8.1 MiB   QtGui.framework/QtGui
6.4 MiB   PySide6/QtGui.abi3.so
5.8 MiB   QtWidgets.framework/QtWidgets
5.8 MiB   QtCore.framework/QtCore
5.6 MiB   PySide6/QtCore.abi3.so
5.2 MiB   Python.framework/Python
```

This confirms that the remaining bundle size is dominated by the Python data stack and native Qt/PySide components, not application source code.

## Idle-process probe

The packaged GUI was launched with the Qt offscreen platform and sampled after two seconds.

```text
process_alive_after_2s=True
rss_kib=84608
```

That is approximately 82.6 MiB resident memory. This is an offscreen smoke measurement, not a production GUI benchmark and not an exact startup-time measurement.

## Frozen worker smoke test

The packaged executable successfully dispatched the converter CLI path and listed all eight formats:

```bash
'dist/Nesstar Converter.app/Contents/MacOS/NesstarConverter' formats
```

Supported formats observed:

- parquet
- csv
- tsv
- excel
- stata
- json
- jsonl
- fwf

## Baseline limitations

- No real Nesstar files were available.
- No official text exports were available.
- Linux package size and runtime were not measured on this host.
- Accessibility was not measured.
- Offscreen RSS is only an approximate comparison point.
- The current working tree is not a clean release commit.

## WP-B0 decision

WP-B0 is complete for the available host. WP-F0 may proceed, with synthetic fixtures treated as mandatory and real-data parity explicitly deferred to protected/local qualification.
