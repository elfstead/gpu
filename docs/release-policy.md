# Experimental source/SDK compatibility and upgrades

This policy describes preparation for the first Vulkan-scoped experimental
checkpoint. It is not a published release announcement or stable API commitment.

## Identity and compatibility

Pin the full source commit, not just `master`, package version `0.1.0`, or an ABI number.
Use the installed `share/ogpu/manifest.json`: source revision/dirty state, target,
profile, compiler, dependency licenses and file hashes identify the SDK contents.
Hashes detect accidental mismatch; the local manifest is not a signed provenance
attestation. A dirty build identifies modified files, not a reproducible release.

The default foundation SDK has its own `ogpu-foundation.pc` and manifest
entry. Its revision-1 structures remain mutable across source commits. Require
exact commit/header/library/artifact matching; the legacy ABI number below does
not govern foundation compatibility. `--legacy` additionally builds/packages ABI20;
the manifest records `default_api`, `legacy_sdk`, `cargo_default_features` and
`cargo_features`. No header/package alias silently translates one API into the other.
The default Linux SDK omits legacy execution/probe code and exports; it builds with
`--no-default-features`. Explicit legacy packages enable only the `legacy` feature.
Plain Cargo defaults still include legacy for in-tree consumers, and native Metal
is unchanged. Removing that remaining source is a separate migration/retirement task.

The legacy C ABI number changes for incompatible public layout/signature/behavior changes.
It is a rejection guard at `ogpu_probe_create`, not a loader symbol-version scheme,
semantic-version promise or compatibility negotiation. That call leaves `out_error`
untouched on ABI mismatch; handle the returned status directly. A missing newer
symbol can fail at link/load time before that check. Same ABI does not promise
identical fixes, enabled features, compiler output or behavior across revisions.

Until stabilization, upgrade the header, library, application build and generated
shader interface/artifacts together. Regenerate consumer-owned shaders with the
declared compiler/tool versions and check actual enabled device capabilities/limits.
Rebuild every binding that embeds public struct layouts or signatures. No automatic
conversion, persisted GPU-address validity, command-list serialization, old-driver
fallback or cross-version binary compatibility is promised.

## Upgrade and rollback

1. Keep the working SDK and application binary. Select a clean full commit and
   read its working status, public header and support boundaries.
2. Install into a **new prefix** whose parent exists. The installer refuses existing
   directories/files/symlinks; never merge the new SDK into an old installation.
3. Set `PKG_CONFIG_PATH`/`PKG_CONFIG_LIBDIR` to the new `lib/pkgconfig` and remove
   stale compiler/library search paths. Rebuild the application and its generated
   interfaces as applicable. The examples embed a RUNPATH; `LD_LIBRARY_PATH` can
   override it. Check `ldd` and the manifest rather than trusting pkg-config alone.
4. Run your correctness, failure/lifetime and representative performance gates
   on the deployment driver. The installed examples are useful smoke checks, not
   coverage for arbitrary workloads. Reject unmet requirements explicitly.
5. Switch the application only after those checks pass. Roll back by selecting the
   previous application **and matching SDK**, not by swapping a library underneath
   a running process. Drain/tear down GPU work before process/backend replacement.

Retain old prefixes until you no longer need rollback. Removal is a separate user
action; the installer does not uninstall, migrate or delete anything for you.

## Distribution and stabilization

The supported packaging path is source plus a native Linux x86-64 local build.
Drivers, Cargo dependencies and optional shader tools are external prerequisites.
Do not redistribute a Nix-linked local `.so` as a universal Linux binary. Portable
binary bundles need a specified libc/toolchain deployment target and tests there.
The SDK contains MIT and dependency license texts; preserve applicable notices.

Preparing a clean commit, test receipt or CI definition is not publishing/tagging
a release, provisioning a runner or requesting external testing. Those are separate
actions. No external adoption or remote CI result may be inferred from local tests.

API alternatives remain welcome when they better meet project goals. M1–M5 now
establish bounded consumers and the foundation contract has a
[consolidated disposition](core-contract-review.md). Broader physical GPU/memory
coverage, native Metal foundation evidence, independent adoption and unresolved
native-strategy gaps remain prerequisites for stronger promises, not reasons to
freeze the interface now.
