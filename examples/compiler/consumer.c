#include <transform.generated.h>
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
    OgpuRecordingStorage *storage = NULL;
    OgpuCommandList *list = NULL;
    const int replay = getenv("OGPU_EXAMPLE_REPLAY") != NULL;
    const int explicit_storage = replay || getenv("OGPU_EXAMPLE_RECORDING_STORAGE") != NULL;
    OgpuError error = {0};
    const uint32_t count = 4099, guard = 0xa55adeadu;
    const uint64_t bytes = (uint64_t)(count + 2) * sizeof(uint32_t);
    uint32_t *expected = NULL, *actual = NULL;
    OgpuCapabilities caps = {0};
    OgpuDeviceLimits limits = {0};
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t devices = 0;
    TRY(ogpu_probe_device_count(probe, &devices));
    for (uint32_t i = 0; i < devices; ++i) {
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) {
            fprintf(stderr, "Device %u unsupported: %s\n", i, error.message);
            continue;
        }
        TRY(status);
        TRY(ogpu_device_capabilities(device, &caps, &error));
        TRY(ogpu_device_limits(device, &limits, &error));
        if (transform_compatible(&caps, &limits)) {
            OgpuDeviceInfo info;
            TRY(ogpu_probe_device_info(probe, i, &info));
            printf("Compiler consumer device: %s (backend=%u)\n", info.name, info.backend);
            break;
        }
        ogpu_device_destroy(device); device = NULL;
    }
    REQUIRE(device != NULL);
    if (explicit_storage) {
        TRY(ogpu_recording_storage_create(device, &storage, &error));
        TRY(ogpu_recording_storage_trim(storage, &error));
        TRY(ogpu_batch_create_in(storage, &batch, &error));
        REQUIRE(ogpu_recording_storage_trim(storage, &error) == OGPU_ERROR_INVALID_ARGUMENT);
        ogpu_batch_destroy(batch); batch = NULL;
    }
    // Negatives use the generated predicate, never submit an incompatible module.
    OgpuCapabilities missing = caps;
    missing.buffer_device_address = 0;
    REQUIRE(!transform_compatible(&missing, &limits));
    OgpuDeviceLimits too_small = limits;
    too_small.max_push_data_bytes = sizeof(TransformArguments) - 1;
    REQUIRE(!transform_compatible(&caps, &too_small));
    too_small = limits; too_small.max_group_invocations = 0;
    REQUIRE(!transform_compatible(&caps, &too_small));
    // This consumer has a 1D logical extent; the compiler does not infer it.
    REQUIRE(transform_local[1] == 1 && transform_local[2] == 1);
    const uint32_t groups = count / transform_local[0] + (count % transform_local[0] != 0);
    REQUIRE(groups <= limits.max_dispatch[0]);
    ogpu_probe_destroy(probe); probe = NULL;
    TRY(ogpu_buffer_create(device, bytes, OGPU_MEMORY_HOST, &buffer, &error));
    OgpuShaderDesc shader = transform_shader();
    TRY(ogpu_kernel_create(device, &shader, sizeof(TransformArguments), &kernel, &error));
    expected = malloc((size_t)bytes); actual = malloc((size_t)bytes);
    REQUIRE(expected && actual);
    expected[0] = expected[count + 1] = guard;
    for (uint32_t i = 0; i < count; ++i) expected[i + 1] = i;
    TRY(ogpu_buffer_write(buffer, 0, expected, bytes, &error));
    TransformArguments args = {0};
    TRY(ogpu_buffer_device_address(buffer, &args.arg_data, &error));
    args.arg_data += sizeof(uint32_t);
    args.arg_count = count;
    for (unsigned pass = 0; pass < 3; ++pass) {
        if (pass == 2) {
            expected[2] = 123;
            TRY(ogpu_buffer_write(buffer, 2 * sizeof(uint32_t), &expected[2], sizeof(uint32_t), &error));
        }
        if (!replay || pass == 0) {
            if (explicit_storage) {
                TRY(ogpu_batch_create_in(storage, &batch, &error));
                OgpuBatch *rejected = NULL;
                REQUIRE(ogpu_batch_create_in(storage, &rejected, &error) == OGPU_ERROR_INVALID_ARGUMENT);
                REQUIRE(rejected == NULL);
                REQUIRE(ogpu_batch_submit(batch, NULL, &error) == OGPU_ERROR_INVALID_ARGUMENT);
                REQUIRE(ogpu_recording_storage_trim(storage, &error) == OGPU_ERROR_INVALID_ARGUMENT);
            } else { TRY(ogpu_batch_create(device, &batch, &error)); }
            TRY(ogpu_batch_retain_buffer(batch, buffer, &error));
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE,
                OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &error));
            TRY(ogpu_batch_dispatch(batch, kernel, groups, 1, 1, &args, sizeof(args), &error));
            if (replay) {
                REQUIRE(ogpu_batch_compile(batch, NULL, &error) == OGPU_ERROR_INVALID_ARGUMENT);
                TRY(ogpu_batch_compile(batch, &list, &error));
                ogpu_batch_destroy(batch); batch = NULL;
                args = (TransformArguments){0}; // The list owns its original root copy.
            }
        }
        if (pass == 2) {
            ogpu_device_destroy(device); device = NULL;
            // Batch/submission owns the storage lease after public-parent release.
            ogpu_recording_storage_destroy(storage); storage = NULL;
        }
        if (replay) {
            TRY(ogpu_command_list_submit(list, &done, &error));
            if (pass == 2) { ogpu_command_list_destroy(list); list = NULL; }
        } else { TRY(ogpu_batch_submit(batch, &done, &error)); }
        TRY(ogpu_completion_wait(done, &error));
        if (storage && replay) {
            REQUIRE(ogpu_recording_storage_trim(storage, &error) == OGPU_ERROR_INVALID_ARGUMENT);
        } else if (storage && pass == 1) {
            // Old consumed batch and receipt survive explicit capacity release.
            TRY(ogpu_recording_storage_trim(storage, &error));
            TRY(ogpu_recording_storage_trim(storage, &error));
        }
        ogpu_completion_destroy(done); done = NULL;
        ogpu_batch_destroy(batch); batch = NULL;
        for (uint32_t i = 0; i < count; ++i) expected[i + 1] = expected[i + 1] * 3u + 7u;
        TRY(ogpu_buffer_read(buffer, 0, actual, bytes, &error));
        for (uint32_t i = 0; i < count + 2; ++i) REQUIRE(actual[i] == expected[i]);
    }
    printf("Compiler consumer PASS: %u integers, three passes, guards, partial upload, parent destruction; root=%zu local=%u\n",
        count, sizeof(args), transform_local[0]);
    result = EXIT_SUCCESS;
    if (replay) puts("Reusable command list: copied roots/mutable data/persistent ownership/early destruction PASS");
    else if (explicit_storage) puts("Explicit recording storage: reserve/reject/reuse/trim/early-owner-destruction PASS");
cleanup:
    ogpu_completion_destroy(done);
    ogpu_batch_destroy(batch);
    ogpu_command_list_destroy(list);
    ogpu_recording_storage_destroy(storage);
    ogpu_kernel_destroy(kernel);
    ogpu_buffer_destroy(buffer);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    free(expected); free(actual);
    return result;
}
