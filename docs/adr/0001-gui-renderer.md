# ADR 0001: Use eframe with the glow renderer

## Decision

The production GUI will use `eframe = 0.33.0` with default features disabled
and `accesskit`, `default_fonts`, and `glow` enabled. `rfd` will provide native
file and folder dialogs with minimal platform features.

## Evidence

WP-S1 measured a 5.6 MiB glow spike executable versus 11 MiB for wgpu. Glow
also avoided the explicit backend configuration required by wgpu. Both variants
exceeded the idle-memory target in the raw-spike measurement, so WP-G1 and
WP-R1 must remeasure the packaged application and retain accessibility tests.

## Consequences

No wgpu dependency is approved for production. The GUI process remains only a
job launcher and event viewer; conversion stays in the worker process.
