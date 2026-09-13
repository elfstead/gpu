# libplacebo consumer — reference/compiler gate

This directory currently contains **upstream Vulkan reference and compiler-audit
tools, not an OGPU backend**. See the [consumer brief](../../docs/consumer-libplacebo.md).
No runtime API or ABI changes are made at this gate.

## Reproduce

Linux x86-64 prerequisites: C/C++ compilers (C11/C++20 for upstream, C++17 for the
audit tools), Meson >=1.3, Ninja, pkg-config, Python 3 with Jinja2/MarkupSafe,
Vulkan 1.4 headers/loader and registry, shaderc development files, `glslc`,
SPIRV-Tools, Git, Bash and ripgrep. Locally tested: Clang 21.1.8, Meson 1.10.2,
shaderc 2026.1/glslang 16.4.0 and SPIRV-Tools 1.4.357.0.
The repository's pinned Vulkan-Headers submodule must be initialized.

```sh
git clone https://github.com/haasn/libplacebo.git target/libplacebo-source
git -C target/libplacebo-source checkout --detach 3330a515d62139259c26239014f286e233bd3a5c
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
    bash integrations/libplacebo/run-reference.sh target/libplacebo-source
```

Select one driver with `VK_DRIVER_FILES` when comparing physical and software
execution. The scripts do not install layers or prove they activated; validation
must be available in the environment. The reference chooses an upstream Vulkan
device; it does not use OGPU probe indices or `OGPU_VULKAN_LIBRARY`.

The script verifies the source revision and absence of tracked upstream changes,
builds a separate shared libplacebo with its Vulkan backend, and runs three extents
through A/B/A input updates. No submodule downloads or upstream edits are needed;
Jinja and Vulkan headers are supplied by the build environment. Builds and captures
live under ignored `target/`. Do not run multiple builds in that directory at once.

For each reference run, the printed artifact directory contains original generated
GLSL, a resource/constant/grid manifest, output RGBA bytes and the execution log.
Use that directory (not the example placeholder below) for the compiler gate:

```sh
bash integrations/libplacebo/check-compiler.sh target/libplacebo-integration/reference.XXXXXXXX
target/libplacebo-integration/compare-reference /path/to/radv-capture /path/to/llvmpipe-capture
```

## What is actually verified

- Upstream `pl_shader_sample_polar` with EWA Lanczos produces a compute pass.
  Upstream nearest sampling then produces a fragment pass. Both execute through
  libplacebo's existing Vulkan backend; no hand-written filtering implementation.
- Inputs `16x16`, `31x17`, `64x33` upscale to `33x33`, `63x35`, `129x67`.
  A/B/A updates give repeatable A outputs and changed B outputs. Intermediate and
  final pixels match exactly for the nearest output pass; alpha is 255 throughout.
- Six compiled passes are reused across nine compute and nine raster invocations.
  Textures/dispatch/LUT state are reused per extent and explicitly destroyed.
- Capture intercepts pinned private `pl_gpu_fns` callbacks without changing their
  implementation. The host-vertex upload helper re-enters `pass_run`, so only the
  outer invocation counts as a dispatched operation. This harness is serialized.
- `heap-lower` changes only the known resource declaration lines into native image
  and sampler arrays plus GLSL aliases. Processing bodies, workgroup declarations,
  push layouts and specialization constants stay unchanged. Unsupported bound
  declarations fail; this is deliberately not a general GLSL translator.
- `glslc` compilation and SPIR-V validation succeed, with native heap capabilities
  and no descriptor-set/binding decorations. Tests cover body preservation, slot
  mapping and rejection. Compiler version is recorded with each output.

The compiler probe uses fixed per-pass heap slots corresponding to original binding
numbers. It does not bind those heaps or execute the compiled binaries. In
particular, specialization values in the manifest still need to be applied before
execution: compiled GLSL defaults are **not** the processing configuration. Vertex
inputs, the 1D float LUT and multidimensional dispatch also need an adapter/API
decision. Compiler success does not establish runtime compatibility.

## Licensing boundary

libplacebo is LGPL-2.1-or-later. Its source stays in the separate checkout and is
built as a shared library; its license/notices remain upstream. Generated shader
text contains upstream processing logic: it is an ignored build artifact here,
not redistributed as an MIT shader fixture. The OGPU repository's MIT license
does not relicense libplacebo. Packaging/distributing the combined integration
must preserve the dependency's applicable licensing requirements.
