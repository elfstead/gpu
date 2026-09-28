# Two-slot arena/replay consumer

Copy this entire directory outside the installation, set `PKG_CONFIG_PATH` to the
installed `lib/pkgconfig`, then run:

```sh
python3 build.py
python3 run.py                 # 1,000 checked frames; retains a fresh run-* directory
python3 run.py --frames 12     # smoke check, not sustained acceptance
```

Needs a C11 compiler/linker, pkg-config, Python standard library and a compatible
Linux x86-64 Vulkan device/loader. No Cargo, Slang, Vulkan headers, source checkout,
download or preexisting fixtures. Select one device using `VK_DRIVER_FILES` when
multiple ICDs are installed. See the SDK QUICKSTART for the exact device baseline.
Metal is not supported by this installer or this HOST-view/compiled-list handoff.
The SDK is revision-pinned and experimental, not a stable ABI release.

This is the measured learned-image consumer, not a second implementation or a new
allocator API. `main.c` enables its public C API branch. `support/` preserves the
shared source layout (including slot submission/drain helpers from `small.c`);
native controls are excluded and their dependencies are not shipped. Generated
headers embed the shaders. The model and independent binary64 CPU oracle generate
two deterministic 65x47 inputs, resized to 131x95, locally on each run. There is no
training or shader compilation step.

The runner first checks 12 one-slot dedicated/re-record frames against the CPU
oracle and captures same-device serial images. It then runs two arena-backed slots
with one serial compiled list each. Every output must match both its CPU oracle
(RGB error <=1, alpha exact) and serial image (byte-exact). Each slot alternates its
input on reuse. Guards, input/weight integrity and HOST padding are checked after
drain. A JSON report retains SDK/source/executable identity, fixtures, policy,
device, ranges, results and process logs. Errors/timeouts leave `complete: false`.
This is correctness/handoff evidence, not performance or external adoption evidence.

## Ownership obligations

- The application owns three backing arenas, dedicated images and draw arguments.
  Ranges borrow their parent's handles; never destroy a borrowed slice separately.
- Query each HOST view's actual cache granularity and coherence. Slot ranges own
  whole cache atoms; flush before submission and invalidate after completion when
  noncoherent. The 4 KiB per-range capacity budget is not an assumed cache atom.
  Setup rejects insufficient capacity; no silent growth or per-frame allocation.
- Externally serialize helper access. A generation ticket is local to its slot.
  Only confirmed completion followed by CPU consumption permits range reuse.
  Pending/ready slots reject reacquisition; direct pointer writes bypass this check.
- Retain buffers/images/shaders through execution AND compiled-list lifetime.
  Each list is serial: don't resubmit while its previous execution is pending.
  GPU barriers stay explicit even after host waits.
- This executable exits on API errors; it does not retry or recover a lost device.
  Unknown submit outcomes quarantine; failed waits do not authorize output use.
  Teardown drains every accepted receipt, destroys lists/recordings, then resources
  and recording storage/device. The wait/submit error contracts supply the drain
  guarantee. Destroying a pending completion also drains, but cannot return an
  error diagnostic; explicitly wait first when the outcome matters.

Reports preserve partial stdout/stderr on failure. The separate repository fault
suite covers synthetic failure/recovery transitions; its loader shim is deliberately
not part of this distributable application. Real device loss, noncoherent hardware,
multi-thread/queue use, image pooling and arbitrary workloads are not validated here.
