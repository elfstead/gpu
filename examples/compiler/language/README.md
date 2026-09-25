# Native language probes

Pinned Slang 2026.14.1 diagnostics for the M2 language decision. These do not
expand the installed shader generator or the public capability vocabulary.

```sh
export SLANGC=/absolute/path/to/slangc
python3 examples/compiler/language/probe.py
python3 -m unittest discover -s examples/compiler/language -p 'test_*.py'
# Select one ICD and enable Vulkan/synchronization validation explicitly:
python3 examples/compiler/language/probe.py --gpu --timing
```

The runner writes reflection, validated native artifacts, disassembly and
`report.json` under `target/compiler-language`. It reproduces the accepted
structured and graphics headers and compares the two workgroup kernels' barrier
scopes/semantics with the checked-in GLSL artifacts. The checks deliberately
describe these fixtures, not a generic shader validator or new profile. Negative
tests mutate inspection inputs, not executable artifacts sent to the driver.

Reduction/matrix run through the existing public C consumers. An additional
reduction oracle checks every partial, input integrity and guards at ten boundary
counts including zero and A/B/A reuse. Matrix tolerances are unchanged. The matrix
probe is ordinary FP32 tiling, not cooperative-matrix acceleration.

Subgroups use a direct Vulkan diagnostic with no OGPU linkage, reusing the existing
native loader/buffer/batch helper. It requires one selected device and queries
compute-stage BASIC/ARITHMETIC support before pipeline creation. Unsupported is a
failure, not a passing skip. Membership is keyed by the minimum global invocation
ID; independent sums, counts, unique lane IDs, workgroup containment, unchanged
input and guards are checked without assuming contiguous invocation grouping.
No required-subgroup-size or varying-subgroup-size feature is enabled. OGPU's
missing public subgroup-operation query remains an explicit limitation.

Structured original/direct-pointer variants use the unchanged semantic C consumer.
`--timing` additionally compares them with identical host policy at 4,099 and
1,048,576 elements: three warmups and 21 samples per variant, alternating AB/BA.
Identity-valued runtime coefficients prevent repeated in-place overflow; all
elements, inactive suffix, guards and parameter bytes are checked after every
sample. Numerical non-identity cases remain in the ordinary semantic consumer.
The timestamps cover one dispatch plus boundary dependencies, not isolated ISA
cycles or host overhead. Use validation runs only for correctness. For timing,
unset validation and run `target/compiler-language/structured-timing` with the
`structured/transform.spv` and `structured-direct/transform.spv` paths in separate
processes. This is a source-lowering comparison, not API/native parity evidence.

The generator must still reject the workgroup and subgroup probes. Manual roots
are cross-checked against both reflection and SPIR-V before execution; generated
headers remain the supported interface for accepted structured/graphics sources.
No runtime, ABI, numerical permission, installed SDK behavior or Metal claim changes.
