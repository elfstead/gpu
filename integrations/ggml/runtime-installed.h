#pragma once
// Temporary migration control and existing Metal path, selected at build time.
#include "ogpu.h"
#include <memory>
#include <stdexcept>
#include <string>

namespace gpu {
template <class T, void (*Destroy)(T *)> using Owner = std::unique_ptr<T, decltype(Destroy)>;
using Kernel = Owner<OgpuKernel,ogpu_kernel_destroy>;
inline Kernel empty_kernel() { return Kernel(nullptr,ogpu_kernel_destroy); }
struct Memory {
    Owner<OgpuBuffer,ogpu_buffer_destroy> handle{nullptr,ogpu_buffer_destroy};
    uint64_t address = 0;
    void write(size_t off,const void *data,size_t size) {
        OgpuError e{}; if (ogpu_buffer_write(handle.get(),off,data,size,&e) != OGPU_SUCCESS) throw std::runtime_error(e.message);
    }
    void read(size_t off,void *data,size_t size) {
        OgpuError e{}; if (ogpu_buffer_read(handle.get(),off,data,size,&e) != OGPU_SUCCESS) throw std::runtime_error(e.message);
    }
};
struct Runtime {
    Owner<OgpuDevice,ogpu_device_destroy> device{nullptr,ogpu_device_destroy};
    Owner<OgpuBatch,ogpu_batch_destroy> batch{nullptr,ogpu_batch_destroy};
    std::string description;
    static void check(OgpuResult status,const OgpuError &e) {
        if (status != OGPU_SUCCESS) throw std::runtime_error(e.message);
    }
    void initialize(uint32_t index) {
        OgpuError e{}; OgpuProbe *p = nullptr;
        check(ogpu_probe_create(OGPU_ABI_VERSION,&p,&e),e);
        Owner<OgpuProbe,ogpu_probe_destroy> probe(p,ogpu_probe_destroy);
        OgpuDeviceInfo info{};
        if (ogpu_probe_device_info(p,index,&info) != OGPU_SUCCESS) throw std::runtime_error("invalid OGPU device index");
        description = info.name;
        OgpuDevice *d = nullptr; check(ogpu_device_create(p,index,&d,&e),e); device.reset(d);
        OgpuCapabilities enabled{}; check(ogpu_device_capabilities(d,&enabled,&e),e);
        if (!enabled.storage_buffer_16bit_access) throw std::runtime_error("FP16 storage support is required");
    }
    Memory allocate(size_t size,bool host) {
        OgpuError e{}; OgpuBuffer *raw = nullptr;
        const OgpuBufferDesc desc = {size,host ? OGPU_MEMORY_HOST : OGPU_MEMORY_DEVICE,0};
        check(ogpu_buffer_create(device.get(),&desc,&raw,&e),e);
        Memory m; m.handle.reset(raw); check(ogpu_buffer_device_address(raw,&m.address,&e),e); return m;
    }
    Kernel kernel(const void *code,size_t bytes,uint32_t root_size,uint32_t) {
        OgpuError e{}; OgpuKernel *raw = nullptr;
        const OgpuShaderDesc desc = {code,bytes,nullptr,nullptr,0,OGPU_SHADER_SPIRV,{0,0,0},0};
        check(ogpu_kernel_create(device.get(),&desc,root_size,&raw,&e),e); return Kernel(raw,ogpu_kernel_destroy);
    }
    void begin() {
        OgpuError e{}; OgpuBatch *raw = nullptr;
        check(ogpu_batch_create(device.get(),&raw,&e),e); batch.reset(raw);
    }
    void barrier(bool compute) {
        constexpr uint32_t access = OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE;
        OgpuError e{}; check(ogpu_batch_barrier(batch.get(),access,compute ? OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE :
            OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE,&e),e);
    }
    void copy(Memory &dst,size_t dst_off,Memory &src,size_t src_off,size_t bytes) {
        OgpuError e{}; check(ogpu_batch_copy_buffer(batch.get(),src.handle.get(),src_off,dst.handle.get(),dst_off,bytes,&e),e);
    }
    void dispatch(const Kernel &kernel,uint32_t groups,const void *root,uint32_t size) {
        OgpuError e{}; check(ogpu_batch_dispatch(batch.get(),kernel.get(),groups,1,1,root,size,&e),e);
    }
    void finish() {
        OgpuError e{}; OgpuCompletion *raw = nullptr;
        check(ogpu_batch_submit(batch.get(),&raw,&e),e);
        Owner<OgpuCompletion,ogpu_completion_destroy> done(raw,ogpu_completion_destroy);
        check(ogpu_completion_wait(raw,&e),e); batch.reset();
    }
};
} // namespace gpu
