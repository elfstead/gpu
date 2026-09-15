# Native Metal backend and refactor handoff

Updated 2026-09-16 on `metal-backend`. ABI **12**; use matching headers, library
and callers. This is an experimental compute backend, not a portability claim.
The refactor was developed on Linux; native Mac acceptance remains outstanding.

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

Residency is private to the backend. A weak registry contains **live allocations
only**, removing entries on destruction. Compute encoders declare those resources;
root pointers remain opaque. `retain_buffer` means ownership retention, not a new
resource-access declaration language. This is still O(live allocations) per
dispatch, not a final residency-performance design.

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

## Modern baseline and remaining architectural work

This checkpoint explicitly gates execution on Apple Silicon / Apple7-family
features **and Metal 3** (macOS 13+). It retains the original classic command-buffer
implementation: separate encoders and native hazard tracking. It is **not Metal 4**.
The target direction is a modern Metal 4 implementation using native argument
tables, residency sets and explicit dependencies, not a permanent stack of old
and new compatibility paths. That replacement needs native SDK/GPU work; this
Linux refactor does not claim to have implemented or validated it.

Encoder-per-operation and full live-registry scans are private implementation
choices, not OGPU semantics. Replace them during that work without changing
caller-owned scheduling, pointer interpretation or completion ownership.

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

Also exercise native allocation failure (not just the deterministic oversize
rejection), pending polls, destruction of pending receipts and real driver error
diagnostics. The shared state tests cover abstract terminal failures; injecting a
cached error after real work drains is **not** native device-loss evidence.
Check Metal validation on the byte-copy path and indirect resource declarations
before running a consumer. No Metal performance, real ML consumer, images/graphics
or Metal 4 acceptance is claimed.

Linux verification and exact remaining coverage are recorded in [the plan](plan.md).
Apple-target `cargo check` can catch Rust errors here using `DOCS_RS=1` to skip the
SPIRV-Cross C++ build; it cannot validate Apple SDK linking, MSL compilation or GPU
execution and is not a substitute for the commands above.
