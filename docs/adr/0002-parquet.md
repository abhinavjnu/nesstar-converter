# ADR 0002: Use Apache Parquet with Arrow UTF-8 arrays

## Decision

Use Apache Rust `parquet` and Arrow arrays with default features disabled.
Enable only the Parquet `arrow` and `snap` features. Every output field is a
non-null Arrow UTF-8 string and missing values are written as empty strings.

## Evidence

WP-S2 produced Parquet data that PyArrow and DuckDB both read with the expected
schema, column order, leading zeros, empty values, Unicode, and row count.

## Consequences

No dataframe dependency is allowed. WP-W2 must stream bounded batches and add
independent-reader integration tests.
