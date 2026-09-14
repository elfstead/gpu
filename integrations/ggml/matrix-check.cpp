#include "backend.h"
#include "ggml-backend.h"
#include "ggml.h"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool value, const char *message) {
    if (!value) throw std::runtime_error(message);
}
using Context = std::unique_ptr<ggml_context, decltype(&ggml_free)>;
using Backend = std::unique_ptr<ggml_backend, decltype(&ggml_backend_free)>;
using Buffer = std::unique_ptr<ggml_backend_buffer, decltype(&ggml_backend_buffer_free)>;

static void check(const OgpuGgmlSession &session, int k, int m, int n) {
    Context ctx(ggml_init({1 << 20, nullptr, true}), ggml_free);
    Backend backend(ggml_backend_init_by_name("OGPU", nullptr), ggml_backend_free);
    require(bool(ctx) && bool(backend), "matrix check setup failed");
    auto *a = ggml_new_tensor_2d(ctx.get(), GGML_TYPE_F16, k, m);
    auto *b = ggml_new_tensor_2d(ctx.get(), GGML_TYPE_F32, k, n);
    auto *c = ggml_mul_mat(ctx.get(), a, b);
    auto *graph = ggml_new_graph(ctx.get());
    ggml_build_forward_expand(graph, c);
    Buffer ab(ggml_backend_alloc_buffer(backend.get(), ggml_nbytes(a) + 2), ggml_backend_buffer_free);
    Buffer bb(ggml_backend_alloc_buffer(backend.get(), ggml_nbytes(b)), ggml_backend_buffer_free);
    Buffer cb(ggml_backend_alloc_buffer(backend.get(), ggml_nbytes(c)), ggml_backend_buffer_free);
    require(bool(ab) && bool(bb) && bool(cb), "matrix check allocation failed");
    // Deliberately not four-byte aligned, and odd K makes alternating rows so too.
    require(ggml_backend_tensor_alloc(ab.get(), a, static_cast<char *>(ggml_backend_buffer_get_base(ab.get())) + 2) == GGML_STATUS_SUCCESS &&
                ggml_backend_tensor_alloc(bb.get(), b, ggml_backend_buffer_get_base(bb.get())) == GGML_STATUS_SUCCESS &&
                ggml_backend_tensor_alloc(cb.get(), c, ggml_backend_buffer_get_base(cb.get())) == GGML_STATUS_SUCCESS,
            "matrix check tensor allocation failed");
    std::vector<ggml_fp16_t> weights(k * m);
    std::vector<float> wide(weights.size()), input(k * n), actual(m * n);
    for (size_t i = 0; i < weights.size(); ++i) {
        const float value = i == 0 ? 8.0f : i % 7 == 0 ? 0.00006103515625f :
                            float(int(i % 19) - 9) / 8.0f;
        weights[i] = ggml_fp32_to_fp16(value);
        wide[i] = ggml_fp16_to_fp32(weights[i]);
    }
    for (size_t i = 0; i < input.size(); ++i)
        input[i] = i == 0 ? 1.0003f : float(int(i % 17) - 8) * 0.1234567f;
    ggml_backend_tensor_set(a, weights.data(), 0, ggml_nbytes(a));
    ggml_backend_tensor_set(b, input.data(), 0, ggml_nbytes(b));
    std::vector<ggml_fp16_t> readback(weights.size());
    ggml_backend_tensor_get(a, readback.data(), 0, ggml_nbytes(a));
    require(weights == readback, "two-byte-aligned half transfer changed bits");
    const auto count = session.dispatch_count();
    double max_error = 0;
    for (int pass = 0; pass < 3; ++pass) {
        std::fill(actual.begin(), actual.end(), std::numeric_limits<float>::quiet_NaN());
        ggml_backend_tensor_set(c, actual.data(), 0, ggml_nbytes(c));
        require(ggml_backend_graph_compute(backend.get(), graph) == GGML_STATUS_SUCCESS,
                "mixed matrix execution failed");
        ggml_backend_tensor_get(c, actual.data(), 0, ggml_nbytes(c));
        for (int y = 0; y < n; ++y) for (int x = 0; x < m; ++x) {
            double reference = 0;
            for (int j = 0; j < k; ++j) reference += double(wide[x * k + j]) * input[y * k + j];
            const double error = std::abs(double(actual[y * m + x]) - reference);
            require(std::isfinite(actual[y * m + x]) && error <= 1e-4 + 1e-4 * std::abs(reference),
                    "mixed matrix reference mismatch (including activation precision)");
            max_error = std::max(max_error, error);
        }
    }
    require(session.dispatch_count() == count + 3, "matrix dispatch count mismatch");
    const auto saved = *c;
    auto reject = [&] {
        require(!ggml_backend_supports_op(backend.get(), c), "advertised unsupported half operation");
        require(ggml_backend_graph_compute(backend.get(), graph) == GGML_STATUS_FAILED,
                "unsupported half graph executed");
        require(session.dispatch_count() == count + 3, "rejected half graph dispatched");
        *c = saved;
    };
    c->src[1] = ggml_new_tensor_2d(ctx.get(), GGML_TYPE_F16, k, n);
    reject(); // Half activations, with correct contiguous metadata.
    *c = *ggml_new_tensor_2d(ctx.get(), GGML_TYPE_F16, m, n);
    c->op = GGML_OP_MUL_MAT; c->src[0] = a; c->src[1] = b;
    reject(); // Half output.
    auto *half_element = ggml_new_tensor_2d(ctx.get(), GGML_TYPE_F16, m, n);
    c->op = GGML_OP_ADD; c->src[0] = half_element;
    c->src[1] = ggml_new_tensor_1d(ctx.get(), GGML_TYPE_F32, m);
    reject();
    auto *relu = ggml_relu(ctx.get(), half_element);
    *c = *relu;
    reject();
    void *saved_address = a->data;
    a->data = static_cast<char *>(saved_address) - 1; // In range, but odd address.
    require(ggml_backend_supports_op(backend.get(), c) &&
                ggml_backend_graph_compute(backend.get(), graph) == GGML_STATUS_FAILED &&
                session.dispatch_count() == count + 3,
            "misaligned half address executed");
    a->data = saved_address;
    std::vector<float> after(actual.size());
    ggml_backend_tensor_get(c, after.data(), 0, ggml_nbytes(c));
    require(after == actual, "rejected half graph changed output");
    std::printf("mixed_matrix k=%d m=%d n=%d alignment=2 repeated=3 max_error=%.9g PASS\n", k, m, n, max_error);
}

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    try {
        const std::string memory = argv[3];
        require(memory == "host" || memory == "device", "bad memory placement");
        size_t end = 0;
        const auto index = std::stoul(argv[2], &end);
        require(end == std::string(argv[2]).size() && index <= UINT32_MAX, "bad device index");
        OgpuGgmlSession session(static_cast<uint32_t>(index), argv[1],
                               memory == "host" ? OgpuGgmlMemory::Host : OgpuGgmlMemory::Device);
        check(session, 13, 11, 3);
        check(session, 1, 1, 1); // Detects accidental FP16 rounding of activations.
        return 0;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "mixed matrix check failed: %s\n", e.what());
        return 1;
    }
}
