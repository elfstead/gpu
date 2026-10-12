# Use OGPU from your own application (Linux x86-64)

The default SDK is the explicit foundation interface: caller-owned memory,
recording storage, queues/timelines, shader arguments and synchronization. It is
experimental and revision-pinned, not a stable API or ABI. No account, service,
registry publication or project approval is needed to use it.

## Install a pinned revision

```sh
git clone https://github.com/elfstead/gpu.git
cd gpu
git checkout --detach YOUR_SELECTED_COMMIT
python3 tools/install.py --prefix /absolute/path/to/new-ogpu-install
```

The prefix must be new and its parent must exist. Runtime builds need Rust/Cargo,
locked dependencies, a native linker and Python 3. Neither Vulkan headers nor
Slang is needed for the ordinary build. The library retains host libc/toolchain
dependencies; it is not a universal Linux binary. Installation currently supports
native Linux x86-64 only.

The package contains `ogpu_next.h`, `libogpu.so`, `ogpu-foundation.pc`, offline
shader tools, licenses and a revision/hash manifest. Five self-contained examples
cover compute, graphics, learned-image processing, indexed/depth scenes and
range/count/replay. Pinned GGML and libplacebo adapters are also supplied; they
require their separate upstream sources. See the [foundation guide](foundation-quickstart.md).

## Build outside the checkout

```sh
export OGPU_PREFIX=/absolute/path/to/new-ogpu-install
mkdir my-gpu-app
cp -R "$OGPU_PREFIX/share/ogpu/examples/foundation-compute/." my-gpu-app/
cd my-gpu-app
export PKG_CONFIG_PATH="$OGPU_PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
python3 build.py
./consumer
```

Consumer builds need C11, Python and pkg-config, not Rust, Slang, Vulkan headers,
the OGPU checkout or earlier test artifacts. Provided generated headers embed
native shader binaries. Link with `pkg-config --cflags --libs ogpu-foundation`.
Installed `ogpu-shader` and `ogpu-graphics` generate foundation interfaces by default.
Read [the foundation guide](foundation-quickstart.md) for generation, mutation,
independent acceptance and the explicit lifetime/synchronization contract.

## Execution requirements and compatibility

Execution needs a Vulkan loader and the modern native baseline: Vulkan 1.4,
buffer device addresses, timeline semaphores, synchronization2, maintenance5,
storageBuffer16BitAccess, `VK_EXT_descriptor_heap`,
`VK_KHR_device_address_commands` and `VK_KHR_shader_untyped_pointers`, including
their required feature bits. Vulkan version alone is insufficient. Unified image
layouts and FP16 arithmetic are not baseline requirements. Graphics consumers
add their declared raster/format requirements; no display server is needed.

Select one supported ICD for the supplied single-adapter examples. For validation,
enable the Khronos validation layer and synchronization validation when installed.
The installer does not install drivers or shader compilers. Native Metal foundation
parity is not established; packaging changes do not change that support boundary.

Pin matching header, library and generated artifacts from one exact commit.
Foundation revision 1 is mutable; neither it nor package version 0.1.0 promises
cross-commit compatibility. See [support](support.md) and [upgrade/rollback policy](release-policy.md).

## Explicit legacy access

`--legacy` additionally builds the ABI20 runtime and packages `ogpu.h`, `ogpu.pc`
and the ABI20 examples.
Legacy shader generation must explicitly pass `--api installed`. The default
package deliberately lacks those files, so an old consumer cannot silently link
against another SDK profile. `--foundation` remains accepted as a no-op spelling
for existing scripts; it no longer adds legacy files.

See the [legacy guide](legacy-sdk.md) for the old workflow. Default Linux SDK builds
use `--no-default-features` and omit ABI20 execution/probe exports. `--legacy` enables
the Cargo `legacy` feature. Plain Cargo builds retain that feature for remaining
in-tree consumers; native Metal remains unchanged. Neither profile translates one
API into the other. The manifest records the actual Cargo feature selection.
