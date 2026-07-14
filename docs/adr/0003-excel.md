# ADR 0003: Use rust_xlsxwriter for Excel output

## Decision

Use `rust_xlsxwriter` with default features disabled for Excel output.

## Evidence

WP-S2's workbook was read successfully by openpyxl and preserved strings,
leading zeros, Unicode, and embedded newlines.

## Consequences

WP-W3 must use constant-memory mode and deterministically split sheets before
the Excel row limit. Those requirements remain release gates.
