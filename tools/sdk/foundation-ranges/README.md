# Foundation indexed ranges

An independent C11 consumer of the experimental foundation revision-1 interface.
It generates indexed geometry, indirect records and counts on the GPU; observes
native draw identity; and owns all memory, recording storage and completion.
It uses only the installed `ogpu_next.h`, runtime and generated shader interfaces.

```sh
export PKG_CONFIG_PATH=/path/to/sdk/lib/pkgconfig
python3 build.py
python3 run.py
```

Select one supported Vulkan ICD. Enable Vulkan/synchronization validation when
available. Neither building nor running requires Rust, Slang, Vulkan headers,
the OGPU checkout, or earlier local evidence. Checked-in generated headers contain
shader binaries and typed argument/requirement metadata; `.slang` sources are
provided for inspection and optional regeneration with the SDK compiler tools.

For example, regenerate the compute interface with:

```sh
ogpu-shader --api foundation --source frontier.slang \
  --output foundation_scene_range_prepare.generated.h --build-dir shader-build \
  --name scene_range_prepare --stage compute
```

Add `--check` to reject a stale header without replacing it. Reordering the four
root fields does not require host C changes: the caller uses generated named
fields. Rebuild and rerun acceptance after any shader/interface edit; typed layout
does not establish address bounds, synchronization or resource lifetime.

The runner first checks a 30-frame analytic color/depth baseline, then all nine
1/64/512-record single/multi/count cases (72 frames). Replay/reset compares every
output byte with these accepted results in one/two caller-owned slots. Sixteen
frames per case exercise all eight count variants in each slot, including empty,
partial, full and above-capacity counts. Separate-call cases additionally check
CLEAR/LOAD scopes with persistent bindings. That is 48 replay cases / 768 frames.
`--scale` repeats the matrix at 1280x720.

Each invocation writes `run-*/report.json` with source/executable/runtime hashes,
commands, outputs, device identity and explicit complete/incomplete status. Failures
do not become success receipts. This proves bounded correctness, not timing,
native command counts, complete graphics support, or Metal portability. Native
trace/fault controls remain in the checkout. The foundation API is not stable.
