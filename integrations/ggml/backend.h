#pragma once
#include <cstdint>

#include <memory>

// Scoped, externally serialized registration. Destroy after associated models,
// backend streams and buffers, before process static teardown. No GPU statics.
class OgpuGgmlSession {
  public:
    OgpuGgmlSession(uint32_t device_index, const char *shader_directory);
    ~OgpuGgmlSession();
    OgpuGgmlSession(const OgpuGgmlSession &) = delete;
    OgpuGgmlSession &operator=(const OgpuGgmlSession &) = delete;
    uint64_t dispatch_count() const;

  private:
    struct Impl;
    std::unique_ptr<Impl> impl;
};
