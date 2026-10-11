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

Run `python3 build.py` in the copied directory. Run `./consumer` for compute or
graphics; run `python3 run.py` for learned-image acceptance. The last command
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
the preview to a fresh space-containing prefix, builds all three examples outside
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
three smaller consumer checks.

Local acceptance (2026-10-11): Radeon/RADV and llvmpipe pass all three copied
consumers with synchronization validation, header/runtime identity checks and
missing-loader rejection. The learned-image receipt checks all 21 output files
per mode against independent CPU gates and exact cross-mode/replay equality.
Installed compute/graphics regeneration and changed compute interfaces pass on
Radeon. The ABI-20 compute/stage-pair examples and default generator also pass
from the same preview installation. Six installer safety tests cover both opt-in
settings, and two host oracle tests reject malformed/nonfinite/out-of-bound data.
