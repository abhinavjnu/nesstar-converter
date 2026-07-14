# GUI and renderer spike (WP-S1)

**Recorded:** 2026-07-14  
**Host:** macOS 26.5.1 (25F80), Apple Silicon (`arm64`)  
**Scope:** two deliberately isolated, non-converting eframe applications under `spikes/gui/`.

## What was built

Both variants provide the same representative screen, initialized with two queued `.Nesstar` files so the populated-queue state is visible without private data:

- drag-and-drop target and native file picker;
- queue with per-file removal controls;
- eight format checkboxes;
- native output-folder picker using `rfd`;
- progress label/bar, activity log, abort button, and result list;
- no conversion code, worker process, source-file reads, or network behavior.

The wgpu crate uses the glow crate's source with `include!` on purpose: this keeps all application code byte-for-byte equivalent and changes only the eframe renderer feature. It is an isolated Cargo package, not a root workspace member.

| Variant | Package | Renderer feature |
| --- | --- | --- |
| Glow | `spikes/gui/eframe-glow` | `eframe/glow` |
| Wgpu | `spikes/gui/eframe-wgpu` | `eframe/wgpu` plus `wgpu/metal` and `wgpu/vulkan` |

## Exact dependency selection

Recommended renderer: **`eframe/glow`**.

Use the following exact features when the production workspace is created:

```toml
eframe = { version = "=0.33.0", default-features = false, features = ["accesskit", "default_fonts", "glow"] }
egui = { version = "=0.33.0", default-features = false, features = ["accesskit", "default_fonts"] }
rfd = { version = "=0.15.4", default-features = false, features = ["xdg-portal"] }
```

`accesskit` is explicit because the product requires controls and progress to be exposed to the accessibility tree. The `rfd` portal feature avoids adding GTK to the Linux dependency closure while keeping native Cocoa dialogs on macOS.

Rejected alternative: **`eframe/wgpu`**. It remains functionally represented in this spike but is not recommended for the initial product: its 11.3 MB standalone executable is 5.4 MB (93%) larger than glow's 5.9 MB executable, and it requires explicit `metal`/`vulkan` backend selection when eframe defaults are disabled. The first wgpu launch aborted before a backend was enabled; the corrected manifest adds:

```toml
wgpu = { version = "=27.0.1", default-features = false, features = ["metal", "vulkan"] }
```

After that correction it built, tested, and launched. This is still not a packaged `.app`/DMG measurement, so WP-D0 must retain the package-size gate.

## Measurements and checks

Validation used Cargo 1.97.0 and Rust 1.97.0 on the host described above. Both manifests resolved their own `Cargo.lock`, passed their focused unit test, and built release executables. Values below are direct standalone executable sizes, not packaged app sizes.

```bash
/Users/abhishekmaurya/.cargo/bin/cargo --version
/Users/abhishekmaurya/.cargo/bin/rustc --version
```

The first launch of the wgpu variant exposed an essential configuration detail: `eframe/wgpu` alone is insufficient with `default-features = false`; it panicked with “No wgpu backend feature that is implemented for the target platform was enabled.” Adding the direct, pinned `wgpu` dependency above fixed the launch. No production files were changed.

| Check | glow | wgpu | Status / limitation |
| --- | ---: | ---: | --- |
| Standalone release executable size | 5,885,888 bytes (5.6 MiB) | 11,341,680 bytes (11 MiB) | `stat`/`du`; no `.app` bundle or DMG was produced. |
| Process-observed launch latency | 0.008 s | 0.013 s | Time from `exec` to the process becoming observable, not a UI-ready/cold-start measurement. |
| Idle RSS after 2 s | 111,152 KiB (108.5 MiB) | 106,976 KiB (104.5 MiB) | One local sample each; includes macOS graphics/runtime allocation and is above the 100 MB target. |
| Keyboard navigation | Not interactively measured | Not interactively measured | Controls are named in the eframe source. The raw executable has no macOS app registration, so the host UI inspector could not target it for Tab/Space/Enter automation. |
| Accessibility tree | Not interactively measured | Not interactively measured | `accesskit` compiled, including `accesskit_macos`, but the raw executable is not discoverable by the available macOS accessibility automation interface. VoiceOver/Accessibility Inspector check remains mandatory on a packaged app. |
| Drag and drop | Not interactively measured | Not interactively measured | Code compiled and accepts egui dropped-file events filtered to `.Nesstar`; an OS drag/drop smoke test remains required. |
| Native file/folder dialog | Not interactively measured | Not interactively measured | Code compiled and invokes synchronous `rfd::FileDialog`; a Cocoa dialog smoke test remains required. |

Linux checks are **unavailable**: this work package has access only to the macOS ARM64 host, with no Linux host, VM, container GUI session, or Linux artifact supplied. Linux release size, startup/RSS, accessibility, drag/drop, and native-dialog checks are not measured.

## Commands actually run

```bash
/Users/abhishekmaurya/.cargo/bin/cargo test --manifest-path spikes/gui/eframe-glow/Cargo.toml
/Users/abhishekmaurya/.cargo/bin/cargo test --manifest-path spikes/gui/eframe-wgpu/Cargo.toml
/Users/abhishekmaurya/.cargo/bin/cargo build --manifest-path spikes/gui/eframe-glow/Cargo.toml --release
/Users/abhishekmaurya/.cargo/bin/cargo build --manifest-path spikes/gui/eframe-wgpu/Cargo.toml --release
stat -f '%N %z bytes' spikes/gui/eframe-glow/target/release/nesstar-gui-spike-glow spikes/gui/eframe-wgpu/target/release/nesstar-gui-spike-wgpu
du -h spikes/gui/eframe-glow/target/release/nesstar-gui-spike-glow spikes/gui/eframe-wgpu/target/release/nesstar-gui-spike-wgpu
```

Results: glow test `1 passed`; wgpu test `1 passed`; both release builds succeeded. The wgpu test/build emitted a Rust future-incompatibility warning for transitive `block v0.1.6`. `cargo fmt --check` and `cargo clippy` were attempted for both variants but could not run because the installed Rust toolchain lacks the `rustfmt` and `clippy` components. Release processes were launched for two seconds each and sampled with `ps`; the process-observed and RSS figures are recorded above. The available macOS Computer Use accessibility inspector rejected both the raw executable name and absolute path as invalid applications, so it could not expose a tree or drive keyboard interaction.

## Remaining validation

Run from repository root, recording architecture and toolchain version with the results:

```bash
for variant in spikes/gui/eframe-glow spikes/gui/eframe-wgpu; do
  cargo fmt --manifest-path "$variant/Cargo.toml" -- --check
  cargo test --manifest-path "$variant/Cargo.toml"
  cargo clippy --manifest-path "$variant/Cargo.toml" --all-targets -- -D warnings
  cargo build --manifest-path "$variant/Cargo.toml" --release
done
```

Install `rustfmt` and `clippy`, rerun the checks above, package the chosen renderer, then conduct a manual macOS VoiceOver/Accessibility Inspector, Tab/Space/Enter, drag/drop, and file/folder-dialog smoke test. Repeat on a supported Linux desktop before accepting the renderer ADR. A GUI-only artifact above 75 MB triggers the plan's architecture-review stop condition.

## Remaining risks

- The 104.5–108.5 MiB idle RSS samples exceed the plan's 100 MB target; remeasure a packaged build and investigate before accepting the GUI runtime gate.
- These are unbundled macOS ARM64 executable measurements, not app/DMG or Linux release measurements.
- Accessibility, keyboard navigation, drag/drop, and native dialogs remain unverified with real macOS assistive technology and input.
- The installed toolchain lacks `rustfmt` and `clippy`; the wgpu dependency graph also warns that transitive `block v0.1.6` will be rejected by a future Rust release.
- The spike lockfiles are intentionally local to the isolated packages; WP-P0 must resolve and commit the final application workspace lockfile.
