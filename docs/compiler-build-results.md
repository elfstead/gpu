# M2 transitive-build acceptance

Accepted 2026-09-25 at clean `839edab`, completing
[M2 slice 4](compiler-contract-plan.md). The remaining M2 work is the
language-direction decision; M2/P7 are not yet accepted. Runtime and public API
remain unchanged at ABI 17.

## Result and boundary

Single-stage and graphics-pair generation now optionally publish a build receipt
alongside the header: `--manifest build.json --source-root /source/tree`.
Compiler-reported input paths/hashes, native artifact hashes, compiler options,
ordered include/import paths, adapter hashes and the published header hash make
the application's build inputs and output explicit. Paths are relative for
relocation. Outside-root files/search paths and symlink escapes reject; the tool
does not silently discard them from the record.

The checker catches edits to nested includes and imported source modules even when
native words and header text remain identical. It also checks missing files,
changed import resolution, stale/missing outputs and profile/tool identity changes.
Slang's reflection-only mode does not emit a depfile, so its include report supplies
that graph; the device compilation's depfile independently agrees. Input hashes
must stay unchanged across compilation and named-pointee reflection queries.
Generated query sources are scratch, not caller dependencies.

This is **opt-in offline build tooling**, not runtime reflection, a cache, a
hermetic package/toolchain manager, or a stable shader packaging standard. Without
a receipt, existing checks still recompile but do not promise detection of edits
that leave the header identical. Compiler binaries/shared libraries, implicit
standard-library contents and SPIRV-Tools are not bundled or fully hashed. Keep
the toolchain pinned and inputs stable during generation. Precompiled packages
are outside the profile. Header and receipt publication is not a multi-file atomic
transaction. See the [exact scope](compiler-workflow.md#optional-application-build-receipts).

## Evidence

Pinned Slang 2026.14.1, Rust 1.97.1, Radeon RX 5700 XT/RADV Mesa 26.2.1 and
llvmpipe/LLVM 21.1.8; Vulkan and synchronization validation enabled:

- The new five-file fixture uses nested includes plus an imported source module.
  The original affine C consumer passes both drivers with original and included-
  root/local-size mutations: six executions per driver, checking all 4,099 values
  and guards with exact binary32 results. Local size changes 64 → 32; the top-level
  shader and C source need no semantic changes for the included-root mutation.
- Fourteen dependency tests pass: actual include/import graph, dependency-only
  edits with identical native code, missing inputs/outputs, relocation and spaces,
  import search order, symlink escape, output/source collision, tool/profile receipt
  changes, named-pointee query inputs, shared graphics inputs, Make escaping,
  reported precompiled-input rejection, and an injected mid-compilation input edit.
  GPU runner logs contain thirteen tests; the final added concurrent-edit test
  passed in the separate fourteen-test run before committing.
- A clean SDK installs and relocates. On both drivers the installed dependency
  consumer runs original/reordered layouts, rejects a comment-only nested edit,
  detects a missing nested file without changing the published header, regenerates
  and executes. Existing transform/affine/aggregate/heap examples still pass.
- Installed graphics tests factor both varying declarations into one shared include.
  The receipt records three source inputs and both native artifacts remain byte-
  identical to their pre-factoring forms. A comment-only shared edit rejects a stale
  receipt. Original, reordered and shared-include graphics consumers execute with
  exact pixels/guards, including prior mismatch/stale-header rejection checks.
- Existing standalone generated headers reproduce. Transform (16), affine (6),
  aggregate (20), heap (11), stage (15) and learned-image build tests pass. All 38
  small M1 cases in original/mutated and normal/diagnostic modes pass on both drivers
  (152 frames each): maximum float error `1.71016513e-7`, error/bound `0.00667210166`,
  RGB delta 1, with original/mutated outputs byte-identical within each driver.
- Forty ordinary tests, strict Clippy and formatting pass. A pre-existing unpacked
  Cargo cache was incomplete; a fresh isolated cache reconstructed from locally
  cached archives fixed the environment without changing repository dependencies
  or the old cache. No runtime change, so the full runtime GPU/ABI suites were not
  rerun; no new Metal or other physical GPU claim.

The optional receipt adds no execution work. The module helper compiles into the
entry's scalar multiply/add and physical load/store; its native output contains
no function call, copy operation or barrier added by build integration. This is
bounded code inspection and unchanged consumer behavior, not a new timing result
or proof of general compiler optimization quality.

## Reproduce and receipts

```sh
python3 examples/compiler/dependency_workflow.py --check
python3 examples/compiler/dependency_workflow.py
python3 tools/install.py --prefix /absolute/new-prefix
python3 tools/test-install.py --prefix /absolute/new-prefix --dependencies --stage-pair --structured --heap-image --affine --shader-check
```

Select `SLANGC`, `VK_DRIVER_FILES` and validation layers explicitly as in the
[stage receipt](compiler-stage-results.md). The
[quickstart](quickstart.md) documents independent installed usage.

Accepted SDK `/tmp/ogpu-sdk-dependencies-m2`; relocated applications
`/tmp/ogpu-external-cukcxw7q` (Radeon) and `/tmp/ogpu-external-76szpp60` (llvmpipe).
Local diagnostic receipts, not distribution dependencies:

| Log under `/tmp/` | SHA-256 |
|---|---|
| `ogpu-dependencies-tests-final.log` | `1b59ade8980b65e58b73f98f9f4e959e0e7af0fbb034e9c0cde38ae83127f8e9` |
| `ogpu-dependencies-radv-final.log` | `1d5dbad78957b434764f6542122ae846e2730b970596b0c3bcd0a88be5247960` |
| `ogpu-dependencies-lvp-final.log` | `c3f6742a36e8facb1dfc986a73141fd104f4d258b88a147efc110f8ddb1e0f04` |
| `ogpu-dependencies-m1-radv.log` | `8daf64bb48b3405d78cf0de87e055d3aa45427b3ce5c80b00ffeaa0c4d3fb744` |
| `ogpu-dependencies-m1-lvp.log` | `d5b9987266d50526e80c396ace51f23365ab2c97d73359b39278e8709ee155a5` |
| `ogpu-dependencies-install.log` | `dd3289e99272b0eb13156b5173215f9a4672515b2530e2bb82a471bc11edca6c` |
| `ogpu-dependencies-sdk-radv.log` | `5a098ea0276ed2941b2f85aaefb9d70e2166ea70ed00f87feaaf563c2e500e96` |
| `ogpu-dependencies-sdk-lvp.log` | `d23ddf9f301e3956f1661792c81076a159f57a5a7632f35fd3011385d59f152e` |

Next: inspect the selected structured-pointer, workgroup, subgroup, matrix and
graphics native outputs, then record whether a pinned Slang profile suffices,
needs a small project device library, or exposes a concrete reason for a frontend.
Adapter limits alone are not evidence that the fundamental API must impose them.
