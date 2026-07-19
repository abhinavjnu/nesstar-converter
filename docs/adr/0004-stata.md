# ADR 0004: Stata output

## Status

Accepted (supersedes original deferral)

## Decision

Stata DTA v118 output is implemented in the Rust converter via a custom writer
in `crates/nesstar-core/src/formats/dta.rs`. The writer produces files readable
by Stata 14+ and pandas.

## Context

WP-S2 initially deferred Stata because no existing Rust DTA crate passed
round-trip validation. A custom writer was subsequently implemented that handles
leading-zero strings, Unicode labels, and collision-safe variable names.

## Consequences

Stata (.dta) is available as an output format in both the CLI and GUI.
