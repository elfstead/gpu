#pragma once
#include <libplacebo/gpu.h>
void diagnostic_attach(pl_gpu gpu, bool native);
void diagnostic_phase(pl_gpu gpu, bool measured);
void diagnostic_collect(pl_gpu gpu);
void diagnostic_detach(pl_gpu gpu);
