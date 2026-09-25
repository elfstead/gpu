/* Targeted compiler-source control, NOT an OGPU-versus-Vulkan benchmark.
 * Same root and host policy; identity-valued coefficients make repeated in-place
 * work exactly checkable without overflow. Every sample checks full output.
 */
#define main structured_correctness_main
#include "../structured.c"
#undef main

static int compare_ns(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return (x > y) - (x < y);
}

int main(int argc, char **argv) {
    int result = EXIT_FAILURE;
    OgpuProbe *probe = NULL; OgpuDevice *device = NULL; OgpuKernel *kernels[2] = {0};
    OgpuBuffer *data = NULL, *parameters = NULL; OgpuBatch *batch = NULL; OgpuCompletion *done = NULL;
    OgpuError error = {0}; FILE *file = NULL; uint32_t *code = NULL;
    enum { MAX_COUNT = 1048576, SAMPLES = 21, WARMUPS = 3 };
    const uint32_t counts[] = {4099, MAX_COUNT};
    const size_t bytes = (MAX_COUNT+2u)*sizeof(float);
    float *source = NULL, *actual = NULL;
    structured_type_Block blocks[2] = {0}, unchanged[2];
    REQUIRE(argc == 3);
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t devices = 0;
    TRY(ogpu_probe_device_count(probe, &devices)); REQUIRE(devices == 1);
    TRY(ogpu_device_create(probe, 0, &device, &error));
    OgpuCapabilities caps = {0}; OgpuDeviceLimits limits = {0}; OgpuTimingInfo timing = {0};
    TRY(ogpu_device_capabilities(device, &caps, &error));
    TRY(ogpu_device_limits(device, &limits, &error)); REQUIRE(structured_compatible(&caps, &limits));
    TRY(ogpu_device_timing_info(device, &timing, &error));
    REQUIRE(timing.timestamp_valid_bits >= 36 && timing.timestamp_valid_bits <= 64 && timing.timestamp_period_ns > 0);
    double wrap_ns = timing.timestamp_period_ns;
    for (unsigned i = 0; i < timing.timestamp_valid_bits; ++i) wrap_ns *= 2;
    for (unsigned v = 0; v < 2; ++v) {
        file = fopen(argv[v+1], "rb"); REQUIRE(file);
        REQUIRE(fseek(file, 0, SEEK_END) == 0);
        long length = ftell(file); REQUIRE(length >= 20 && length <= 1024*1024 && !(length%4));
        REQUIRE(fseek(file, 0, SEEK_SET) == 0);
        code = malloc((size_t)length); REQUIRE(code);
        REQUIRE(fread(code, 1, (size_t)length, file) == (size_t)length);
        fclose(file); file = NULL;
        OgpuShaderDesc shader = {.code=code, .code_size=(uint64_t)length, .format=OGPU_SHADER_SPIRV};
        TRY(ogpu_kernel_create(device, &shader, sizeof(StructuredArguments), &kernels[v], &error));
        free(code); code = NULL;
    }
    source = malloc(bytes); actual = malloc(bytes); REQUIRE(source && actual);
    source[0] = -8192; source[MAX_COUNT+1] = 8192;
    for (unsigned i = 0; i < MAX_COUNT; ++i) source[i+1] = (float)((int)(i%257)-128);
    TRY(ogpu_buffer_create(device, bytes, OGPU_MEMORY_HOST, &data, &error));
    TRY(ogpu_buffer_create(device, sizeof(blocks), OGPU_MEMORY_HOST, &parameters, &error));
    TRY(ogpu_buffer_device_address(data, &blocks[1].arg_data, &error)); blocks[1].arg_data += 4;
    for (unsigned c = 0; c < 2; ++c) blocks[1].arg_coefficients[c].arg_scaleBias[0] = 1;
    StructuredArguments root = {.arg_chosen=1};
    root.arg_controls.arg_mapping[1] = 1;
    TRY(ogpu_buffer_device_address(parameters, &root.arg_blocks, &error));
    for (unsigned extent = 0; extent < 2; ++extent) {
        const uint32_t count = counts[extent], groups = (count+63)/64;
        REQUIRE(groups <= limits.max_dispatch[0]);
        blocks[1].arg_count = count;
        TRY(ogpu_buffer_write(parameters, 0, blocks, sizeof(blocks), &error));
        TRY(ogpu_buffer_write(data, 0, source, bytes, &error));
        double samples[2][SAMPLES] = {{0}};
        for (unsigned round = 0; round < WARMUPS+SAMPLES; ++round) {
            for (unsigned order = 0; order < 2; ++order) {
                unsigned v = (round+order)%2; /* Alternate AB / BA order. */
                TRY(ogpu_batch_create(device, &batch, &error));
                TRY(ogpu_batch_enable_timing(batch, &error));
                TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE,
                    OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &error));
                TRY(ogpu_batch_dispatch(batch, kernels[v], groups, 1, 1, &root, sizeof(root), &error));
                TRY(ogpu_batch_submit(batch, &done, &error));
                TRY(ogpu_completion_wait(done, &error));
                double elapsed = 0;
                TRY(ogpu_completion_elapsed_ns(done, &elapsed, &error)); REQUIRE(elapsed >= 0 && elapsed < wrap_ns/2);
                if (round >= WARMUPS) {
                    samples[v][round-WARMUPS] = elapsed;
                    printf("STRUCTURED_SAMPLE count=%u variant=%u round=%u ns=%.0f\n", count, v, round-WARMUPS, elapsed);
                }
                TRY(ogpu_buffer_read(data, 0, actual, bytes, &error)); REQUIRE(!memcmp(source, actual, bytes));
                TRY(ogpu_buffer_read(parameters, 0, unchanged, sizeof(unchanged), &error));
                REQUIRE(!memcmp(blocks, unchanged, sizeof(blocks)));
                ogpu_completion_destroy(done); done = NULL; ogpu_batch_destroy(batch); batch = NULL;
            }
        }
        for (unsigned v = 0; v < 2; ++v) {
            qsort(samples[v], SAMPLES, sizeof(double), compare_ns);
            printf("STRUCTURED_TIMING count=%u variant=%u min_ns=%.0f median_ns=%.0f max_ns=%.0f\n",
                count, v, samples[v][0], samples[v][SAMPLES/2], samples[v][SAMPLES-1]);
        }
    }
    puts("Structured timing PASS: all samples exact; guards and parameter blocks unchanged");
    result = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(done); ogpu_batch_destroy(batch);
    for (unsigned v = 0; v < 2; ++v) ogpu_kernel_destroy(kernels[v]);
    ogpu_buffer_destroy(parameters); ogpu_buffer_destroy(data);
    ogpu_device_destroy(device); ogpu_probe_destroy(probe);
    if (file) fclose(file);
    free(code); free(source); free(actual);
    return result;
}
