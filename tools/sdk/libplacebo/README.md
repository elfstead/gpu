# Independent foundation libplacebo consumer

Copy this whole directory outside the OGPU checkout. It builds only against the
installed `ogpu_next.h`/library and a separately obtained, pinned libplacebo source.
The bounded adapter, native reference and unchanged processing tests are included;
there is no ABI20 adapter or runtime fallback in this package.

Requirements: Linux x86-64; C11/C++20 compilers (adapter helpers use C++17), Meson
1.3+, Ninja, Git, Python 3.11+ with Jinja2/MarkupSafe, pkg-config, shaderc with
native descriptor-heap support (tested 2026.1), SPIRV-Tools development files
(tested 1.4.357), and Vulkan 1.4 headers/loader plus matching `vk.xml`. Runtime
execution needs the modern foundation baseline and a combined graphics/compute
queue. No Rust, Slang, display server or OGPU checkout is needed by this consumer.

```sh
git clone https://code.videolan.org/videolan/libplacebo.git /path/to/libplacebo
git -C /path/to/libplacebo checkout --detach 3330a515d62139259c26239014f286e233bd3a5c
export PKG_CONFIG_PATH=/path/to/preview/lib/pkgconfig:$PKG_CONFIG_PATH
python3 build.py --libplacebo-source /path/to/libplacebo \
    --vulkan-registry /path/to/vulkan/registry/vk.xml
export VK_DRIVER_FILES=/path/to/one/icd.json
python3 run.py
```

The build checks upstream revision/tracked modifications, does not download
dependencies, and creates a fresh `build-*` directory. Pin SDK header/library to
the exact same source commit; foundation revision 1 is not a stability guarantee.
Each successful build writes an identity/hash receipt; a failed build does not
replace the last successful one. pkg-config must find `ogpu-foundation`, `shaderc`,
`SPIRV-Tools` and `vulkan`. Space-containing paths are tested with pkg-config 0.29.2;
some pkgconf versions over-escape the SDK's quoted flags.

The default run covers rejection/lifetime tests, queued specialization, SDR A/B/A,
72 streaming frames, nine HDR cases with exact parameter checks, and 64x33 input
resident/transfer comparisons against upstream Vulkan. `python3 run.py --large`
adds 960x540 and 1920x1080 input. Neither mode performs timing measurements. Gates
are unchanged from the in-tree acceptance tests. Validation layers must be supplied
and enabled by the environment; no script installs or claims to activate them.

Logs, generated shaders/pixels and complete/incomplete reports are preserved in
fresh `run-*` directories, including failures. Runtime/header, executable/source
and upstream-library hashes are recorded and checked against the build receipt.
Select one ICD exposing one device so both paths test the same adapter. The native
reference uses the system Vulkan loader; use that same loader if overriding
`OGPU_VULKAN_LIBRARY`. No fallback is selected when foundation setup fails.

This is a local dependency-boundary handoff, not proof of independent adoption,
general libplacebo support, performance parity, or Metal execution. Host toolchain
and library dependencies remain; this is not a universal Linux binary.

libplacebo is LGPL-2.1-or-later. Its source and notices remain in the separate
checkout and it is built as a shared library. Generated shader text incorporates
upstream processing code and is a local build/run artifact, not redistributed here
as an MIT fixture. The OGPU MIT license does not relicense libplacebo. Distribution
of a combined application must honor the applicable dependency licenses.
