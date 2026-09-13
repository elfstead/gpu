#include "ogpu.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(condition) do { if (!(condition)) { \
    fprintf(stderr, "Check failed at line %d: %s\n", __LINE__, #condition); goto cleanup; \
} } while (0)
#define TRY(operation) do { OgpuResult status_ = (operation); if (status_ != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: status %" PRId32 ", %s\n", #operation, status_, error.message); goto cleanup; \
} } while (0)

typedef struct DrawRoot { uint64_t vertices, draw; } DrawRoot;
typedef struct ProcessRoot { uint64_t source, destination; uint32_t width, height; } ProcessRoot;
typedef struct ReadRoot { uint64_t pixels; uint32_t width, reserved; } ReadRoot;
_Static_assert(sizeof(DrawRoot) == 16 && offsetof(DrawRoot, draw) == 8, "draw root ABI");
_Static_assert(sizeof(ProcessRoot) == 24 && offsetof(ProcessRoot, destination) == 8
    && offsetof(ProcessRoot, width) == 16 && offsetof(ProcessRoot, height) == 20, "process root ABI");
_Static_assert(sizeof(ReadRoot) == 16 && offsetof(ReadRoot, width) == 8, "read root ABI");

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

/* Do not prescribe edge coverage. Every covered pixel must have the shader's
 * coordinate color; interior/background probes prevent a vacuous all-clear pass. */
static int check_pixels(uint8_t *images[3], uint32_t width, uint32_t height) {
    const size_t size = (size_t)width * height * 4;
    for (unsigned image = 0; image < 3; ++image) {
        for (unsigned b = 0; b < 4; ++b)
            if (images[image][b] != 0xa5 || images[image][4 + size + b] != 0xa5) return 0;
    }
    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            const size_t i = ((size_t)y * width + x) * 4;
            const uint8_t pattern[4] = {(uint8_t)(x * 17 + y * 3),
                (uint8_t)(x * 5 + y * 29), (uint8_t)(x * 11 + y * 7), 255};
            const uint8_t black[4] = {0, 0, 0, 255};
            const uint8_t *first = images[0] + 4 + i;
            if (memcmp(first, pattern, 4) && memcmp(first, black, 4)) return 0;
            if (width >= 8 && height >= 8) {
                if (x == width / 2 && y == height / 3 && memcmp(first, pattern, 4)) return 0;
                if (x == 0 && y == 0 && memcmp(first, black, 4)) return 0;
            }
            const uint8_t *source = images[0] + 4 + ((size_t)(height - 1 - y) * width + x) * 4;
            const uint8_t expected[4] = {source[2], (uint8_t)(255 - source[1]), source[0], source[3]};
            for (unsigned image = 1; image < 3; ++image) {
                if (memcmp(images[image] + 4 + i, expected, 4)) {
                    fprintf(stderr, "Image %u mismatch at (%" PRIu32 ", %" PRIu32 ")\n", image, x, y);
                    return 0;
                }
            }
        }
    }
    return 1;
}

static int run_case(OgpuDevice *device, OgpuKernel *producer, OgpuKernel *processor,
    OgpuRaster *first_raster, OgpuRaster *last_raster, uint32_t width, uint32_t height) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuTarget *first = NULL, *last = NULL;
    OgpuBuffer *vertices = NULL, *indirect = NULL, *buffers[3] = {0};
    uint8_t *images[3] = {0};
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    const uint64_t size = (uint64_t)width * height * 4;
    TRY(ogpu_target_create_rgba8(device, width, height, &first, &error));
    TRY(ogpu_target_create_rgba8(device, width, height, &last, &error));
    TRY(ogpu_buffer_create(device, 48, OGPU_MEMORY_HOST, &vertices, &error));
    TRY(ogpu_buffer_create(device, sizeof(OgpuDrawArguments), OGPU_MEMORY_HOST, &indirect, &error));
    DrawRoot draw = {0};
    ProcessRoot process = {.width = width, .height = height};
    ReadRoot read = {.width = width};
    TRY(ogpu_buffer_device_address(vertices, &draw.vertices, &error));
    TRY(ogpu_buffer_device_address(indirect, &draw.draw, &error));
    for (unsigned i = 0; i < 3; ++i) {
        TRY(ogpu_buffer_create(device, size + 8, OGPU_MEMORY_HOST, &buffers[i], &error));
        images[i] = malloc((size_t)size + 8);
        REQUIRE(images[i] != NULL);
    }
    TRY(ogpu_buffer_device_address(buffers[0], &process.source, &error));
    TRY(ogpu_buffer_device_address(buffers[1], &process.destination, &error));
    process.source += 4; /* Keep a guard word before and after each image. */
    process.destination += 4;
    read.pixels = process.destination;

    for (unsigned pass = 0; pass < 2; ++pass) {
        /* Poison again after the previous wait: stale output must not pass reuse checks. */
        for (unsigned i = 0; i < 3; ++i) {
            memset(images[i], 0xa5, (size_t)size + 8);
            TRY(ogpu_buffer_write(buffers[i], 0, images[i], size + 8, &error));
        }
        TRY(ogpu_batch_create(device, &batch, &error));
        if (pass != 0) {
            /* Reusing allocations requires dependencies on their previous GPU
             * accesses as well. Completion is host synchronization, not a new
             * implicit GPU memory dependency. Target layouts are managed internally. */
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_VERTEX_READ
                | OGPU_ACCESS_INDIRECT_READ | OGPU_ACCESS_FRAGMENT_READ | OGPU_ACCESS_TRANSFER_WRITE,
                OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_TRANSFER_WRITE, &error));
        }
        TRY(ogpu_batch_dispatch(batch, producer, 1, &draw, sizeof(draw), &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE,
            OGPU_ACCESS_VERTEX_READ | OGPU_ACCESS_INDIRECT_READ, &error));
        TRY(ogpu_batch_draw_indirect(batch, first_raster, first, indirect, 0, &draw, sizeof(draw), OGPU_ATTACHMENT_CLEAR, &error));
        TRY(ogpu_batch_copy_target(batch, first, buffers[0], 4, &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_TRANSFER_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
        ProcessRoot copied = process;
        TRY(ogpu_batch_dispatch(batch, processor, (width * height + 63) / 64, &copied, sizeof(copied), &error));
        memset(&copied, 0, sizeof(copied)); /* Recording must already have copied it. */
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_FRAGMENT_READ, &error));
        TRY(ogpu_batch_draw_indirect(batch, last_raster, last, indirect, 0, &read, sizeof(read), OGPU_ATTACHMENT_CLEAR, &error));
        TRY(ogpu_batch_copy_target(batch, last, buffers[2], 4, &error));
        TRY(ogpu_batch_submit(batch, &completion, &error));
        ogpu_batch_destroy(batch);
        batch = NULL;

        /* This is the only wait/readback point: nothing crosses the CPU mid-loop. */
        TRY(ogpu_completion_wait(completion, &error));
        for (unsigned i = 0; i < 3; ++i)
            TRY(ogpu_buffer_read(buffers[i], 0, images[i], size + 8, &error));
        REQUIRE(check_pixels(images, width, height));
        ogpu_completion_destroy(completion);
        completion = NULL;
    }
    printf("Verified %" PRIu32 "x%" PRIu32 " image loop twice, including intermediate pixels and guards.\n", width, height);
    exit_code = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(completion); /* Drain before freeing raw-address pointees. */
    ogpu_batch_destroy(batch);
    ogpu_target_destroy(last);
    ogpu_target_destroy(first);
    ogpu_buffer_destroy(indirect);
    ogpu_buffer_destroy(vertices);
    for (unsigned i = 0; i < 3; ++i) {
        ogpu_buffer_destroy(buffers[i]);
        free(images[i]);
    }
    return exit_code;
}

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuKernel *producer = NULL, *processor = NULL;
    OgpuRaster *first = NULL, *last = NULL;
    uint32_t *words[6] = {0};
    uint64_t counts[6] = {0};
    REQUIRE(argc == 7); /* triangle.comp, triangle.vert, image-pattern.frag,
                       * image-process.comp, fullscreen.vert, image-read.frag */
    for (unsigned i = 0; i < 6; ++i)
        REQUIRE(read_shader(argv[i + 1], &words[i], &counts[i]) == EXIT_SUCCESS);
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t count = 0, tested = 0;
    TRY(ogpu_probe_device_count(probe, &count));
    for (uint32_t i = 0; i < count; ++i) {
        OgpuResult status = ogpu_device_create_graphics(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) {
            fprintf(stderr, "Skipping image-loop device %u: %s\n", i, error.message);
            continue;
        }
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Executing graphics -> compute -> graphics on %s\n", info.name);
        TRY(ogpu_kernel_create(device, words[0], counts[0], sizeof(DrawRoot), &producer, &error));
        TRY(ogpu_kernel_create(device, words[3], counts[3], sizeof(ProcessRoot), &processor, &error));
        TRY(ogpu_raster_create(device, words[1], counts[1], words[2], counts[2], sizeof(DrawRoot), &first, &error));
        TRY(ogpu_raster_create(device, words[4], counts[4], words[5], counts[5], sizeof(ReadRoot), &last, &error));
        const uint32_t sizes[][2] = {{1, 1}, {2, 3}, {63, 65}, {64, 64}, {65, 63}, {97, 65}};
        for (unsigned j = 0; j < sizeof(sizes) / sizeof(sizes[0]); ++j)
            REQUIRE(run_case(device, producer, processor, first, last, sizes[j][0], sizes[j][1]) == EXIT_SUCCESS);
        ogpu_raster_destroy(last); last = NULL;
        ogpu_raster_destroy(first); first = NULL;
        ogpu_kernel_destroy(processor); processor = NULL;
        ogpu_kernel_destroy(producer); producer = NULL;
        ogpu_device_destroy(device); device = NULL;
        ++tested;
    }
    REQUIRE(tested != 0);
    exit_code = EXIT_SUCCESS;
cleanup:
    ogpu_raster_destroy(last);
    ogpu_raster_destroy(first);
    ogpu_kernel_destroy(processor);
    ogpu_kernel_destroy(producer);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    for (unsigned i = 0; i < 6; ++i) free(words[i]);
    return exit_code;
}
