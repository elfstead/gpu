#include "ogpu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(x) do { if (!(x)) { fprintf(stderr, "Check failed line %d: %s\n", __LINE__, #x); goto cleanup; } } while (0)
#define TRY(x) do { if ((x) != OGPU_SUCCESS) { fprintf(stderr, "%s: %s\n", #x, error.message); goto cleanup; } } while (0)
typedef struct Root { uint32_t image, second, width, height; } Root;
_Static_assert(sizeof(Root) == 16 && offsetof(Root, width) == 8, "heap root ABI");

static int shader(const char *path, uint32_t **words, uint64_t *count) {
    int result = EXIT_FAILURE;
    FILE *file = fopen(path, "rb");
    REQUIRE(file && fseek(file, 0, SEEK_END) == 0);
    long size = ftell(file);
    REQUIRE(size >= 20 && size % 4 == 0 && fseek(file, 0, SEEK_SET) == 0);
    *words = malloc((size_t)size);
    REQUIRE(*words && fread(*words, 1, (size_t)size, file) == (size_t)size);
    *count = (uint64_t)size / 4;
    result = EXIT_SUCCESS;
cleanup:
    if (file) fclose(file);
    return result;
}

static int run_case(OgpuDevice *device, OgpuKernel *compute, OgpuRaster *pattern,
    OgpuRaster *sample, uint32_t width, uint32_t height) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuTarget *source = NULL, *processed = NULL, *final = NULL;
    OgpuImageTable *tables[2] = {0};
    OgpuBuffer *indirect = NULL, *readback = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    size_t size = (size_t)width * height * 4;
    uint8_t *pixels = malloc(2 * (size + 8));
    REQUIRE(pixels);
    TRY(ogpu_target_create_rgba8(device, width, height, &source, &error));
    TRY(ogpu_target_create_rgba8(device, width, height, &processed, &error));
    TRY(ogpu_target_create_rgba8(device, width, height, &final, &error));
    // The two tables deliberately permute entries. Shader roots must select the
    // right descriptor at runtime; fixed compiler bindings cannot pass both modes.
    const OgpuImageEntry a[] = {{processed, OGPU_IMAGE_SAMPLED, 0},
        {source, OGPU_IMAGE_SAMPLED, 0}, {processed, OGPU_IMAGE_STORAGE, 0}};
    const OgpuImageEntry b[] = {{processed, OGPU_IMAGE_STORAGE, 0},
        {processed, OGPU_IMAGE_SAMPLED, 0}, {source, OGPU_IMAGE_SAMPLED, 0}};
    TRY(ogpu_image_table_create(device, a, 3, &tables[0], &error));
    TRY(ogpu_image_table_create(device, b, 3, &tables[1], &error));
    TRY(ogpu_buffer_create(device, sizeof(OgpuDrawArguments), &indirect, &error));
    const OgpuDrawArguments draw = {3, 1, 0, 0};
    TRY(ogpu_buffer_write(indirect, 0, &draw, sizeof(draw), &error));
    TRY(ogpu_buffer_create(device, 2 * (size + 8), &readback, &error));
    for (unsigned pass = 0; pass < 2; ++pass) {
        memset(pixels, 0xa5, 2 * (size + 8));
        TRY(ogpu_buffer_write(readback, 0, pixels, 2 * (size + 8), &error));
        TRY(ogpu_batch_create(device, &batch, &error));
        TRY(ogpu_batch_bind_image_table(batch, tables[1 - pass], &error));
        TRY(ogpu_batch_bind_image_table(batch, tables[pass], &error));
        TRY(ogpu_batch_draw_indirect(batch, pattern, source, indirect, 0, NULL, 0, &error));
        TRY(ogpu_batch_discard_target(batch, processed, &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COLOR_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
        Root process = {pass ? 2 : 1, pass ? 0 : 2, width, height};
        Root sampling = {pass ? 1 : 0, 0, width, height};
        TRY(ogpu_batch_dispatch(batch, compute, (width * height + 63) / 64,
            &process, sizeof(process), &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_FRAGMENT_READ, &error));
        TRY(ogpu_batch_draw_indirect(batch, sample, final, indirect, 0,
            &sampling, sizeof(sampling), &error));
        TRY(ogpu_batch_copy_target(batch, final, readback, 4, &error));
        // Diagnostic copies happen only after the full GPU chain. The processed
        // target was initialized by discard, never by a draw.
        TRY(ogpu_batch_copy_target(batch, processed, readback, size + 12, &error));
        if (pass == 1) {
            // Recorded bindings/draws/discard retain everything, even before submit.
            for (unsigned i = 0; i < 2; ++i) { ogpu_image_table_destroy(tables[i]); tables[i] = NULL; }
            ogpu_target_destroy(source); source = NULL;
            ogpu_target_destroy(processed); processed = NULL;
            ogpu_target_destroy(final); final = NULL;
        }
        TRY(ogpu_batch_submit(batch, &completion, &error));
        ogpu_batch_destroy(batch); batch = NULL;
        TRY(ogpu_completion_wait(completion, &error));
        TRY(ogpu_buffer_read(readback, 0, pixels, 2 * (size + 8), &error));
        for (unsigned image = 0; image < 2; ++image) {
            uint8_t *data = pixels + image * (size + 8);
            for (unsigned i = 0; i < 4; ++i) REQUIRE(data[i] == 0xa5 && data[size + 4 + i] == 0xa5);
            for (uint32_t y = 0; y < height; ++y) for (uint32_t x = 0; x < width; ++x) {
                uint32_t sy = height - 1 - y;
                const uint8_t expected[] = {(uint8_t)(x * 11 + sy * 7),
                    (uint8_t)(255 - ((x * 5 + sy * 29) & 255)), (uint8_t)(x * 17 + sy * 3), 255};
                REQUIRE(memcmp(data + 4 + ((size_t)y * width + x) * 4, expected, 4) == 0);
            }
        }
        ogpu_completion_destroy(completion); completion = NULL;
    }
    printf("heap-image %ux%u: two permutations, target reuse, early handle release, pixels/guards PASS\n", width, height);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(completion);
    ogpu_batch_destroy(batch);
    for (unsigned i = 0; i < 2; ++i) ogpu_image_table_destroy(tables[i]);
    ogpu_target_destroy(final);
    ogpu_target_destroy(processed);
    ogpu_target_destroy(source);
    ogpu_buffer_destroy(readback);
    ogpu_buffer_destroy(indirect);
    free(pixels);
    return result;
}

int main(int argc, char **argv) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuKernel *compute = NULL;
    OgpuRaster *pattern = NULL, *sample = NULL;
    uint32_t *words[4] = {0};
    uint64_t counts[4] = {0};
    REQUIRE(argc == 5);
    for (unsigned i = 0; i < 4; ++i) REQUIRE(shader(argv[i + 1], &words[i], &counts[i]) == EXIT_SUCCESS);
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t count = 0, tested = 0;
    TRY(ogpu_probe_device_count(probe, &count));
    for (uint32_t i = 0; i < count; ++i) {
        OgpuResult status = ogpu_device_create_graphics(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        TRY(ogpu_kernel_create(device, words[2], counts[2], sizeof(Root), &compute, &error));
        TRY(ogpu_raster_create(device, words[0], counts[0], words[1], counts[1], 0, &pattern, &error));
        TRY(ogpu_raster_create(device, words[0], counts[0], words[3], counts[3], sizeof(Root), &sample, &error));
        const uint32_t sizes[][2] = {{1, 1}, {2, 3}, {63, 65}, {64, 64}, {65, 63}, {97, 65}};
        for (unsigned j = 0; j < sizeof(sizes) / sizeof(sizes[0]); ++j)
            REQUIRE(run_case(device, compute, pattern, sample, sizes[j][0], sizes[j][1]) == EXIT_SUCCESS);
        ogpu_raster_destroy(sample); sample = NULL;
        ogpu_raster_destroy(pattern); pattern = NULL;
        ogpu_kernel_destroy(compute); compute = NULL;
        ogpu_device_destroy(device); device = NULL;
        ++tested;
    }
    REQUIRE(tested);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_raster_destroy(sample);
    ogpu_raster_destroy(pattern);
    ogpu_kernel_destroy(compute);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    for (unsigned i = 0; i < 4; ++i) free(words[i]);
    return result;
}
