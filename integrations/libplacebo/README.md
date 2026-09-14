# libplacebo consumer — bounded OGPU execution

This directory contains a bounded OGPU backend and its independent upstream Vulkan
reference, plus declaration-lowering and executable-preparation audits. The selected
compute/raster workflow passes on RADV and llvmpipe; this is **not a general libplacebo
backend**. See the [consumer brief](../../docs/consumer-libplacebo.md). Use matching
ABI-10 headers/library: specialization, triangle strips, grids/images/uploads and
cached limits remain; enabled capabilities and exact image support are now queryable.
Startup validates the bounded format/usage combinations before publishing its format
table. Image creation still checks the actual extent; this is not a general format catalog.

Each pass keeps its last completed receipt while rewriting heaps/vertex data for
the next frame. ABI 9 retires submission resources during wait, not receipt destruction.
The consumer checks 12 such reuses; the A/B/A specialization check verifies two.
Ordinary calls remain synchronous per operation. The separate
[two-frame checkpoint](../../docs/libplacebo-inflight.md) adds bounded, explicit
consumer-managed slots; the public OGPU runtime remains ABI 10.

## Reproduce

Linux x86-64 prerequisites: C/C++ compilers (C11/C++20 for upstream, C++17 for the
audit tools), Meson >=1.3, Ninja, pkg-config, Python 3 with Jinja2/MarkupSafe,
Rust/Cargo for OGPU, Vulkan 1.4 headers/loader and registry, shaderc development files, `glslc`,
SPIRV-Tools, Git, Bash and ripgrep. Locally tested: Clang 21.1.8, Meson 1.10.2,
shaderc 2026.1/glslang 16.4.0 and SPIRV-Tools 1.4.357.0.
The repository's pinned Vulkan-Headers submodule must be initialized.

```sh
git clone https://github.com/haasn/libplacebo.git target/libplacebo-source
git -C target/libplacebo-source checkout --detach 3330a515d62139259c26239014f286e233bd3a5c
VK_DRIVER_FILES=/path/to/one/icd.json \
    VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
    bash integrations/libplacebo/run-consumer.sh target/libplacebo-source
```

Select one driver with `VK_DRIVER_FILES` when comparing physical and software
execution. The acceptance runner requires a single ICD file; use an ICD exposing
one device to ensure reference and OGPU select the same GPU. The scripts do not
install layers or prove they activated; validation
must be available in the environment. The reference chooses an upstream Vulkan
device; it does not use OGPU probe indices or `OGPU_VULKAN_LIBRARY`. The consumer
uses OGPU device index 0 and honors `OGPU_VULKAN_LIBRARY`; use the same loader build
for both when overriding it.

The script verifies the source revision and absence of tracked upstream changes,
builds a separate shared libplacebo with its Vulkan backend, and runs three extents
through A/B/A input updates. No submodule downloads or upstream edits are needed;
Jinja and Vulkan headers are supplied by the build environment. Builds and captures
live under ignored `target/`. Do not run multiple builds in that directory at once.
The runner checks adapter rejection/cleanup, runs the independent reference and
consumer, then compares both intermediate and final images at the predeclared RGB
tolerance of 2/255 with exact alpha. Both tested drivers currently match exactly.
The runner additionally executes 36 frames in synchronous control and two-slot
mode, comparing all intermediate/final images to the same reference's repeated
A/B/A inputs. It checks allocations stabilize after warmup and reports waits,
polls, callbacks and resource high-water marks. No wall-time speedup claim.

## Bounded frame scheduling

The [resident-gap diagnosis](../../docs/libplacebo-diagnosis.md) uses separate
diagnostic binaries, leaving the ordinary runtime and benchmark unchanged:

```sh
# Baseline capture/timestamps; optional final "verify" skips timing.
bash integrations/libplacebo/run-profile.sh /path/to/pinned/libplacebo
# Radeon placement control: requires an eligible non-host-visible local image type.
bash integrations/libplacebo/run-image-placement.sh /path/to/pinned/libplacebo
```

Both expect one ICD and the existing build environment. `run-diagnostics.sh`
tests the isolated optimized-compiler variant; that variant currently fails the
SPIR-V validation gate and is intentionally not adopted. Command omissions via
`OGPU_DIAGNOSTIC_OMIT` are diagnostic-only and must not be called workload throughput.
See the brief for boundaries, raw results and the recommended allocator correction.

For the separate [performance comparison](../../docs/libplacebo-performance.md),
`ogpu_pl_frame_begin_batched` uses the same slots/banks but records one batch until
frame end. One frame receipt retires all commands; a failed frame discards its
unsubmitted batch before clearing banks/delivering failure callbacks. The normal
`ogpu_pl_frame_begin` retains per-operation submission as the diagnostic control.
Both use unchanged runtime ABI 10.

Native source uploads use explicit reusable host-visible staging through the
public libplacebo buffer API; this is not its default pointer-upload heuristic.

```sh
# One driver at a time; validate every extent/mode, then measure without layers.
bash integrations/libplacebo/run-perf.sh target/libplacebo-source
# Software correctness only, with no measured runs:
bash integrations/libplacebo/run-perf.sh target/libplacebo-source verify
```

`perf.c` runs native Vulkan and both OGPU policies with the same upstream
processing helper. Its in-memory verification avoids large disk captures. The
runner checks all configurations with validation, then runs three fresh processes
per configuration, rotating engine order, with eight warmup and 64 timed frames.
Inputs are prepared before timing; timed frames perform no output validation,
logging or file I/O. Resident mode has no timed image transfers; transfer mode
uploads input and reads back intermediate/final outputs. Setup/warmup are separate.
`summary.txt` contains median/range results; raw `PERF` rows and an environment
manifest remain under the printed ignored directory. All per-frame `*_ms` metrics
are averages except latency p50/p95; setup/warm/elapsed are whole intervals.
Latency is host-observed collection latency, not GPU or presentation latency.
Native submissions/waits/polls use -1 for unavailable; native staging payload 0
means unmeasured, not zero allocation. Common texture payload excludes the LUT;
OGPU staging includes it. RSS is process memory, not VRAM. See the brief for limits.

`stream.c` owns two source/intermediate/output texture sets and one shared
upstream dispatch cache/LUT per extent. `ogpu_pl_frame_begin(gpu, slot)` selects
slot 0 or 1; uploads, passes and callback readbacks submit immediately, without
waiting. `ogpu_pl_frame_end` closes the frame. Both slots are submitted before
the older one is collected; reuse requires `ogpu_pl_frame_collect` first.
These helpers live in the adapter header, not `include/ogpu.h`.

Each frame has a fixed eight-operation receipt array. Each pass has two mutable
heap/vertex banks and shared prepared executables/immutable indirect arguments.
Texture staging is separate per used slot. A bank or staging allocation may be
used only once per frame; excess operations and premature reuse fail closed.
This fixed limit describes this integration, not a general scheduler.

Collection polls the last receipt and optionally waits for it, then observes
every earlier receipt to release its retained resources before clearing banks.
It copies readbacks and fires callbacks only after retirement. Uploads copy the
caller bytes before returning; readback destinations and callback state must
remain valid until callback delivery. Callbacks are serialized, must not reenter
this adapter, and signal lifetime release even on failure; callers must check
`pl_gpu_is_failed` before interpreting output. No background progress thread.

`pl_tex_poll` can collect relevant closed slots; finite timeouts are nonblocking
polls, while `UINT64_MAX` permits waiting. `pl_gpu_flush` needs no extra submission.
Finish and child destruction drain outstanding frames (closing a partial frame
if necessary), including failed frames. Outside explicit frames, operations drain
pending slots and use the synchronous control path. Keep device teardown after
all children. The two-slot harness does not destroy resources in its steady loop.

## Independent preparation audits

For a reference-only capture, run
`bash integrations/libplacebo/run-reference.sh target/libplacebo-source`.

For each reference run, the printed artifact directory contains original generated
GLSL, a resource/constant/grid manifest, output RGBA bytes and the execution log.
Use that directory (not the example placeholder below) for the compiler gate:

```sh
bash integrations/libplacebo/check-compiler.sh target/libplacebo-integration/reference.XXXXXXXX
target/libplacebo-integration/compare-reference /path/to/radv-capture /path/to/llvmpipe-capture
```

The compiler gate prints its own output directory. Pair it with the capture it
was generated from to create the six executables with their actual constants:

```sh
bash integrations/libplacebo/check-executables.sh \
    target/libplacebo-integration/reference.XXXXXXXX \
    target/libplacebo-integration/compiler.YYYYYYYY
```

This last check needs the OGPU modern graphics baseline and uses
`OGPU_VULKAN_LIBRARY`. It submits no GPU work. Regenerate captures from older
checkpoints: the compiler gate now requires captured vertex metadata.

## What each check verifies

- Upstream `pl_shader_sample_polar` with EWA Lanczos produces a compute pass.
  Upstream nearest sampling then produces a fragment pass. The reference executes
  both through libplacebo's Vulkan backend; the consumer uses OGPU. Neither uses a
  hand-written filtering implementation.
- Inputs `16x16`, `31x17`, `64x33` upscale to `33x33`, `63x35`, `129x67`.
  A/B/A updates give repeatable A outputs and changed B outputs. Intermediate and
  final pixels match exactly for the nearest output pass; alpha is 255 throughout.
- Six compiled passes are reused across nine compute and nine raster invocations.
  Textures/dispatch/LUT state are reused per extent and explicitly destroyed.
- Capture intercepts pinned private `pl_gpu_fns` callbacks without changing their
  implementation. The host-vertex upload helper re-enters `pass_run`, so only the
  outer invocation counts as a dispatched operation. This harness is serialized.
- `heap-lower` changes known resource declarations into native image/sampler arrays
  and replaces the two known vertex inputs with address-pulled aliases. The original
  four 16-byte vertex records form a triangle strip. Vertex lowering adds an 8-byte
  address root to the originally root-free raster stage; the 56-byte compute root,
  processing bodies, workgroup declarations and specialization declarations stay
  unchanged. Unsupported declarations/layouts fail; this is not a general translator.
- `glslc` compilation and SPIR-V validation succeed, with native heap capabilities
  and no descriptor-set/binding decorations. Tests cover body preservation, slot
  mapping and rejection. Compiler version is recorded with each output.
- The executable audit creates three kernels and three raster programs through
  OGPU using the captured 32-bit specialization values. Separate public-API GPU
  tests execute specialization and the same vertex lowering with original test
  shaders; these preparation checks do not substitute for consumer acceptance.
- `run-consumer.sh` executes the shared workload through the bounded adapter:
  upstream LUT generation, sampled images, compute output, then a real raster pass.
  The consumer creates no upstream Vulkan device. Both stages' downloaded images
  are compared to the reference; A/B/A updates and exact nearest copying are checked
  inside each executable. Counters require actual submissions, resource reuse and
  zero live texture/pass objects at shutdown.
- `check-backend.sh` exercises partial-upload, unsupported-image/shader and live-child
  rejection. It expects a deliberate abort for the last contract violation; the
  other cases check failed-state reporting and clean resource destruction. A separate
  A/B/A specialization test updates one live pass and checks its pixels after each
  executable replacement, then verifies clean teardown.
- Frame tests reject premature slots, repeated staging/bank use and operation
  overflow, drain partial-frame failures and queued child destruction, and replace
  specialization executables while two slots remain uncollected. Test-only link
  wrappers force a pending poll, transient poll error and submission rejection;
  real GPU receipts still drain before callbacks/resource release. Failed-frame
  callbacks do not make poisoned output valid. No injection code is linked into
  the normal consumer or streaming executable.

The compiler probe uses fixed per-pass heap slots corresponding to original binding
numbers. Neither audit binds heaps or executes the upstream binaries. The compiler
output retains GLSL defaults; the executable audit supplies the captured values at
creation, including shared-array sizes. Those values are mandatory for this
processing configuration. The consumer uses the same lowering helpers but compiles
its own upstream-generated shaders live; it does not replay capture files or reuse
the reference's output. Executable preparation and G2 execution remain separate gates.

The adapter supports only the selected push-root/image workflow. It does not provide
general buffers, UBOs, global uniforms, upstream emulation, partial transfers, indexed
draws or timers. Callback transfers use the bounded protocol above, not a general
asynchronous backend. Private object/table layout is pinned; no private allocator
or finalizer symbols are linked. This narrow push-only profile is not a replacement
for upstream's general backend capability contract. Unsupported operations fail,
with no alternate execution backend. Scheduling is externally serialized and each
slot is collected before staging or heap-bank reuse.

## Licensing boundary

libplacebo is LGPL-2.1-or-later. Its source stays in the separate checkout and is
built as a shared library; its license/notices remain upstream. Generated shader
text contains upstream processing logic: it is an ignored build artifact here,
not redistributed as an MIT shader fixture. The OGPU repository's MIT license
does not relicense libplacebo. Packaging/distributing the combined integration
must preserve the dependency's applicable licensing requirements.
