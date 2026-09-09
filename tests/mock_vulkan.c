/* Hardware-independent loader double: strict query validation, not a Vulkan driver.
 * Built only by cargo xtask mock; never linked into the runtime. */
#include <vulkan/vulkan_core.h>
#include <assert.h>
#include <stdlib.h>
#include <string.h>

static int instance_token, device_token;
static unsigned live_instances;

VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateInstanceVersion(uint32_t *version) {
    const char *mode = getenv("OGPU_MOCK_MODE");
    *version = mode && !strcmp(mode, "instance11") ? VK_API_VERSION_1_1 : VK_API_VERSION_1_3;
    return VK_SUCCESS;
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateInstance(const VkInstanceCreateInfo *info, const VkAllocationCallbacks *allocator, VkInstance *instance) {
    assert(info->sType == VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO && allocator == NULL);
    assert(info->pApplicationInfo->apiVersion == VK_API_VERSION_1_3);
    assert(info->enabledExtensionCount == 0 && info->enabledLayerCount == 0);
    *instance = (VkInstance)&instance_token;
    ++live_instances;
    return VK_SUCCESS;
}
VKAPI_ATTR void VKAPI_CALL vkDestroyInstance(VkInstance instance, const VkAllocationCallbacks *allocator) {
    assert(instance == (VkInstance)&instance_token && allocator == NULL && live_instances == 1);
    --live_instances;
}
/* Confirms cleanup even when enumeration fails after successful creation. */
__attribute__((destructor)) static void verify_cleanup(void) { assert(live_instances == 0); }

VKAPI_ATTR VkResult VKAPI_CALL vkEnumeratePhysicalDevices(VkInstance instance, uint32_t *count, VkPhysicalDevice *devices) {
    assert(instance == (VkInstance)&instance_token);
    const char *mode = getenv("OGPU_MOCK_MODE");
    if (mode && !strcmp(mode, "error")) return VK_ERROR_INITIALIZATION_FAILED;
    if (mode && !strcmp(mode, "empty")) { *count = 0; return VK_SUCCESS; }
    if (devices) { assert(*count >= 1); devices[0] = (VkPhysicalDevice)&device_token; }
    *count = 1;
    return VK_SUCCESS;
}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceProperties(VkPhysicalDevice device, VkPhysicalDeviceProperties *properties) {
    assert(device == (VkPhysicalDevice)&device_token);
    memset(properties, 0, sizeof(*properties));
    properties->apiVersion = VK_API_VERSION_1_3;
    properties->deviceType = VK_PHYSICAL_DEVICE_TYPE_CPU;
    strcpy(properties->deviceName, "OGPU mock device");
}
VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateDeviceExtensionProperties(VkPhysicalDevice device, const char *layer, uint32_t *count, VkExtensionProperties *properties) {
    assert(device == (VkPhysicalDevice)&device_token && layer == NULL);
    if (properties) {
        assert(*count >= 1);
        memset(properties, 0, sizeof(*properties));
        strcpy(properties->extensionName, VK_EXT_DESCRIPTOR_HEAP_EXTENSION_NAME);
    }
    *count = 1;
    return VK_SUCCESS;
}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceFeatures2(VkPhysicalDevice device, VkPhysicalDeviceFeatures2 *root) {
    assert(device == (VkPhysicalDevice)&device_token && root->sType == VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2);
    memset(&root->features, 0, sizeof(root->features));
    if (!root->pNext) return;
    VkBaseOutStructure *p = root->pNext;
    assert(p->pNext == NULL);
    switch (p->sType) {
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_BUFFER_DEVICE_ADDRESS_FEATURES:
        ((VkPhysicalDeviceBufferDeviceAddressFeatures *)p)->bufferDeviceAddress = VK_TRUE; break;
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_FEATURES:
        ((VkPhysicalDeviceTimelineSemaphoreFeatures *)p)->timelineSemaphore = VK_TRUE; break;
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SYNCHRONIZATION_2_FEATURES:
        ((VkPhysicalDeviceSynchronization2Features *)p)->synchronization2 = VK_TRUE; break;
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_FLOAT16_INT8_FEATURES:
        ((VkPhysicalDeviceShaderFloat16Int8Features *)p)->shaderFloat16 = VK_TRUE; break;
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_8BIT_STORAGE_FEATURES:
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES:
    case VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_FEATURES_EXT:
        break; /* intentionally unsupported feature bits */
    default: assert(!"Queried an unadvertised extension");
    }
}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceQueueFamilyProperties(VkPhysicalDevice device, uint32_t *count, VkQueueFamilyProperties *properties) {
    assert(device == (VkPhysicalDevice)&device_token);
    if (properties) {
        assert(*count >= 1);
        memset(properties, 0, sizeof(*properties));
        properties->queueCount = 1;
        properties->queueFlags = VK_QUEUE_GRAPHICS_BIT | VK_QUEUE_COMPUTE_BIT;
    }
    *count = 1;
}
VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance instance, const char *name) {
    (void)instance;
#define COMMAND(command) if (!strcmp(name, #command)) return (PFN_vkVoidFunction)command
    COMMAND(vkEnumerateInstanceVersion);
    COMMAND(vkCreateInstance);
    COMMAND(vkDestroyInstance);
    COMMAND(vkEnumeratePhysicalDevices);
    COMMAND(vkGetPhysicalDeviceProperties);
    COMMAND(vkEnumerateDeviceExtensionProperties);
    COMMAND(vkGetPhysicalDeviceFeatures2);
    COMMAND(vkGetPhysicalDeviceQueueFamilyProperties);
#undef COMMAND
    return NULL;
}
