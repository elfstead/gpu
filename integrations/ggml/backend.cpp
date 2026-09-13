// Thin GGML consumer adapter; the GPU runtime remains the Rust library.
#include "backend.h"
#include "ggml-backend-impl.h"
#include "ogpu.h"

#include <algorithm>
#include <chrono>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
template <class T, void (*Destroy)(T *)> using Owner = std::unique_ptr<T, decltype(Destroy)>;
using Device = Owner<OgpuDevice, ogpu_device_destroy>;
using Kernel = Owner<OgpuKernel, ogpu_kernel_destroy>;
using Buffer = Owner<OgpuBuffer, ogpu_buffer_destroy>;
using Batch = Owner<OgpuBatch, ogpu_batch_destroy>;
using Completion = Owner<OgpuCompletion, ogpu_completion_destroy>;

void check(OgpuResult result, const OgpuError &error) {
    if (result != OGPU_SUCCESS)
        throw std::runtime_error(error.message);
}
#define GPU(call)                                                                                  \
    do {                                                                                           \
        OgpuError error{};                                                                         \
        const auto result = (call);                                                                \
        check(result, error);                                                                      \
    } while (false)

struct Root {
    uint64_t a, b, c;
    uint32_t m, n, k, operation;
};
static_assert(sizeof(Root) == 40 && offsetof(Root, a) == 0 && offsetof(Root, b) == 8 &&
              offsetof(Root, c) == 16 && offsetof(Root, m) == 24 && offsetof(Root, n) == 28 &&
              offsetof(Root, k) == 32 && offsetof(Root, operation) == 36);

struct State : std::enable_shared_from_this<State> {
    Device device{nullptr, ogpu_device_destroy};
    Kernel matrix{nullptr, ogpu_kernel_destroy}, element{nullptr, ogpu_kernel_destroy};
    uint64_t dispatches = 0;
    OgpuGgmlMemory memory = OgpuGgmlMemory::Device;
    OgpuGgmlStats stats;
    Buffer staging{nullptr, ogpu_buffer_destroy};
    size_t staging_size = 0;
    std::string description;
    ggml_backend_device device_interface{};
    ggml_backend_buffer_type buffer_type{};
    ggml_backend_reg registration{};
};

struct Measure {
    double &milliseconds;
    std::chrono::steady_clock::time_point start = std::chrono::steady_clock::now();
    ~Measure() {
        milliseconds += std::chrono::duration<double, std::milli>(
            std::chrono::steady_clock::now() - start).count();
    }
};

constexpr uint32_t gpu_access = OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE |
                                OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE;

Kernel load_kernel(State &state, const std::string &path) {
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    if (!file)
        throw std::runtime_error("cannot open shader: " + path);
    const auto bytes = file.tellg();
    if (bytes < 20 || bytes % 4 != 0)
        throw std::runtime_error("bad shader size");
    std::vector<uint32_t> words(static_cast<size_t>(bytes) / 4);
    file.seekg(0);
    if (!file.read(reinterpret_cast<char *>(words.data()), bytes))
        throw std::runtime_error("short shader read");
    OgpuKernel *raw = nullptr;
    const OgpuShaderDesc shader = {words.data(), words.size(), nullptr, 0, 0};
    GPU(ogpu_kernel_create(state.device.get(), &shader, sizeof(Root), &raw,
                           &error));
    return Kernel(raw, ogpu_kernel_destroy);
}

struct Allocation {
    std::shared_ptr<State> owner;
    Buffer buffer{nullptr, ogpu_buffer_destroy};
    uint64_t address = 0;
    // GGML performs pointer arithmetic on tensor data even for non-host buffers.
    // Real token storage makes that arithmetic valid; it is NOT a data mirror.
    std::vector<unsigned char> tokens;
    size_t size = 0;
    unsigned char *base() {
        const auto ptr = reinterpret_cast<uintptr_t>(tokens.data());
        return tokens.data() + ((64 - ptr % 64) % 64);
    }
};
Allocation &allocation(ggml_backend_buffer_t b) {
    if (!b || !b->context)
        throw std::runtime_error("foreign tensor buffer");
    return *static_cast<Allocation *>(b->context);
}
size_t offset(Allocation &a, const ggml_tensor *t, size_t off, size_t size) {
    const auto base = reinterpret_cast<uintptr_t>(a.base());
    const auto ptr = reinterpret_cast<uintptr_t>(t->data);
    if (ptr < base || ptr - base > a.size)
        throw std::runtime_error("invalid tensor token");
    const size_t begin = ptr - base;
    if (off > a.size - begin || size > a.size - begin - off)
        throw std::runtime_error("tensor buffer range exceeded");
    return begin + off;
}
uint64_t address(const ggml_tensor *t) {
    auto &a = allocation(t->buffer);
    const uint64_t result = a.address + offset(a, t, 0, ggml_nbytes(t));
    if (result % 4 != 0)
        throw std::runtime_error("unaligned tensor address");
    return result;
}
template <class F> void transfer(F f) noexcept {
    try {
        f();
    } catch (const std::exception &e) {
        // GGML's buffer callbacks cannot report failure. Never return stale data.
        std::fprintf(stderr, "OGPU tensor transfer failed: %s\n", e.what());
        std::abort();
    }
}

// These GGML callbacks and graph_compute are synchronous. A single session-owned
// staging buffer can therefore be reused after each copy completion is destroyed.
// No tensor mirror or placement migration; GPU allocations/addresses stay stable.
void copy_bytes(Allocation &a, size_t off, const void *input, void *output, size_t size) {
    if (off > a.size || size > a.size - off)
        throw std::runtime_error("allocation transfer range exceeded");
    if (size == 0)
        return;
    const bool upload = output == nullptr;
    auto &state = *a.owner;
    Measure measure{state.stats.transfer_ms};
    if (state.memory == OgpuGgmlMemory::Host) {
        if (upload)
            GPU(ogpu_buffer_write(a.buffer.get(), off, input, size, &error));
        else
            GPU(ogpu_buffer_read(a.buffer.get(), off, output, size, &error));
    } else {
        if (size > state.staging_size) {
            OgpuBuffer *raw = nullptr;
            GPU(ogpu_buffer_create(state.device.get(), size, OGPU_MEMORY_HOST, &raw, &error));
            state.staging.reset(raw);
            state.staging_size = size;
            ++state.stats.staging_allocations;
        }
        if (upload)
            GPU(ogpu_buffer_write(state.staging.get(), 0, input, size, &error));
        OgpuBatch *raw = nullptr;
        GPU(ogpu_batch_create(state.device.get(), &raw, &error));
        Batch batch(raw, ogpu_batch_destroy);
        // Covers earlier graphs/copies, including READ -> WRITE on reused storage.
        GPU(ogpu_batch_barrier(raw, gpu_access,
                               OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE, &error));
        GPU(ogpu_batch_copy_buffer(raw, upload ? state.staging.get() : a.buffer.get(),
                                   upload ? 0 : off, upload ? a.buffer.get() : state.staging.get(),
                                   upload ? off : 0, size, &error));
        OgpuCompletion *done = nullptr;
        GPU(ogpu_batch_submit(raw, &done, &error));
        Completion completion(done, ogpu_completion_destroy);
        GPU(ogpu_completion_wait(done, &error));
        if (!upload)
            GPU(ogpu_buffer_read(state.staging.get(), 0, output, size, &error));
    }
    if (upload) {
        ++state.stats.uploads;
        state.stats.upload_bytes += size;
    } else {
        ++state.stats.downloads;
        state.stats.download_bytes += size;
    }
}
void set_tensor(ggml_backend_buffer_t b, ggml_tensor *t, const void *data, size_t off,
                size_t size) {
    transfer([&] {
        auto &a = allocation(b);
        copy_bytes(a, offset(a, t, off, size), data, nullptr, size);
    });
}
void get_tensor(ggml_backend_buffer_t b, const ggml_tensor *t, void *data, size_t off,
                size_t size) {
    transfer([&] {
        auto &a = allocation(b);
        copy_bytes(a, offset(a, t, off, size), nullptr, data, size);
    });
}
void memset_tensor(ggml_backend_buffer_t b, ggml_tensor *t, uint8_t value, size_t off,
                   size_t size) {
    transfer([&] {
        std::vector<uint8_t> data(size, value);
        set_tensor(b, t, data.data(), off, size);
    });
}
void clear_buffer(ggml_backend_buffer_t b, uint8_t value) {
    transfer([&] {
        auto &a = allocation(b);
        std::vector<uint8_t> data(b->size, value);
        copy_bytes(a, 0, data.data(), nullptr, data.size());
    });
}
ggml_backend_buffer_t alloc_buffer(ggml_backend_buffer_type_t buft, size_t size) {
    try {
        auto state = static_cast<State *>(buft->context)->shared_from_this();
        auto a = std::make_unique<Allocation>();
        a->owner = state;
        if (size == 0 || size > PTRDIFF_MAX - 63)
            throw std::runtime_error("invalid buffer size");
        a->size = size;
        a->tokens.resize(size + 63);
        OgpuBuffer *raw = nullptr;
        GPU(ogpu_buffer_create(state->device.get(), size,
                               state->memory == OgpuGgmlMemory::Host ? OGPU_MEMORY_HOST : OGPU_MEMORY_DEVICE,
                               &raw, &error));
        a->buffer.reset(raw);
        GPU(ogpu_buffer_device_address(raw, &a->address, &error));
        ggml_backend_buffer_i iface{};
        iface.free_buffer = [](ggml_backend_buffer_t b) {
            delete static_cast<Allocation *>(b->context);
        };
        iface.get_base = [](ggml_backend_buffer_t b) -> void * {
            return static_cast<Allocation *>(b->context)->base();
        };
        iface.memset_tensor = memset_tensor;
        iface.set_tensor = set_tensor;
        iface.get_tensor = get_tensor;
        iface.clear = clear_buffer;
        auto result = ggml_backend_buffer_init(buft, iface, a.get(), size);
        if (result)
            a.release();
        return result;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "OGPU allocation failed: %s\n", e.what());
        return nullptr;
    }
}

bool tensor_profile(const ggml_tensor *t) {
    return t && t->type == GGML_TYPE_F32 && !t->view_src && ggml_is_contiguous(t) && t->ne[0] > 0 &&
           t->ne[0] <= 1024 && t->ne[1] > 0 && t->ne[1] <= 1024 && t->ne[2] == 1 && t->ne[3] == 1;
}
bool supports_op(ggml_backend_dev_t, const ggml_tensor *t) {
    if (!tensor_profile(t))
        return false;
    if (t->op == GGML_OP_NONE)
        return true;
    const auto *a = t->src[0];
    const auto *b = t->src[1];
    if (!tensor_profile(a))
        return false;
    if (t->op == GGML_OP_UNARY)
        return ggml_get_unary_op(t) == GGML_UNARY_OP_RELU && ggml_are_same_shape(t, a);
    if (!tensor_profile(b))
        return false;
    if (t->op == GGML_OP_ADD)
        return ggml_are_same_shape(t, a) && b->ne[0] == t->ne[0] && b->ne[1] == 1;
    if (t->op == GGML_OP_MUL_MAT)
        return a->ne[0] == b->ne[0] && t->ne[0] == a->ne[1] && t->ne[1] == b->ne[1];
    return false;
}

ggml_status graph_compute(ggml_backend_t backend, ggml_cgraph *graph) {
    try {
        auto state = *static_cast<std::shared_ptr<State> *>(backend->context);
        auto owned_address = [&](const ggml_tensor *tensor) {
            if (!tensor->buffer || tensor->buffer->buft != &state->buffer_type)
                throw std::runtime_error("foreign tensor buffer");
            return address(tensor);
        };
        // Complete preflight before allocating a batch: no partial graph execution.
        for (int i = 0; i < ggml_graph_n_nodes(graph); ++i) {
            const auto *t = ggml_graph_node(graph, i);
            if (!supports_op(&state->device_interface, t))
                throw std::runtime_error("unsupported graph operation/type/layout");
            (void)owned_address(t);
            if (t->op != GGML_OP_NONE) {
                for (int source_index = 0; source_index < 2; ++source_index) {
                    const auto *source = t->src[source_index];
                    if (!source)
                        continue;
                    const auto dst = owned_address(t), src = owned_address(source);
                    // Exact source-0 alias is safe for elementwise operations:
                    // invocation i reads/writes only element i. No restrict
                    // qualifier is used in the shader. Broadcast bias aliases,
                    // partial overlaps and all matrix aliases can race.
                    const bool safe_in_place = source_index == 0 && dst == src &&
                                               ggml_nbytes(t) == ggml_nbytes(source) &&
                                               (t->op == GGML_OP_ADD || t->op == GGML_OP_UNARY);
                    if (!safe_in_place && dst < src + ggml_nbytes(source) &&
                        src < dst + ggml_nbytes(t))
                        throw std::runtime_error("overlapping input/output tensors");
                }
            }
        }
        OgpuBatch *raw = nullptr;
        Measure measure{state->stats.graph_ms};
        GPU(ogpu_batch_create(state->device.get(), &raw, &error));
        Batch batch(raw, ogpu_batch_destroy);
        uint64_t dispatches = 0;
        for (int i = 0; i < ggml_graph_n_nodes(graph); ++i) {
            const auto *t = ggml_graph_node(graph, i);
            if (t->op == GGML_OP_NONE)
                continue;
            // Include READ -> WRITE for allocator reuse, and prior submissions.
            GPU(ogpu_batch_barrier(raw, gpu_access,
                                   OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &error));
            Root root{};
            root.a = address(t->src[0]);
            root.c = address(t);
            root.m = static_cast<uint32_t>(t->ne[0]);
            root.n = static_cast<uint32_t>(t->ne[1]);
            uint32_t groups = (root.m * root.n + 63) / 64;
            OgpuKernel *kernel = state->element.get();
            if (t->op == GGML_OP_MUL_MAT) {
                root.b = address(t->src[1]);
                root.k = static_cast<uint32_t>(t->src[0]->ne[0]);
                groups = ((root.m + 7) / 8) * ((root.n + 7) / 8);
                kernel = state->matrix.get();
            } else if (t->op == GGML_OP_ADD) {
                root.b = address(t->src[1]);
                root.operation = 1;
            }
            GPU(ogpu_batch_dispatch(raw, kernel, groups, 1, 1, &root, sizeof(root), &error));
            ++dispatches;
        }
        OgpuCompletion *done = nullptr;
        GPU(ogpu_batch_submit(raw, &done, &error));
        Completion completion(done, ogpu_completion_destroy);
        GPU(ogpu_completion_wait(done, &error));
        state->dispatches += dispatches;
        return GGML_STATUS_SUCCESS;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "OGPU graph failed: %s\n", e.what());
        return GGML_STATUS_FAILED;
    }
}

ggml_backend_t init_backend(ggml_backend_dev_t dev, const char *) {
    static ggml_guid guid = {0x6f, 0x67, 0x70, 0x75, 0x2d, 0x67, 0x67, 0x6d,
                             0x6c, 0,    0,    0,    0,    0,    0,    1};
    auto *backend = new (std::nothrow) ggml_backend{};
    if (!backend)
        return nullptr;
    auto *owner = new (std::nothrow)
        std::shared_ptr<State>(static_cast<State *>(dev->context)->shared_from_this());
    if (!owner) {
        delete backend;
        return nullptr;
    }
    backend->context = owner;
    backend->guid = &guid;
    backend->device = dev;
    backend->iface.get_name = [](ggml_backend_t) { return "OGPU"; };
    backend->iface.free = [](ggml_backend_t b) {
        delete static_cast<std::shared_ptr<State> *>(b->context);
        delete b;
    };
    backend->iface.graph_compute = graph_compute;
    return backend;
}
} // namespace

struct OgpuGgmlSession::Impl {
    std::shared_ptr<State> state = std::make_shared<State>();
};
uint64_t OgpuGgmlSession::dispatch_count() const { return impl->state->dispatches; }
OgpuGgmlStats OgpuGgmlSession::stats() const { return impl->state->stats; }

OgpuGgmlSession::~OgpuGgmlSession() {
    if (impl->state.use_count() != 1) {
        std::fprintf(stderr, "OGPU session destroyed with live backends/buffers\n");
        std::abort();
    }
    ggml_backend_unload(&impl->state->registration);
}

OgpuGgmlSession::OgpuGgmlSession(uint32_t index, const char *shaders, OgpuGgmlMemory memory)
    : impl(std::make_unique<Impl>()) {
    if (ggml_backend_reg_by_name("OGPU"))
        throw std::runtime_error("OGPU backend already registered");
    auto &state = impl->state;
    state->memory = memory;
    auto &device = state->device_interface;
    auto &buffer_type = state->buffer_type;
    auto &registration = state->registration;
    OgpuProbe *raw_probe = nullptr;
    GPU(ogpu_probe_create(OGPU_ABI_VERSION, &raw_probe, &error));
    Owner<OgpuProbe, ogpu_probe_destroy> probe(raw_probe, ogpu_probe_destroy);
    OgpuDeviceInfo info{};
    if (ogpu_probe_device_info(raw_probe, index, &info) != OGPU_SUCCESS)
        throw std::runtime_error("invalid OGPU device index");
    state->description = info.name;
    OgpuDevice *raw_device = nullptr;
    GPU(ogpu_device_create(raw_probe, index, &raw_device, &error));
    state->device.reset(raw_device);
    state->matrix = load_kernel(*state, std::string(shaders) + "/matrix.comp.spv");
    state->element = load_kernel(*state, std::string(shaders) + "/element.comp.spv");

    device.context = state.get();
    buffer_type.context = state.get();
    registration.context = state.get();
    buffer_type.device = &device;
    buffer_type.iface.get_name = [](ggml_backend_buffer_type_t) { return "OGPU"; };
    buffer_type.iface.alloc_buffer = alloc_buffer;
    buffer_type.iface.get_alignment = [](ggml_backend_buffer_type_t) -> size_t { return 64; };
    device.iface.get_name = [](ggml_backend_dev_t) { return "OGPU"; };
    device.iface.get_description = [](ggml_backend_dev_t dev) {
        return static_cast<State *>(dev->context)->description.c_str();
    };
    device.iface.get_memory = [](ggml_backend_dev_t, size_t *free, size_t *total) {
        *free = *total = 0;
    };
    device.iface.get_type = [](ggml_backend_dev_t) { return GGML_BACKEND_DEVICE_TYPE_GPU; };
    device.iface.get_props = [](ggml_backend_dev_t dev, ggml_backend_dev_props *props) {
        *props = {};
        props->name = "OGPU";
        props->description = static_cast<State *>(dev->context)->description.c_str();
        props->type = GGML_BACKEND_DEVICE_TYPE_GPU;
    };
    device.iface.init_backend = init_backend;
    device.iface.get_buffer_type = [](ggml_backend_dev_t dev) {
        return &static_cast<State *>(dev->context)->buffer_type;
    };
    device.iface.supports_op = supports_op;
    device.iface.supports_buft = [](ggml_backend_dev_t dev, ggml_backend_buffer_type_t b) {
        return b == &static_cast<State *>(dev->context)->buffer_type;
    };
    registration.api_version = GGML_BACKEND_API_VERSION;
    registration.iface.get_name = [](ggml_backend_reg_t) { return "OGPU"; };
    registration.iface.get_device_count = [](ggml_backend_reg_t) -> size_t { return 1; };
    registration.iface.get_device = [](ggml_backend_reg_t reg, size_t index) -> ggml_backend_dev_t {
        return index == 0 ? &static_cast<State *>(reg->context)->device_interface : nullptr;
    };
    device.reg = &registration;
    ggml_backend_register(&registration);
}
