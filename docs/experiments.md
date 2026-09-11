# Experiment ledger

Updated 2026-09-12. This page records evidence, not API-stability promises. See
[the current design](design.md) for direction and [development](development.md)
for commands. "Implemented" does not mean production-ready or performance-tuned.

## Implemented baseline

| Experiment | Run | Evidence | Still not established |
|---|---|---|---|
| Discovery and C boundary | `cargo xtask smoke`, `cargo xtask mock`, `cargo xtask abi` | Capability reporting, loader failures, C/Rust layouts, version/pointer checks | Portable capability negotiation or feature enablement |
| Address-based round trip | `cargo xtask compute` | CPU upload, repeated dispatch, partial updates, readback, retained device ownership | Device-local staging, suballocation, actual non-coherent hardware coverage |
| Dependent compute kernels | `cargo xtask batch` | Copied roots, intermediate storage, explicit dependency, one submission/wait | Workgroup cooperation, arithmetic acceleration, replay or performance |
| Compute → offscreen graphics | `cargo xtask graphics` | GPU-generated vertices/indirect arguments, image rendering/readback, shared batches | Sampling, compute image access, full graphics → compute → graphics loop |
| Backend ownership/failure paths | `cargo xtask gpu-tests` | Batch states, retained resources, image reuse, preparation/submission/wait failure injection | Real hardware device loss, arbitrary shader faults, all driver behavior |

These paths have been verified locally on the RX 5700 XT (RADV) and llvmpipe, with
Vulkan synchronization validation for execution tests. The discovery mocks and
ordinary unit tests do not need a GPU. CI is configured to run validation, not
claimed to have run remotely. These are correctness results, not benchmarks.

## Next experiment: cooperative reduction

Status: planned. Sum an unsigned integer array using workgroup-shared memory and
multiple dependent dispatch levels. A CPU reference should define exact modulo-2^32
results, including empty input, partial groups, and overflow cases. Keep intermediate
data on the GPU and wait only after the final level. Test boundaries where the
number of dispatch levels changes, not just one large input.

Questions: can existing allocation/address/root/batch primitives express this
without a reduction-specific API? What obligations appear around scratch ownership,
dispatch sizing, workgroup barriers, and padding lanes? This is not yet a test of
floating-point summation accuracy, subgroup instructions, or matrix acceleration.

## Queued experiments

1. Image processing in a graphics → compute → graphics chain, without intermediate
   CPU round trips. Expose image access/representation and synchronization decisions.
2. Matrix kernels with numerical references and separately measured runtime,
   transfer, compilation, and execution costs; then optional accelerated variants.
3. A compiler/runtime consumer and a second backend for the common compute model.

For each new experiment record: hypothesis, exact workload, required API changes,
independent checks, hardware/toolchain used, observed result, untested cases, and
the next design question. A successful example is a reason to continue testing,
not a reason to freeze its temporary constraints.
