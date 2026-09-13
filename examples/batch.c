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

/* An application-defined argument layout shared with both shaders. */
typedef struct Root {
    uint64_t input_address;
    uint64_t output_address;
    uint32_t count;
    uint32_t reserved;
} Root;
_Static_assert(sizeof(Root) == 24, "push block size");
_Static_assert(offsetof(Root, output_address) == 8, "shader output address offset");
_Static_assert(offsetof(Root, count) == 16, "shader count offset");

static int load_kernel(OgpuDevice *device, const char *path, OgpuKernel **out_kernel) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
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
    TRY(ogpu_kernel_create(device, words, (uint64_t)bytes / 4, sizeof(Root), out_kernel, &error));
    exit_code = EXIT_SUCCESS;
cleanup:
    free(words);
    if (file) fclose(file);
    return exit_code;
}

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuBuffer *source = NULL, *intermediate = NULL, *result = NULL;
    OgpuKernel *producer = NULL, *consumer = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    uint32_t *input = NULL, *output = NULL;
    const uint32_t count = 4099;
    const uint64_t size = (uint64_t)count * sizeof(uint32_t);
    REQUIRE(argc == 3); /* producer.spv consumer.spv */

    /* Select the first execution-capable device. */
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t device_count = 0;
    TRY(ogpu_probe_device_count(probe, &device_count));
    for (uint32_t i = 0; i < device_count; ++i) {
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Executing batch on %s\n", info.name);
        break;
    }
    REQUIRE(device != NULL);

    /* Three independent allocations: input → intermediate → result. */
    TRY(ogpu_buffer_create(device, size, OGPU_MEMORY_HOST, &source, &error));
    TRY(ogpu_buffer_create(device, size, OGPU_MEMORY_HOST, &intermediate, &error));
    TRY(ogpu_buffer_create(device, size, OGPU_MEMORY_HOST, &result, &error));
    REQUIRE(load_kernel(device, argv[1], &producer) == EXIT_SUCCESS);
    REQUIRE(load_kernel(device, argv[2], &consumer) == EXIT_SUCCESS);
    input = malloc((size_t)size);
    output = malloc((size_t)size);
    REQUIRE(input != NULL && output != NULL);
    for (uint32_t i = 0; i < count; ++i) input[i] = i;
    TRY(ogpu_buffer_write(source, 0, input, size, &error));

    Root produce = {0}, consume = {0};
    TRY(ogpu_buffer_device_address(source, &produce.input_address, &error));
    TRY(ogpu_buffer_device_address(intermediate, &produce.output_address, &error));
    consume.input_address = produce.output_address;
    TRY(ogpu_buffer_device_address(result, &consume.output_address, &error));
    produce.count = consume.count = count;

    /* Record two dispatches with an explicit producer-write → consumer-read dependency. */
    TRY(ogpu_batch_create(device, &batch, &error));
    TRY(ogpu_batch_dispatch(batch, producer, (count + 63) / 64, 1, 1, &produce, sizeof(produce), &error));
    TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
    TRY(ogpu_batch_dispatch(batch, consumer, (count + 63) / 64, 1, 1, &consume, sizeof(consume), &error));

    TRY(ogpu_batch_submit(batch, &completion, &error));
    /* Submission returned without waiting. CPU work on unrelated data could go here.
     * Keep all three GPU allocations alive and untouched until completion. */
    TRY(ogpu_completion_wait(completion, &error));
    TRY(ogpu_buffer_read(result, 0, output, size, &error));

    for (uint32_t i = 0; i < count; ++i) {
        uint32_t a = input[i] * 3u + 7u;
        uint32_t b = input[(i + 1u) % count] * 3u + 7u;
        REQUIRE(output[i] == a + b * 5u + 11u);
    }
    printf("Verified %" PRIu32 " integers: two kernels, one batch, one wait, no intermediate readback.\n", count);
    exit_code = EXIT_SUCCESS;
cleanup:
    /* Completion destruction also drains pending work on error paths. */
    ogpu_completion_destroy(completion);
    ogpu_batch_destroy(batch);
    ogpu_kernel_destroy(consumer);
    ogpu_kernel_destroy(producer);
    ogpu_buffer_destroy(result);
    ogpu_buffer_destroy(intermediate);
    ogpu_buffer_destroy(source);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    free(output);
    free(input);
    return exit_code;
}
