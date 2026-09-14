// Genuine OGPU execution. This executable has no upstream Vulkan device or fallback.
#include "backend.h"
#include <libplacebo/shaders/sampling.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(x) do { if (!(x)) { fprintf(stderr, "check failed: %s:%d: %s\n", \
    __FILE__, __LINE__, #x); exit(1); } } while (0)
static const char *output_dir;
static unsigned log_errors;
static void log_message(void *priv, enum pl_log_level level, const char *message)
{
    fprintf(stderr, "libplacebo[%d]: %s\n", level, message);
    if (level <= PL_LOG_ERR) ++log_errors;
}
static void save(unsigned pass, const char *suffix, const void *data, size_t size)
{
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/pass-%u.%s", output_dir, pass, suffix);
    CHECK(n > 0 && (size_t) n < sizeof(path));
    FILE *file = fopen(path, "wb");
    CHECK(file);
    CHECK(fwrite(data, 1, size, file) == size);
    CHECK(fclose(file) == 0);
}
#include "workload.h"
static struct workload_counts workload_counts(pl_gpu gpu)
{
    const struct ogpu_stats s = ogpu_pl_stats(gpu);
    return (struct workload_counts) {s.creates, s.compute, s.raster};
}
int main(int argc, char **argv)
{
    CHECK(argc == 2);
    output_dir = argv[1];
    pl_log log = pl_log_create(PL_API_VER, pl_log_params(.log_cb=log_message, .log_level=PL_LOG_INFO));
    CHECK(log);
    pl_gpu gpu = ogpu_pl_create(log, 0);
    CHECK(gpu);
    run_case(gpu, 16, 16, "ogpu");
    run_case(gpu, 31, 17, "ogpu");
    run_case(gpu, 64, 33, "ogpu");
    pl_gpu_finish(gpu);
    const struct ogpu_stats s = ogpu_pl_stats(gpu);
    CHECK(!pl_gpu_is_failed(gpu) && !s.textures && !s.passes);
    CHECK(s.creates == 6 && s.compute == 9 && s.raster == 9 && s.uploads == 12 && s.downloads == 18);
    CHECK(s.receipt_reuses == 12);
    ogpu_pl_destroy(&gpu);
    pl_log_destroy(&log);
    CHECK(!log_errors);
    printf("ogpu-consumer creates=%u compute=%u raster=%u uploads=%u downloads=%u live=0 PASS\n",
           s.creates, s.compute, s.raster, s.uploads, s.downloads);
}
