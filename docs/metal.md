# Native Metal 4 backend

Updated 2026-09-16 on `metal-backend`. ABI **12**; use matching headers, library
and callers. This is an experimental compute backend, not a portability claim.
The backend is validated natively on Apple M4 with macOS 26 and Xcode 26.5.

## Boundary and current implementation

Common Rust code owns range/copy/root/dispatch checks, one-shot recording rules,
deduplicated retention, terminal resource retirement, shader-description parsing,
C diagnostics and common ABI data types. Vulkan and Metal use the same submission
state: preparation owns resources, accepted work keeps them, terminal observation
releases them while preserving the result. Transient Vulkan polling errors do not
grant release permission. Native encoders, submission, draining and timing remain
backend responsibilities; there is no trait that mirrors either native API.
The platform C entry-point implementations are still separate; this is a first
contract extraction, not a completed unification of the entire runtime.

Metal implements HOST/DEVICE buffers, GPU addresses, prepared compute kernels,
XYZ dispatch, copies, batches, barriers, explicit retention, poll and wait.
Graphics, images/heaps and timestamps remain unsupported. Creation outputs are
cleared on failure. Allocation checks `maxBufferLength` and the nullable native
buffer result before constructing an owning Rust wrapper. Oversize roots are
rejected before shader compilation; workgroups are checked against pipeline limits.

`OgpuDeviceInfo.backend` identifies Vulkan or Metal explicitly. Vulkan API-version
fields and Vulkan-only feature-audit flags are zero on Metal. In particular,
`timeline_semaphore` and `synchronization2` are not portable tests for OGPU completion
and barrier behavior. The common execution contract guarantees that behavior.
Native numeric capabilities remain distinct from storage-access capabilities.

Residency is private to the backend. A device residency set tracks allocations as
they are created and destroyed, so dispatch no longer scans every live allocation.
Metal 4 argument tables bind the root-data GPU address; embedded pointers remain
opaque. `retain_buffer` means ownership retention, not a new resource-access
declaration language.

Aligned copies use native blits. Unaligned offsets/lengths use a small cached MSL
byte-copy kernel, including disjoint self-copies; overlapping self-copies fail.
Validated zero-byte copies encode and retain nothing. This preserves OGPU's byte
contract despite [macOS blit alignment constraints](https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy%28from%3Asourceoffset%3Ato%3Adestinationoffset%3Asize%3A%29).

## Executable artifacts, not a mandated source language

`OgpuShaderDesc` now uses a byte blob, byte size, format, entry name and local size.
All inputs are consumed during creation; the ABI does not expose SPIRV-Cross types.

| Artifact | Vulkan | Metal | Metadata |
|---|---|---|---|
| SPIR-V | Native | Optional SPIRV-Cross adapter | `main`, zero `local_size`, existing 32-bit specialization |
| MSL source | Unsupported | Native compilation | Entry name, nonzero XYZ `local_size`, pre-specialized |
| metallib | Unsupported | Native library loading | Entry name, nonzero XYZ `local_size`, pre-specialized |

Native Metal root bytes occupy `constant ... [[buffer(0)]]`; embedded pointers are
native GPU addresses. Other argument bindings are not supplied. Native artifacts
currently reject specialization lists; they do not pretend that untyped
SPIR-V `{id,bits}` values specify every native function-constant type.
SPIR-V translation lives in `metal/spirv.rs`, outside native pipeline preparation.
Build with `--no-default-features` to omit that adapter and its C++ dependency.
Native copies and native executable tests do not depend on it.

## Metal 4 execution model

Execution requires Apple Silicon / Apple7-family features and the Metal 4 API
(macOS 26+). Each batch uses a Metal 4 command allocator and command buffer with a
single compute encoder for dispatches and copies. Barriers lower to explicit
dispatch/blit stage dependencies with device visibility. Queue timeline events
implement nonblocking poll, blocking wait and terminal resource retirement.
There is no classic-command-buffer compatibility path.

## Acceptance handoff

On Apple Silicon with Xcode's Metal compiler installed:

```sh
cargo test --locked -p ogpu
cargo test --locked -p ogpu metal:: -- --ignored --nocapture --test-threads=1
cargo test --locked -p ogpu --no-default-features
cargo test --locked -p ogpu --no-default-features metal::native_tests -- --ignored --nocapture --test-threads=1
cargo xtask smoke
cargo xtask compute
cargo xtask batch
cargo xtask reduction
cargo xtask retirement
```

Enable Metal API validation when running the native tests. They fail rather than
silently pass when a GPU/compiler is missing. Tests cover translated specialization,
MSL and compiled-metallib execution without translation, copied root data,
destroyed kernel handles, one-shot submission, repeated receipt observation,
buffer-size/root-size rejection, byte copies through private memory, overlap and
zero-copy rules, live-registry cleanup and cached-failure poll outputs.

Native acceptance covers pending polls, destruction of pending receipts, byte-copy
validation and argument-table residency. Native allocation failure and device-loss
injection remain environment-dependent. No Metal performance, real ML consumer, or
images/graphics acceptance is claimed.

Linux verification and exact remaining coverage are recorded in [the plan](plan.md).
Apple-target `cargo check` can catch Rust errors here using `DOCS_RS=1` to skip the
SPIRV-Cross C++ build; it cannot validate Apple SDK linking, MSL compilation or GPU
execution and is not a substitute for the commands above.
