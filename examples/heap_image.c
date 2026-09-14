#include "ogpu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(x) do { if (!(x)) { fprintf(stderr, "Check failed line %d: %s\n", __LINE__, #x); goto cleanup; } } while (0)
#define TRY(x) do { if ((x) != OGPU_SUCCESS) { fprintf(stderr, "%s: %s\n", #x, error.message); goto cleanup; } } while (0)
typedef struct Root { uint32_t image, second, width, height; } Root;
_Static_assert(sizeof(Root) == 16 && offsetof(Root, width) == 8, "heap root ABI");
typedef struct SampleRoot { uint32_t image, sampler, width, height; float offset_x; } SampleRoot;
_Static_assert(sizeof(SampleRoot) == 20 && offsetof(SampleRoot, offset_x) == 16, "sample root ABI");

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
    OgpuImage *source = NULL, *processed = NULL, *final = NULL;
    OgpuImageHeap *heaps[2] = {0};
    OgpuSamplerHeap *samplers = NULL;
    OgpuBuffer *indirect = NULL, *readback = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *completions[3] = {0};
    size_t size = (size_t)width * height * 4;
    uint8_t *pixels = malloc(2 * (size + 8));
    REQUIRE(pixels);
    OgpuImageDesc image_desc = {OGPU_IMAGE_2D, width, height, OGPU_FORMAT_RGBA8_UNORM,
        OGPU_IMAGE_USAGE_COLOR | OGPU_IMAGE_USAGE_SAMPLED | OGPU_IMAGE_USAGE_COPY_SRC, 0};
    TRY(ogpu_image_create(device, &image_desc, &source, &error));
    image_desc.usage = OGPU_IMAGE_USAGE_SAMPLED | OGPU_IMAGE_USAGE_STORAGE | OGPU_IMAGE_USAGE_COPY_SRC;
    TRY(ogpu_image_create(device, &image_desc, &processed, &error));
    image_desc.usage = OGPU_IMAGE_USAGE_COLOR | OGPU_IMAGE_USAGE_COPY_SRC;
    TRY(ogpu_image_create(device, &image_desc, &final, &error));
    // The two heaps deliberately permute entries. Shader roots must select the
    // right descriptor at runtime; fixed compiler bindings cannot pass both modes.
    const OgpuImageEntry a[] = {{processed, OGPU_IMAGE_SAMPLED, 0},
        {source, OGPU_IMAGE_SAMPLED, 0}, {processed, OGPU_IMAGE_STORAGE, 0}};
    const OgpuImageEntry b[] = {{processed, OGPU_IMAGE_STORAGE, 0},
        {processed, OGPU_IMAGE_SAMPLED, 0}, {source, OGPU_IMAGE_SAMPLED, 0}};
    TRY(ogpu_image_heap_create(device, 3, &heaps[0], &error));
    TRY(ogpu_image_heap_create(device, 3, &heaps[1], &error));
    TRY(ogpu_sampler_heap_create(device, 2, &samplers, &error));
    const OgpuSamplerDesc descriptions[] = {{OGPU_FILTER_NEAREST, OGPU_FILTER_NEAREST, OGPU_ADDRESS_CLAMP, OGPU_ADDRESS_CLAMP},
        {OGPU_FILTER_LINEAR, OGPU_FILTER_LINEAR, OGPU_ADDRESS_REPEAT, OGPU_ADDRESS_REPEAT}};
    TRY(ogpu_sampler_heap_write(samplers, 0, descriptions, 2, &error));
    TRY(ogpu_image_heap_write(heaps[0], 3, NULL, 0, &error));
    TRY(ogpu_sampler_heap_write(samplers, 2, NULL, 0, &error));
    OgpuImageEntry invalid = {source, OGPU_IMAGE_SAMPLED, 1};
    REQUIRE(ogpu_image_heap_write(heaps[0], 0, &invalid, 1, &error) == OGPU_ERROR_INVALID_ARGUMENT);
    TRY(ogpu_buffer_create(device, sizeof(OgpuDrawArguments), OGPU_MEMORY_HOST, &indirect, &error));
    const OgpuDrawArguments draw = {3, 1, 0, 0};
    TRY(ogpu_buffer_write(indirect, 0, &draw, sizeof(draw), &error));
    TRY(ogpu_buffer_create(device, 2 * (size + 8), OGPU_MEMORY_HOST, &readback, &error));
    for (unsigned pass = 0; pass < 4; ++pass) {
        unsigned variant = pass % 2;
        // Reuse the same allocations after all previous recorded references die.
        TRY(ogpu_image_heap_clear(heaps[0], 0, 3, &error));
        TRY(ogpu_image_heap_clear(heaps[1], 0, 3, &error));
        TRY(ogpu_image_heap_write(heaps[0], 0, a, 3, &error));
        TRY(ogpu_image_heap_write(heaps[1], 0, b, 3, &error));
        memset(pixels, 0xa5, 2 * (size + 8));
        TRY(ogpu_buffer_write(readback, 0, pixels, 2 * (size + 8), &error));
        TRY(ogpu_batch_create(device, &batch, &error));
        TRY(ogpu_batch_draw_indirect(batch, pattern, source, indirect, 0, NULL, 0, OGPU_ATTACHMENT_CLEAR, &error));
        TRY(ogpu_batch_discard_image(batch, processed, &error));
        TRY(ogpu_batch_submit(batch, &completions[0], &error));
        ogpu_batch_destroy(batch); batch = NULL;
        TRY(ogpu_batch_create(device, &batch, &error));
        TRY(ogpu_batch_bind_image_heap(batch, heaps[1 - variant], &error));
        TRY(ogpu_batch_bind_image_heap(batch, heaps[variant], &error));
        REQUIRE(ogpu_image_heap_clear(heaps[1 - variant], 0, 1, &error) == OGPU_ERROR_INVALID_ARGUMENT);
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COLOR_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
        Root process = {variant ? 2 : 1, variant ? 0 : 2, width, height};
        SampleRoot sampling = {variant ? 1 : 0, variant, width, height, pass < 2 ? 0.0f : variant ? 0.5f : 1.0f};
        TRY(ogpu_batch_dispatch(batch, compute, (width * height + 63) / 64, 1, 1,
            &process, sizeof(process), &error));
        TRY(ogpu_batch_submit(batch, &completions[1], &error));
        ogpu_batch_destroy(batch); batch = NULL;
        TRY(ogpu_batch_create(device, &batch, &error));
        TRY(ogpu_batch_bind_image_heap(batch, heaps[variant], &error));
        TRY(ogpu_batch_bind_sampler_heap(batch, samplers, &error));
        REQUIRE(ogpu_sampler_heap_write(samplers, 0, descriptions, 2, &error) == OGPU_ERROR_INVALID_ARGUMENT);
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_FRAGMENT_READ, &error));
        TRY(ogpu_batch_draw_indirect(batch, sample, final, indirect, 0,
            &sampling, sizeof(sampling), OGPU_ATTACHMENT_CLEAR, &error));
        TRY(ogpu_batch_copy_image_to_buffer(batch, final, readback, 4, &error));
        // Diagnostic copies happen only after the full GPU chain. The processed
        // target was initialized by discard, never by a draw.
        TRY(ogpu_batch_copy_image_to_buffer(batch, processed, readback, size + 12, &error));
        if (pass == 3) {
            // Recorded bindings/draws/discard retain everything, even before submit.
            for (unsigned i = 0; i < 2; ++i) { ogpu_image_heap_destroy(heaps[i]); heaps[i] = NULL; }
            ogpu_sampler_heap_destroy(samplers); samplers = NULL;
            ogpu_image_destroy(source); source = NULL;
            ogpu_image_destroy(processed); processed = NULL;
            ogpu_image_destroy(final); final = NULL;
        }
        TRY(ogpu_batch_submit(batch, &completions[2], &error));
        ogpu_batch_destroy(batch); batch = NULL;
        TRY(ogpu_completion_wait(completions[2], &error));
        if (pass != 3) {
            // Earlier unobserved submission still retains both image heaps;
            // the sampler heap was used only by the just-retired submission.
            REQUIRE(ogpu_image_heap_clear(heaps[0], 0, 1, &error) == OGPU_ERROR_INVALID_ARGUMENT);
            TRY(ogpu_sampler_heap_write(samplers, 0, descriptions, 2, &error));
            TRY(ogpu_completion_wait(completions[1], &error));
            TRY(ogpu_image_heap_clear(heaps[0], 0, 3, &error));
            TRY(ogpu_image_heap_clear(heaps[1], 0, 3, &error));
            uint32_t complete = 0;
            TRY(ogpu_completion_poll(completions[2], &complete, &error));
            REQUIRE(complete); // All receipts remain alive during edits.
        }
        TRY(ogpu_buffer_read(readback, 0, pixels, 2 * (size + 8), &error));
        for (unsigned image = 0; image < 2; ++image) {
            uint8_t *data = pixels + image * (size + 8);
            for (unsigned i = 0; i < 4; ++i) REQUIRE(data[i] == 0xa5 && data[size + 4 + i] == 0xa5);
            for (uint32_t y = 0; y < height; ++y) for (uint32_t x = 0; x < width; ++x) {
                uint32_t sy = height - 1 - y;
                uint32_t sx = image == 0 && pass == 2 && x + 1 < width ? x + 1 : x;
                uint8_t expected[] = {(uint8_t)(sx * 11 + sy * 7),
                    (uint8_t)(255 - ((sx * 5 + sy * 29) & 255)), (uint8_t)(sx * 17 + sy * 3), 255};
                if (image == 0 && pass == 3) {
                    uint32_t next = (x + 1) % width;
                    const uint8_t other[] = {(uint8_t)(next * 11 + sy * 7),
                        (uint8_t)(255 - ((next * 5 + sy * 29) & 255)), (uint8_t)(next * 17 + sy * 3), 255};
                    for (unsigned c = 0; c < 3; ++c) expected[c] = (uint8_t)(((unsigned)expected[c] + other[c] + 1) / 2);
                }
                const uint8_t *actual = data + 4 + ((size_t)y * width + x) * 4;
                for (unsigned c = 0; c < 4; ++c) {
                    int delta = (int)actual[c] - expected[c];
                    int tolerance = image == 0 && variant && c < 3 ? 1 : 0;
                    REQUIRE(delta >= -tolerance && delta <= tolerance);
                }
            }
        }
        for (unsigned i = 0; i < 3; ++i) { ogpu_completion_destroy(completions[i]); completions[i] = NULL; }
    }
    printf("heap-image %ux%u: three submissions, independent heaps, four sampler/index cases, retention/mutation, pixels/guards PASS\n", width, height);
    result = EXIT_SUCCESS;
cleanup:
    for (unsigned i = 0; i < 3; ++i) ogpu_completion_destroy(completions[i]);
    ogpu_batch_destroy(batch);
    for (unsigned i = 0; i < 2; ++i) ogpu_image_heap_destroy(heaps[i]);
    ogpu_sampler_heap_destroy(samplers);
    ogpu_image_destroy(final);
    ogpu_image_destroy(processed);
    ogpu_image_destroy(source);
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
        if (status == OGPU_ERROR_UNSUPPORTED) {
            fprintf(stderr, "Skipping heap-image device %u: %s\n", i, error.message);
            continue;
        }
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Executing heap-image on %s\n", info.name);
        TRY(ogpu_kernel_create(device, &(OgpuShaderDesc){words[2], counts[2], NULL, 0, 0}, sizeof(Root), &compute, &error));
        TRY(ogpu_raster_create(device, &(OgpuShaderDesc){words[0], counts[0], NULL, 0, 0},
            &(OgpuShaderDesc){words[1], counts[1], NULL, 0, 0}, 0, OGPU_TOPOLOGY_TRIANGLE_LIST, &pattern, &error));
        TRY(ogpu_raster_create(device, &(OgpuShaderDesc){words[0], counts[0], NULL, 0, 0},
            &(OgpuShaderDesc){words[3], counts[3], NULL, 0, 0}, sizeof(SampleRoot), OGPU_TOPOLOGY_TRIANGLE_LIST, &sample, &error));
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
