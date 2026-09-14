// Pinned, deliberately bounded libplacebo backend. Callers destroy all children first.
#pragma once
#include <libplacebo/gpu.h>
struct ogpu_stats {
    unsigned creates, compute, raster, uploads, downloads, textures, passes, receipt_reuses;
    unsigned submissions, waits, polls, frames, collected, inflight, peak_inflight;
    unsigned outstanding, peak_outstanding, callbacks, texture_creates;
    unsigned staging_allocs, banks, peak_banks, peak_textures;
    size_t staging_bytes, peak_staging_bytes, texture_bytes, peak_texture_bytes;
};
pl_gpu ogpu_pl_create(pl_log log, unsigned device_index);
void ogpu_pl_destroy(pl_gpu *gpu);
struct ogpu_stats ogpu_pl_stats(pl_gpu gpu);
// Bounded integration policy, NOT OGPU runtime API. Exactly two slots, eight
// operations per frame. Call end before collect. Reuse only after collection.
// false from nonblocking collect means pending OR failed; inspect gpu_is_failed.
// Callback destinations stay live through collection; callbacks must not reenter.
bool ogpu_pl_frame_begin(pl_gpu gpu, unsigned slot);
// Same slot rules, but record one batch and submit at frame_end. Failure discards
// unsubmitted commands and releases callbacks during collection with failed state.
bool ogpu_pl_frame_begin_batched(pl_gpu gpu, unsigned slot);
bool ogpu_pl_frame_end(pl_gpu gpu);
bool ogpu_pl_frame_collect(pl_gpu gpu, unsigned slot, bool wait);
