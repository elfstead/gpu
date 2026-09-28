# M7 public-contract audit

Reviewed 2026-09-28 against the ABI-17 implementation following M3 (`436ebaa`).
This is a bounded release-readiness review with concrete regression gates, not a
formal proof, security audit, API freeze or approval of all performance choices.
The new public comments clarify existing behavior; no signature/layout/runtime
change or ABI increment is selected.

## Findings corrected

- The public header still described macOS 13. `metal.rs` explicitly requires Metal
  4/macOS 26; correct the header and distinguish historical ABI-12 native acceptance
  from the current ABI-17 stubs in the Metal status page.
- The newly installed reuse README said completion destruction was not a wait.
  `batch.rs`'s `Drop for Completion` calls `wait()`; the public header already
  documented draining destruction. Correct the README: explicitly wait to obtain
  diagnostics, but destroying a pending receipt drains rather than cancels work.
- Discovery intentionally does not write the possibly incompatible error struct
  on ABI mismatch, nor when `out_probe` is NULL. Make that exception explicit;
  callers must not print a stale error as the ABI-mismatch reason.
- Qualify images/heaps on “both execution profiles” as **Vulkan** profiles. Metal
  exports unsupported stubs; the previous phrase could imply backend parity.

## Contract-to-implementation/test map

| Contract | Inspected boundary / implementation | Regression gates |
|---|---|---|
| Version mismatch, NULL outputs, diagnostic clearing and untouched query outputs | `lib.rs`, `boundary.rs`, `execution_api.rs`; version checked before versioned writes, creation clears handle before work | `invalid_arguments_and_version_do_not_load_vulkan`, `invalid_*_arguments_need_no_driver`, C probe/mock suite and ABI layout/signature checks |
| Non-owning addresses, retained explicit objects, retirement | `contract::Submission`, `batch.rs` resource/list owners and `Drop for Completion`; raw root addresses are never traversed | `gpu_retirement`, `gpu_replay_retirement`, `gpu_completion_receipts`, M3 failures |
| Batch consumption exceptions and serial/simultaneous list use | `take_for_preparation`, `compile_with_flags`, `CommandList::submit`; unmatched dependencies/unsupported modes reject before consumption | `gpu_split_dependencies`, `gpu_command_lists`, `gpu_owned_recording_storage` |
| Transient errors vs terminal drain/loss | `Completion::enqueue/poll/wait`, sticky device loss and indeterminate-submit drain; pending polls retain resources | `gpu_batch_failures`, `wait_error_does_not_release_pending_resources`, `unexpected_submit_errors_require_queue_draining`, M3 fault matrix |
| Bounds/overflow and no hidden placement fallback | `contract::range/copy_ranges`, buffer size/placement checks, `ImageDesc::validate`, descriptor heap count/reservation/alignment arithmetic | Ordinary range/placement/layout tests; GPU transfer/image/heap rejection tests; M3 checked ranges |
| Direct HOST visibility and cache atoms | `host_atom_range`, view/cache boundary, checked subtraction before range addition and bounded atom expansion | `host_atom_ranges_cover_atoms_without_overflow`, `gpu_host_views`, `gpu_host_view_ranges`; real coherent backing plus synthetic noncoherent callbacks |
| Capability/unsupported boundary | Vulkan baseline checks vs enabled queries; Metal optional-operation stubs; per-image support checks | `modern_baseline_rejects_each_missing_requirement`, executable/image tests, mock unsupported device and installed missing-loader failures |
| Persistent list/heap ownership | List owners reserve command storage and retained objects between executions; heap mutation requires exclusive ownership | Replay/retirement/heap tests and installed heap + replay consumers |

Implementation sources are under `crates/ogpu/src/`; each named gate can be found
there or in `examples/probe.c` / `tests/mock_vulkan.c`. Existing test counts alone
are not evidence they ran at the new checkpoint: the fresh-source receipt records
commands and results separately in the [accepted checkpoint](release-checkpoint.md).
Historical M3 fault/performance evidence is not
silently labeled a fresh run.

## Explicit residual risks

- Valid, aligned, non-overlapping C pointers and live handles are caller obligations.
  Checks for NULL/ranges cannot make arbitrary dangling pointers safe. Host OOM may
  abort; contained Rust panics do not make all native faults recoverable.
- The single-device call serialization rule is required by internal ownership and
  shared native state. Neither independent ranges nor simultaneous list execution
  permits concurrent host calls on that device.
- Shaders/artifacts are trusted. Header checks do not validate arbitrary SPIR-V,
  pointer bounds, shader synchronization or floating-point accuracy.
- Persistent driver wait errors can block indefinitely. No timeout/cancellation or
  safe reclamation while the GPU might still access raw memory is promised.
- Known rejected submission and indeterminate submission are distinct. A failed
  submit does not retire earlier receipts or authorize reuse of unrelated slots.
- Overflow checks were reviewed at public sizes/ranges and native allocation
  calculations on supported 64-bit hosts, not exhaustively model-checked.
- Real noncoherent hardware, hardware device loss, other physical vendors and
  current Metal execution remain unvalidated. The C entry points remain separate
  per backend; Linux tests cannot establish current Metal parity.
- Replay avoids repeated encoding for fixed workloads, but mutable-root/direct
  encoding, host concurrency and total command-memory strategy questions remain
  open. Release preparation does not relax the native-performance criterion.

Disposition: proceed with the Vulkan-scoped source/local-build checkpoint and the
documented experimental upgrade policy. No runtime refactor is justified solely by
these documentation findings. Stronger stabilization claims remain out of scope.
