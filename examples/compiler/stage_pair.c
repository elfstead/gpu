#include <pattern.generated.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(x) do { if (!(x)) { fprintf(stderr, "Check failed line %d: %s\n", __LINE__, #x); goto cleanup; } } while (0)
#define TRY(x) do { if ((x) != OGPU_SUCCESS) { fprintf(stderr, "%s: %s\n", #x, error.message); goto cleanup; } } while (0)

static int run_case(OgpuDevice *device, OgpuRaster *raster, uint32_t width, uint32_t height) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuImage *image = NULL;
    OgpuBuffer *indirect = NULL, *readback = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    const size_t size = (size_t)width * height * 4;
    uint8_t *pixels = malloc(size + 128);
    REQUIRE(pixels);
    OgpuImageDesc desc = {OGPU_IMAGE_2D, width, height, OGPU_FORMAT_RGBA8_UNORM,
        OGPU_IMAGE_USAGE_COLOR | OGPU_IMAGE_USAGE_COPY_SRC, 0};
    TRY(ogpu_image_create(device, &desc, &image, &error));
    TRY(ogpu_buffer_create(device, 16, OGPU_MEMORY_HOST, &indirect, &error));
    TRY(ogpu_buffer_create(device, size + 128, OGPU_MEMORY_HOST, &readback, &error));
    const uint32_t triangle[] = {3, 1, 0, 0};
    TRY(ogpu_buffer_write(indirect, 0, triangle, sizeof(triangle), &error));
    for (unsigned frame = 0; frame < 3; ++frame) {
        memset(pixels, 0xa5, size + 128);
        TRY(ogpu_buffer_write(readback, 0, pixels, size + 128, &error));
        TRY(ogpu_batch_create(device, &batch, &error));
        Pattern_fragmentArguments root = {.arg_width = width, .arg_height = height};
        TRY(ogpu_batch_draw_indirect(batch, raster, image, indirect, 0, &root, sizeof(root), OGPU_ATTACHMENT_CLEAR, &error));
        memset(&root, 0xff, sizeof(root));
        TRY(ogpu_batch_copy_image_to_buffer(batch, image, readback, 64, &error));
        TRY(ogpu_batch_submit(batch, &completion, &error));
        ogpu_batch_destroy(batch); batch = NULL;
        TRY(ogpu_completion_wait(completion, &error));
        TRY(ogpu_buffer_read(readback, 0, pixels, size + 128, &error));
        for (size_t i = 0; i < 64; ++i) REQUIRE(pixels[i] == 0xa5 && pixels[64 + size + i] == 0xa5);
        for (uint32_t y = 0; y < height; ++y) for (uint32_t x = 0; x < width; ++x) {
            const uint8_t expected[] = {(uint8_t)(x * 17 + y * 3), (uint8_t)(x * 5 + y * 29),
                                        (uint8_t)(x * 11 + y * 7), 255};
            for (unsigned channel = 0; channel < 4; ++channel) {
                uint8_t actual = pixels[64 + ((size_t)y * width + x) * 4 + channel];
                if (actual != expected[channel]) {
                    fprintf(stderr, "Stage pair %ux%u (%u,%u) channel %u: %u != %u\n",
                            width, height, x, y, channel, actual, expected[channel]);
                    goto cleanup;
                }
            }
        }
        ogpu_completion_destroy(completion); completion = NULL;
    }
    printf("Stage pair %ux%u: three draws, exact pixels/guards, root snapshot PASS\n", width, height);
    result = EXIT_SUCCESS;
cleanup:
    if (completion) (void)ogpu_completion_wait(completion, &error);
    ogpu_completion_destroy(completion);
    ogpu_batch_destroy(batch);
    ogpu_buffer_destroy(readback);
    ogpu_buffer_destroy(indirect);
    ogpu_image_destroy(image);
    free(pixels);
    return result;
}

int main(void) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuRaster *raster = NULL;
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t count = 0, tested = 0;
    TRY(ogpu_probe_device_count(probe, &count));
    for (uint32_t i = 0; i < count; ++i) {
        OgpuResult status = ogpu_device_create_graphics(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) {
            fprintf(stderr, "Skipping stage-pair device %u: %s\n", i, error.message);
            continue;
        }
        TRY(status);
        OgpuDeviceInfo info;
        OgpuCapabilities caps;
        OgpuDeviceLimits limits;
        TRY(ogpu_probe_device_info(probe, i, &info));
        TRY(ogpu_device_capabilities(device, &caps, &error));
        TRY(ogpu_device_limits(device, &limits, &error));
        REQUIRE(pattern_vertex_compatible(&caps, &limits) && pattern_fragment_compatible(&caps, &limits));
        printf("Executing stage pair on %s\n", info.name);
        OgpuShaderDesc vertex = pattern_vertex_shader(), fragment = pattern_fragment_shader();
        TRY(ogpu_raster_create(device, &vertex, &fragment, pattern_push_size,
            OGPU_TOPOLOGY_TRIANGLE_LIST, OGPU_FORMAT_RGBA8_UNORM, &raster, &error));
        const uint32_t sizes[][2] = {{1, 1}, {2, 3}, {63, 65}, {64, 64}, {65, 63}, {97, 65}};
        for (unsigned j = 0; j < sizeof(sizes) / sizeof(sizes[0]); ++j)
            REQUIRE(run_case(device, raster, sizes[j][0], sizes[j][1]) == EXIT_SUCCESS);
        ogpu_raster_destroy(raster); raster = NULL;
        ogpu_device_destroy(device); device = NULL;
        ++tested;
    }
    REQUIRE(tested);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_raster_destroy(raster);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    return result;
}
