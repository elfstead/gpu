# Use OGPU from your own application (Linux x86-64)

OGPU is an MIT-licensed experimental runtime for explicit GPU memory, programmable
execution and synchronization. You can use the public C ABI independently of our
examples and compiler tooling. No account, service, registry publication or project
approval is needed. Pin a source revision; this is not a stable release or ABI.

## Requirements

- To build/install the runtime: Git, Rust/Cargo, the native linker and Python 3
  standard library. The workspace declares Rust 1.85; recent acceptance used
  Rust 1.97.1, not a revalidation of the declared minimum. Cargo needs its locked
  dependencies available from the registry or local cache. No submodule checkout,
  Vulkan SDK headers, bindgen or Slang is needed for the ordinary runtime build.
- To build the installed example: a C11 compiler/linker, `pkg-config` (or pkgconf)
  and Python 3. Rust/Cargo and shader tools are not needed on the consumer side
  when using the installed library and supplied generated header.
- To execute: a Vulkan loader (`libvulkan.so.1`) and a driver exposing **Vulkan
  1.4, buffer device addresses, timeline semaphores, synchronization2, maintenance5,
  storageBuffer16BitAccess, VK_EXT_descriptor_heap, VK_KHR_device_address_commands
  and VK_KHR_shader_untyped_pointers**, with their required feature bits. Vulkan
  version alone is insufficient. The runtime checks execution compatibility and
  reports why a device cannot be created. Unified image layouts and FP16 arithmetic
  are not required. This example needs compute, not graphics or a display server.

The installer below is native Linux x86-64 only. It does not change Metal support,
install drivers, download a shader compiler or produce a universal Linux binary.
The resulting library retains host libc/toolchain dependencies (including Nix store
paths when built with Nix). Build on the deployment system or a deliberately
compatible build environment; copying it to an arbitrary machine is not validated.

## Build and install a pinned revision

Clone the public repository, select the full commit you want, and run from it:

```sh
git clone https://github.com/elfstead/gpu.git
cd gpu
git checkout --detach YOUR_SELECTED_COMMIT
python3 tools/install.py --prefix /absolute/path/to/new-ogpu-install
```

Choose a user-writable **new** directory whose parent exists; no `sudo` is needed.
The installer refuses existing destinations, so an upgrade cannot overwrite an
older installation. It builds the release library and installs:

```text
include/ogpu.h
lib/libogpu.so
lib/pkgconfig/ogpu.pc
bin/ogpu-shader                         # optional offline shader tool
bin/ogpu-graphics                       # optional checked graphics-pair tool
share/ogpu/manifest.json                # revision, ABI, dirty flag, file hashes
share/ogpu/licenses/
share/ogpu/examples/transform/
share/ogpu/examples/affine/              # generated FP32-root example
share/ogpu/examples/structured/          # generated nested root and pointer blocks
share/ogpu/examples/heap-image/          # generated image/sampler heap roots
share/ogpu/examples/stage-pair/          # checked vertex/fragment varyings
share/ogpu/examples/dependencies/        # nested includes/import + build receipt
share/ogpu/QUICKSTART.md
```

Do not edit the source or run another build into the same Cargo target directory
while installing. Failed installs retain their reported `.ogpu-install-*` staging
directory for inspection. The installer never deletes or merges an old installation.

The manifest identifies what was installed; a dirty checkout is explicitly marked,
not presented as a clean commit build. Use matching header/library/shader artifacts
from one revision. `OGPU_ABI_VERSION` is passed to discovery and detects incompatible
ABI versions; it is not a guarantee that arbitrary same-ABI revisions are identical.
The package version remains experimental `0.1.0`; use the revision for identity.

## Build outside the repository

In another directory, copy just the installed example files:

```sh
export OGPU_PREFIX=/absolute/path/to/new-ogpu-install
mkdir my-gpu-app
cp -R "$OGPU_PREFIX/share/ogpu/examples/transform/." my-gpu-app/
cd my-gpu-app
export PKG_CONFIG_PATH="$OGPU_PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
python3 build.py
./transform
```

The example checks enabled capabilities and limits, executes three integer
transforms on 4,099 elements, and verifies every result and guard. It uses only
`ogpu.h`, the installed library and its own generated shader header. There are
no build references to our checkout, `target/`, Cargo, test fixtures or temporary
development environment. The generated header embeds the shader, so execution
does not search for shader files. You can replace `build.py` with your build
system using `pkg-config --cflags --libs ogpu`.

The sample build embeds the selected installed library directory as RUNPATH.
Rebuild the application after relocating the installation, or deliberately set
your loader search path. `LD_LIBRARY_PATH` can override RUNPATH: unset stale OGPU
entries if the wrong library loads. `ogpu.pc` itself is relative to its installation
directory. Query identity with `pkg-config --variable=ogpu_revision ogpu` and
`pkg-config --variable=ogpu_abi ogpu`.

No supported device is a failure, not a silently successful skip. For troubleshooting,
check the reported device-creation reason. `VK_DRIVER_FILES` can select an installed
ICD; `OGPU_VULKAN_LIBRARY` selects a nonstandard loader path. Validation is optional
and requires installed layers: set `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`
and `VK_LAYER_VALIDATE_SYNC=1` during development.

## Change the shader (optional)

Install Slang **2026.14.1** and SPIRV-Tools supporting Vulkan 1.4 separately. From
your application directory, regenerate its header using only the installed tool:

```sh
SLANGC=/path/to/slangc "$OGPU_PREFIX/bin/ogpu-shader" \
  --source transform.slang --output transform.generated.h \
  --build-dir shader-build --name transform --stage compute
python3 build.py
./transform
```

Add `--check` to require byte-identical reproduction without overwriting the header.
The generator supports one `main` entry, uint32/FP32 fields and scalar device
pointers, nested C-layout structs, fixed arrays, 2/3/4-lane vectors and named struct
pointers, fixed compute workgroups and a narrow fullscreen vertex/display fragment
interface. It rejects unsupported layouts/resources/capabilities. This is an
optional pinned compiler adapter, not the runtime's full shader contract or a new
language requirement. You may compile compatible SPIR-V with another tool and
supply `OgpuShaderDesc` yourself; you then own its host/shader layout and requirements.
Generated headers target C11; C++ wrapper/header generation is not promised.

The installed `examples/affine` directory uses FP32 scale/bias root values and a
device pointer. Copy it like the transform example, then run
`python3 build.py --output affine` and `./affine`. To regenerate, use
`--source affine.slang --output affine.generated.h --name affine --stage compute` with the same
installed shader tool. This verifies exact transport/layout, not general floating-
point accuracy.

The installed `examples/structured` directory tests nested root fields and an
array of pointed-to blocks containing further GPU addresses. Build with
`python3 build.py --output structured`; regenerate with
`--source structured.slang --output structured.generated.h --name structured --stage compute`.
Generated host types supply layout/stride; callers still own all reachable buffers.
Pointee metadata comes from a separate reflection-only compilation, not extra device
arguments. Vulkan root arrays require uniform indices; the adapter rejects cases
its bounded proof cannot establish. Dynamic per-invocation indexing of pointed-to
arrays is separate. Recursive types, matrices and opaque layouts remain unsupported.
See the [device-code contract](device-code-contract.md) for details.

The installed `examples/heap-image` directory needs a compatible graphics device,
not a display server. Copy the directory, run `python3 build.py --output heap-image`,
then `./heap-image fullscreen.vert.spv image-pattern.frag.spv`. These two supplied
non-heap raster artifacts are still file inputs. The compute and sample artifacts
are embedded in generated headers. Regenerate those with the installed shader tool:

```sh
"$OGPU_PREFIX/bin/ogpu-shader" --native-heaps --stage compute --source heap-process.slang --name heap_process --output heap_process.generated.h --build-dir shader-build/process
"$OGPU_PREFIX/bin/ogpu-shader" --native-heaps --stage fragment --source heap-sample.slang --name heap_sample --output heap_sample.generated.h --build-dir shader-build/sample
```

Add `--check` to verify existing headers. The optional repository-side installed
test is `python3 tools/test-install.py --prefix "$OGPU_PREFIX" --heap-image --shader-check`.
Heap contents and index validity remain application-owned; generated declarations
do not add ownership tracking or bounds checks to shader accesses.

The installed `examples/stage-pair` directory has both stages embedded in one
checked header. Copy it, run `python3 build.py --output stage-pair` and `./stage-pair`.
It verifies interpolated coordinates, a flat integer and exact RGBA8 pixels/guards.
To regenerate or check the pair:

```sh
"$OGPU_PREFIX/bin/ogpu-graphics" --vertex-source stage_vertex.slang --fragment-source stage_fragment.slang --name pattern --output pattern.generated.h --build-dir shader-build --check
```

Remove `--check` to regenerate. A mismatched pair rejects before updating the
published header; it is not deferred to GPU execution. The optional installed
test adds `--stage-pair --shader-check` to `tools/test-install.py`. This tool is a
bounded offline interface checker, not a general linker or stable shader package.

For a shader split across includes/modules, copy the installed `examples/dependencies`
directory. Its C build still needs no shader compiler: run
`python3 build.py --output dependency` and `./dependency`. To check the shader and
its declared build inputs using pinned Slang and SPIRV-Tools:

```sh
"$OGPU_PREFIX/bin/ogpu-shader" --source affine.slang --name affine --stage compute --output affine.generated.h --build-dir shader-build --source-root . --manifest build.json --check
```

Remove `--check` to regenerate the header and receipt. `--manifest` is opt-in and
requires `--source-root`; dependency-only edits are not guaranteed detectable by
the header check alone. Nested include/import paths must stay inside that root.
Repeated `--include-dir` options select ordered search paths, also inside the root.
The same options work with `ogpu-graphics`; a shared include is tracked for both
stages. The receipt uses relative source/output paths and hashes so it survives
moving the whole tree. Your build system owns scheduling; there is no runtime
compiler or reflection dependency. See the
[receipt scope and limits](compiler-workflow.md#optional-application-build-receipts).

The repository-side installed test adds `--dependencies --stage-pair --shader-check`
to `tools/test-install.py`. It tests edits that leave native code unchanged,
missing inputs, regenerated layouts and shared stage declarations.

For your own application, preserve the important contracts: addresses do not own
allocations, referenced memory must remain live and in bounds, GPU dependencies
are explicit, and calls on one device/its children are externally serialized.
Shaders are trusted, not sandboxed. The installed header documents exact contracts.
The runtime does not infer a tensor graph, migrate memory or schedule your algorithm.

## What this establishes

Our out-of-tree acceptance test can establish an installation boundary on the
existing machine, not independent adoption or clean-machine portability. Real
third-party use may expose documentation or API issues our own example misses.
The public repository's `docs/plan.md` records current support; the installed
manifest records the particular revision you built.
