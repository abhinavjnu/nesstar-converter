# ADR 0004: Defer Stata output

## Decision

Stata output is deferred from the first production Rust release. The worker
must return a clear unsupported-format error when it is requested.

## Evidence

WP-S2 found no Rust DTA writer that completed independent pandas and R/haven
round trips with leading-zero strings, Unicode, labels, and collision-safe
names.

## Consequences

WP-W4 does not begin until a replacement ADR names a validated writer. The
pipeline may proceed without this deferred format.
