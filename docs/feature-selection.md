# Counted feature selection — foundation revision 9

The 61 optional features implemented in revision 8 nearly exhausted its 64-bit
public mask. Remaining raster, ray, memory and compute/media work needs independent
native enables. Grouping unrelated capabilities into one bit would change which
devices can support a request. Increasing the public integer width would postpone
the same problem and make native Rust and C consumers depend on runtime storage.

Revision 9 uses one representation throughout discovery, device creation and
shader/mesh requirements:

```c
typedef uint32_t ogpu_next_feature;
typedef struct ogpu_next_features {
    const ogpu_next_feature *ids;
    uint32_t count, reserved;
} ogpu_next_features;

const ogpu_next_feature ids[] = {OGPU_NEXT_FEATURE_RASTER};
ogpu_next_features required = {ids, 1, 0};
```

IDs are opaque identifiers, not bit values. Their names retain their independent
native semantics. A runtime rejects an unknown ID with `UNSUPPORTED` and zero
with `INVALID`; it does not silently discard a requirement. Input order is
irrelevant and duplicates are idempotent, so an application/compiler can concatenate
requirements without needing a core set-union operation. Empty lists ignore the
pointer; nonempty lists require readable storage. Reserved fields must be zero.
The usual explicit dependencies still apply (for example task → mesh → raster).
No dependency or unsupported feature is enabled implicitly.

Input arrays are borrowed only through the host call. Shader requirement arrays
must remain alive through executable preparation; executables do not retain them.
`QUERY_FEATURES` still returns one `ogpu_next_feature_info`, now containing two
counted lists plus the existing baseline/timeline/scope metadata. Its sorted,
unique available/enabled IDs borrow immutable adapter/device snapshot storage;
callers must not modify or free it. Adapter lists expire with discovery, device
lists with the device. Copy IDs to keep them longer. Cloned device snapshots own
their storage independently of discovery. Querying itself does not allocate.

This borrowing makes discovery a single query without another output-buffer
protocol or hot-path object. C callers own the lifetime obligation. Native Rust's
`rust::adapter_features` returns a `FeatureSupport<'_>` with borrowed slices tied
to the adapter reference; `rust::device_create` accepts `&[u32]`. Nested C-shaped
shader records use `Features::from_slice` and retain their existing unsafe host-call
obligations. This is not a general safe ownership wrapper.

## Cost and implementation boundary

Device creation and executable preparation decode requests once. Existing command
checks remain direct private native-enable checks. No list traversal, allocation,
resource tracking or fallback is added to recording, submission or GPU work.
The private implementation now represents the 67 native enables in a `u128`;
it can grow or change that encoding without another public representation change.
The public list count is not a mask width. Unsupported future IDs are rejected
because the runtime does not implement them, not because the ABI cannot represent
them. A focused host test accepts a 129-element duplicate request.

The large C validation fixture keeps its own bounded test-family mask and converts
it to IDs before calling OGPU. This is fixture policy, not an alternate public
interface. The application examples instead combine generated lists in caller
storage. The small merge helpers are example code, shipped with each example,
not new runtime exports. Generated stage pairs concatenate requirements; repeated
RASTER IDs are legal. Generated compatibility checks inspect enabled IDs, not just
physical availability.

## Migration and remaining verification

Rebuild the header, runtime, native Rust consumers and generated interfaces
at the same source commit. Revision-8 records are rejected; old masks cannot be
reinterpreted as lists. All feature constants change representation, while format,
stage, access and other unrelated flags remain masks. There is no compatibility
adapter in the public API.

Source checks cover C/Rust records, every feature constant, current operation
signatures, malformed/unknown/duplicate lists, snapshot lifetime independence,
generated compute/graphics metadata, and existing feature dependency rejection.
Local Radeon execution covers the existing device/feature tests and full C fixture.
[Installed C and Rust/Wayland acceptance](results/m8-feature-selection-2026-11-06.md)
now passes; the receipt identifies both clean commits and unchanged C artifact
identity after the final Rust-consumer fix. Windows/Metal foundation acceptance remains open; no new backend
execution or global performance claim follows from this representation change.

Revision 10 now adds [multiview](multiview.md) using this same counted-list
representation: native enabling/limits, shader requirements, pipeline/render/
secondary masks and array-layer correctness. Shading rate, tile-local/pass
continuation and remaining stages follow in M8; ray tracing remains M9.

Revision 11 adds [shading-rate inputs](shading-rate.md), including feature IDs above
64, without changing the public counted-list representation. Private storage was
widened before that addition; callers never size or interpret it.
