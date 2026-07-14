# Rust migration fixture contract

`fixtures/` contains only synthetic values created for this project (CC0-1.0); it contains no restricted survey data. The Python converter is the oracle for each expected table.

Run `python tools/generate_rust_fixtures.py` to regenerate the fixtures, then `python tools/generate_rust_fixtures.py --check` to verify byte-for-byte reproducibility. `fixtures/manifest.json` records SHA-256 hashes, dimensions, layout method, and encoding coverage.

The two positive fixtures are deliberately small but cover both extraction paths:

- `synthetic/metadata-scan.Nesstar` plus its namespaced DDI: fixed-width ASCII, offset little-endian integers (including missing), and little-endian doubles (including NaN).
- `synthetic/resource-index.Nesstar` plus its non-namespaced DDI: fixed ASCII, NUL-terminated UTF-8, compact nibble/high-low ordering, unsigned 8/16/24/32/40-bit values, compact doubles, additive offsets, missing markers, and the raw-byte numeric heuristic.

`expected/*.json` and `expected/*.tsv` are decoded from those fixtures through `nesstar_converter.py`. JSON is the cell-by-cell contract; TSV makes the human-readable schema and ordering easy to inspect.

`malformed/` has a bad magic header, a metadata slot truncated by one byte, and a truncated resource index. They must produce errors rather than successful conversion.
