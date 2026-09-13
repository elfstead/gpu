#include "ogpu.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

/* All operations and checks remain active in release builds. */
#define REQUIRE(condition) do { if (!(condition)) { \
    fprintf(stderr, "Check failed at line %d: %s\n", __LINE__, #condition); goto cleanup; \
} } while (0)
#define TRY(operation) do { OgpuResult status_ = (operation); if (status_ != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: status %" PRId32 ", %s\n", #operation, status_, error.message); goto cleanup; \
} } while (0)

typedef struct Root {
    uint64_t address;
    uint32_t count;
    uint32_t reserved;
} Root;
_Static_assert(sizeof(Root) == 16, "push block size");
_Static_assert(offsetof(Root, count) == 8, "shader count offset");

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    FILE *file = NULL;
    uint32_t *words = NULL, *input = NULL, *output = NULL;
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuBuffer *buffer = NULL;
    OgpuKernel *kernel = NULL;
    OgpuError error = {0};
    OgpuDeviceInfo info;
    const uint32_t count = 4099;
    const uint64_t size = (uint64_t)count * sizeof(uint32_t);
    REQUIRE(argc == 2);
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
    REQUIRE(ogpu_device_create(probe, device_count, &device, &error) == OGPU_ERROR_OUT_OF_RANGE && device == NULL);
    for (uint32_t i = 0; i < device_count; ++i) {
        TRY(ogpu_probe_device_info(probe, i, &info));
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        REQUIRE(status == OGPU_SUCCESS);
        break;
    }
    REQUIRE(device != NULL);
    printf("Executing on %s\n", info.name);
    /* The device retains the instance; discovery can now be released. */
    ogpu_probe_destroy(probe); probe = NULL;

    REQUIRE(ogpu_buffer_create(device, 0, OGPU_MEMORY_HOST, &buffer, &error) == OGPU_ERROR_INVALID_ARGUMENT && buffer == NULL);
    REQUIRE(ogpu_buffer_create(device, size, 99, &buffer, &error) == OGPU_ERROR_INVALID_ARGUMENT && buffer == NULL);
    TRY(ogpu_buffer_create(device, size, OGPU_MEMORY_DEVICE, &buffer, &error));
    uint32_t unchanged = 0xabcdef01;
    REQUIRE(ogpu_buffer_read(buffer, 0, &unchanged, sizeof(unchanged), &error) == OGPU_ERROR_INVALID_ARGUMENT);
    REQUIRE(unchanged == 0xabcdef01);
    REQUIRE(ogpu_buffer_write(buffer, 0, &unchanged, sizeof(unchanged), &error) == OGPU_ERROR_INVALID_ARGUMENT);
    REQUIRE(ogpu_buffer_write(buffer, size, NULL, 0, &error) == OGPU_ERROR_INVALID_ARGUMENT);
    ogpu_buffer_destroy(buffer); buffer = NULL;
    TRY(ogpu_buffer_create(device, size, OGPU_MEMORY_HOST, &buffer, &error));
    uint32_t invalid_module[5] = {0};
    REQUIRE(ogpu_kernel_create(device, &(OgpuShaderDesc){invalid_module, 5, NULL, 0, 0}, 16, &kernel, &error) == OGPU_ERROR_INVALID_ARGUMENT && kernel == NULL);
    TRY(ogpu_kernel_create(device, &(OgpuShaderDesc){words, (uint64_t)byte_count / 4, NULL, 0, 0}, sizeof(Root), &kernel, &error));
    /* Children retain their device too. This tests ownership, not just shutdown order. */
    ogpu_device_destroy(device); device = NULL;

    input = malloc((size_t)size);
    output = malloc((size_t)size); /* Deliberately not preinitialized: read fills it. */
    REQUIRE(input != NULL && output != NULL);
    for (uint32_t i = 0; i < count; ++i) input[i] = i;
    TRY(ogpu_buffer_write(buffer, 0, input, size, &error));
    TRY(ogpu_buffer_write(buffer, size, NULL, 0, &error));
    REQUIRE(ogpu_buffer_write(buffer, UINT64_MAX, input, 4, &error) == OGPU_ERROR_OUT_OF_RANGE);
    Root root = {0};
    TRY(ogpu_buffer_device_address(buffer, &root.address, &error));
    REQUIRE(root.address != 0);
    root.count = count;
    REQUIRE(ogpu_dispatch_wait(kernel, 0, 1, 1, &root, sizeof(root), &error) == OGPU_ERROR_INVALID_ARGUMENT);
    REQUIRE(ogpu_dispatch_wait(kernel, 1, 1, 1, &root, 12, &error) == OGPU_ERROR_INVALID_ARGUMENT);

    for (uint32_t pass = 0; pass < 3; ++pass) {
        if (pass == 2) {
            input[1] = 123;
            TRY(ogpu_buffer_write(buffer, 4, &input[1], 4, &error));
        }
        TRY(ogpu_dispatch_wait(kernel, (count + 63) / 64, 1, 1, &root, sizeof(root), &error));
        for (uint32_t i = 0; i < count; ++i) input[i] = input[i] * 3u + 7u;
        TRY(ogpu_buffer_read(buffer, 0, output, size, &error));
        for (uint32_t i = 0; i < count; ++i) REQUIRE(output[i] == input[i]);
    }
    uint32_t sentinel = 42;
    REQUIRE(ogpu_buffer_read(buffer, size, &sentinel, 4, &error) == OGPU_ERROR_OUT_OF_RANGE && sentinel == 42);
    TRY(ogpu_buffer_read(buffer, size, NULL, 0, &error));
    printf("Verified %" PRIu32 " integers across three dispatches, partial upload, and parent-handle destruction.\n", count);
    exit_code = EXIT_SUCCESS;
cleanup:
    ogpu_kernel_destroy(kernel);
    ogpu_buffer_destroy(buffer);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    free(output); free(input); free(words);
    if (file) fclose(file);
    return exit_code;
}
