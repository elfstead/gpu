/* Semantic fields only: no duplicated root/pointee offsets, widths or strides. */
#include <structured.generated.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(c) do { if (!(c)) { fprintf(stderr, "Check failed at %d: %s\n", __LINE__, #c); goto cleanup; } } while (0)
#define TRY(op) do { OgpuResult s = (op); if (s != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: %" PRId32 " %s\n", #op, s, error.message); goto cleanup; } } while (0)

int main(void) {
    int result = EXIT_FAILURE;
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuBuffer *data[2] = {NULL, NULL}, *parameters = NULL;
    OgpuKernel *kernel = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *done = NULL;
    OgpuError error = {0};
    OgpuCapabilities caps = {0};
    OgpuDeviceLimits limits = {0};
    const uint32_t count = 4099;
    const size_t bytes = (count + 2u) * sizeof(float);
    float *expected[2] = {NULL, NULL}, *actual = NULL;
    structured_type_Block blocks[2];
    memset(blocks, 0, sizeof(blocks));
    // Guard offset is application allocation policy, not an argument layout.
    const size_t prefix = 64, parameter_bytes = 128 + sizeof(blocks);
    uint8_t *parameter_input = NULL, *parameter_output = NULL;
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t devices = 0;
    TRY(ogpu_probe_device_count(probe, &devices));
    for (uint32_t i = 0; i < devices; ++i) {
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        TRY(ogpu_device_capabilities(device, &caps, &error));
        TRY(ogpu_device_limits(device, &limits, &error));
        if (structured_compatible(&caps, &limits)) {
            OgpuDeviceInfo info;
            TRY(ogpu_probe_device_info(probe, i, &info));
            printf("Structured device: %s (backend=%u)\n", info.name, info.backend);
            break;
        }
        ogpu_device_destroy(device); device = NULL;
    }
    REQUIRE(device != NULL);
    OgpuDeviceLimits too_small = limits;
    too_small.max_push_data_bytes = sizeof(StructuredArguments) - 1;
    REQUIRE(!structured_compatible(&caps, &too_small));
    OgpuCapabilities missing = caps;
    missing.buffer_device_address = 0;
    REQUIRE(!structured_compatible(&missing, &limits));
    REQUIRE(structured_local[1] == 1 && structured_local[2] == 1);
    const uint32_t groups = count / structured_local[0] + (count % structured_local[0] != 0);
    REQUIRE(groups <= limits.max_dispatch[0]);
    OgpuShaderDesc shader = structured_shader();
    TRY(ogpu_kernel_create(device, &shader, sizeof(StructuredArguments), &kernel, &error));
    actual = malloc(bytes); parameter_input = malloc(parameter_bytes); parameter_output = malloc(parameter_bytes);
    REQUIRE(actual && parameter_input && parameter_output);
    memset(parameter_input, 0xa5, parameter_bytes);
    for (unsigned b = 0; b < 2; ++b) {
        expected[b] = malloc(bytes); REQUIRE(expected[b]);
        expected[b][0] = -8192.0f; expected[b][count + 1] = 8192.0f;
        for (uint32_t i = 0; i < count; ++i) expected[b][i + 1] = (float)((int)(i % 257) - 128 + (int)b);
        TRY(ogpu_buffer_create(device, bytes, OGPU_MEMORY_HOST, &data[b], &error));
        TRY(ogpu_buffer_write(data[b], 0, expected[b], bytes, &error));
        TRY(ogpu_buffer_device_address(data[b], &blocks[b].arg_data, &error));
        blocks[b].arg_data += sizeof(float);
        blocks[b].arg_count = count - 2*b;
        blocks[b].arg_flags = b;
        for (unsigned c = 0; c < 2; ++c) {
            blocks[b].arg_coefficients[c].arg_scaleBias[0] = c ? -2.0f : 0.5f;
            blocks[b].arg_coefficients[c].arg_scaleBias[1] = c ? 0.25f : -0.5f;
            blocks[b].arg_coefficients[c].arg_selectors[0] = b + c;
            blocks[b].arg_coefficients[c].arg_selectors[1] = b + c + 1;
        }
    }
    TRY(ogpu_buffer_create(device, parameter_bytes, OGPU_MEMORY_HOST, &parameters, &error));
    uint64_t address = 0;
    TRY(ogpu_buffer_device_address(parameters, &address, &error));
    REQUIRE((address + prefix) % _Alignof(structured_type_Block) == 0);
    for (unsigned pass = 0; pass < 3; ++pass) {
        const unsigned chosen = pass % 2;
        StructuredArguments args = {.arg_blocks = address + prefix, .arg_chosen = chosen, .arg_epoch = pass};
        args.arg_controls.arg_gainBias[0] = 0.5f;
        args.arg_controls.arg_gainBias[1] = -0.25f;
        args.arg_controls.arg_mapping[0] = pass % 2;
        args.arg_controls.arg_mapping[1] = 1 - pass % 2;
        blocks[chosen].arg_coefficients[0].arg_scaleBias[1] += 0.25f;
        memcpy(parameter_input + prefix, blocks, sizeof(blocks));
        TRY(ogpu_buffer_write(parameters, 0, parameter_input, parameter_bytes, &error));
        TRY(ogpu_batch_create(device, &batch, &error));
        // Retain every reachable allocation explicitly, not only the pointer block.
        TRY(ogpu_batch_retain_buffer(batch, parameters, &error));
        for (unsigned b = 0; b < 2; ++b) TRY(ogpu_batch_retain_buffer(batch, data[b], &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE,
            OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &error));
        TRY(ogpu_batch_dispatch(batch, kernel, groups, 1, 1, &args, sizeof(args), &error));
        // Scalar reference shares semantic inputs, not device results or layout offsets.
        for (uint32_t i = 0; i < blocks[chosen].arg_count; ++i) {
            const unsigned c = args.arg_controls.arg_mapping[i % 2];
            expected[chosen][i + 1] = expected[chosen][i + 1] * blocks[chosen].arg_coefficients[c].arg_scaleBias[0]
                + blocks[chosen].arg_coefficients[c].arg_scaleBias[1]
                + args.arg_controls.arg_gainBias[0] * (float)blocks[chosen].arg_coefficients[c].arg_selectors[i % 2]
                + args.arg_controls.arg_gainBias[1] + (float)(blocks[chosen].arg_flags + pass);
        }
        memset(&args, 0, sizeof(args)); // Root is copied, pointees remain caller-owned.
        TRY(ogpu_batch_submit(batch, &done, &error));
        TRY(ogpu_completion_wait(done, &error));
        for (unsigned b = 0; b < 2; ++b) {
            TRY(ogpu_buffer_read(data[b], 0, actual, bytes, &error));
            for (uint32_t i = 0; i < count + 2; ++i) {
                if (actual[i] != expected[b][i]) fprintf(stderr,
                    "Structured mismatch pass=%u buffer=%u word=%u got=%.9g expected=%.9g\n",
                    pass, b, i, actual[i], expected[b][i]);
                REQUIRE(actual[i] == expected[b][i]);
            }
        }
        TRY(ogpu_buffer_read(parameters, 0, parameter_output, parameter_bytes, &error));
        REQUIRE(memcmp(parameter_input, parameter_output, parameter_bytes) == 0);
        ogpu_completion_destroy(done); done = NULL;
        ogpu_batch_destroy(batch); batch = NULL;
    }
    printf("Structured PASS: two guarded arrays, three A/B/A passes, nested vectors/arrays, pointer block and guards unchanged; root=%zu block=%zu local=%u\n",
        sizeof(StructuredArguments), sizeof(structured_type_Block), structured_local[0]);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_completion_destroy(done); ogpu_batch_destroy(batch); ogpu_kernel_destroy(kernel);
    ogpu_buffer_destroy(parameters);
    for (unsigned b = 0; b < 2; ++b) { ogpu_buffer_destroy(data[b]); free(expected[b]); }
    ogpu_device_destroy(device); ogpu_probe_destroy(probe);
    free(actual); free(parameter_input); free(parameter_output);
    return result;
}
