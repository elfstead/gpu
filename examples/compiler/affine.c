/* Exact binary32 affine transport/layout fixture, not a floating-point accuracy benchmark. */
#include <affine.generated.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

#define REQUIRE(c) do { if (!(c)) { fprintf(stderr, "Check failed at %d: %s\n", __LINE__, #c); goto cleanup; } } while (0)
#define TRY(op) do { OgpuResult s = (op); if (s != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: %" PRId32 " %s\n", #op, s, error.message); goto cleanup; } } while (0)

int main(void) {
    int result = EXIT_FAILURE;
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuBuffer *buffer = NULL;
    OgpuKernel *kernel = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *done = NULL;
    OgpuError error = {0};
    OgpuCapabilities caps = {0};
    OgpuDeviceLimits limits = {0};
    const uint32_t count = 4099;
    const size_t bytes = (count + 2u) * sizeof(float);
    float *expected = NULL, *actual = NULL;
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t devices = 0;
    TRY(ogpu_probe_device_count(probe, &devices));
    for (uint32_t i = 0; i < devices; ++i) {
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        TRY(ogpu_device_capabilities(device, &caps, &error));
        TRY(ogpu_device_limits(device, &limits, &error));
        if (affine_compatible(&caps, &limits)) {
            OgpuDeviceInfo info;
            TRY(ogpu_probe_device_info(probe, i, &info));
            printf("Affine device: %s (backend=%u)\n", info.name, info.backend);
            break;
        }
        ogpu_device_destroy(device); device = NULL;
    }
    REQUIRE(device != NULL);
    OgpuDeviceLimits too_small = limits;
    too_small.max_push_data_bytes = sizeof(AffineArguments) - 1;
    REQUIRE(!affine_compatible(&caps, &too_small));
    OgpuCapabilities missing = caps;
    missing.buffer_device_address = 0;
    REQUIRE(!affine_compatible(&missing, &limits));
    REQUIRE(affine_local[1] == 1 && affine_local[2] == 1);
    const uint32_t groups = count / affine_local[0] + (count % affine_local[0] != 0);
    REQUIRE(groups <= limits.max_dispatch[0]);
    TRY(ogpu_buffer_create(device, bytes, OGPU_MEMORY_HOST, &buffer, &error));
    OgpuShaderDesc shader = affine_shader();
    TRY(ogpu_kernel_create(device, &shader, sizeof(AffineArguments), &kernel, &error));
    expected = malloc(bytes); actual = malloc(bytes);
    REQUIRE(expected && actual);
    expected[0] = -8192.0f; expected[count + 1] = 8192.0f;
    for (uint32_t i = 0; i < count; ++i) expected[i + 1] = (float)((int)(i % 257) - 128);
    TRY(ogpu_buffer_write(buffer, 0, expected, bytes, &error));
    uint64_t address = 0;
    TRY(ogpu_buffer_device_address(buffer, &address, &error));
    const float scales[] = {0.5f, -2.0f, 0.25f}, biases[] = {0.25f, -0.5f, 1.0f};
    for (unsigned pass = 0; pass < 3; ++pass) {
        AffineArguments args = {.arg_data = address + sizeof(float), .arg_count = count,
            .arg_scale = scales[pass], .arg_bias = biases[pass]};
        TRY(ogpu_batch_create(device, &batch, &error));
        TRY(ogpu_batch_retain_buffer(batch, buffer, &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE,
            OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &error));
        TRY(ogpu_batch_dispatch(batch, kernel, groups, 1, 1, &args, sizeof(args), &error));
        args = (AffineArguments){0}; // Recorded values, not this object's address.
        TRY(ogpu_batch_submit(batch, &done, &error));
        TRY(ogpu_completion_wait(done, &error));
        TRY(ogpu_buffer_read(buffer, 0, actual, bytes, &error));
        for (uint32_t i = 0; i < count; ++i)
            expected[i + 1] = expected[i + 1] * scales[pass] + biases[pass];
        for (uint32_t i = 0; i < count + 2; ++i) REQUIRE(actual[i] == expected[i]);
        ogpu_completion_destroy(done); done = NULL;
        ogpu_batch_destroy(batch); batch = NULL;
    }
    printf("Affine PASS: %u exact binary32 values, three changing roots, all outputs and guards; root=%zu local=%u\n",
        count, sizeof(AffineArguments), affine_local[0]);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(done);
    ogpu_batch_destroy(batch);
    ogpu_kernel_destroy(kernel);
    ogpu_buffer_destroy(buffer);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    free(expected); free(actual);
    return result;
}
