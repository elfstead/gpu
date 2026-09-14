# Open GPU Interface

A minimal, open GPU interface for graphics, compute, and machine learning, built around memory, pointers, programmable execution, and explicit synchronization.

Start with [the current design](docs/design.md) and [working status and milestone plan](docs/plan.md).
The [experiment ledger](docs/experiments.md) records the supporting evidence.

The Rust/C prototype supports address-based compute and [offscreen graphics](docs/graphics.md)
in shared asynchronous batches. See [building and testing](docs/development.md).
The first consumer is [GGML's MNIST classifier](integrations/ggml/README.md),
running its forward graph through the public API.

Licensed under the [MIT License](LICENSE).
