#pragma once
#include <cstdint>

// Local, pinned GGML extension. Register exactly once, before constructing models.
// All backend instances and buffer operations are externally serialized.
void ogpu_ggml_register(uint32_t device_index, const char *shader_directory);
// Call after all models/backends/buffers are freed, before process static teardown.
void ogpu_ggml_shutdown();
uint64_t ogpu_ggml_dispatch_count();
