#pragma once
#include <cstdint>

#include <memory>

enum class OgpuGgmlMemory { Host, Device };
struct OgpuGgmlStats {
    uint64_t uploads = 0, downloads = 0, upload_bytes = 0, download_bytes = 0;
    uint64_t staging_allocations = 0;
    double transfer_ms = 0, graph_ms = 0;
};

// Scoped, externally serialized registration. Destroy after associated models,
// backend streams and buffers, before process static teardown. No GPU statics.
class OgpuGgmlSession {
  public:
    OgpuGgmlSession(uint32_t device_index, const char *shader_directory,
                    OgpuGgmlMemory memory = OgpuGgmlMemory::Device);
    ~OgpuGgmlSession();
    OgpuGgmlSession(const OgpuGgmlSession &) = delete;
    OgpuGgmlSession &operator=(const OgpuGgmlSession &) = delete;
    uint64_t dispatch_count() const;
    OgpuGgmlStats stats() const;

  private:
    struct Impl;
    std::unique_ptr<Impl> impl;
};
