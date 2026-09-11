#include "ogpu.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#define REQUIRE(condition) do { if (!(condition)) { \
    fprintf(stderr, "Check failed at line %d: %s\n", __LINE__, #condition); goto cleanup; \
} } while (0)
#define TRY(operation) do { OgpuResult status_ = (operation); if (status_ != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: status %" PRId32 ", %s\n", #operation, status_, error.message); goto cleanup; \
} } while (0)

/* Compute uses both addresses; the vertex shader reads only the first. */
typedef struct Root {
    uint64_t vertices;
    uint64_t draw;
} Root;
_Static_assert(sizeof(Root) == 16, "root block size");
_Static_assert(offsetof(Root, draw) == 8, "shader draw address offset");
_Static_assert(sizeof(OgpuDrawArguments) == 16, "indirect record size");

static int read_shader(const char *path, uint32_t **out_words, uint64_t *out_count) {
    int exit_code = EXIT_FAILURE;
    uint32_t *words = NULL;
    FILE *file = fopen(path, "rb");
    REQUIRE(file != NULL);
    REQUIRE(fseek(file, 0, SEEK_END) == 0);
    long bytes = ftell(file);
    REQUIRE(bytes >= 20 && bytes % 4 == 0);
    REQUIRE(fseek(file, 0, SEEK_SET) == 0);
    words = malloc((size_t)bytes);
    REQUIRE(words != NULL);
    REQUIRE(fread(words, 1, (size_t)bytes, file) == (size_t)bytes);
    *out_words = words;
    *out_count = (uint64_t)bytes / 4;
    words = NULL;
    exit_code = EXIT_SUCCESS;
cleanup:
    free(words);
    if (file) fclose(file);
    return exit_code;
}

static int pixel_matches(const uint8_t *pixels, uint32_t x, uint32_t y, int red) {
    const uint8_t *p = pixels + (y * 64 + x) * 4;
    if (p[0] == (red ? 255 : 0) && p[1] == 0 && p[2] == 0 && p[3] == 255) return 1;
    fprintf(stderr, "Pixel (%" PRIu32 ", %" PRIu32 "): got RGBA (%u, %u, %u, %u)\n",
        x, y, (unsigned)p[0], (unsigned)p[1], (unsigned)p[2], (unsigned)p[3]);
    return 0;
}

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuKernel *producer = NULL;
    OgpuRaster *raster = NULL;
    OgpuTarget *target = NULL;
    OgpuBuffer *vertices = NULL, *indirect = NULL, *readback = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    uint32_t *compute_words = NULL, *vertex_words = NULL, *fragment_words = NULL;
    uint64_t compute_count = 0, vertex_count = 0, fragment_count = 0;
    uint8_t *pixels = NULL;
    const uint64_t image_size = 64 * 64 * 4;
    REQUIRE(argc == 4); /* triangle.comp.spv triangle.vert.spv triangle.frag.spv */

    /* This example opts into graphics; ordinary device creation still permits compute-only. */
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t device_count = 0;
    TRY(ogpu_probe_device_count(probe, &device_count));
    for (uint32_t i = 0; i < device_count; ++i) {
        OgpuResult status = ogpu_device_create_graphics(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Executing compute + graphics on %s\n", info.name);
        break;
    }
    REQUIRE(device != NULL);
    REQUIRE(read_shader(argv[1], &compute_words, &compute_count) == EXIT_SUCCESS);
    REQUIRE(read_shader(argv[2], &vertex_words, &vertex_count) == EXIT_SUCCESS);
    REQUIRE(read_shader(argv[3], &fragment_words, &fragment_count) == EXIT_SUCCESS);
    TRY(ogpu_kernel_create(device, compute_words, compute_count, sizeof(Root), &producer, &error));
    TRY(ogpu_raster_create(device, vertex_words, vertex_count, fragment_words, fragment_count,
        sizeof(Root), &raster, &error));
    TRY(ogpu_target_create_rgba8(device, 64, 64, &target, &error));

    /* Ordinary allocations, not special vertex/indirect memory types. The GPU will
     * generate all three vec4 positions and all four indirect-draw fields. */
    TRY(ogpu_buffer_create(device, 3 * 4 * sizeof(float), &vertices, &error));
    TRY(ogpu_buffer_create(device, sizeof(OgpuDrawArguments), &indirect, &error));
    TRY(ogpu_buffer_create(device, image_size, &readback, &error));
    pixels = malloc((size_t)image_size);
    REQUIRE(pixels != NULL);
    Root root = {0};
    TRY(ogpu_buffer_device_address(vertices, &root.vertices, &error));
    TRY(ogpu_buffer_device_address(indirect, &root.draw, &error));
    REQUIRE(root.vertices % 16 == 0 && root.draw % 4 == 0);

    /* One batch: compute → vertex/indirect dependency → draw → image copy. */
    TRY(ogpu_batch_create(device, &batch, &error));
    TRY(ogpu_batch_dispatch(batch, producer, 1, &root, sizeof(root), &error));
    TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE,
        OGPU_ACCESS_VERTEX_READ | OGPU_ACCESS_INDIRECT_READ, &error));
    TRY(ogpu_batch_draw_indirect(batch, raster, target, indirect, 0, &root, sizeof(root), &error));
    TRY(ogpu_batch_copy_target(batch, target, readback, 0, &error));
    TRY(ogpu_batch_submit(batch, &completion, &error));

    /* No host inspection of the generated vertices or draw arguments. */
    TRY(ogpu_completion_wait(completion, &error));
    TRY(ogpu_buffer_read(readback, 0, pixels, image_size, &error));
    REQUIRE(pixel_matches(pixels, 32, 24, 1));
    REQUIRE(pixel_matches(pixels, 24, 24, 1));
    REQUIRE(pixel_matches(pixels, 40, 24, 1));
    REQUIRE(pixel_matches(pixels, 32, 40, 1));
    REQUIRE(pixel_matches(pixels, 0, 0, 0));
    REQUIRE(pixel_matches(pixels, 63, 63, 0));
    REQUIRE(pixel_matches(pixels, 4, 32, 0));
    REQUIRE(pixel_matches(pixels, 60, 32, 0));
    printf("Verified a 64x64 offscreen triangle: GPU-generated vertices and indirect draw, one batch, one wait.\n");
    exit_code = EXIT_SUCCESS;
cleanup:
    /* Drain any accepted work before releasing pointer-referenced allocations. */
    ogpu_completion_destroy(completion);
    ogpu_batch_destroy(batch);
    ogpu_target_destroy(target);
    ogpu_raster_destroy(raster);
    ogpu_kernel_destroy(producer);
    ogpu_buffer_destroy(readback);
    ogpu_buffer_destroy(indirect);
    ogpu_buffer_destroy(vertices);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    free(pixels);
    free(fragment_words);
    free(vertex_words);
    free(compute_words);
    return exit_code;
}
