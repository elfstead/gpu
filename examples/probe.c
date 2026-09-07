#include "ogpu.h"
#include <assert.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

static void print_capabilities(const OgpuCapabilities *c) {
#define SHOW(field) do { assert(c->field <= 1); printf("  %-38s %s\n", #field, c->field ? "yes" : "no"); } while (0)
    SHOW(graphics_queue);
    SHOW(compute_queue);
    SHOW(buffer_device_address);
    SHOW(timeline_semaphore);
    SHOW(synchronization2);
    SHOW(descriptor_heap);
    SHOW(device_address_commands);
    SHOW(shader_untyped_pointers);
    SHOW(cooperative_matrix);
    SHOW(storage_buffer_8bit_access);
    SHOW(storage_buffer_16bit_access);
    SHOW(shader_float16);
    SHOW(shader_int8);
    SHOW(shader_int16);
    SHOW(shader_int64);
    SHOW(shader_float64);
    SHOW(shader_bfloat16);
    SHOW(shader_bfloat16_cooperative_matrix);
    SHOW(shader_float8);
    SHOW(shader_float8_cooperative_matrix);
    SHOW(shader_float4);
    SHOW(shader_float6);
    SHOW(shader_float8_unsigned_e8m0);
    SHOW(shader_mx_int8);
#undef SHOW
}

int main(int argc, char **argv) {
    const char *mode = argc == 2 ? argv[1] : "";
    if (argc > 2 || (argc == 2 && strcmp(mode, "--expect-loader-error") &&
        strcmp(mode, "--expect-mock") && strcmp(mode, "--expect-empty") &&
        strcmp(mode, "--expect-vulkan-error"))) {
        fprintf(stderr, "Unknown probe test mode\n");
        return 2;
    }
    OgpuProbe *probe = NULL;
    OgpuError error = {0};
    error.vulkan_result = 42;
    assert(ogpu_probe_create(OGPU_ABI_VERSION + 1, &probe, &error) == OGPU_ERROR_ABI_MISMATCH);
    assert(probe == NULL && error.vulkan_result == 42);
    assert(ogpu_probe_create(OGPU_ABI_VERSION, NULL, NULL) == OGPU_ERROR_INVALID_ARGUMENT);
    uint32_t count = 42;
    assert(ogpu_probe_device_count(NULL, &count) == OGPU_ERROR_INVALID_ARGUMENT && count == 42);
    assert(ogpu_probe_device_info(NULL, 0, NULL) == OGPU_ERROR_INVALID_ARGUMENT);
    ogpu_probe_destroy(NULL);

    OgpuResult result = ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error);
    if (!strcmp(mode, "--expect-loader-error") || !strcmp(mode, "--expect-vulkan-error")) {
        OgpuResult expected = !strcmp(mode, "--expect-loader-error") ? OGPU_ERROR_LOADER : OGPU_ERROR_VULKAN;
        assert(result == expected && probe == NULL);
        assert(error.message[0] != 0 && error.message[255] == 0);
        if (expected == OGPU_ERROR_VULKAN) assert(error.vulkan_result == -3); /* mock initialization failure */
        printf("Expected failure handled: %s\n", error.message);
        return 0;
    }
    if (result != OGPU_SUCCESS) {
        fprintf(stderr, "Probe failed (%" PRId32 ", Vulkan %" PRId32 "): %s\n", result, error.vulkan_result, error.message);
        return 1;
    }
    assert(probe != NULL && error.vulkan_result == 0 && error.message[0] == 0);
    assert(ogpu_probe_device_count(probe, NULL) == OGPU_ERROR_INVALID_ARGUMENT);
    assert(ogpu_probe_device_count(probe, &count) == OGPU_SUCCESS);
    printf("Devices: %" PRIu32 "\n", count);
    if (!strcmp(mode, "--expect-empty")) assert(count == 0);
    if (!strcmp(mode, "--expect-mock")) assert(count == 1);
    for (uint32_t i = 0; i < count; ++i) {
        OgpuDeviceInfo info;
        assert(ogpu_probe_device_info(probe, i, &info) == OGPU_SUCCESS);
        assert(info.name[255] == 0);
        printf("%s (vendor %04" PRIx32 ", device %04" PRIx32 ", Vulkan %" PRIu32 ".%" PRIu32 ".%" PRIu32 ")\n",
               info.name, info.vendor_id, info.device_id, info.vulkan_api_major, info.vulkan_api_minor, info.vulkan_api_patch);
        print_capabilities(&info.capabilities);
        if (!strcmp(mode, "--expect-mock")) {
            assert(!strcmp(info.name, "OGPU mock device"));
            assert(info.capabilities.graphics_queue == 1 && info.capabilities.compute_queue == 1);
            assert(info.capabilities.buffer_device_address == 1);
            assert(info.capabilities.timeline_semaphore == 1 && info.capabilities.synchronization2 == 1);
            assert(info.capabilities.descriptor_heap == 0); /* advertised extension, feature is false */
            assert(info.capabilities.cooperative_matrix == 0); /* extension absent */
            assert(info.capabilities.shader_float16 == 1 && info.capabilities.storage_buffer_8bit_access == 0);
        }
    }
    OgpuDeviceInfo untouched;
    unsigned char before[sizeof(untouched)];
    memset(&untouched, 0xa5, sizeof(untouched));
    memcpy(before, &untouched, sizeof(untouched));
    assert(ogpu_probe_device_info(probe, count, &untouched) == OGPU_ERROR_OUT_OF_RANGE);
    assert(!memcmp(before, &untouched, sizeof(untouched)));
    ogpu_probe_destroy(probe);
    puts("C API smoke test passed.");
    return 0;
}
