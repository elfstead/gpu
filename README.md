# Open GPU Interface

A minimal, open GPU interface for graphics, compute, and machine learning, built around memory, pointers, programmable execution, and explicit synchronization.

Start with [the current design](docs/design.md), [working status](docs/plan.md),
and the [development roadmap](docs/roadmap.md).
The [whole-API design](docs/whole-api-design.md) describes the foundation and its remaining target surface.
To use the runtime from your own application, see the [Linux quickstart](docs/quickstart.md).
The default SDK exposes the [experimental foundation interface](docs/foundation-quickstart.md);
ABI20 packaging requires explicit legacy selection.
The [native Rust consumer](examples/foundation_rust/README.md) demonstrates direct
resource, compute, image and interactive Wayland graphics below the C ABI.
See the [support matrix](docs/support.md) and [upgrade policy](docs/release-policy.md)
for the experimental compatibility boundary.
For a reproducible starting point, see the [experimental source/SDK checkpoint](docs/release-checkpoint.md).
The [experiment ledger](docs/experiments.md) records the supporting evidence.

The Rust/C prototype implements explicit memory, queues, command storage, compute
and graphics through the Linux/Vulkan foundation, including an experimental
Wayland presentation path. Native Metal 4 compute remains on the older interface;
Linux interactive acceptance precedes Metal foundation migration.
See [building and testing](docs/development.md) and the [Metal backend](docs/metal.md).
Bounded integrations run [GGML MNIST, transformer and quantized-linear consumers](integrations/ggml/README.md)
and [libplacebo compute/raster processing](integrations/libplacebo/README.md)
through the public runtime API. A [learned-image application](examples/learned_image/README.md)
combines denoising, resize/color processing and offscreen rendering in one GPU
pipeline. The interface remains experimental.

Licensed under the [MIT License](LICENSE).
