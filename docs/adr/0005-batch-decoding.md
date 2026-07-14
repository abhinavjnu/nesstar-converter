# ADR 0005: Decode in bounded batches from a reviewed memory-map wrapper

## Decision

Use random-access bounded batches and a single reviewed `memmap2` wrapper in
`nesstar-core::source`. Check every offset and length before reading.

## Evidence

WP-S3 passed parity for both fixture layouts at batches 1, 2, and 64, and its
owned decoded rows grew with requested batch size. Its whole-file test input is
explicitly not production design.

## Consequences

WP-P4 owns the source wrapper and must show memory bounded by batch size on a
larger generated fixture. No other module may map source files directly.
