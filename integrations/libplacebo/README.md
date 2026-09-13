# libplacebo consumer — bounded OGPU execution

This directory contains a bounded OGPU backend and its independent upstream Vulkan
reference, plus declaration-lowering and executable-preparation audits. The selected
compute/raster workflow passes on RADV and llvmpipe; this is **not a general libplacebo
backend**. See the [consumer brief](../../docs/consumer-libplacebo.md). Use matching
ABI-8 headers/library: specialization, triangle strips, grids/images/uploads and
cached limits remain; enabled capabilities and exact image support are now queryable.
Startup validates the bounded format/usage combinations before publishing its format
table. Image creation still checks the actual extent; this is not a general format catalog.

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
No performance claim: the adapter waits after each upload/pass/readback.

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

The compiler probe uses fixed per-pass heap slots corresponding to original binding
numbers. Neither audit binds heaps or executes the upstream binaries. The compiler
output retains GLSL defaults; the executable audit supplies the captured values at
creation, including shared-array sizes. Those values are mandatory for this
processing configuration. The consumer uses the same lowering helpers but compiles
its own upstream-generated shaders live; it does not replay capture files or reuse
the reference's output. Executable preparation and G2 execution remain separate gates.

The adapter supports only the selected push-root/image workflow. It does not provide
general buffers, UBOs, global uniforms, upstream emulation, partial transfers, indexed
draws, callbacks or timers. Private object/table layout is pinned; no private allocator
or finalizer symbols are linked. This narrow push-only profile is not a replacement
for upstream's general backend capability contract. Unsupported operations fail,
with no alternate execution backend. Scheduling is externally serialized and each
operation is drained before staging or heap reuse.

## Licensing boundary

libplacebo is LGPL-2.1-or-later. Its source stays in the separate checkout and is
built as a shared library; its license/notices remain upstream. Generated shader
text contains upstream processing logic: it is an ignored build artifact here,
not redistributed as an MIT shader fixture. The OGPU repository's MIT license
does not relicense libplacebo. Packaging/distributing the combined integration
must preserve the dependency's applicable licensing requirements.
