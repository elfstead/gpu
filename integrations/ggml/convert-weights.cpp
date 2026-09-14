// Deterministic, CPU-only derivative fixtures. Never edits the pinned original.
#include "ggml.h"
#include "gguf.h"
#include <array>
#include <cmath>
#include <cstdio>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool value, const char *message) {
    if (!value) throw std::runtime_error(message);
}
using Context = std::unique_ptr<ggml_context, decltype(&ggml_free)>;
using Gguf = std::unique_ptr<gguf_context, decltype(&gguf_free)>;

int main(int argc, char **argv) {
    if (argc != 4) {
        std::fprintf(stderr, "usage: %s original.gguf half.gguf widened.gguf\n", argv[0]);
        return 2;
    }
    try {
        require(std::string(argv[1]) != argv[2] && std::string(argv[1]) != argv[3] &&
                    std::string(argv[2]) != argv[3], "distinct output paths required");
        ggml_context *raw = nullptr;
        const gguf_init_params params{false, &raw};
        Gguf input(gguf_init_from_file(argv[1], params), gguf_free);
        Context tensors(raw, ggml_free);
        require(bool(input) && bool(tensors), "cannot load original fixture");
        require(gguf_get_n_tensors(input.get()) == 4, "expected four model parameters");
        Gguf half(gguf_init_empty(), gguf_free), wide(gguf_init_empty(), gguf_free);
        gguf_set_kv(half.get(), input.get());
        gguf_set_kv(wide.get(), input.get());
        std::array<std::vector<ggml_fp16_t>, 2> rounded;
        std::array<std::vector<float>, 2> widened;
        size_t matrices = 0, biases = 0, original_bytes = 0, half_bytes = 0;
        // The allocated context also contains GGUF's raw I8 data blob. Iterate
        // declared file tensors, not every allocation in that context.
        for (int64_t i = 0; i < gguf_get_n_tensors(input.get()); ++i) {
            auto *t = ggml_get_tensor(raw, gguf_get_tensor_name(input.get(), i));
            require(t && t->type == GGML_TYPE_F32 && ggml_is_contiguous(t), "expected contiguous F32");
            const std::string name = ggml_get_name(t);
            gguf_add_tensor(half.get(), t);
            gguf_add_tensor(wide.get(), t);
            if (name == "fc1.weight" || name == "fc2.weight") {
                require(matrices < rounded.size(), "duplicate matrix weights");
                auto &h = rounded[matrices];
                auto &w = widened[matrices++];
                h.resize(ggml_nelements(t)); w.resize(h.size());
                ggml_fp32_to_fp16_row(static_cast<const float *>(t->data), h.data(), h.size());
                ggml_fp16_to_fp32_row(h.data(), w.data(), h.size());
                for (float value : w) require(std::isfinite(value), "nonfinite rounded weight");
                gguf_set_tensor_type(half.get(), name.c_str(), GGML_TYPE_F16);
                gguf_set_tensor_data(half.get(), name.c_str(), h.data());
                gguf_set_tensor_data(wide.get(), name.c_str(), w.data());
                original_bytes += ggml_nbytes(t); half_bytes += h.size() * sizeof(ggml_fp16_t);
            } else {
                require(name == "fc1.bias" || name == "fc2.bias", "unexpected parameter");
                ++biases;
            }
        }
        require(matrices == 2 && biases == 2 && original_bytes == 2 * half_bytes,
                "unexpected fixture payload");
        require(gguf_write_to_file(half.get(), argv[2], false) &&
                    gguf_write_to_file(wide.get(), argv[3], false), "cannot write derivatives");
        std::printf("matrix_payload original=%zu half=%zu biases=unchanged PASS\n", original_bytes, half_bytes);
        return 0;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "weight conversion failed: %s\n", e.what());
        return 1;
    }
}
