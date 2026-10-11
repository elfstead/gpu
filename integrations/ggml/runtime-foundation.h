#pragma once
// GGML-owned synchronous policy, not a runtime compatibility wrapper. No native API.
#include "ogpu_next.h"
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace gpu {
#define GGML_RECORD(type,kind) {kind,OGPU_NEXT_VERSION,sizeof(type),0,nullptr}
template <class T, void (*Destroy)(T *)> using Owner = std::unique_ptr<T, decltype(Destroy)>;
using Kernel = Owner<ogpu_next_executable,ogpu_next_executable_destroy>;
inline Kernel empty_kernel() { return Kernel(nullptr,ogpu_next_executable_destroy); }
inline void check(ogpu_next_status status) {
    if (status != OGPU_NEXT_OK) throw std::runtime_error("OGPU foundation status "+std::to_string(status));
}
inline ogpu_next_query query(uint32_t kind,void *data,uint32_t capacity,uint32_t size) {
    return {GGML_RECORD(ogpu_next_query,kind),data,capacity,0,size,0};
}
struct Memory {
    Owner<ogpu_next_memory,ogpu_next_memory_destroy> handle{nullptr,ogpu_next_memory_destroy};
    uint64_t address = 0;
    ogpu_next_mapping mapping{};
    void write(size_t off,const void *data,size_t size) {
        std::memcpy(static_cast<unsigned char *>(mapping.data)+off,data,size);
        check(ogpu_next_memory_flush({handle.get(),off,size}));
    }
    void read(size_t off,void *data,size_t size) {
        check(ogpu_next_memory_invalidate({handle.get(),off,size}));
        std::memcpy(data,static_cast<unsigned char *>(mapping.data)+off,size);
    }
};
struct Runtime {
    // Reverse destruction: recorded references are discarded before their device.
    Owner<ogpu_next_device,ogpu_next_device_destroy> device{nullptr,ogpu_next_device_destroy};
    Owner<ogpu_next_timeline,ogpu_next_timeline_destroy> timeline{nullptr,ogpu_next_timeline_destroy};
    Owner<ogpu_next_arena,ogpu_next_arena_destroy> arena{nullptr,ogpu_next_arena_destroy};
    ogpu_next_queue *queue = nullptr;
    ogpu_next_encoder *encoder = nullptr;
    bool dirty = false;
    uint64_t value = 0;
    ogpu_next_execution_limits limits{};
    std::vector<ogpu_next_memory_type_info> memory_types;
    std::vector<std::max_align_t> scratch;
    uint64_t scratch_size = 0;
    std::string description;
    void initialize(uint32_t index) {
        ogpu_next_discovery *raw = nullptr; check(ogpu_next_discovery_create(OGPU_NEXT_VERSION,&raw));
        Owner<ogpu_next_discovery,ogpu_next_discovery_destroy> discovery(raw,ogpu_next_discovery_destroy);
        if (index >= ogpu_next_discovery_count(raw)) throw std::runtime_error("invalid OGPU device index");
        const auto *adapter = ogpu_next_discovery_adapter(raw,index);
        ogpu_next_adapter_info info{}; auto q = query(OGPU_NEXT_QUERY_INFO,&info,1,sizeof(info)); check(ogpu_next_adapter_query(adapter,&q));
        description = info.name;
        ogpu_next_feature_info features{}; q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); check(ogpu_next_adapter_query(adapter,&q));
        // The baseline includes storageBuffer16BitAccess, not Float16 arithmetic.
        if (!features.baseline_supported) throw std::runtime_error("modern foundation baseline required");
        q = query(OGPU_NEXT_QUERY_QUEUES,nullptr,0,sizeof(ogpu_next_queue_info)); check(ogpu_next_adapter_query(adapter,&q));
        std::vector<ogpu_next_queue_info> queues(q.count);
        q.data = queues.data(); q.capacity = q.count; check(ogpu_next_adapter_query(adapter,&q));
        uint32_t domain = UINT32_MAX;
        for (const auto &candidate : queues) if (candidate.count && (candidate.flags & OGPU_NEXT_QUEUE_COMPUTE)) { domain = candidate.domain; break; }
        if (domain == UINT32_MAX) throw std::runtime_error("compute queue required");
        const ogpu_next_queue_request request{domain,1,.5f};
        const ogpu_next_device_desc desc{GGML_RECORD(ogpu_next_device_desc,OGPU_NEXT_DEVICE_DESC),&request,1,0,0};
        ogpu_next_device *d = nullptr; check(ogpu_next_device_create(adapter,&desc,&d)); device.reset(d);
        queue = ogpu_next_device_queue(d,domain,0);
        q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS,&limits,1,sizeof(limits)); check(ogpu_next_device_query(d,&q));
        if (limits.max_groups[0] < 16384 || limits.max_inline_size < 40 || limits.max_local_size[0] < 64 ||
            limits.max_local_invocations < 64 || limits.max_shared_memory < 512)
            throw std::runtime_error("GGML shader profile exceeds device limits");
        q = query(OGPU_NEXT_QUERY_MEMORY_TYPES,nullptr,0,sizeof(ogpu_next_memory_type_info)); check(ogpu_next_device_query(d,&q));
        memory_types.resize(q.count); q.data = memory_types.data(); q.capacity = q.count; check(ogpu_next_device_query(d,&q));
        ogpu_next_host_requirements req{}; check(ogpu_next_submit_scratch_requirements(1,0,1,&req));
        if (req.alignment > alignof(std::max_align_t)) throw std::runtime_error("unsupported scratch alignment");
        scratch.resize((req.size+sizeof(std::max_align_t)-1)/sizeof(std::max_align_t)); scratch_size = req.size;
        ogpu_next_timeline *t = nullptr; check(ogpu_next_timeline_create(d,0,&t)); timeline.reset(t);
        const ogpu_next_arena_desc ad{GGML_RECORD(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
        ogpu_next_arena *a = nullptr; check(ogpu_next_arena_create(d,&ad,&a)); arena.reset(a);
    }
    Memory allocate(size_t size,bool host) {
        ogpu_next_memory_desc desc{GGML_RECORD(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),size,64,
            OGPU_NEXT_USAGE_STORAGE | OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,nullptr,0,0};
        std::vector<uint32_t> compatible(memory_types.size());
        ogpu_next_requirements req{}; req.compatible_types = compatible.data(); req.compatible_type_capacity = static_cast<uint32_t>(compatible.size());
        check(ogpu_next_memory_requirements(device.get(),&desc,&req));
        const uint32_t required = host ? OGPU_NEXT_MEMORY_HOST_VISIBLE : OGPU_NEXT_MEMORY_LOCAL;
        for (uint32_t i = 0; i < req.compatible_type_count && desc.memory_type == UINT32_MAX; ++i)
            for (const auto &type : memory_types) if (type.id == compatible[i] && (type.properties & required) == required) { desc.memory_type = type.id; break; }
        if (desc.memory_type == UINT32_MAX) throw std::runtime_error("no compatible requested memory type");
        ogpu_next_memory *raw = nullptr; check(ogpu_next_memory_create(device.get(),&desc,&raw));
        Memory memory; memory.handle.reset(raw); check(ogpu_next_memory_address({raw,0,size},&memory.address));
        if (host) check(ogpu_next_memory_map({raw,0,size},&memory.mapping));
        return memory;
    }
    Kernel kernel(const void *code,size_t bytes,uint32_t root_size,uint32_t shared) {
        const ogpu_next_argument_interface interface{GGML_RECORD(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),root_size,0,nullptr};
        const ogpu_next_shader shader{OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,bytes},"main",&interface.header,nullptr,nullptr};
        const ogpu_next_shader_requirements requirements{GGML_RECORD(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),0,{64,1,1},shared};
        const ogpu_next_executable_desc desc{GGML_RECORD(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),
            OGPU_NEXT_EXECUTABLE_COMPUTE,1,&shader,nullptr,0,&requirements.header,nullptr,0,0};
        ogpu_next_executable *raw = nullptr; check(ogpu_next_executable_create(device.get(),&desc,&raw)); return Kernel(raw,ogpu_next_executable_destroy);
    }
    void begin() {
        // Successful callbacks already discarded their references. Reset here
        // only when recovering from a local recording/end failure.
        if (dirty) check(ogpu_next_arena_reset(arena.get()));
        const ogpu_next_recording_desc desc{GGML_RECORD(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,OGPU_NEXT_PRIMARY,nullptr};
        dirty = true;
        check(ogpu_next_commands_begin(arena.get(),&desc,&encoder));
    }
    void barrier(bool compute) {
        const ogpu_next_dependency dep{GGML_RECORD(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COMPUTE | OGPU_NEXT_STAGE_COPY,
            compute ? OGPU_NEXT_STAGE_COMPUTE : OGPU_NEXT_STAGE_COPY,
            OGPU_NEXT_ACCESS_STORAGE_READ | OGPU_NEXT_ACCESS_STORAGE_WRITE | OGPU_NEXT_ACCESS_COPY_READ | OGPU_NEXT_ACCESS_COPY_WRITE,
            compute ? OGPU_NEXT_ACCESS_STORAGE_READ | OGPU_NEXT_ACCESS_STORAGE_WRITE : OGPU_NEXT_ACCESS_COPY_READ | OGPU_NEXT_ACCESS_COPY_WRITE,
            0,0,0,nullptr,nullptr,nullptr,0};
        ogpu_next_barrier(encoder,&dep);
    }
    void copy(Memory &dst,size_t dst_off,Memory &src,size_t src_off,size_t bytes) {
        ogpu_next_copy_memory(encoder,{dst.handle.get(),dst_off,bytes},{src.handle.get(),src_off,bytes});
    }
    void dispatch(const Kernel &kernel,uint32_t groups,const void *root,uint32_t size) {
        ogpu_next_bind_executable(encoder,kernel.get()); ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_COMPUTE,0,size,root);
        const ogpu_next_launch launch{{groups,1,1},0,nullptr}; ogpu_next_dispatch(encoder,&launch);
    }
    void finish() {
        // HOST reads after graph/copy completion use explicit visibility + invalidate.
        const ogpu_next_dependency dep{GGML_RECORD(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COMPUTE | OGPU_NEXT_STAGE_COPY,
            OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_STORAGE_WRITE | OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ,
            0,0,0,nullptr,nullptr,nullptr,0};
        ogpu_next_barrier(encoder,&dep);
        ogpu_next_list *list = nullptr; check(ogpu_next_commands_end(encoder,&list)); encoder = nullptr;
        if (value == std::numeric_limits<uint64_t>::max()) throw std::runtime_error("timeline exhausted");
        const ogpu_next_sync_point signal{{timeline.get(),++value},OGPU_NEXT_STAGE_ALL};
        const ogpu_next_submit_desc submit{GGML_RECORD(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,nullptr,&signal,scratch.data(),scratch_size};
        const auto submitted = ogpu_next_queue_submit(queue,&submit);
        // Unknown completion must never unwind borrowed allocation lifetimes.
        if (submitted != OGPU_NEXT_OK || ogpu_next_timeline_wait(signal.point,UINT64_MAX) != OGPU_NEXT_OK) {
            std::fprintf(stderr,"OGPU foundation submission/completion failed; refusing unsafe resource teardown\n"); std::abort();
        }
        // No retained references into GGML-owned allocations between callbacks.
        check(ogpu_next_arena_reset(arena.get()));
        dirty = false;
    }
};
#undef GGML_RECORD
} // namespace gpu
