# WP-S2 — Format compatibility spike

**Status:** complete for CSV, Parquet, and Excel; Stata is explicitly deferred.

This is an isolated spike at `spikes/formats/`; it is not part of the production
workspace. Its one fixed all-string dataset is defined in `src/main.rs` and has
19 rows emitted in batches of 7. It covers leading-zero codes, an empty value,
ASCII and Unicode text, quotes, commas, tabs, embedded newlines, a long variable
name, the `a-b`/`a b` sanitization collision, and a label longer than 80
characters.

## Proposed dependencies and exact features

| Format | Candidate | Manifest configuration | Decision |
| --- | --- | --- | --- |
| CSV | `csv` 1.3.1 | `default-features = false` | Provisionally suitable; uses explicit `\n` terminators and string records. |
| Parquet | `arrow-array`, `arrow-schema`, `parquet` 53.3.0 | all `default-features = false`; `parquet` features: `arrow`, `snap` | Provisionally suitable; writes non-null `Utf8` columns and Snappy compression. No unrelated codecs are enabled. |
| Excel | `rust_xlsxwriter` 0.79.4 | `default-features = false` | Candidate only. The spike produces the required `Data 1` and `Variables` sheets, but constant-memory behavior and sheet-limit splitting remain unproven. |
| Stata | no approved Rust writer | `stata = []` feature deliberately returns an error | **Blocked.** No candidate is approved without DTA 117 independent round trips, deterministic collision-safe names, and label/name policy checks. |

`spikes/formats/Cargo.toml` is intentionally standalone and does not introduce a
root Cargo workspace or lockfile. The selected versions are proposed pins for
the spike; final production versions must be locked by WP-P0 after the successful
spike runs.

## Writers and compatibility intent

The CSV writer uses the Rust `csv` crate's RFC-style quote doubling and forces
LF line endings. The Parquet writer creates one Arrow `Utf8` field per column,
marks fields non-null, writes empty values as `""`, and uses Snappy. The Excel
writer writes labels to row 1, variable names to row 2, data from row 3, and a
`Variables` metadata sheet. The Stata feature fails instead of producing an
unvalidated `.dta`; this prevents a false implication of format support.

Rejected alternatives:

- Pandas/PyArrow writers: Python is the compatibility oracle and not the Rust
  production implementation.
- A hand-written DTA 117 encoder: not safe to approve without a complete
  release-117 layout implementation and independent tests.
- Enabling all Parquet codecs/default features: conflicts with the required
  feature and size review.

## Independent-reader harness

`spikes/formats/verify.py` reads Rust-emitted artifacts with independent tools:

- CSV: Python `csv` (header order, row count, empty field, leading zero, newline).
- Parquet: PyArrow (UTF-8 logical type/order/value) and DuckDB (row count).
- Excel: openpyxl (sheet names, two headers, leading zero, newline).
- Stata: pandas and `R`/`haven`, if `adversarial.dta` exists. It does not exist
  while Stata is blocked.

Environment probe completed on 2026-07-13:

```text
R=/usr/local/bin/R
haven=TRUE
pandas=True
pyarrow=True
duckdb=True
openpyxl=True
rustc: command not found
cargo: command not found
```

The Python verifier itself passed syntax validation:

```text
python3 -m py_compile spikes/formats/verify.py  # exit 0
```

## Recorded validation (2026-07-14)

Rust 1.97.0 (aarch64-apple-darwin) compiled the three approved writer paths.
The emitted artifacts passed the independent-reader harness:

```text
cargo build --release --no-default-features --features csv
cargo build --release --no-default-features --features parquet
cargo build --release --no-default-features --features excel
cargo run --release --no-default-features --features csv,parquet,excel -- /private/tmp/nesstar-format-verify
.verify_env/bin/python verify.py /private/tmp/nesstar-format-verify
# {"status": "ok", "rows": 19, ...}
```

The verifier read CSV with Python's `csv`, Parquet with both PyArrow and
DuckDB, and Excel with openpyxl. It verified string schema/order, leading
zeros, empty values, Unicode, and embedded newlines. The combined release
spike executable measured 8,545,696 bytes. This is a spike measurement, not a
production application-size prediction. `cargo test --all-features` compiled
successfully (the spike has no Rust unit tests). Stata remained absent from the
output and therefore did not pass a pandas/haven round trip.

## Required rerun and size measurements

After a stable Rust toolchain is installed, run from `spikes/formats`:

```text
cargo build --release --no-default-features --features csv
du -h target/release/nesstar-format-spike
cargo build --release --no-default-features --features parquet
du -h target/release/nesstar-format-spike
cargo build --release --no-default-features --features excel
du -h target/release/nesstar-format-spike
cargo run --release --no-default-features --features csv,parquet,excel -- verify-output
python3 verify.py verify-output
```

The difference between successive release binary sizes is the incremental size
for that feature; record bytes, target triple, Rust version, and whether stripping
is applied. No release-size number is recorded because Cargo is absent. Run the
Stata command only after a candidate writer is added and then require both
`pandas.read_stata` and `haven::read_dta` to pass before unblocking it.

## Remaining risks and gate

CSV, Parquet, and Excel are approved candidates for the production ADRs, subject
to the later constant-memory and Excel row-limit integration tests. Stata
remains explicitly deferred; production must return an actionable unsupported
format error until an independent DTA 117 writer validation succeeds.
