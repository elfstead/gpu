/* Diagnostic only. Query native arithmetic support; no OGPU calls or linkage. */
#define main learned_image_native_main
#include "../../learned_image/native.c"
#undef main

#define CHECK(c) do { if (!(c)) { fprintf(stderr, "Subgroup check line %d: %s\n", __LINE__, #c); goto cleanup; } } while (0)

typedef struct Root {
    uint64_t input_data, output_data;
    uint32_t count, reserved;
} Root;
_Static_assert(sizeof(Root) == 24 && _Alignof(Root) == 8 && offsetof(Root, input_data) == 0
    && offsetof(Root, output_data) == 8 && offsetof(Root, count) == 16, "probe root ABI");

int main(int argc, char **argv) {
    Native n = {0}; NativeProgram program = {0}; NativeBatch batch = {0};
    NativeBuffer input = {0}, output = {0};
    FILE *file = NULL; uint32_t *code = NULL;
    enum { MAX_LAUNCHED = 192, OUTPUT_WORDS = MAX_LAUNCHED*4+2 };
    const uint32_t poison = 0xbaadf00du, local[3] = {64, 1, 1};
    const uint32_t counts[] = {1, 63, 64, 65, 137, 1};
    uint32_t source[MAX_LAUNCHED+2], unchanged[MAX_LAUNCHED+2], observed[OUTPUT_WORDS];
    int result = 1;
    CHECK(argc == 2);
    file = fopen(argv[1], "rb"); CHECK(file);
    CHECK(fseek(file, 0, SEEK_END) == 0);
    long bytes = ftell(file); CHECK(bytes >= 20 && bytes <= 1024*1024 && !(bytes % 4));
    CHECK(fseek(file, 0, SEEK_SET) == 0);
    code = malloc((size_t)bytes); CHECK(code);
    CHECK(fread(code, 1, (size_t)bytes, file) == (size_t)bytes);
    fclose(file); file = NULL;
    CHECK(native_create(&n));
    VkPhysicalDeviceSubgroupProperties subgroup = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SUBGROUP_PROPERTIES};
    VkPhysicalDeviceProperties2 properties = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2, .pNext=&subgroup};
    n.vkGetPhysicalDeviceProperties2(n.physical, &properties);
    printf("Native subgroup: %s; size=%u stages=0x%x operations=0x%x\n", properties.properties.deviceName,
        subgroup.subgroupSize, subgroup.supportedStages, subgroup.supportedOperations);
    CHECK(subgroup.subgroupSize > 0 && subgroup.subgroupSize <= 128);
    CHECK(subgroup.supportedStages & VK_SHADER_STAGE_COMPUTE_BIT);
    CHECK((subgroup.supportedOperations & (VK_SUBGROUP_FEATURE_BASIC_BIT | VK_SUBGROUP_FEATURE_ARITHMETIC_BIT))
        == (VK_SUBGROUP_FEATURE_BASIC_BIT | VK_SUBGROUP_FEATURE_ARITHMETIC_BIT));
    CHECK(native_compute(&n, &program, code, (size_t)bytes, sizeof(Root), local));
    CHECK(native_buffer_create(&n, &input, sizeof(source), 1));
    CHECK(native_buffer_create(&n, &output, sizeof(observed), 1));
    for (unsigned pass = 0; pass < sizeof(counts)/sizeof(counts[0]); ++pass) {
        uint32_t count = counts[pass], launched = ((count+63)/64)*64;
        for (unsigned i = 0; i < MAX_LAUNCHED+2; ++i) source[i] = poison;
        for (unsigned i = 0; i < count; ++i) source[i+1] = (i*1664525u+1013904223u) ^ (i>>5);
        for (unsigned i = 0; i < OUTPUT_WORDS; ++i) observed[i] = poison;
        CHECK(native_write(&n, &input, 0, source, sizeof(source)));
        CHECK(native_write(&n, &output, 0, observed, sizeof(observed)));
        Root root = {.input_data=input.address+4, .output_data=output.address+4, .count=count};
        CHECK(native_begin(&n, &batch));
        CHECK(native_dispatch(&n, &batch, &program, (Launch){.x=launched/64, .y=1}, &root, sizeof(root)));
        CHECK(native_submit(&n, &batch)); CHECK(native_wait(&n, &batch));
        CHECK(native_read(&n, &input, 0, unchanged, sizeof(unchanged)));
        CHECK(!memcmp(source, unchanged, sizeof(source)));
        CHECK(native_read(&n, &output, 0, observed, sizeof(observed)));
        CHECK(observed[0] == poison);
        for (unsigned i = launched*4+1; i < OUTPUT_WORDS; ++i) CHECK(observed[i] == poison);
        /* Group by the reported minimum invocation ID, NOT contiguous lanes.
         * Recompute every group's sum from independent input/member indices. */
        for (unsigned i = 0; i < launched; ++i) {
            const uint32_t *record = &observed[1+4*i];
            uint32_t leader = record[1], members = 0, sum = 0, minimum = UINT32_MAX;
            unsigned char lanes[128] = {0};
            CHECK(leader < launched && record[3] < subgroup.subgroupSize);
            for (unsigned j = 0; j < launched; ++j) {
                const uint32_t *peer = &observed[1+4*j];
                if (peer[1] != leader) continue;
                CHECK(j/64 == i/64); /* A subgroup is contained in a workgroup. */
                CHECK(peer[3] < subgroup.subgroupSize && !lanes[peer[3]]);
                lanes[peer[3]] = 1; ++members;
                if (j < minimum) minimum = j;
                if (j < count) sum += source[j+1];
            }
            CHECK(leader == minimum && record[2] == members && record[0] == sum);
        }
        native_batch_destroy(&n, &batch);
        printf("Subgroup count=%u launched=%u: exact sums/membership/lanes, unchanged input and guards PASS\n", count, launched);
    }
    result = 0;
cleanup:
    if (file) fclose(file);
    native_batch_destroy(&n, &batch);
    native_buffer_destroy(&n, &output); native_buffer_destroy(&n, &input);
    native_program_destroy(&n, &program); native_destroy(&n); free(code);
    return result;
}
