# WP-S3 decoder feasibility spike

## Scope and outcome

The isolated package in `spikes/decoder/` parses the two synthetic DDIs
(including the namespaced DDI), selects one metadata-scan layout and one
resource-index layout, and decodes only a requested row batch at a time. It
compares the complete decoded row maps, including schema order through the
DDI-driven column traversal, to `fixtures/expected/*.json`.

The spike covers the fixture's fixed ASCII, NUL UTF-8, offset integer,
little-endian double, nibble (high nibble first), compact unsigned 8/16/24/32/
40-bit, additive offset, missing-marker, and raw-byte-numeric paths. Every
slice is range checked before it is read. The `cancellation_happens_between_batches`
test permits batch zero and declines batch one, proving the cooperative check
occurs at the batch boundary rather than after a whole block.

## Decisions

- The spike is a separate Cargo package, not a root workspace or production
  API. It has no dependencies, avoiding an unreviewed production dependency.
- Batch decoding allocates a row batch and appends it only in this parity
  harness. The `peak_owned_batch_bytes` output measures the values retained by
  that bounded batch; a production pipeline must instead pass each batch
  directly to writers and validators.
- The fixture source is read into `Vec<u8>` so the experiment stays
  dependency-free. This is not approval to use whole-file reads in production:
  WP-P4 must use the approved reviewed `memmap2` source wrapper and retain the
  checked range arithmetic demonstrated here.

## Validation and memory measurement

Run from `spikes/decoder` after Rust is available:

```sh
cargo test
cargo run -- --batch 1
cargo run -- --batch 4
/usr/bin/time -l cargo run --release -- --batch 1
/usr/bin/time -l cargo run --release -- --batch 4
```

The first two runs report the batch-owned peak for the two requested batch
sizes; the last two report macOS maximum resident set size. Record both values
in the batch-size ADR before WP-P0. Batch size 1 and 4 are deliberately used:
the resource-index fixture has five rows, so the latter exercises a partial
final batch. A sufficiently large repeated synthetic fixture is still needed
before this establishes the plan's large-data memory criterion.

### Results in this checkout

On 2026-07-14, Rust 1.97.0 (aarch64-apple-darwin) was installed and the
following validations passed:

```text
cargo test --manifest-path spikes/decoder/Cargo.toml
# 3 passed: parity at batches 1, 2, and 64; cancellation; malformed inputs

cargo run --manifest-path spikes/decoder/Cargo.toml -- --batch 1
# metadata peak_owned_batch_bytes=24; resource peak_owned_batch_bytes=71

cargo run --manifest-path spikes/decoder/Cargo.toml --release -- --batch 4
# metadata peak_owned_batch_bytes=88; resource peak_owned_batch_bytes=243
```

The release batch-1 run took 1.83 seconds including the initial release build.
`/usr/bin/time -l` could not report RSS in the sandbox because its `sysctl`
probe was denied, so no process-RSS figure is claimed. These small fixtures
show that owned decoded-row memory grows with batch size; they do not establish
the large-data memory target.

## Remaining risks

- The handwritten XML and expected-JSON readers are intentionally narrow and
  only suitable for the fixture contract. WP-P1 must replace them with the
  approved XML/serialization crates and malformed-XML tests.
- The spike does not test resource fallback after a resource-layout decoding
  failure, duplicate payloads, metadata width reduction, or files larger than
  addressable memory.
- Current allocations are bounded by batch size only for decoded rows; source
  bytes remain whole-file resident until WP-P4's memory-map wrapper exists.
