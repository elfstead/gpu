# Independent foundation textured scene

Requires C11, Python 3, pkg-config and a matching installed OGPU foundation SDK.
No OGPU checkout, Rust, Slang, Vulkan headers or previous output receipts are needed.
The supplied generated interfaces embed the shaders. Use one supported Vulkan ICD.

```sh
export PKG_CONFIG_PATH=/path/to/sdk/lib/pkgconfig
python3 build.py
python3 run.py
```

The six cases combine GPU-generated indexed/depth geometry with uploaded mip
chains, sampled-image views, nearest/linear filtering, transparency and changed
viewport/scissor state. Thirty-six frames cover 257x193 and 640x360; `--scale` adds
eighteen 1280x720 frames. Every case executes A/B/A data, geometry/guards and an
independent CPU pixel/depth oracle. The restricted-view case must equal explicit
LOD selection, and repeated A output must match exactly. No performance is claimed.

`run-*/report.json` starts incomplete and records SDK/runtime/header, source and
executable hashes, device identity and acceptance. Failed runs retain their logs
and incomplete receipt. The complete flag is correctness acceptance, not evidence
that Vulkan validation was enabled; the environment is recorded explicitly.
Native comparison and injected failure tests remain in the repository harness.

To change the graphics shaders, use the SDK's compiler helper (requires its pinned
Slang/SPIRV-Tools toolchain) and rebuild the unchanged host:

```sh
/path/to/sdk/bin/ogpu-graphics --native-heaps \
  --vertex-source textured.vert.slang --fragment-source textured.frag.slang \
  --name textured_scene --output textured_scene.generated.h --build-dir shader-build
python3 build.py
python3 run.py
```

The vertex/fragment argument structs are two views of one inline bank; host static
assertions reject disagreements in their offsets. Change their shared root layout
together. Until stabilization, keep header, runtime and generated artifacts pinned
to the same source revision. Presentation and native Metal are not exercised here.
