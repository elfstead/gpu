# Generated indexed/depth scene

Copy this whole directory out of the SDK. Set `PKG_CONFIG_PATH` to its installed
`lib/pkgconfig`, select one compatible Vulkan device/ICD, then run:

```sh
python3 build.py --output scene
python3 run.py
python3 run.py --scale  # additionally 1280×720; selected local scale gate is Radeon
```

Needs a C11 compiler, pkg-config, Python standard library, and the installed Linux
x86-64 OGPU runtime/Vulkan baseline. No Cargo, Slang, Vulkan headers, checkout,
downloads, native-control binaries or preexisting output fixtures are needed.
Supplied generated headers embed the unchanged native shader artifacts. `main.c`
selects typed generated argument structures in the same `public.c` scene used for
the native/public comparison, not a second renderer.

The runner checks 30 frames for each of UINT32 and UINT16 indices: GPU-generated
vertices/indices/indirect records, overlapping geometry, depth modes, independent
color/depth CLEAR/LOAD, zero counts and A/B/A changes. Its analytic oracle checks
interior color/depth; generated geometry and guards are exact. Repeated frames,
equivalent draw-order/LOAD cases and index-width images must match every byte,
including edges. This is a serial one-shot scene handoff, not a replay benchmark,
general raster conformance, full textured M4 consumer or independently authored
adoption evidence. The separate draw-ranges example exercises counts/identity/replay.

Fresh `run-*` directories retain reports, source/executable identities, device and
environment, all outputs and logs. Failure/timeout leaves an incomplete report.
The low-level executable writes results and drains work; only `run.py` checks the
oracle. Callers own raw-pointer root backing until completion, use explicit
compute→index/vertex/indirect dependencies, and drain before destroying resources.
The API remains experimental and revision-pinned.

Optional regeneration with installed adapters and the pinned compiler/toolchain:

```sh
ogpu-shader --stage compute --source prepare.slang --name scene_prepare --output scene_prepare.generated.h --build-dir build-32
ogpu-shader --stage compute --source prepare16.slang --name scene_prepare16 --output scene_prepare16.generated.h --build-dir build-16
ogpu-graphics --vertex-source scene.vert.slang --fragment-source scene.frag.slang --name scene_pair --output scene_pair.generated.h --build-dir build-pair
```

Add `--check` to detect stale artifacts. `prepare16.slang` includes `prepare.slang`;
both generated compute headers must be regenerated after changing the shared root.
The host uses generated named fields, not manually assumed shader offsets.
