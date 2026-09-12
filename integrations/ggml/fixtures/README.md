# Fixed regression weights

`mnist-fc-f32.gguf` is the 1.6 MB FP32 trained model used for the original paired-
driver acceptance. [SHA256SUMS](SHA256SUMS) pins its exact bytes; routine acceptance
must check this hash, not merely print it. The fixture is licensed under the
repository's [MIT license](../../../LICENSE).

Prepared locally on 2026-09-12 using unmodified GGML revision
`7840aaba1989c6deeefede1d77d5aaf8f52b947e`, its CPU `mnist-train` tool, and the
MNIST training IDX files pinned in [dataset.sha256](../dataset.sha256). It uses
30 epochs, the upstream 57,000/3,000 train/validation split, random initialization,
and FP32 weights for the 784 → 500 → 10 fully connected network. No test images
were used for training. Original hardware: Ryzen 9 5900X; compiler Clang 21.1.8.
Model accuracy on the 10,000-image test set: 98.01%.

Training was not seeded, so repeating training cannot reproduce these exact
weights. Preserving this artifact makes numerical regression inputs reproducible.
`bash integrations/ggml/train.sh /path/to/pinned/ggml` remains a separate optional
preparation test and writes `target/ggml-data/mnist-fc-trained.gguf`, never this
fixture. Dataset attribution remains with MNIST's authors; no dataset images or
labels are redistributed here.
