// Pinned, deliberately bounded libplacebo backend. Callers destroy all children first.
#pragma once
#include <libplacebo/gpu.h>
struct ogpu_stats { unsigned creates, compute, raster, uploads, downloads, textures, passes; };
pl_gpu ogpu_pl_create(pl_log log, unsigned device_index);
void ogpu_pl_destroy(pl_gpu *gpu);
struct ogpu_stats ogpu_pl_stats(pl_gpu gpu);
