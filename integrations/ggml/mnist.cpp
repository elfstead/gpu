#include "backend.h"
#include "mnist-common.h"

#include <cmath>
#include <cstdio>
#include <fstream>
#include <limits>
#include <memory>
#include <stdexcept>

static void require(bool condition, const char *message) {
    if (!condition)
        throw std::runtime_error(message);
}
static std::vector<uint8_t> read_file(const char *path, size_t size) {
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    require(file && file.tellg() == static_cast<std::streamoff>(size), "invalid dataset file size");
    std::vector<uint8_t> bytes(size);
    file.seekg(0);
    require(bool(file.read(reinterpret_cast<char *>(bytes.data()), size)), "short dataset read");
    return bytes;
}
static uint32_t be32(const std::vector<uint8_t> &b, size_t i) {
    return uint32_t(b[i]) << 24 | uint32_t(b[i + 1]) << 16 | uint32_t(b[i + 2]) << 8 | b[i + 3];
}
using BufferOwner = std::unique_ptr<ggml_backend_buffer, decltype(&ggml_backend_buffer_free)>;

static ggml_cgraph *build(mnist_model &model) {
    require(model.arch == "mnist-fc", "only the FC model is supported");
    for (auto *t : {model.fc1_weight, model.fc1_bias, model.fc2_weight, model.fc2_bias})
        require(t->type == GGML_TYPE_F32, "model weights must be F32");
    mnist_model_build(model); // The upstream application graph, unmodified.
    auto *graph = ggml_new_graph(model.ctx_compute);
    ggml_build_forward_expand(graph, model.logits);
    int operations = 0;
    for (int i = 0; i < ggml_graph_n_nodes(graph); ++i) {
        auto *t = ggml_graph_node(graph, i);
        if (t->op != GGML_OP_NONE)
            ++operations;
    }
    require(operations == 5, "upstream forward graph changed");
    return graph;
}

static void rejection_checks(mnist_model &model, ggml_cgraph *graph) {
    auto backend = model.backends[0];
    auto *first = ggml_graph_node(graph, 0);
    for (int i = 0; first->op == GGML_OP_NONE && i < ggml_graph_n_nodes(graph); ++i)
        first = ggml_graph_node(graph, i);
    require(first->op != GGML_OP_NONE, "missing first operation");
    std::vector<float> before(ggml_nelements(first), -12345.0f), after(before.size());
    ggml_backend_tensor_set(first, before.data(), 0, ggml_nbytes(first));
    const auto count = ogpu_ggml_dispatch_count();
    auto *last = model.logits;
    const auto saved = last->op;
    last->op = GGML_OP_MUL; // Supported earlier nodes must not execute either.
    require(!ggml_backend_supports_op(backend, last), "advertised unsupported multiply");
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "unsupported graph accepted");
    last->op = saved;
    ggml_backend_tensor_get(first, after.data(), 0, ggml_nbytes(first));
    require(before == after && count == ogpu_ggml_dispatch_count(),
            "partial execution of rejected graph");

    auto *half = ggml_new_tensor_2d(model.ctx_compute, GGML_TYPE_F16, 16, 16);
    require(!ggml_backend_supports_op(backend, half), "advertised FP16 support");
    auto *transposed = ggml_transpose(model.ctx_compute, model.images);
    require(!ggml_backend_supports_op(backend, transposed), "advertised strided view support");

    const auto saved_type = last->type;
    last->type = GGML_TYPE_F16;
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "FP16 graph accepted");
    last->type = saved_type;
    const auto saved_stride = last->nb[0];
    last->nb[0] += sizeof(float);
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "strided graph accepted");
    last->nb[0] = saved_stride;
    // Valid-sized alias into the first matrix's input buffer must also be rejected.
    auto *saved_data = first->data;
    auto *saved_buffer = first->buffer;
    first->data = first->src[0]->data;
    first->buffer = first->src[0]->buffer;
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "aliased matrix output accepted");
    first->data = saved_data;
    first->buffer = saved_buffer;
    ggml_backend_tensor_get(first, after.data(), 0, ggml_nbytes(first));
    require(before == after && count == ogpu_ggml_dispatch_count(), "rejected graph changed data");
}

static void evaluate(const char *weights, const std::vector<uint8_t> &images,
                     const std::vector<uint8_t> &labels, int batch_size) {
    // Upstream constructs fallback schedulers, but this driver never executes
    // those schedulers. Each graph is sent directly to its one named backend.
    auto cpu = mnist_model_init_from_file(weights, "CPU", batch_size, batch_size);
    auto gpu = mnist_model_init_from_file(weights, "OGPU", batch_size, batch_size);
    ggml_backend_cpu_set_n_threads(cpu.backends[0], 4);
    auto *cpu_graph = build(cpu);
    auto *gpu_graph = build(gpu);
    BufferOwner cpu_compute(ggml_backend_alloc_ctx_tensors(cpu.ctx_compute, cpu.backends[0]),
                            ggml_backend_buffer_free);
    BufferOwner gpu_compute(ggml_backend_alloc_ctx_tensors(gpu.ctx_compute, gpu.backends[0]),
                            ggml_backend_buffer_free);
    require(cpu_compute && gpu_compute, "compute allocation failed");
    rejection_checks(gpu, gpu_graph);
    const auto initial_dispatches = ogpu_ggml_dispatch_count();
    uint64_t calls = 0;
    size_t correct = 0;
    double max_error = 0;
    std::vector<float> input(784 * batch_size), reference(10 * batch_size),
        actual(reference.size());
    // All three sizes cover the complete test set. Tails are zero-padded; their
    // logits are also checked, but padded rows do not count toward accuracy.
    for (int start = 0; start < 10000; start += batch_size) {
        const int active = std::min(batch_size, 10000 - start);
        std::fill(input.begin(), input.end(), 0.0f);
        for (int i = 0; i < active * 784; ++i)
            input[i] = images[16 + start * 784 + i] / 255.0f;
        ggml_backend_tensor_set(cpu.images, input.data(), 0, input.size() * sizeof(float));
        ggml_backend_tensor_set(gpu.images, input.data(), 0, input.size() * sizeof(float));
        // Poison outputs to detect skipped writes, including tail lanes.
        std::fill(actual.begin(), actual.end(), std::numeric_limits<float>::quiet_NaN());
        ggml_backend_tensor_set(gpu.logits, actual.data(), 0, actual.size() * sizeof(float));
        require(ggml_backend_graph_compute(cpu.backends[0], cpu_graph) == GGML_STATUS_SUCCESS,
                "CPU compute failed");
        require(ggml_backend_graph_compute(gpu.backends[0], gpu_graph) == GGML_STATUS_SUCCESS,
                "OGPU compute failed");
        ++calls;
        ggml_backend_tensor_get(cpu.logits, reference.data(), 0, reference.size() * sizeof(float));
        ggml_backend_tensor_get(gpu.logits, actual.data(), 0, actual.size() * sizeof(float));
        for (size_t i = 0; i < actual.size(); ++i) {
            const double error = std::abs(double(actual[i]) - reference[i]);
            require(std::isfinite(actual[i]) && std::isfinite(reference[i]) &&
                        error <= 1e-4 + 1e-4 * std::abs(double(reference[i])),
                    "logit tolerance exceeded");
            max_error = std::max(max_error, error);
        }
        for (int row = 0; row < active; ++row) {
            const auto a = actual.begin() + row * 10, r = reference.begin() + row * 10;
            const auto prediction = std::max_element(a, a + 10) - a;
            require(prediction == std::max_element(r, r + 10) - r, "top-1 prediction mismatch");
            correct += prediction == labels[8 + start + row];
        }
    }
    require(ogpu_ggml_dispatch_count() - initial_dispatches == 5 * calls,
            "wrong GPU dispatch count");
    require(correct >= 9000, "trained fixture accuracy below 90%");
    std::printf(
        "batch=%d images=10000 calls=%llu dispatches=%llu correct=%zu max_logit_error=%.9g PASS\n",
        batch_size, static_cast<unsigned long long>(calls),
        static_cast<unsigned long long>(5 * calls), correct, max_error);
    std::fflush(stdout);
}

int main(int argc, char **argv) {
    if (argc != 6) {
        std::fprintf(stderr,
                     "usage: %s model.gguf t10k-images t10k-labels shader-directory device-index\n",
                     argv[0]);
        return 2;
    }
    try {
        const auto images = read_file(argv[2], 16 + 10000 * 784);
        const auto labels = read_file(argv[3], 8 + 10000);
        require(be32(images, 0) == 2051 && be32(images, 4) == 10000 && be32(images, 8) == 28 &&
                    be32(images, 12) == 28 && be32(labels, 0) == 2049 && be32(labels, 4) == 10000,
                "bad IDX headers");
        for (size_t i = 8; i < labels.size(); ++i)
            require(labels[i] < 10, "bad digit label");
        size_t end = 0;
        const auto index = std::stoul(argv[5], &end);
        require(end == std::string(argv[5]).size() && index <= UINT32_MAX, "bad device index");
        for (int batch : {1, 17, 64}) {
            struct Lifetime {
                ~Lifetime() { ogpu_ggml_shutdown(); }
            } lifetime;
            // The upstream constructor passes registry order to a scheduler
            // requiring CPU last. Reorder statically linked CPU registration
            // before creating models; do not patch upstream or duplicate devices.
            auto cpu_reg = ggml_backend_reg_by_name("CPU");
            require(cpu_reg != nullptr, "CPU backend unavailable");
            ggml_backend_unload(cpu_reg);
            ogpu_ggml_register(static_cast<uint32_t>(index), argv[4]);
            ggml_backend_register(cpu_reg);
            evaluate(argv[1], images, labels, batch);
            // Includes explicit GPU device/kernel teardown, not process-exit cleanup.
        }
        return 0;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "MNIST acceptance failed: %s\n", e.what());
        return 1;
    }
}
