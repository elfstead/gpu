/* Standalone deployment audit, not a second execution backend. */
#include <vulkan/vulkan.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(call) do { VkResult r = (call); if (r != VK_SUCCESS) { \
    fprintf(stderr, "%s: Vulkan %d\n", #call, r); return 1; } } while (0)

static int advertised(const VkExtensionProperties *exts, uint32_t count, const char *name) {
    for (uint32_t i = 0; i < count; ++i)
        if (!strcmp(exts[i].extensionName, name)) return 1;
    return 0;
}

int main(void) {
    const char *path = getenv("OGPU_VULKAN_LIBRARY");
    void *library = dlopen(path ? path : "libvulkan.so.1", RTLD_NOW | RTLD_LOCAL);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); return 1; }
    PFN_vkGetInstanceProcAddr get = (PFN_vkGetInstanceProcAddr)dlsym(library, "vkGetInstanceProcAddr");
    if (!get) return 1;
#define LOAD(name) PFN_##name name = (PFN_##name)get(instance, #name); if (!name) return 1
    VkInstance instance = VK_NULL_HANDLE;
    LOAD(vkCreateInstance);
    VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO, .apiVersion = VK_API_VERSION_1_4};
    VkInstanceCreateInfo create = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app};
    CHECK(vkCreateInstance(&create, NULL, &instance));
    LOAD(vkDestroyInstance);
    LOAD(vkEnumeratePhysicalDevices);
    LOAD(vkGetPhysicalDeviceProperties2);
    LOAD(vkEnumerateDeviceExtensionProperties);
    LOAD(vkGetPhysicalDeviceFeatures2);
    uint32_t count = 0;
    CHECK(vkEnumeratePhysicalDevices(instance, &count, NULL));
    VkPhysicalDevice *devices = calloc(count ? count : 1, sizeof(*devices));
    if (!devices) return 1;
    CHECK(vkEnumeratePhysicalDevices(instance, &count, devices));
    unsigned ready = 0;
    for (uint32_t i = 0; i < count; ++i) {
        VkPhysicalDeviceProperties2 properties = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2};
        vkGetPhysicalDeviceProperties2(devices[i], &properties);
        printf("%s: Vulkan %u.%u.%u\n", properties.properties.deviceName,
               VK_API_VERSION_MAJOR(properties.properties.apiVersion),
               VK_API_VERSION_MINOR(properties.properties.apiVersion),
               VK_API_VERSION_PATCH(properties.properties.apiVersion));
        uint32_t n = 0;
        CHECK(vkEnumerateDeviceExtensionProperties(devices[i], NULL, &n, NULL));
        VkExtensionProperties *extensions = calloc(n ? n : 1, sizeof(*extensions));
        if (!extensions) return 1;
        CHECK(vkEnumerateDeviceExtensionProperties(devices[i], NULL, &n, extensions));
        int core = properties.properties.apiVersion >= VK_API_VERSION_1_4;
#define FEATURE(type, tag, field, extension) \
        type field = {.sType = tag}; \
        int field##_advertised = advertised(extensions, n, extension); \
        if (field##_advertised) { \
            VkPhysicalDeviceFeatures2 query = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2, .pNext = &field}; \
            vkGetPhysicalDeviceFeatures2(devices[i], &query); \
        }
        FEATURE(VkPhysicalDeviceDescriptorHeapFeaturesEXT, VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_FEATURES_EXT, heap, VK_EXT_DESCRIPTOR_HEAP_EXTENSION_NAME);
        FEATURE(VkPhysicalDeviceDeviceAddressCommandsFeaturesKHR, VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_ADDRESS_COMMANDS_FEATURES_KHR, address, VK_KHR_DEVICE_ADDRESS_COMMANDS_EXTENSION_NAME);
        FEATURE(VkPhysicalDeviceShaderUntypedPointersFeaturesKHR, VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_UNTYPED_POINTERS_FEATURES_KHR, untyped, VK_KHR_SHADER_UNTYPED_POINTERS_EXTENSION_NAME);
        FEATURE(VkPhysicalDeviceUnifiedImageLayoutsFeaturesKHR, VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_UNIFIED_IMAGE_LAYOUTS_FEATURES_KHR, image, VK_KHR_UNIFIED_IMAGE_LAYOUTS_EXTENSION_NAME);
#undef FEATURE
        VkPhysicalDeviceVulkan12Features v12 = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES};
        VkPhysicalDeviceVulkan13Features v13 = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES};
        VkPhysicalDeviceVulkan14Features v14 = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_4_FEATURES};
        if (core) {
            v12.pNext = &v13; v13.pNext = &v14;
            VkPhysicalDeviceFeatures2 query = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2, .pNext = &v12};
            vkGetPhysicalDeviceFeatures2(devices[i], &query);
        }
        VkPhysicalDeviceDescriptorHeapPropertiesEXT limits = {.sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_PROPERTIES_EXT};
        if (heap_advertised) {
            properties.pNext = &limits;
            vkGetPhysicalDeviceProperties2(devices[i], &properties);
        }
#define SHOW(name, value) printf("  %-32s %u\n", name, (unsigned)(value))
        SHOW("bufferDeviceAddress", v12.bufferDeviceAddress);
        SHOW("timelineSemaphore", v12.timelineSemaphore);
        SHOW("synchronization2", v13.synchronization2);
        SHOW("dynamicRendering", v13.dynamicRendering);
        SHOW("maintenance5", v14.maintenance5);
        SHOW("descriptorHeap", heap.descriptorHeap);
        SHOW("deviceAddressCommands", address.deviceAddressCommands);
        SHOW("shaderUntypedPointers", untyped.shaderUntypedPointers);
        SHOW("unifiedImageLayouts", image.unifiedImageLayouts);
        SHOW("maxPushDataSize", limits.maxPushDataSize);
#undef SHOW
        int compute = core && v12.bufferDeviceAddress && v12.timelineSemaphore && v13.synchronization2
            && v14.maintenance5 && heap.descriptorHeap && address.deviceAddressCommands && untyped.shaderUntypedPointers;
        printf("  feature baseline: compute=%s graphics=%s (queue/creation/execution not tested)\n",
               compute ? "yes" : "no", compute && v13.dynamicRendering && image.unifiedImageLayouts ? "yes" : "no");
        ready += compute;
        free(extensions);
    }
    printf("%u/%u devices meet compute feature baseline\n", ready, count);
    free(devices);
    vkDestroyInstance(instance, NULL);
    dlclose(library);
    return ready ? 0 : 2;
}
