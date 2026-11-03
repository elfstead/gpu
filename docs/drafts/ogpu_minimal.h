/* C1 consolidated surface INDEX, 2026-11-03. DESIGN ONLY, not an installed ABI.
 * The exact operation disposition is minimal-core-migration.json; semantics and
 * record deletions are ../minimal-core-migration.md. Imported installed declarations
 * supply shared types and unchanged operations, not permission to keep superseded
 * entry points. ogpu_draft_* names become ogpu_next_* at coordinated cutover.
 * Future M8-M11 coverage remains in ogpu_next.h; it is not C2 implemented support.
 */
#ifndef OGPU_MINIMAL_DESIGN_DRAFT_H
#define OGPU_MINIMAL_DESIGN_DRAFT_H
#include "ogpu_arguments.h"
#include "ogpu_descriptors.h"
#include "ogpu_presentation.h"
#include "ogpu_dependencies.h"
#ifdef __cplusplus
extern "C" {
#endif
enum { OGPU_DRAFT_INHERITANCE_DESC = 3002, OGPU_DRAFT_GENERATED_ARGUMENT = 2 };
/* recording_desc.inheritance points to this header for a secondary. It replaces
 * old INHERITANCE/HEAP_BINDING records; rendering/query metadata is unchanged.
 */
typedef struct ogpu_draft_inheritance {
    ogpu_next_record header;
    uint32_t occlusion, query_flags;
    uint64_t statistics;
    const ogpu_next_record *rendering;
    ogpu_draft_descriptor_heap *resource_heap, *sampler_heap;
} ogpu_draft_inheritance;

/* One explicit borrowed argument layout also governs GPU-written argument tokens.
 * Tokens keep their wire offsets, but use the new layout's field/atomicity rules.
 */
typedef struct ogpu_draft_generated_layout_desc {
    ogpu_next_record header;
    ogpu_draft_argument_layout *arguments;
    ogpu_next_stages stages;
    uint32_t flags, stride, token_count, reserved;
    const ogpu_next_generated_token *tokens;
} ogpu_draft_generated_layout_desc;
ogpu_next_status ogpu_draft_generated_layout_create(
    ogpu_next_device *, const ogpu_draft_generated_layout_desc *, ogpu_next_generated_layout **);
#ifdef __cplusplus
}
#endif
#endif
