/* C1 presentation lifecycle, 2026-11-03. DESIGN ONLY: not installed/linkable.
 * Existing Vulkan setup records, objects and synchronization are imported.
 * See ../minimal-core-presentation.md for lifetime and outcome contracts.
 */
#ifndef OGPU_DRAFT_PRESENTATION_H
#define OGPU_DRAFT_PRESENTATION_H
#include "../../include/ogpu_next.h"
#ifdef __cplusplus
extern "C" {
#endif

enum {
    OGPU_DRAFT_PRESENT_VULKAN = 1,
    OGPU_DRAFT_PRESENT_DXGI = 2,
    OGPU_DRAFT_PRESENTATION_WIN32 = 2,
    OGPU_DRAFT_WIN32_SURFACE_DESC = 3000,
    OGPU_DRAFT_DXGI_CHAIN_DESC = 3001
};
enum {
    OGPU_DRAFT_OCCLUDED = 5,
    OGPU_DRAFT_STILL_DRAWING = 6
};
enum {
    OGPU_DRAFT_AVAILABILITY_SIGNALS = 1,
    OGPU_DRAFT_AVAILABILITY_QUEUE_ORDERED = 2
};
enum {
    OGPU_DRAFT_PRESENT_REPLACE = 1,
    OGPU_DRAFT_PRESENT_RESIZE = 2,
    OGPU_DRAFT_PRESENT_RETIRE_FENCE = 4,
    OGPU_DRAFT_PRESENT_PACING_HANDLE = 8,
    OGPU_DRAFT_PRESENT_BINARY_WAIT = 16
};
enum {
    OGPU_DRAFT_CHAIN_NONE = 0,
    OGPU_DRAFT_CHAIN_ACTIVE = 1,
    OGPU_DRAFT_CHAIN_RETIRED = 2,
    OGPU_DRAFT_CHAIN_NO_IMAGES = 3,
    OGPU_DRAFT_CHAIN_LOST = 4
};
enum {
    OGPU_DRAFT_IMAGES_UNCHANGED = 0,
    OGPU_DRAFT_IMAGES_REPLACED = 1,
    OGPU_DRAFT_IMAGES_INVALIDATED = 2,
    OGPU_DRAFT_FRAME_RETAINED = 0,
    OGPU_DRAFT_FRAME_RELEASED = 1,
    OGPU_DRAFT_FRAME_UNKNOWN = 2,
    OGPU_DRAFT_EFFECT_NONE = 0,
    OGPU_DRAFT_EFFECT_ENQUEUED = 1,
    OGPU_DRAFT_EFFECT_UNKNOWN = 2,
    OGPU_DRAFT_WAITABLE_WIN32 = 1
};

typedef struct ogpu_draft_win32_surface_desc {
    ogpu_next_record header;
    void *hwnd;
} ogpu_draft_win32_surface_desc;

/* Native option numbers belong to DXGI, never to the Vulkan mode namespace.
 * Exact selection: no default queue, image count, mode or latency policy.
 */
typedef struct ogpu_draft_dxgi_chain_desc {
    ogpu_next_record header;
    ogpu_next_surface *surface;
    ogpu_next_queue *queue;
    uint32_t format, extent[2], image_count;
    uint64_t usage;
    uint32_t swap_effect, scaling, alpha, color_space;
    uint32_t chain_flags, maximum_frame_latency;
} ogpu_draft_dxgi_chain_desc;

typedef struct ogpu_draft_presentation_contract {
    uint32_t profile, availability, capabilities, max_present_count;
} ogpu_draft_presentation_contract;

typedef struct ogpu_draft_chain_info {
    uint64_t generation;
    uint32_t state, image_count;
    ogpu_next_queue *ordered_queue; /* non-NULL only for QUEUE_ORDERED */
} ogpu_draft_chain_info;

typedef struct ogpu_draft_chain_create_outcome {
    ogpu_next_swapchain *chain; /* NULL on failure */
    uint32_t previous_state, reserved;
} ogpu_draft_chain_create_outcome;

typedef struct ogpu_draft_chain_resize_desc {
    uint32_t format, extent[2], image_count;
    uint32_t chain_flags, reserved;
} ogpu_draft_chain_resize_desc;

typedef struct ogpu_draft_chain_resize_outcome {
    uint64_t generation;
    uint32_t state, image_effect;
} ogpu_draft_chain_resize_outcome;

typedef struct ogpu_draft_acquire_desc {
    uint64_t timeout_ns;
    ogpu_next_binary *binary;
    ogpu_next_fence *fence;
} ogpu_draft_acquire_desc;

/* Borrowed value, no allocation or reference count; copying confers no new lease.
 * Availability objects are caller-provided, never invented by acquisition.
 */
typedef struct ogpu_draft_frame {
    ogpu_next_swapchain *chain;
    ogpu_next_image *image;
    uint64_t generation;
    uint32_t index, availability;
    ogpu_next_queue *ordered_queue;
    ogpu_next_binary *binary;
    ogpu_next_fence *fence;
} ogpu_draft_frame;

typedef struct ogpu_draft_present_item {
    ogpu_draft_frame frame;
    ogpu_next_fence *retire_fence;
    uint32_t sync_interval, native_flags; /* DXGI only; zero on Vulkan */
} ogpu_draft_present_item;

typedef struct ogpu_draft_present_result {
    ogpu_next_status status;
    uint32_t frame_effect, fence_effect, reserved;
} ogpu_draft_present_result;

typedef struct ogpu_draft_present_desc {
    uint32_t wait_count, item_count;
    ogpu_next_binary *const *waits;
    const ogpu_draft_present_item *items;
    ogpu_draft_present_result *results; /* required, item_count entries */
    ogpu_next_host_span scratch;
} ogpu_draft_present_desc;

typedef struct ogpu_draft_present_outcome {
    uint32_t work_effect, wait_effect;
} ogpu_draft_present_outcome;

typedef struct ogpu_draft_pacing_handle {
    uint32_t kind, reserved;
    void *handle; /* borrowed; chain closes it, caller must not */
} ogpu_draft_pacing_handle;

/* Wayland entry point and live surface queries keep their existing operations.
 * Discovery setup additionally advertises/enables the typed Win32 platform.
 */
ogpu_next_status ogpu_draft_surface_win32_create(
    const ogpu_next_adapter *, const ogpu_draft_win32_surface_desc *, ogpu_next_surface **);
ogpu_next_status ogpu_draft_presentation_contract_query(
    const ogpu_next_adapter *, const ogpu_next_surface *, uint32_t profile,
    ogpu_draft_presentation_contract *);
/* Accepts existing SWAPCHAIN_DESC (Vulkan) or draft DXGI_CHAIN_DESC. */
ogpu_next_status ogpu_draft_chain_create(
    ogpu_next_device *, const ogpu_next_record *, ogpu_draft_chain_create_outcome *);
void ogpu_draft_chain_destroy(ogpu_next_swapchain *);
void ogpu_draft_chain_info_query(const ogpu_next_swapchain *, ogpu_draft_chain_info *);
/* Existing swapchain_images enumeration, now borrowed until resize/destruction. */
ogpu_next_status ogpu_draft_chain_images(
    const ogpu_next_swapchain *, uint32_t capacity, ogpu_next_image **, uint32_t *count);
ogpu_next_status ogpu_draft_chain_resize(
    ogpu_next_swapchain *, const ogpu_draft_chain_resize_desc *, ogpu_draft_chain_resize_outcome *);
ogpu_next_status ogpu_draft_chain_pacing_handle(
    const ogpu_next_swapchain *, ogpu_draft_pacing_handle *);
ogpu_next_status ogpu_draft_acquire(
    ogpu_next_swapchain *, const ogpu_draft_acquire_desc *, ogpu_draft_frame *);
ogpu_next_status ogpu_draft_abandon_scratch_requirements(
    const ogpu_next_device *, uint32_t count, ogpu_next_host_requirements *);
ogpu_next_status ogpu_draft_abandon(
    ogpu_next_swapchain *, uint32_t count, const ogpu_draft_frame *,
    ogpu_next_host_span scratch, uint32_t *frame_effect);
ogpu_next_status ogpu_draft_present_scratch_requirements(
    const ogpu_next_device *, uint32_t waits, uint32_t items, ogpu_next_host_requirements *);
ogpu_next_status ogpu_draft_queue_present(
    ogpu_next_queue *, const ogpu_draft_present_desc *, ogpu_draft_present_outcome *);

#ifdef __cplusplus
}
#endif
#endif
