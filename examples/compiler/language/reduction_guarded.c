/* Additional full-partial/guard oracle; same public API and root as reduction.c. */
#define main original_reduction_main
#include "../../reduction.c"
#undef main
#include <string.h>

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0}; OgpuProbe *probe = NULL; OgpuDevice *device = NULL;
    OgpuKernel *kernel = NULL; OgpuBuffer *input = NULL, *output = NULL;
    OgpuBatch *batch = NULL; OgpuCompletion *completion = NULL;
    FILE *file = NULL; uint32_t *code = NULL;
    enum { CAPACITY = 1025, PARTIALS = 9 };
    uint32_t source[CAPACITY+2], readback[CAPACITY+2], partial[PARTIALS+2];
    const uint32_t counts[] = {0, 1, 63, 64, 65, 127, 128, 129, 1025, 1};
    const uint32_t poison = 0xbaadf00du;
    REQUIRE(argc == 2);
    file = fopen(argv[1], "rb"); REQUIRE(file);
    REQUIRE(fseek(file, 0, SEEK_END) == 0);
    long bytes = ftell(file); REQUIRE(bytes >= 20 && bytes <= 1024*1024 && !(bytes % 4));
    REQUIRE(fseek(file, 0, SEEK_SET) == 0);
    code = malloc((size_t)bytes); REQUIRE(code);
    REQUIRE(fread(code, 1, (size_t)bytes, file) == (size_t)bytes);
    fclose(file); file = NULL;
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t device_count = 0;
    TRY(ogpu_probe_device_count(probe, &device_count)); REQUIRE(device_count == 1);
    TRY(ogpu_device_create(probe, 0, &device, &error));
    TRY(ogpu_kernel_create(device, &(OgpuShaderDesc){.code=code, .code_size=(uint64_t)bytes,
        .format=OGPU_SHADER_SPIRV}, sizeof(Root), &kernel, &error));
    TRY(ogpu_buffer_create(device, sizeof(source), OGPU_MEMORY_HOST, &input, &error));
    TRY(ogpu_buffer_create(device, sizeof(partial), OGPU_MEMORY_HOST, &output, &error));
    Root root = {0};
    TRY(ogpu_buffer_device_address(input, &root.input_address, &error));
    TRY(ogpu_buffer_device_address(output, &root.output_address, &error));
    root.input_address += 4; root.output_address += 4;
    for (unsigned pass = 0; pass < sizeof(counts)/sizeof(counts[0]); ++pass) {
        root.count = counts[pass];
        uint32_t groups = root.count/128 + (root.count%128 != 0);
        if (!groups) groups = 1;
        for (unsigned i = 0; i < CAPACITY+2; ++i) source[i] = poison;
        for (unsigned i = 0; i < root.count; ++i) source[i+1] = (i*1664525u+1013904223u) ^ (i>>5);
        for (unsigned i = 0; i < PARTIALS+2; ++i) partial[i] = poison;
        TRY(ogpu_buffer_write(input, 0, source, sizeof(source), &error));
        TRY(ogpu_buffer_write(output, 0, partial, sizeof(partial), &error));
        TRY(ogpu_batch_create(device, &batch, &error));
        TRY(ogpu_batch_dispatch(batch, kernel, groups, 1, 1, &root, sizeof(root), &error));
        TRY(ogpu_batch_submit(batch, &completion, &error));
        TRY(ogpu_completion_wait(completion, &error));
        TRY(ogpu_buffer_read(input, 0, readback, sizeof(readback), &error));
        REQUIRE(!memcmp(source, readback, sizeof(source)));
        TRY(ogpu_buffer_read(output, 0, partial, sizeof(partial), &error));
        REQUIRE(partial[0] == poison);
        for (unsigned i = groups+1; i < PARTIALS+2; ++i) REQUIRE(partial[i] == poison);
        for (unsigned group = 0; group < groups; ++group) {
            uint32_t sum = 0;
            for (unsigned i = group*128; i < (group+1)*128 && i < root.count; ++i) sum += source[i+1];
            REQUIRE(partial[group+1] == sum);
        }
        ogpu_completion_destroy(completion); completion = NULL;
        ogpu_batch_destroy(batch); batch = NULL;
        printf("Reduction count=%u: every partial, unchanged input and guards PASS\n", root.count);
    }
    exit_code = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(completion); ogpu_batch_destroy(batch);
    ogpu_buffer_destroy(output); ogpu_buffer_destroy(input); ogpu_kernel_destroy(kernel);
    ogpu_device_destroy(device); ogpu_probe_destroy(probe);
    if (file) fclose(file);
    free(code);
    return exit_code;
}
