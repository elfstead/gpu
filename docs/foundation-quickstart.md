# Opt-in foundation preview (Linux x86-64)

The foundation is a mutable, revision-pinned low-level contract, **not** ABI 20 or
a stabilized replacement for it. The normal installer still selects the ABI-20
SDK. To additionally export the implemented `ogpu_next_*` surface and public-only
source examples, build a new installation from your selected commit:

```sh
python3 tools/install.py --foundation --prefix /absolute/path/to/new-preview
export PKG_CONFIG_PATH=/absolute/path/to/new-preview/lib/pkgconfig
```

The manifest records both the source commit and foundation revision. Match the
header, library and generated artifacts from that **exact commit**: revision 1
alone does not promise compatibility across commits. New prefixes are mandatory;
the installer never overwrites or merges an existing installation. ABI-20 headers,
examples and `ogpu.pc` remain unchanged. Preview consumers use the separately named
`ogpu-foundation.pc`. There is no runtime policy wrapper or fallback to ABI 20.

Copy any of these entire directories to an application directory outside the repo:

- `share/ogpu/examples/foundation-compute`: generated arguments, explicit memory,
  queue/timeline completion and changed-input replay.
- `share/ogpu/examples/foundation-graphics`: generated stage pair, explicit image
  backing and offscreen indirect-draw replay across six extents.
- `share/ogpu/examples/foundation-learned-image`: the mixed denoising/resize/raster
  caller, frozen model and independent scalar reference.
- `share/ogpu/examples/foundation-indexed-scene`: generated compute/graphics
  interfaces, GPU-generated UINT32/UINT16 indexed geometry and depth/split-render
  modes, with analytic, guard, A/B/A, cross-width and one/two-slot reset/replay checks.
- `share/ogpu/examples/foundation-indexed-ranges`: generated 1/64/512-record
  single/multi/count draws, native draw identity and one/two-slot reset/replay,
  including per-draw scopes with persistent bindings.

Run `python3 build.py` in the copied directory. Run `./consumer` for compute or
graphics; run `python3 run.py` for learned-image, indexed-scene or indexed-ranges acceptance.
Indexed-scene checks 30 fresh frames per index width plus 32 reset/replay cases
(256 frames) at 257x193 (`--scale` adds 1280x720);
see its packaged README. Learned-image
generates its own deterministic A/B inputs and FP64 references, then checks 12
normal/diagnostic resident/end-to-end A/B/A frames. It preserves logs and an explicit
complete/incomplete receipt in a fresh `run-*` directory, including failures.
It is a correctness handoff, not a performance comparison or evidence of adoption.

Consumer builds need C11, Python standard library and pkg-config, not Cargo, Vulkan
headers, Slang, a source checkout or preexisting fixture outputs. Runtime builds
need Rust/Cargo with the locked dependencies available. Execution needs the modern
Vulkan baseline documented in `QUICKSTART.md`; graphics examples additionally need
a graphics+compute queue and raster features. No display server is used. Metal
foundation support is not claimed. Built libraries retain host libc/toolchain
dependencies; this is not a universal Linux binary or clean-machine test.
Relocation into space-containing paths is checked with `pkg-config` 0.29.2.
The locally available pkgconf 2.4.3/2.5.1 over-escape those paths for the SDK's
quoted `.pc` metadata; use `PKG_CONFIG=/path/to/pkg-config` for that case. No
runtime compatibility fallback hides this packaging limitation.

## Independent GGML adapter

The preview also includes `share/ogpu/examples/foundation-ggml`. Copy that directory,
then follow its README: `python3 build.py --ggml-source /path/to/pinned/ggml` and
`python3 run.py`. This example additionally needs Git, C/C++17, CMake, Ninja and
Python 3.11+, plus the pinned upstream source (not an OGPU checkout). The default
run validates HOST/DEVICE mixed-precision matrices, lifetime violations and
recording-error recovery; it does not claim full MNIST acceptance.

`python3 run.py --data-dir /path/to/extracted-mnist` adds both F32/F16 full-dataset
inference against unchanged CPU references. No data downloads, training, Rust
build or shader compilation occur. MNIST is not bundled; its hashes and attribution,
and the project-authored frozen model/provenance, are supplied. See
[the migration boundary](foundation-ggml.md) for scope and retained control code.

## Independent libplacebo adapter

The preview also includes `share/ogpu/examples/foundation-libplacebo`. Copy it
outside the checkout and follow its README:

```sh
python3 build.py --libplacebo-source /path/to/pinned/libplacebo \
  --vulkan-registry /path/to/matching/vk.xml
VK_DRIVER_FILES=/path/to/one/icd.json python3 run.py
```

This consumer compiles upstream-generated GLSL live, so it additionally needs
Meson, Ninja, C/C++ compilers, Python/Jinja, shaderc, SPIRV-Tools development files
and Vulkan headers/registry. These are consumer/compiler/reference dependencies,
not imports of OGPU implementation headers. It does not build OGPU or require
Rust/Slang. The pinned upstream checkout remains a separate LGPL dependency;
it is neither downloaded by the scripts nor vendored into the MIT example.

Default acceptance checks frame lifetimes/rejections, SDR/streaming/HDR output
and small resident/transfer comparisons against native libplacebo. `run.py --large`
adds 960x540 and 1920x1080 input; neither command measures performance. Complete
and incomplete receipts preserve source/executable/upstream-library identities;
stale builds and mismatched SDKs fail. No installed-API adapter is packaged here.

Maintainers can add `--libplacebo-source ... --vulkan-registry ...` to
`tools/test-foundation-install.py` for a relocated build from a separate upstream
clone, public OGPU header/runtime checks, missing-loader and stale-source rejection,
and the default GPU acceptance. `--no-gpu` retains build/dependency/failure checks.
Its shader/compiler dependencies are optional and do not burden the smaller
preview examples or the ordinary no-GPU CI build.

## Optional shader generation

Optional shader regeneration uses the installed tools and pinned Slang 2026.14.1
plus suitable SPIRV-Tools. For the compute example:

```sh
/absolute/path/to/new-preview/bin/ogpu-shader --api foundation \
  --source transform.slang --output foundation.generated.h \
  --build-dir shader-build --name transform --stage compute --check
```

`ogpu-graphics --api foundation` likewise selects foundation records for linked
vertex/fragment stages. The default for both tools remains `installed` (ABI 20).
Application allocation, bounds, retirement, synchronization and numerical policy
remain caller-owned; generated layouts are not safe resource-lifetime tracking.

Maintainer checks: `python3 tools/test-foundation-install.py --prefix ...` copies
the preview to a fresh space-containing prefix, builds all five examples outside
the checkout, checks header dependencies and the actual linked library, rejects a
missing Vulkan loader, and runs GPU acceptance. `--no-gpu` retains build/link and
failure-path checks. `--shader-check` additionally reproduces both generated
interfaces and tests stale-header rejection plus changed-root/local-size execution
using only the installed compiler tools. This is a local dependency-boundary test,
not a sandbox proof against malicious example code or an external adoption claim.

Add `--ggml-source /path/to/pinned/ggml` to the maintainer test to check the packaged
adapter as well. The test makes a separate local upstream clone, verifies compiler
dependencies contain the relocated public header and no OGPU checkout files,
checks the actual loaded library and incomplete missing-loader receipt, and runs
matrix/lifetime acceptance. `--ggml-data /path/to/extracted-mnist` additionally
selects the full dataset gates. Those optional prerequisites do not burden the
five smaller consumer checks.

Local acceptance (2026-10-11): Radeon/RADV and llvmpipe pass all three copied
consumers with synchronization validation, header/runtime identity checks and
missing-loader rejection. The learned-image receipt checks all 21 output files
per mode against independent CPU gates and exact cross-mode/replay equality.
Installed compute/graphics regeneration and changed compute interfaces pass on
Radeon. The ABI-20 compute/stage-pair examples and default generator also pass
from the same preview installation. Six installer safety tests cover both opt-in
settings, and two host oracle tests reject malformed/nonfinite/out-of-bound data.

The 2026-10-12 follow-up adds the independent indexed/depth scene: all four copied
consumers pass on both local drivers, including 30 scene frames per index width,
exact cross-width images, public-only dependency checks and missing-loader
rejection. The scene's in-tree native comparison also passes the unchanged
analytic/geometry/guard gates; see [its checkpoint](foundation-indexed-scene.md).
Generated control-pointer reset/replay is now included in the copied example;
both local drivers pass all 32 cases, with public-only dependency and loaded-runtime
checks for both executables. In-tree matching native traces and sanitized failure
controls remain separate from the SDK package; see [replay coverage](foundation-scene-reuse.md).

The independent range/count/identity example also passes on both local drivers:
30 analytic baseline frames, 72 fresh range frames and 768 reset/replay/scope
frames. All five smaller examples pass relocation, public-only dependency and
loaded-runtime checks plus missing-loader rejection. See [range coverage](foundation-indexed-ranges.md).
