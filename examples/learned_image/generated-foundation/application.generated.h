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
#define application_feature_ids fullscreen_feature_ids, display_feature_ids
#define application_feature_count (fullscreen_feature_count + display_feature_count)
static const ogpu_next_feature application_feature_storage[] = {application_feature_ids};
#define application_feature_list {application_feature_count ? application_feature_storage : NULL, application_feature_count, 0}
static const ogpu_next_features application_required_features = application_feature_list;
static const ogpu_next_argument_field application_argument_field = {
    0, application_push_size, OGPU_NEXT_ARGUMENT_INLINE, 0, OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT, OGPU_NEXT_ARGUMENT_DIRECT, OGPU_NEXT_ARGUMENT_DIRECT};
static const ogpu_next_argument_layout_desc application_arguments = {
    OGPU_NEXT_ARGUMENT_GRAPHICS, application_push_size, (application_push_size) != 0, 0, (application_push_size) ? &application_argument_field : NULL};
static const ogpu_next_shader_requirements application_requirements = {
    .header = {OGPU_NEXT_SHADER_REQUIREMENTS, OGPU_NEXT_VERSION, sizeof(ogpu_next_shader_requirements), 0, NULL},
    .features = application_feature_list, .local_size = {0, 0, 0}, .shared_memory = 0};
static inline void application_shaders(ogpu_next_shader out[2]) {
    out[0] = fullscreen_shader(); out[1] = display_shader();
}
static inline int application_compatible(const ogpu_next_feature_info *c, const ogpu_next_execution_limits *l) {
    return fullscreen_compatible(c, l) && display_compatible(c, l);
}
#endif
