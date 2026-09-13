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

static void rejection_checks(mnist_model &model, ggml_cgraph *graph,
                             const OgpuGgmlSession &session) {
    auto backend = model.backends[0];
    auto *first = ggml_graph_node(graph, 0);
    for (int i = 0; first->op == GGML_OP_NONE && i < ggml_graph_n_nodes(graph); ++i)
        first = ggml_graph_node(graph, i);
    require(first->op != GGML_OP_NONE, "missing first operation");
    std::vector<float> before(ggml_nelements(first), -12345.0f), after(before.size());
    ggml_backend_tensor_set(first, before.data(), 0, ggml_nbytes(first));
    const auto count = session.dispatch_count();
    auto *last = model.logits;
    const auto saved = last->op;
    last->op = GGML_OP_MUL; // Supported earlier nodes must not execute either.
    require(!ggml_backend_supports_op(backend, last), "advertised unsupported multiply");
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "unsupported graph accepted");
    last->op = saved;
    ggml_backend_tensor_get(first, after.data(), 0, ggml_nbytes(first));
    require(before == after && count == session.dispatch_count(),
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
    auto *bias_add = first;
    for (int i = 0; i < ggml_graph_n_nodes(graph); ++i) {
        auto *node = ggml_graph_node(graph, i);
        if (node->op == GGML_OP_ADD) {
            bias_add = node;
            break;
        }
    }
    require(bias_add->op == GGML_OP_ADD, "missing bias operation");
    saved_data = bias_add->data;
    saved_buffer = bias_add->buffer;
    bias_add->data = static_cast<char *>(bias_add->src[0]->data) + sizeof(float);
    bias_add->buffer = bias_add->src[0]->buffer;
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "partially overlapping elementwise output accepted");
    bias_add->data = saved_data;
    bias_add->buffer = saved_buffer;
    auto *bias = bias_add->src[1];
    saved_data = bias->data;
    saved_buffer = bias->buffer;
    bias->data = bias_add->data;
    bias->buffer = bias_add->buffer;
    require(ggml_backend_graph_compute(backend, graph) == GGML_STATUS_FAILED,
            "broadcast bias alias accepted");
    bias->data = saved_data;
    bias->buffer = saved_buffer;
    ggml_backend_tensor_get(first, after.data(), 0, ggml_nbytes(first));
    require(before == after && count == session.dispatch_count(), "rejected graph changed data");
}

static bool supported_graph(mnist_model &model, ggml_cgraph *graph) {
    for (int i = 0; i < ggml_graph_n_nodes(graph); ++i) {
        auto *node = ggml_graph_node(graph, i);
        if (!ggml_backend_supports_op(model.backends[0], node))
            return false;
    }
    return true;
}

static void check_placement(mnist_model &model, ggml_cgraph *graph) {
    require(ggml_backend_sched_get_n_splits(model.backend_sched) == 1, "unexpected graph split");
    for (int i = 0; i < ggml_graph_n_nodes(graph); ++i) {
        auto *node = ggml_graph_node(graph, i);
        if (node->op != GGML_OP_NONE)
            require(ggml_backend_sched_get_tensor_backend(model.backend_sched, node) ==
                        model.backends[0],
                    "scheduler selected fallback backend");
    }
}

static ggml_status scheduled_compute(mnist_model &model, ggml_cgraph *graph) {
    // This acceptance workflow intentionally disallows GGML's normal fallback.
    // Validate the entire graph before scheduler execution, then verify placement.
    if (!supported_graph(model, graph))
        return GGML_STATUS_FAILED;
    check_placement(model, graph);
    return ggml_backend_sched_graph_compute(model.backend_sched, graph);
}

static void evaluate(const char *weights, const std::vector<uint8_t> &images,
                     const std::vector<uint8_t> &labels, int batch_size,
                     const OgpuGgmlSession &session, bool scheduled, OgpuGgmlMemory memory) {
    auto cpu = mnist_model_init_from_file(weights, "CPU", batch_size, batch_size);
    auto gpu = mnist_model_init_from_file(weights, "OGPU", batch_size, batch_size);
    ggml_backend_cpu_set_n_threads(cpu.backends[0], 4);
    auto *cpu_graph = build(cpu);
    auto *gpu_graph = build(gpu);
    BufferOwner cpu_compute(ggml_backend_alloc_ctx_tensors(cpu.ctx_compute, cpu.backends[0]),
                            ggml_backend_buffer_free);
    BufferOwner gpu_compute(nullptr, ggml_backend_buffer_free);
    require(bool(cpu_compute), "CPU compute allocation failed");
    size_t separate_bytes = 0, allocated_bytes = 0;
    int aliases = 0;
    for (int i = 0; i < ggml_graph_n_nodes(gpu_graph); ++i) {
        auto *node = ggml_graph_node(gpu_graph, i);
        if (node->op != GGML_OP_NONE)
            separate_bytes += (ggml_nbytes(node) + 63) / 64 * 64;
    }
    if (scheduled) {
        require(supported_graph(gpu, gpu_graph), "unsupported graph before scheduling");
        ggml_backend_buffer_set_usage(gpu.buf_gguf, GGML_BACKEND_BUFFER_USAGE_WEIGHTS);
        require(ggml_backend_sched_alloc_graph(gpu.backend_sched, gpu_graph),
                "scheduler allocation failed");
        check_placement(gpu, gpu_graph);
        for (auto backend : gpu.backends)
            allocated_bytes += ggml_backend_sched_get_buffer_size(gpu.backend_sched, backend);
        for (int i = 0; i < ggml_graph_n_nodes(gpu_graph); ++i) {
            auto *node = ggml_graph_node(gpu_graph, i);
            if (node->op != GGML_OP_NONE && node->data == node->src[0]->data &&
                node->buffer == node->src[0]->buffer)
                ++aliases;
        }
        require(aliases == 3, "expected bias/ReLU storage reuse absent");
        require(allocated_bytes < separate_bytes, "scheduler did not reduce intermediate storage");
        const auto saved_op = gpu.logits->op;
        const auto initial_count = session.dispatch_count();
        gpu.logits->op = GGML_OP_MUL;
        require(scheduled_compute(gpu, gpu_graph) == GGML_STATUS_FAILED,
                "scheduler silently fell back");
        gpu.logits->op = saved_op;
        require(session.dispatch_count() == initial_count, "rejected scheduled graph dispatched");
    } else {
        gpu_compute.reset(ggml_backend_alloc_ctx_tensors(gpu.ctx_compute, gpu.backends[0]));
        require(bool(gpu_compute), "GPU compute allocation failed");
        allocated_bytes = ggml_backend_buffer_get_size(gpu_compute.get());
    }
    // No-op declarations neither execute nor extend an allocation lifetime. Keep
    // all graph metadata and externally allocated parameters alive during use.
    std::vector<std::pair<void *, ggml_backend_buffer_t>> allocation_snapshot;
    for (int i = 0; i < ggml_graph_n_nodes(gpu_graph); ++i) {
        auto *node = ggml_graph_node(gpu_graph, i);
        allocation_snapshot.emplace_back(node->data, node->buffer);
    }
    rejection_checks(gpu, gpu_graph, session);
    const auto setup = session.stats();
    const auto initial_dispatches = session.dispatch_count();
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
        require((scheduled ? scheduled_compute(gpu, gpu_graph)
                           : ggml_backend_graph_compute(gpu.backends[0], gpu_graph)) ==
                    GGML_STATUS_SUCCESS,
                "OGPU compute failed");
        require(session.dispatch_count() - initial_dispatches == 5 * (calls + 1),
                "per-call GPU dispatch count mismatch");
        for (int i = 0; i < ggml_graph_n_nodes(gpu_graph); ++i) {
            auto *node = ggml_graph_node(gpu_graph, i);
            require(allocation_snapshot[i] == std::make_pair(node->data, node->buffer),
                    "graph allocations changed during repeated execution");
        }
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
    require(session.dispatch_count() - initial_dispatches == 5 * calls, "wrong GPU dispatch count");
    require(correct >= 9000, "trained fixture accuracy below 90%");
    const auto stats = session.stats();
    require(stats.staging_allocations == setup.staging_allocations,
            "staging reallocated during repeated inference");
    require(stats.uploads - setup.uploads == 2 * calls && stats.downloads - setup.downloads == calls,
            "unexpected steady-state transfer count");
    require(stats.upload_bytes - setup.upload_bytes == calls * (input.size() + actual.size()) * 4 &&
                stats.download_bytes - setup.download_bytes == calls * actual.size() * 4,
            "unexpected steady-state transfer bytes (weights/intermediates moved?)");
    require(memory == OgpuGgmlMemory::Host ? stats.staging_allocations == 0
                                          : stats.staging_allocations > 0,
            "wrong staging policy");
    std::printf(
        "batch=%d mode=%s memory=%s images=10000 calls=%llu dispatches=%llu correct=%zu max_logit_error=%.9g "
        "intermediate_bytes=%zu separate_bytes=%zu aliases=%d PASS\n",
        batch_size, scheduled ? "scheduled" : "direct",
        memory == OgpuGgmlMemory::Host ? "host" : "device", static_cast<unsigned long long>(calls),
        static_cast<unsigned long long>(5 * calls), correct, max_error, allocated_bytes,
        separate_bytes, aliases);
    std::printf("memory_stats batch=%d mode=%s memory=%s setup_upload_bytes=%llu "
                "setup_transfer_ms=%.3f uploads=%llu downloads=%llu upload_bytes=%llu "
                "download_bytes=%llu transfer_ms=%.3f graph_ms=%.3f staging_allocations=%llu\n",
                batch_size, scheduled ? "scheduled" : "direct",
                memory == OgpuGgmlMemory::Host ? "host" : "device",
                static_cast<unsigned long long>(setup.upload_bytes), setup.transfer_ms,
                static_cast<unsigned long long>(stats.uploads - setup.uploads),
                static_cast<unsigned long long>(stats.downloads - setup.downloads),
                static_cast<unsigned long long>(stats.upload_bytes - setup.upload_bytes),
                static_cast<unsigned long long>(stats.download_bytes - setup.download_bytes),
                stats.transfer_ms - setup.transfer_ms, stats.graph_ms - setup.graph_ms,
                static_cast<unsigned long long>(stats.staging_allocations));
    std::fflush(stdout);
}

int main(int argc, char **argv) {
    if (argc != 6 && argc != 7) {
        std::fprintf(stderr,
                     "usage: %s model.gguf t10k-images t10k-labels shader-directory device-index [host|device]\n",
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
        const std::string placement = argc == 7 ? argv[6] : "device";
        require(placement == "host" || placement == "device", "bad memory placement");
        const auto memory = placement == "host" ? OgpuGgmlMemory::Host : OgpuGgmlMemory::Device;
        for (bool scheduled : {false, true})
            for (int batch : {1, 17, 64}) {
                // The upstream constructor passes registry order to a scheduler
                // requiring CPU last. Reorder statically linked CPU registration
                // before creating models; do not patch upstream or duplicate devices.
                auto cpu_reg = ggml_backend_reg_by_name("CPU");
                require(cpu_reg != nullptr, "CPU backend unavailable");
                ggml_backend_unload(cpu_reg);
                OgpuGgmlSession session(static_cast<uint32_t>(index), argv[4], memory);
                ggml_backend_register(cpu_reg);
                evaluate(argv[1], images, labels, batch, session, scheduled, memory);
                // Includes explicit GPU device/kernel teardown, not process-exit cleanup.
            }
        return 0;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "MNIST acceptance failed: %s\n", e.what());
        return 1;
    }
}
