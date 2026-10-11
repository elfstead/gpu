// Consumer-local compiler bridge. No upstream processing code is vendored here.
#pragma once
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
struct shader_binary { uint32_t *words; size_t count; };
struct shader_constant { uint32_t id, bits; };
struct shader_metadata { uint32_t local[3], shared; };
// Cold consumer preparation; no mutation of the original binary/constants.
int reflect_compute_native(const struct shader_binary *, const struct shader_constant *,
                           uint32_t count, struct shader_metadata *out);
// stage: 0 compute, 1 fragment, 2 vertex; vertex requires the two validated names.
int compile_native(const char *source, int stage, const char *uv, const char *pos,
                   uint32_t push_bytes, struct shader_binary *out);
#ifdef __cplusplus
}
#endif
