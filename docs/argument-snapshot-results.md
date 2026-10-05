# Argument snapshot correctness and the remaining contract question

Accepted 2026-10-05 at clean **`328282b`**, ABI 19. This is the first diagnostic
increment of the [argument-reuse audit](argument-reuse-plan.md), not timing or a
decision to keep the current fundamental shape.

## Result

All 8/64/256-byte roots pass on RX 5700 XT / RADV and llvmpipe, Mesa 26.2.1.
Both report a 256-byte limit; no requested size is skipped. Each native/public
pair uses identical original Slang artifacts. Four scopes save independent
257×193 outputs before one submission: equal/equal/changed/equal. Within each
scope, two separate indexed calls preserve local DrawIndex=0. The changed scope
uses A for the far draw and B for the near draw at the same host root address.

Every successful draw immediately overwrites that host block; after recording,
the whole block is zeroed before submission. Every public draw is preceded by a
wrong-size rejection and valid retry. Rejected calls must neither append a draw
nor affect the successful retry. The immutable case also crosses rendering
boundaries. GPU-address pointees stay alive through drain; they are not copied
or inferred from root values.

At eight bytes, A/B pointer mutation moves the near quad. Larger roots add
14/62 scalar words, each contributing with a nonzero weight to shader output
color; the test does not benchmark unused padding. A/B also changes these words.
Independent interior color/depth checks pass, with over 90% pixel coverage per
image. Every output byte, including excluded edges, depth and guards, matches
direct Vulkan. Final generated geometry/index/indirect/guard bytes match the
independent reference. Repeated A outputs are byte-identical and B differs.

Total: **48 saved scope images, 96 executed indexed draws and 48 rejected-call
retries**, across two drivers, three root sizes and two implementations. These
are four scopes per submitted diagnostic recording, not 48 independent frames
or sustained replay samples. The CPU suite adds three oracle/log rejection tests;
generated scene reproduction and existing range/reuse parser tests also pass.
The new CI test is configured; remote CI execution is not claimed.

## What this establishes—and does not

The implementation satisfies the current snapshot guarantee for this case.
It would be incorrect to optimize by caching the caller's address and assuming
its bytes remain unchanged. Native Vulkan independently reproduces the intended
snapshots. No API or runtime behavior was changed by these controls.

This does **not** show that resupplying argument bytes on every operation is
the right foundational contract. In `Batch::draw`, validation is followed by a
fresh `root.to_vec()` per accepted call; later `DrawStateCache` compares those
owned bytes to avoid redundant native state emission. Removing Vec allocation
and two-pass encoding is an implementation opportunity. The requirement to
account for newly supplied, shader-observable values on every call is a distinct
contract obligation. Identical native push counts would not prove identical
host input work.

The diagnostic deliberately adds saved-output storage and uses serialized
one-shot recording; it is not the matched-budget one/two-slot control. It has
no timing or instrumented native-push counts. It does not settle dispatch,
compiled-list ownership for a new argument primitive, every rejection branch,
or behavior on another physical GPU/Metal. The known software counted→fixed
failure remains unrelated and unwaived.

Next compare changed command sequences with stable roots: native bind-once,
native per-draw resupply and current public snapshots, at 1/64/512 draws with
one/two slots. Count supplied/copied payloads separately from native pushes and
decompose public collection versus encoding. Then evaluate the
[candidate contracts](argument-reuse-alternatives.md); no ABI change is selected
by this correctness result.

## Receipts

[Committed reports and runner logs](results/argument-snapshot-2026-10-05/)
preserve source/build/runtime/shader hashes, device limits, commands, checks and
output hashes. JSON wrappers retain every original field, original path and
SHA-256; normalized serialization differs from original bytes. Binary outputs
remain at the recorded local paths. Reproduce with
`python3 -B examples/graphics_scene/argument_snapshot.py` under the selected ICD
and Vulkan/synchronization validation. `--build-only` compiles all variants
without a GPU; `test_argument_snapshot.py` needs only Python.
