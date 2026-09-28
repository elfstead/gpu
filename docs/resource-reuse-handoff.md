# M3 installed handoff acceptance

Accepted 2026-09-28 at clean **`89d71e5`**, ABI 17. This closes the last bounded M3
gate after the [reuse](resource-reuse-results.md), [failure/drain](resource-reuse-failures.md)
and [matched-performance decision](resource-reuse-performance.md) checkpoints.
It does not close the wider performance audit, stabilize the API or demonstrate
adoption by an independently controlled third party.

## What shipped

The revision-identified Linux SDK now includes `examples/resource-reuse`: the
same measured public-API stream and consumer helpers, embedded shader headers,
frozen model and independent Python binary64 reference. The installer copies an
explicit source dependency set, not a checkout or mutable `target/` fixtures.
Native Vulkan controls/headers, fault-injection loaders and private Rust interfaces
are not installed or used. Shared `small.c` supplies submission/drain helpers;
its unrelated small-compute test routines are not executed. Keeping this source
graph avoids a second implementation drifting from the accepted controls.

The documented runner selects two arena-backed slots, one serial immutable list
per slot, and per-slot alternating inputs. It generates two 65x47 inputs and
131x95 CPU references locally, validates a 12-frame one-slot dedicated/re-record
baseline, then checks 1,000 arena/replay frames. Every frame passes both the CPU
oracle (RGB <=1, exact alpha) and byte-exact same-device serial output. Input/weight
integrity, intermediate/readback guards, HOST padding and retired generations are
checked after drain. This is validation, not a timing run.

The bundled [consumer instructions](../tools/sdk/resource-reuse/README.md) state
range/cache ownership, list/receipt lifetime and teardown obligations. API errors
stop this executable with retained diagnostics; it is not the recovery/fault suite.
Capacity is bounded and insufficient queried HOST granularity rejects setup.
No runtime/API change or allocator framework was needed for this handoff.

## Reproduction and evidence

```sh
python3 tools/test_installer.py
python3 tools/test-reuse-handoff.py
python3 tools/install.py --prefix /new/absolute/sdk-prefix
# Select a single compatible ICD, then enable Vulkan/synchronization validation:
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
export VK_LAYER_VALIDATE_SYNC=1
python3 tools/test-install.py --prefix /absolute/sdk-prefix --resource-reuse \
  --recording-storage --replay --host-view --split --affine --structured \
  --heap-image --dependencies --stage-pair
```

The installed test verifies manifest hashes, relocates the SDK to a path containing
spaces, copies the application elsewhere, clears inherited header/library search
paths and sets Cargo/Slang commands to deliberately nonexistent paths. Compiler
dependency output confirms that all non-system inputs are inside the copied
application except the relocated public header; `ldd` confirms the relocated
library. Both drivers execute the full command above. A nonexistent Vulkan loader
must fail visibly and preserve an incomplete JSON report plus the original error.
The runner rejects policy/range/guard/validation failures instead of treating any
zero exit as acceptance.

| Gate | Result |
|---|---|
| Radeon RX 5700 XT, RADV | 12 serial + 1,000 candidate frames; all installed regression examples pass |
| llvmpipe | Same 1,012 frames and installed regressions pass; software correctness only |
| Fixture generation | All five generated files byte-identical to the accepted M3 small-case weights/inputs/CPU references |
| Missing loader | Both relocated applications retain incomplete reports; no accepted runs or false success |
| CPU report gates | Nine tests cover missing/duplicate records/gates, wrong policy, tolerance/clocks, ranges/overlap, validation errors and timing-mode rejection |
| Installer safety | Six existing destination/platform rejection tests pass |

Retained evidence includes source/executable/runtime and fixture hashes, both
execution logs, logical ranges, reports, compiler dependency files, missing-loader
diagnostics and the full installed-regression transcript:

- [RADV report/logs](results/resource-reuse-handoff-radv-2026-09-28.json),
  [installed suite](results/resource-reuse-handoff-radv-2026-09-28.txt).
- [llvmpipe report/logs](results/resource-reuse-handoff-lvp-2026-09-28.json),
  [installed suite](results/resource-reuse-handoff-lvp-2026-09-28.txt).

Reports were rechecked against original logs, source/fixture/executable hashes and
the clean install manifest before retention. Toolchain: Rust 1.97.1, Clang 21.1.8,
Python 3.14.7; Mesa 26.2.1. This used the existing local toolchain/Cargo cache, not a
fresh OS or portable binary environment. Consumer execution used no shader compiler.
Optional shader-regeneration checks were not rerun; unchanged M2 evidence covers
that path. No new full M1 scale, fault-injection, Rust GPU/ABI or performance sweep
is claimed: the shared C workload, shaders and runtime are unchanged.

## Decision and boundary

M3 is complete within its declared workload/device bounds. Keep allocation and
reuse policy consumer-side. The next milestone is the Vulkan-scoped M7 experimental
release preparation: contract/upgrade audit, source/local-build reproduction and
consumer-facing failure reporting. No tag, publication or external outreach is
authorized by this acceptance. No Metal HOST-view/replay support, noncoherent
hardware coverage, general native parity or actual outside adoption is inferred.
