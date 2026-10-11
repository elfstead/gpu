/* Generated executable interfaces; application policy is not generated. */
#ifndef OGPU_LEARNED_IMAGE_APPLICATION_H
#define OGPU_LEARNED_IMAGE_APPLICATION_H
#include "hidden.generated.h"
#include "denoise.generated.h"
#include "process.generated.h"
#include "poison.generated.h"
#include "fullscreen.generated.h"
#include "display.generated.h"
enum { application_push_size = 16 };
/* One shared native inline bank, not independently snapshotted stage banks.
 * Stage structs are views of these SAME bytes. The caller must supply
 * simultaneously valid values; this linker never relocates or packs roots. */
#define application_required_features (fullscreen_required_features | display_required_features)
static const ogpu_next_argument_interface application_interface = {
    .header = {OGPU_NEXT_ARGUMENT_INTERFACE, OGPU_NEXT_VERSION, sizeof(ogpu_next_argument_interface), 0, NULL},
    .byte_size = application_push_size, .root_count = 0, .roots = NULL};
static const ogpu_next_shader_requirements application_requirements = {
    .header = {OGPU_NEXT_SHADER_REQUIREMENTS, OGPU_NEXT_VERSION, sizeof(ogpu_next_shader_requirements), 0, NULL},
    .features = application_required_features, .local_size = {0, 0, 0}, .shared_memory = 0};
static inline void application_shaders(ogpu_next_shader out[2]) {
    out[0] = fullscreen_shader(); out[1] = display_shader();
    out[0].interface_metadata = out[1].interface_metadata = &application_interface.header;
}
static inline int application_compatible(const ogpu_next_feature_info *c, const ogpu_next_execution_limits *l) {
    return fullscreen_compatible(c, l) && display_compatible(c, l);
}
#endif
