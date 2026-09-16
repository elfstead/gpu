# Open GPU Interface

A minimal, open GPU interface for graphics, compute, and machine learning, built around memory, pointers, programmable execution, and explicit synchronization.

Start with [the current design](docs/design.md) and [working status and milestone plan](docs/plan.md).
For the implemented scope and a reproducible starting point, see the [two-consumer checkpoint](docs/checkpoint.md).
The [experiment ledger](docs/experiments.md) records the supporting evidence.

The Rust/C prototype supports address-based compute on native Metal and Vulkan, and
[offscreen graphics](docs/graphics.md) on Vulkan, in shared asynchronous batches.
See [building and testing](docs/development.md) and the [Metal backend](docs/metal.md).
Bounded integrations run [GGML's MNIST classifier](integrations/ggml/README.md)
and [libplacebo compute/raster processing](integrations/libplacebo/README.md)
through the public runtime API. A [learned-image application](examples/learned_image/README.md)
combines denoising, resize/color processing and offscreen rendering in one GPU
pipeline. The interface remains experimental.

Licensed under the [MIT License](LICENSE).
