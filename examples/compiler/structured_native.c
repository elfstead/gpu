/* Diagnostic only: same artifact/root under direct Vulkan, no OGPU calls/linkage. */
#define main learned_image_native_main
#include "../learned_image/native.c"
#undef main
#include <structured.generated.h>

#define CHECK(c) do { if (!(c)) { fprintf(stderr, "Native structured check: %s\n", #c); goto cleanup; } } while (0)

int main(void) {
    Native n = {0}; NativeProgram program = {0}; NativeBatch batch = {0};
    NativeBuffer data = {0}, parameters = {0};
    enum { COUNT = 65 };
    float input[COUNT+2], output[COUNT+2];
    structured_type_Block blocks[2] = {0};
    int result = 1;
    CHECK(native_create(&n));
    CHECK(native_compute(&n, &program, structured_code, sizeof(structured_code),
        sizeof(StructuredArguments), structured_local));
    CHECK(native_buffer_create(&n, &data, sizeof(input), 1));
    CHECK(native_buffer_create(&n, &parameters, sizeof(blocks), 1));
    input[0] = -8192.0f; input[COUNT+1] = 8192.0f;
    for (unsigned i = 0; i < COUNT; ++i) input[i+1] = (float)((int)i-128);
    // Select the second block as a further check of the native pointer stride.
    blocks[1].arg_data = data.address + sizeof(float); blocks[1].arg_count = COUNT;
    for (unsigned c = 0; c < 2; ++c) {
        blocks[1].arg_coefficients[c].arg_scaleBias[0] = c ? -2.0f : 0.5f;
        blocks[1].arg_coefficients[c].arg_scaleBias[1] = c ? 0.25f : -0.25f;
        blocks[1].arg_coefficients[c].arg_selectors[0] = c;
        blocks[1].arg_coefficients[c].arg_selectors[1] = c+1;
    }
    StructuredArguments args = {.arg_blocks=parameters.address, .arg_chosen=1};
    args.arg_controls.arg_gainBias[0] = 0.5f;
    args.arg_controls.arg_gainBias[1] = -0.25f;
    args.arg_controls.arg_mapping[0] = 0; args.arg_controls.arg_mapping[1] = 1;
    CHECK(native_write(&n, &data, 0, input, sizeof(input)));
    CHECK(native_write(&n, &parameters, 0, blocks, sizeof(blocks)));
    CHECK(native_begin(&n, &batch));
    CHECK(native_dispatch(&n, &batch, &program,
        (Launch){.x=(COUNT+structured_local[0]-1)/structured_local[0],.y=1}, &args, sizeof(args)));
    CHECK(native_submit(&n, &batch)); CHECK(native_wait(&n, &batch));
    CHECK(native_read(&n, &data, 0, output, sizeof(output)));
    CHECK(output[0] == input[0] && output[COUNT+1] == input[COUNT+1]);
    unsigned mismatches = 0;
    for (unsigned i = 0; i < COUNT; ++i) {
        const unsigned c = i%2;
        float expected = input[i+1] * (c ? -2.0f : 0.5f) + (c ? 0.25f : -0.25f)
            + 0.5f*(float)(2*c) - 0.25f;
        if (output[i+1] != expected) {
            if (mismatches < 4) fprintf(stderr, "Native structured mismatch i=%u got=%.9g expected=%.9g\n", i, output[i+1], expected);
            ++mismatches;
        }
    }
    printf("Native structured oracle: %u/%u mismatches; guards intact\n", mismatches, COUNT);
    CHECK(!mismatches);
    puts("Native structured PASS"); result = 0;
cleanup:
    native_batch_destroy(&n, &batch); // Drains accepted work even on failure.
    native_buffer_destroy(&n, &parameters); native_buffer_destroy(&n, &data);
    native_program_destroy(&n, &program); native_destroy(&n);
    return result;
}
