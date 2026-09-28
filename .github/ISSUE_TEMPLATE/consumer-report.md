---
name: Consumer compatibility or correctness report
about: Report a reproducible issue with an experimental OGPU application
title: ""
labels: ""
assignees: ""
---

## Identity and environment

- Full source commit; clean or modified? Attach the SDK manifest or its relevant fields.
- `pkg-config --variable=ogpu_revision ogpu` and `--variable=ogpu_abi` output.
- Actually loaded library (`ldd` on Linux); header/library/shader revision match?
- OS/architecture, GPU, driver/loader versions and selected `VK_DRIVER_FILES`.
- Compiler/tool versions; shader generator version/options if used.
- Enabled capabilities/limits from the created device (physical support alone is insufficient).

## Reproduction

- Minimal source/shader and exact build/run commands; expected versus observed result.
- Failing API operation, `OgpuResult`, native result and complete error message.
  ABI mismatch does not populate `OgpuError`: do not present old diagnostic bytes as its message.
- Does it fail with Vulkan/synchronization validation enabled? Attach relevant output.
- For async failures: recording/list mode, retained resources, frame/slot/generation,
  last accepted submission and whether completion/drain was observed. Do not free
  pointer-reachable memory just to make a hung reproduction exit.
- For performance: workload and native alternative, host/GPU timing boundaries,
  validation/tracing state and raw samples; separate strategy limits from backend costs.

The installed resource-reuse example keeps `run-*/report.json` and partial process
logs on failure. Review files for private paths/data before attaching; never include
credentials or proprietary inputs without permission. Outside adoption and hardware
coverage are welcome evidence, not prerequisites for reporting a problem.
