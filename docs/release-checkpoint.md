# First experimental source/SDK preparation checkpoint

Accepted locally 2026-09-28 at clean **`eba0eed9c03c59e3a158c7f446c5c9108cd427ec`**,
C ABI **17**. This completes M7's first **local preparation** checkpoint following
M1–M3. It is not a published/tagged release, API stabilization, clean-machine
qualification, current Metal acceptance or evidence of independent outside adoption.

Use the [quickstart](quickstart.md), [support matrix](support.md) and
[upgrade/rollback policy](release-policy.md). The SDK includes the last two alongside
its manifest and examples. A [consumer issue template](../.github/ISSUE_TEMPLATE/consumer-report.md)
asks for identity, driver, enabled requirements, reproducer and lifetime/error context.
The [contract audit](release-contract-audit.md) records the inspected ownership,
error/unsupported and overflow paths and four documentation corrections. Runtime
behavior, shader artifacts and ABI are unchanged; no new compatibility workaround
or allocation/submission policy was added.

## Fresh-source reproduction

The source was cloned locally with `git clone --no-hardlinks`, detached at the full
commit above, with **no prior target directory and no Vulkan-Headers checkout**.
Ordinary build/test and installation ran first. The pinned headers were then cloned
from the local vendor repository at `e3b1eec08173d6b825cd3ac88c885a63b621504a` for
bindings/ABI/mock checks. Both source trees stayed clean.

This reused the existing Nix toolchain and Cargo registry download cache, with
`CARGO_NET_OFFLINE=true`, `CARGO_INCREMENTAL=0` and `CARGO_TARGET_DIR` unset. No
previous build outputs were copied. It was not a fresh OS/container or a test of
network availability/public repository hosting. Dependencies/toolchains remain
external prerequisites. Installed consumer tests cleared inherited compiler/library
paths, relocated the SDK, and built in separate directories containing spaces.

| Gate | Observed result |
|---|---|
| Ordinary build/test without submodule | Pass; 40 tests, 28 GPU tests intentionally ignored in this lane |
| Formatting and workspace/all-target Clippy | Pass; warnings denied |
| Installer safety and reuse report rejection | 6 + 9 CPU tests pass |
| Pinned bindings reproduction | Exact match |
| C/Rust ABI | 760 layout values pass |
| Public C loader/mock cases | ABI mismatch, invalid outputs, unsupported device, empty enumeration, initialization error and missing loader pass |
| Radeon and llvmpipe baseline/GPU contracts | 28 GPU tests pass per driver, Vulkan/synchronization validation enabled |
| Relocated installed SDK, both drivers | Transform, explicit recording storage, replay, HOST views, split replay, affine/structured roots, heaps, dependencies and stage pair pass |
| Installed resource-reuse application | 12 serial + 1,000 candidate frames per driver; CPU/serial output, guards, padding and generation retirement pass; missing-loader failures retain incomplete reports |
| Optional installed shader tools, both drivers | Pinned regeneration, changed consumer interfaces, stale transitive-dependency checks and checked stage pairs pass |

Rust 1.97.1, Clang 21.1.8, Python 3.14.7, Slang 2026.14.1 and Mesa 26.2.1 were used.
Physical hardware remains one Radeon RX 5700 XT; llvmpipe is software correctness
coverage. Neither real noncoherent hardware nor real device loss was exercised.
GPU tests include bounded synthetic error/cache branches; they do not fill those gaps.

The [machine-readable receipt](results/release-checkpoint-2026-09-28/report.json)
retains exact commands, environment, full install manifest, runtime/source/fixture
identities and both reuse reports/logs. All 19 command logs are retained alongside
it and checked against their recorded SHA-256 hashes. SDK manifest files and copied
consumer sources, fixtures and execution logs were rechecked before retention.

## Reproduce the gates

From a clean detached checkout with the quickstart prerequisites:

```sh
cargo build --locked
cargo test --locked
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
python3 -B tools/test_installer.py
python3 -B tools/test-reuse-handoff.py
python3 tools/install.py --prefix /new/absolute/sdk
python3 tools/test-install.py --prefix /absolute/sdk --no-gpu --resource-reuse

git submodule update --init vendor/Vulkan-Headers
cargo xtask bindings --check
cargo xtask abi
cargo xtask mock

# Set VK_DRIVER_FILES to one compatible installed ICD; repeat per driver.
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
export VK_LAYER_VALIDATE_SYNC=1
cargo xtask baseline
cargo xtask gpu-tests
python3 tools/test-install.py --prefix /absolute/sdk --resource-reuse \
  --recording-storage --replay --host-view --split --affine --structured \
  --heap-image --dependencies --stage-pair
# Optional compiler tools, separately from the shader-compiler-free consumer lane:
SLANGC=/path/to/pinned/slangc python3 tools/test-install.py --prefix /absolute/sdk \
  --affine --structured --heap-image --dependencies --stage-pair --shader-check
```

`cargo xtask baseline` is a feature preflight, not an execution test. Unsupported
hardware must reject; do not reinterpret a failed creation as a successful GPU test.
Native Vulkan headers/bindgen are needed only for the explicit contract/generation
gates, not the ordinary runtime installation. Optional compiler tools are separately
required for the final command.

## What this does not close

Hosted CI now includes the reuse CPU/relocation gate, but **no remote workflow was
run or observed here**. The minimum-Rust lane is configured for 1.85; this checkpoint
did not execute it. The manual GPU workflow still needs a separately provisioned
compatible runner and is not silently treated as tested infrastructure.

No new full M1 video-scale, M3 injected-failure matrix or performance sweep was run.
Those accepted measurements remain attributed to their original revisions; the
runtime/workload did not change in this audit. This checkpoint is not a new claim
of fundamental native parity or portable binary deployment. The installed runtime
retains its build host's libc/Nix dependencies.

The first M7 local deliverables are complete. Publication/tagging and external
outreach remain separate decisions; external feedback remains absent. Proceed to
the [M4 offscreen scene brief](graphics-consumer-plan.md), beginning with a native
indexed/depth control and analytical probes before choosing new public contracts.
Refresh this checkpoint after M4–M6; keep better API alternatives open throughout.
