# P1 native Rust source handoff

Date: 2026-10-17. Accepted packaging implementation: `04fdfa3c5c1b29f0885cf298d086e44f562a4c31`.
The tested SDK was built from that **clean** revision. This closes the isolated
packaging/checklist work, not human P1 acceptance or M6 implementation.

## What ships

The normal installer, with or without `--legacy`, now includes
`share/ogpu/native-rust`: runtime and generated Vulkan-binding source crates,
standalone native Rust consumer, unchanged Cargo.lock, compute SPIR-V and textured
scene generated header, corresponding shader source, licenses and instructions.
A small root workspace preserves inherited package metadata without xtask.
The source bundle is approximately 2.6 MiB, excluding fetched registry dependencies.

The consumer's path dependency, build script and shader include paths are unchanged.
Copy the whole bundle. It builds OGPU directly as Rust, bypassing C entry points;
it does not link or require the SDK's `libogpu.so`. Do not mix handles with another
runtime instance. `source.json` records revision/dirty state; the enclosing SDK
manifest hashes all bundled files. Registry dependencies are pinned but not
vendored; Cargo needs network access or an existing cache. GLFW, Vulkan, the native
linker and Rust toolchain remain external. Installation now needs Python 3.11+
for standard-library TOML parsing. No shader compiler or Vulkan headers are needed
to build or execute this copied consumer.

This is a source handoff for Linux x86-64, not a crates.io publication, stable Rust
API, safe wrapper, portable binary, complete runtime test-source distribution,
third-party adoption or clean-machine acceptance.

## Isolated checks

`tools/test-rust-install.py` verifies packaged hashes, copies only the bundle to
a fresh space-containing path, and builds with `--locked --offline` and a fresh
target directory. It checks the resolved local package graph and disabled legacy
features; compiler depfiles must stay inside the bundle, including embedded shader
inputs/generated Rust. Link checks reject dependencies on the SDK shared library
or linked Vulkan/GLFW libraries (the latter remain explicitly loaded at runtime).
Ambient build flags/wrappers and library/header search paths are removed. This is
a dependency-boundary test, not an OS sandbox against hostile build scripts.

Each run retains command logs and a complete/incomplete JSON receipt; human
acceptance always remains separately marked unverified. No-GPU checks include
two consumer host tests and missing-Vulkan-loader rejection. Missing GLFW is tested
only with presentation: discovery precedes its loading, so otherwise an unrelated
discovery failure could falsely pass that negative check. GPU/display checks are
explicitly selected, with bounded direct child-process execution.

Clean SDK: `/tmp/ogpu-p1-rust-sdk-20261017`.

| Check | Result / evidence |
|---|---|
| Installer tests | 8 pass, including default/legacy source graph, hashes, no targets/xtask and source-symlink rejection |
| SDK profile tests | 1 pass |
| Isolated native Rust no-GPU build | `/tmp/ogpu-rust-external-8stpfs7r/report.json`, complete |
| Existing six C SDK examples, no-GPU | Relocated headers/library/build/failure checks pass; `/tmp/ogpu-p1-c-sdk-host-clean.log` |
| Isolated Radeon native Rust | `/tmp/ogpu-rust-external-4f08mvd9/report.json`, complete |
| Isolated llvmpipe native Rust | `/tmp/ogpu-rust-external-mkwg5j3k/report.json`, complete |

Both drivers pass headless resource/compute/image A/B/A, cache/descriptor checks,
binary/timeline handoffs, surface setup, missing-GLFW rejection, eight two-swapchain
presentation batches and old-chain/unused-image lifetime checks. Each passes
18-frame textured pixel checks (36,729 sampled pixels) and 18 frames without
readback, with three application-unretired slots, both requested framebuffer
resizes observed, synthetic zero-extent suspension and close with pending slots.
Compositor-driven recreation counts differ between check and smoke runs; these
are not timing or simultaneous-GPU-overlap claims.

Local environment: Rust 1.98.1, Python 3.14.7, clang 21 linker, external GLFW 3.4,
Wayland desktop, Radeon RX 5700 XT/RADV and llvmpipe 26.2.1; explicit ICD selection
and Khronos 1.4.357 synchronization validation. No validation diagnostics occurred.
Fresh compiler targets were used, but registry dependencies came from an existing
Cargo cache. No new runtime implementation changes were made by this checkpoint.
CI now configures a locked dependency fetch followed by isolated offline/no-GPU
acceptance; no hosted CI result is claimed.

## Remaining P1 acceptance and next work

The [manual checklist](../presentation-acceptance.md) is packaged. Subsequent
[human observations](p1-human-interaction-2026-10-17.md) confirm controls, resizing,
pause and close; resume was not tested and the compositor has no minimize support.
Detailed per-action coverage remains partial. Scripted camera changes, synthetic
zero extent and readback do not replace those observations. P1 is not marked fully
complete. Real surface/device-loss recovery, scanout accuracy and cross-family
presentation remain outside this bounded acceptance.

The next design work is M6's native Metal mapping, especially argument layout,
placement/residency, replay and presentation retirement. No Mac is available here;
native acceptance remains a separate handoff. An outstanding manual observation
does not justify a hidden workaround or block unrelated Linux hardware work.
