/* C1 argument contract, 2026-11-03. DESIGN ONLY: not installed or linkable.
 * Semantics and migration: ../minimal-core-arguments.md
 * Existing scalar/opaque types are imported; none of these symbols exist yet.
 */
#ifndef OGPU_DRAFT_ARGUMENTS_H
#define OGPU_DRAFT_ARGUMENTS_H
#include "../../include/ogpu_next.h"
#ifdef __cplusplus
extern "C" {
#endif

typedef struct ogpu_draft_argument_layout ogpu_draft_argument_layout;

/* Replaces QUERY_EXECUTION_LIMITS: inline upper bound is not a combined native
 * layout budget. argument_kinds is a bitset indexed by field kind. Creation
 * validates the complete layout/artifact mapping and native budget.
 */
typedef struct ogpu_draft_execution_limits {
    uint64_t max_inline_bytes;
    uint32_t max_groups[3], max_local_size[3], max_local_invocations;
    uint32_t max_shared_memory, argument_kinds;
} ogpu_draft_execution_limits;
typedef struct ogpu_draft_shader {
    uint32_t stage, format;
    ogpu_next_bytes code;
    const char *entry;
    const ogpu_next_record *specialization, *subgroup;
} ogpu_draft_shader;
typedef struct ogpu_draft_executable_desc {
    ogpu_next_record header;
    uint32_t kind, shader_count;
    const ogpu_draft_shader *shaders;
    const ogpu_next_record *static_state;
    uint64_t dynamic_state;
    const ogpu_next_record *requirements;
    ogpu_next_executable_cache *cache;
    uint32_t compile_flags, reserved;
} ogpu_draft_executable_desc;

enum {
    OGPU_DRAFT_ARGUMENT_INLINE = 0,
    OGPU_DRAFT_ARGUMENT_UNIFORM_ADDRESS = 1,
    OGPU_DRAFT_ARGUMENT_READ_ADDRESS = 2,
    OGPU_DRAFT_ARGUMENT_READ_WRITE_ADDRESS = 3
};
enum {
    OGPU_DRAFT_ARGUMENT_COMPUTE = 0,
    OGPU_DRAFT_ARGUMENT_GRAPHICS = 1
};
/* INLINE may target a direct push-data declaration. Otherwise space/binding
 * identify a compiler-declared buffer resource, whose role is given by kind.
 * Both must be DIRECT together; DIRECT is legal only for INLINE.
 */
#define OGPU_DRAFT_ARGUMENT_DIRECT UINT32_MAX
typedef struct ogpu_draft_argument_field {
    uint32_t offset;
    uint32_t size;
    uint32_t kind;
    uint32_t address_alignment; /* INLINE: 0; address: requested power of two */
    ogpu_next_stages stages;    /* artifact visibility, not isolated storage */
    uint32_t space;
    uint32_t binding;
} ogpu_draft_argument_field;

typedef struct ogpu_draft_argument_layout_desc {
    uint32_t execution_class;
    uint32_t byte_size;
    uint32_t field_count;
    uint32_t flags; /* zero */
    const ogpu_draft_argument_field *fields;
} ogpu_draft_argument_layout_desc;

enum {
    OGPU_DRAFT_ARGUMENT_BIND_PRESERVES = 0,
    OGPU_DRAFT_ARGUMENT_BIND_INVALIDATES_ON_CHANGE = 1
};
/* Read-only facts about command-local argument storage. These are not handles.
 * IDs compare only within one live device; zero is not a valid domain/key.
 * domain identifies potentially overlapping argument storage. Byte coordinates
 * are offset_base + logical offset. bind_rule applies to that domain.
 * Different layouts may have the same binding_key when natively compatible;
 * sharing the same layout always shares the key. The application must not
 * fabricate keys or compare them across devices/recreations.
 */
typedef struct ogpu_draft_argument_state_info {
    uint64_t domain;
    uint64_t binding_key;
    uint32_t offset_base;
    uint32_t byte_size;
    uint32_t bind_rule;
    uint32_t reserved; /* zero */
} ogpu_draft_argument_state_info;

ogpu_next_status ogpu_draft_argument_layout_create(
    ogpu_next_device *, const ogpu_draft_argument_layout_desc *,
    ogpu_draft_argument_layout **);
void ogpu_draft_argument_layout_destroy(ogpu_draft_argument_layout *);
void ogpu_draft_argument_layout_info(
    const ogpu_draft_argument_layout *, ogpu_draft_argument_state_info *);

/* Replaces executable preparation with an explicit borrowed layout. The old
 * shader.interface_metadata field is deleted, not a second authority. Other
 * requirement/specialization metadata remains as documented.
 * Layout must remain live through executable destruction and all command uses.
 */
ogpu_next_status ogpu_draft_executable_create(
    ogpu_next_device *, const ogpu_draft_executable_desc *,
    ogpu_draft_argument_layout *, ogpu_next_executable **);

/* Replaces set_inline/set_root. The layout must belong to the executable
 * currently bound for its execution class. Record values now, pointees never.
 * offset/size are 4-aligned, in bounds; size=0 permits data=NULL.
 * Any intersected address field must be covered completely (8 bytes).
 * INLINE fields allow 4-byte subranges. Padding is uninterpreted.
 * Full local preflight precedes native writes; invalid recording is poisoned.
 * No heap allocation, payload cache, resource retention or implicit update at
 * dispatch/draw. data need only be valid during this call.
 */
void ogpu_draft_set_arguments(
    ogpu_next_encoder *, const ogpu_draft_argument_layout *,
    uint32_t offset, uint32_t size, const void *data);

#ifdef __cplusplus
}
#endif
#endif
