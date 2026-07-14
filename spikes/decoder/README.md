# Decoder feasibility spike

This isolated Cargo package implements only WP-S3: synthetic DDI parsing,
metadata-scan and resource-index layouts, bounded record batches, parity
comparison, cancellation at a batch boundary, and a batch-owned-memory proxy.

Run from this directory:

```sh
cargo test
cargo run -- --batch 1
cargo run -- --batch 4
/usr/bin/time -l cargo run --release -- --batch 1
/usr/bin/time -l cargo run --release -- --batch 4
```

It intentionally has no dependencies and is not a production API. The source
is read into a `Vec<u8>` only because this small spike cannot introduce the
production reviewed `memmap2` wrapper before WP-P0/ADR approval.
