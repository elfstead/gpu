/* C1 descriptor contract, 2026-11-03. DESIGN ONLY: not installed/linkable.
 * See ../minimal-core-descriptors.md. Existing descriptions/scalars imported.
 */
#ifndef OGPU_DRAFT_DESCRIPTORS_H
#define OGPU_DRAFT_DESCRIPTORS_H
#include "../../include/ogpu_next.h"
#ifdef __cplusplus
extern "C" {
#endif

typedef struct ogpu_draft_descriptor_heap ogpu_draft_descriptor_heap;
enum {
    OGPU_DRAFT_HEAP_HOST = 0,   /* borrow ordinary host bytes; never shader visible */
    OGPU_DRAFT_HEAP_MEMORY = 1, /* borrow explicit GPU backing and optional mapping */
    OGPU_DRAFT_HEAP_NATIVE = 2  /* allocate an opaque native heap at creation */
};
enum {
    OGPU_DRAFT_HEAP_SHADER_VISIBLE = 1,
    OGPU_DRAFT_HEAP_CPU_WRITE = 2,
    OGPU_DRAFT_HEAP_CPU_COPY_SOURCE = 4,
    OGPU_DRAFT_HEAP_CPU_COPY_DESTINATION = 8
};

typedef struct ogpu_draft_heap_request {
    uint32_t kind; /* existing RESOURCE or SAMPLER heap kind */
    uint32_t storage;
    uint32_t usage;
    uint32_t flags; /* zero */
    uint64_t capacity; /* native location units, includes any reserved interval */
} ogpu_draft_heap_request;

typedef struct ogpu_draft_heap_requirements {
    uint64_t unit_bytes; /* 0: opaque units, never a native handle increment */
    uint64_t backing_size;
    uint64_t backing_alignment;
    uint64_t reserved_size;
    uint64_t reserved_alignment; /* reservation values are in location units */
    uint64_t memory_usage; /* minimum backing usage; zero for HOST/NATIVE */
} ogpu_draft_heap_requirements;

typedef struct ogpu_draft_heap_desc {
    ogpu_draft_heap_request request;
    ogpu_next_span memory;      /* MEMORY only; borrowed */
    ogpu_next_host_span host;   /* HOST, or the actual mapped MEMORY interval */
    uint64_t reserved_offset;
    uint64_t reserved_size;     /* units, not bytes */
} ogpu_draft_heap_desc;

typedef struct ogpu_draft_descriptor_shape {
    uint64_t size;
    uint64_t alignment;
    uint64_t array_stride; /* units per descriptor in the native typed-index ABI */
} ogpu_draft_descriptor_shape;

typedef struct ogpu_draft_heap_location {
    ogpu_draft_descriptor_heap *heap;
    uint64_t offset; /* units relative to this heap, never a host/GPU pointer */
} ogpu_draft_heap_location;

typedef struct ogpu_draft_descriptor_copy {
    ogpu_draft_heap_location source;
    ogpu_draft_heap_location destination;
    uint64_t size; /* units; source/destination device and heap kind must match */
} ogpu_draft_descriptor_copy;

ogpu_next_status ogpu_draft_heap_requirements_query(
    const ogpu_next_device *, const ogpu_draft_heap_request *,
    ogpu_draft_heap_requirements *);
ogpu_next_status ogpu_draft_resource_descriptor_shape(
    const ogpu_next_device *, const ogpu_next_resource_descriptor *,
    ogpu_draft_descriptor_shape *);
ogpu_next_status ogpu_draft_sampler_descriptor_shape(
    const ogpu_next_device *, const ogpu_next_sampler_desc *,
    ogpu_draft_descriptor_shape *);
ogpu_next_status ogpu_draft_heap_create(
    ogpu_next_device *, const ogpu_draft_heap_desc *, ogpu_draft_descriptor_heap **);
void ogpu_draft_heap_destroy(ogpu_draft_descriptor_heap *);
/* Returns immutable creation/storage facts. Spans remain borrowed; no mapping,
 * allocation or recipe storage is created by this query. NATIVE has empty spans.
 */
void ogpu_draft_heap_info(const ogpu_draft_descriptor_heap *,
    ogpu_draft_heap_desc *, ogpu_draft_heap_requirements *);

enum {
    OGPU_DRAFT_DESCRIPTOR_WRITE_RESOURCE = 0,
    OGPU_DRAFT_DESCRIPTOR_WRITE_SAMPLER = 1,
    OGPU_DRAFT_DESCRIPTOR_COPY = 2
};
ogpu_next_status ogpu_draft_descriptor_scratch_requirements(
    const ogpu_next_device *, uint32_t operation, uint32_t count,
    ogpu_next_host_requirements *);
ogpu_next_status ogpu_draft_write_resource_descriptors(
    const ogpu_next_device *, uint32_t count, const ogpu_next_resource_descriptor *,
    const ogpu_draft_heap_location *, ogpu_next_host_span scratch);
ogpu_next_status ogpu_draft_write_sampler_descriptors(
    const ogpu_next_device *, uint32_t count, const ogpu_next_sampler_desc *,
    const ogpu_draft_heap_location *, ogpu_next_host_span scratch);
/* CPU-timeline copy. Does not record GPU work or create a shadow source heap. */
ogpu_next_status ogpu_draft_copy_descriptors(
    const ogpu_next_device *, uint32_t count, const ogpu_draft_descriptor_copy *,
    ogpu_next_host_span scratch);
/* Binds this heap's kind, preserving the other kind. O(1) in heap capacity.
 * Retaining current native heap handles in the encoder is allowed; retaining
 * or reconstructing descriptor contents/resource ownership is not.
 */
void ogpu_draft_bind_heap(ogpu_next_encoder *, ogpu_draft_descriptor_heap *);

#ifdef __cplusplus
}
#endif
#endif
