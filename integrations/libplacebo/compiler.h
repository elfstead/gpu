// Consumer-local compiler bridge. No upstream processing code is vendored here.
#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
struct shader_binary { uint32_t *words; size_t count; };
// stage: 0 compute, 1 fragment, 2 vertex; vertex requires the two validated names.
int compile_native(const char *source, int stage, const char *uv, const char *pos,
                   uint32_t push_bytes, struct shader_binary *out);
#ifdef __cplusplus
}
#endif
