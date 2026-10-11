# Retired libplacebo controls

The foundation migration has passed the native-reference SDR/HDR, two-frame,
failure/lifetime and independent SDK handoff gates. The temporary ABI20 adapter
and its diagnostic suite are now removed from current source. Nothing selects
the old API at runtime. Setting `OGPU_LIBPLACEBO_API=installed` fails before build;
the backend test runner also rejects old executables before running GPU tests.

Removed: `backend-installed.c`, receipt-timing/command-omission wrappers,
allocation-tracing/placement loader, optimized-compiler variant, and the three
diagnostic runners plus their build script. These are recoverable from Git, not
deleted evidence. The ordinary native Vulkan reference, per-operation/aggregated
foundation comparison, shader capture/audits and all current correctness checks
remain. No foundation timestamp instrumentation or performance claim is added.

## Replay without mixing generations

The last complete checkpoint containing the ABI20 adapter and diagnostic tools is
`5f4ddf6` ("Export and validate an independent foundation libplacebo handoff").
Use a separate worktree with its own runtime and build output, for example:

```sh
git worktree add --detach ../ogpu-libplacebo-controls 5f4ddf6
cd ../ogpu-libplacebo-controls
git submodule update --init --recursive
OGPU_LIBPLACEBO_API=installed bash integrations/libplacebo/run-consumer.sh /path/to/pinned/libplacebo
```

Supply the toolchain and single-ICD environment described in that revision's
integration README. For its diagnostic runners, select `installed` explicitly
(their build script also defaults to it). Never combine the old wrappers with a
foundation executable, or interpret those new ABI20 runs as foundation results.
The optimizer variant failed its recorded SPIR-V validation gate; its removal
does not mean it was fixed or its performance established.

For the original image-placement investigation, use the exact source revision
recorded in the [raw diagnosis](results/libplacebo-diagnosis-radv-2026-09-15.txt),
not the last control checkpoint: by then the old allocator already contained the
fix. [The diagnosis](libplacebo-diagnosis.md) and
[post-fix observations](results/libplacebo-image-fix-radv-2026-09-15.txt) remain
historical evidence with their original boundaries. The foundation API makes
placement an explicit consumer choice; it does not inherit that allocator policy.

## Remaining project cutover

This retires the **libplacebo** control, not the installed ABI20 runtime. Other
application workflows and the default SDK still use that API. GGML also retains
its explicit installed control/native Metal path. Their migration or retirement
must precede removal of shared legacy runtime code. The opt-in foundation SDK
already packages only the current libplacebo adapter.

Post-retirement checks (2026-10-12): rebuilt SDR A/B/A, 72 streaming frames,
all 29 lifetime/rejection cases and nine HDR cases pass on Radeon/RADV and
llvmpipe. Native/adapter pixels match exactly and HDR parameter checks pass.
Host receipt tests, shell syntax and legacy-selection rejection also pass.
This verifies the cleanup, not a new throughput result.
