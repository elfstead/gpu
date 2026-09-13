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

typedef struct Root {
    uint64_t input_address;
    uint64_t output_address;
    uint32_t count;
    uint32_t reserved;
} Root;
_Static_assert(sizeof(Root) == 24, "push block size");
_Static_assert(offsetof(Root, output_address) == 8, "shader output address offset");
_Static_assert(offsetof(Root, count) == 16, "shader count offset");

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    FILE *file = NULL;
    uint32_t *words = NULL, *input = NULL;
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuKernel *kernel = NULL;
    OgpuBuffer *source = NULL;
    OgpuBuffer *scratch[8] = {0};
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    const uint32_t count = 1048579;
    const uint64_t size = (uint64_t)count * sizeof(uint32_t);
    uint32_t levels = 0;
    REQUIRE(argc == 2); /* reduce.spv */

    file = fopen(argv[1], "rb");
    REQUIRE(file != NULL);
    REQUIRE(fseek(file, 0, SEEK_END) == 0);
    long byte_count = ftell(file);
    REQUIRE(byte_count >= 20 && byte_count % 4 == 0);
    REQUIRE(fseek(file, 0, SEEK_SET) == 0);
    words = malloc((size_t)byte_count);
    REQUIRE(words != NULL);
    REQUIRE(fread(words, 1, (size_t)byte_count, file) == (size_t)byte_count);
    fclose(file); file = NULL;

    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t device_count = 0;
    TRY(ogpu_probe_device_count(probe, &device_count));
    for (uint32_t i = 0; i < device_count; ++i) {
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Reducing on %s\n", info.name);
        break;
    }
    REQUIRE(device != NULL);
    TRY(ogpu_kernel_create(device, words, (uint64_t)byte_count / 4, sizeof(Root), &kernel, &error));
    TRY(ogpu_buffer_create(device, size, OGPU_MEMORY_HOST, &source, &error));
    input = malloc((size_t)size);
    REQUIRE(input != NULL);
    uint64_t reference = 0;
    for (uint32_t i = 0; i < count; ++i) {
        input[i] = (i * 1664525u + 1013904223u) ^ (i >> 5);
        reference += input[i];
    }
    TRY(ogpu_buffer_write(source, 0, input, size, &error));

    TRY(ogpu_batch_create(device, &batch, &error));
    uint32_t remaining = count;
    uint64_t input_address = 0;
    TRY(ogpu_buffer_device_address(source, &input_address, &error));
    do {
        /* 64 invocations process up to 128 values, producing one partial per group.
         * Division/remainder avoids overflow from adding 127 to a large count. */
        uint32_t groups = remaining / 128 + (remaining % 128 != 0);
        if (groups == 0) groups = 1;
        REQUIRE(levels < sizeof(scratch) / sizeof(scratch[0]));
        TRY(ogpu_buffer_create(device, (uint64_t)groups * sizeof(uint32_t), OGPU_MEMORY_HOST, &scratch[levels], &error));
        Root root = {0};
        root.input_address = input_address;
        root.count = remaining;
        TRY(ogpu_buffer_device_address(scratch[levels], &root.output_address, &error));
        TRY(ogpu_batch_dispatch(batch, kernel, groups, 1, 1, &root, sizeof(root), &error));
        printf("  %" PRIu32 " values -> %" PRIu32 " partials\n", remaining, groups);
        if (groups != 1) {
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
        }
        /* Root bytes are copied, but their pointees must stay alive through completion. */
        input_address = root.output_address;
        remaining = groups;
        ++levels;
    } while (remaining > 1);

    TRY(ogpu_batch_submit(batch, &completion, &error));
    TRY(ogpu_completion_wait(completion, &error));
    uint32_t result = 0;
    TRY(ogpu_buffer_read(scratch[levels - 1], 0, &result, sizeof(result), &error));
    REQUIRE(result == (uint32_t)reference);
    printf("Verified sum mod 2^32 = %" PRIu32 ": %" PRIu32 " levels, one batch, one wait.\n", result, levels);
    exit_code = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(completion);
    ogpu_batch_destroy(batch);
    for (size_t i = 0; i < sizeof(scratch) / sizeof(scratch[0]); ++i) ogpu_buffer_destroy(scratch[i]);
    ogpu_buffer_destroy(source);
    ogpu_kernel_destroy(kernel);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    free(input);
    free(words);
    if (file) fclose(file);
    return exit_code;
}
