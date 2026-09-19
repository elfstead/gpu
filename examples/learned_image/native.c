/* Benchmark-only direct Vulkan control. No OGPU runtime calls or linkage.
 * Generated compute/raster validation plus address-copy lifecycle smoke test.
 * Serialized timing uses the same one-shot pool/query policy as OGPU. */
#define _POSIX_C_SOURCE 200809L
#define VK_NO_PROTOTYPES
#include <vulkan/vulkan.h>
#include <dlfcn.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define NATIVE_FUNCTIONS(X) \
    X(vkEnumeratePhysicalDevices) X(vkEnumerateDeviceExtensionProperties) \
    X(vkGetPhysicalDeviceFeatures2) X(vkGetPhysicalDeviceProperties2) \
    X(vkGetPhysicalDeviceMemoryProperties) X(vkGetPhysicalDeviceQueueFamilyProperties) \
    X(vkCreateDevice) X(vkDestroyDevice) X(vkGetDeviceQueue) \
    X(vkCreateSemaphore) X(vkDestroySemaphore) X(vkWaitSemaphores) X(vkQueueWaitIdle) \
    X(vkCreateBuffer) X(vkDestroyBuffer) X(vkGetBufferMemoryRequirements) \
    X(vkAllocateMemory) X(vkFreeMemory) X(vkBindBufferMemory) \
    X(vkMapMemory) X(vkUnmapMemory) X(vkFlushMappedMemoryRanges) \
    X(vkInvalidateMappedMemoryRanges) X(vkGetBufferDeviceAddress) \
    X(vkCreateCommandPool) X(vkDestroyCommandPool) X(vkAllocateCommandBuffers) \
    X(vkBeginCommandBuffer) X(vkEndCommandBuffer) X(vkCmdPipelineBarrier2) \
    X(vkCmdCopyMemoryKHR) X(vkQueueSubmit2) \
    X(vkCreateShaderModule) X(vkDestroyShaderModule) X(vkCreateComputePipelines) \
    X(vkCreateGraphicsPipelines) X(vkDestroyPipeline) X(vkCmdBindPipeline) \
    X(vkCmdPushDataEXT) X(vkCmdDispatch) X(vkGetPhysicalDeviceImageFormatProperties) \
    X(vkCreateImage) X(vkDestroyImage) X(vkGetImageMemoryRequirements) \
    X(vkBindImageMemory) X(vkCreateImageView) X(vkDestroyImageView) \
    X(vkCmdBeginRendering) X(vkCmdEndRendering) X(vkCmdSetViewport) X(vkCmdSetScissor) \
    X(vkCmdDrawIndirect2KHR) X(vkCmdCopyImageToMemoryKHR) \
    X(vkCreateQueryPool) X(vkDestroyQueryPool) X(vkCmdResetQueryPool) \
    X(vkCmdWriteTimestamp2) X(vkGetQueryPoolResults)

typedef struct {
    void *library;
    VkInstance instance;
    PFN_vkDestroyInstance destroy_instance;
    VkPhysicalDevice physical;
    VkDevice device;
    VkQueue queue;
    VkSemaphore timeline;
    uint64_t next_value, observed_value, max_difference;
    int in_flight;
    uint32_t family, timestamp_bits;
    VkPhysicalDeviceMemoryProperties memory;
    VkPhysicalDeviceProperties properties;
    VkDeviceSize max_push_data;
#define DECLARE(name) PFN_##name name;
    NATIVE_FUNCTIONS(DECLARE)
#undef DECLARE
} Native;

typedef struct {
    VkBuffer buffer;
    VkDeviceMemory memory;
    VkDeviceAddress address;
    VkDeviceSize size, allocated;
    uint32_t type;
    void *mapped;
    int coherent;
} NativeBuffer;

typedef struct {
    VkCommandPool pool;
    VkCommandBuffer command;
    VkQueryPool queries;
    uint64_t value;
    int pending, succeeded;
} NativeBatch;

static int vk_ok(VkResult result, const char *operation) {
    if (result == VK_SUCCESS) return 1;
    fprintf(stderr, "Native %s failed: VkResult %d\n", operation, result);
    return 0;
}
#define VK_TRY(call) do { if (!vk_ok((call), #call)) return 0; } while (0)
#define NEED(condition) do { if (!(condition)) { \
    fprintf(stderr, "Native requirement failed: %s\n", #condition); return 0; } } while (0)

/* Match compute.rs: eligible buffers, lexicographic placement score, last tie.
 * Image backing will use its separate policy when the raster slice is added. */
static int native_memory_type(const VkPhysicalDeviceMemoryProperties *memory,
                              uint32_t mask, int host, uint32_t *out) {
    if (memory->memoryTypeCount > VK_MAX_MEMORY_TYPES) return 0;
    int best = -1;
    for (uint32_t i = 0; i < memory->memoryTypeCount; ++i) {
        VkMemoryPropertyFlags f = memory->memoryTypes[i].propertyFlags;
        VkMemoryPropertyFlags required = host ? VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT : VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
        if (!(mask & (1u << i)) || !(f & required)
            || (f & (VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD | VK_MEMORY_PROPERTY_PROTECTED_BIT))) continue;
        int local = !!(f & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
        int visible = !!(f & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT);
        int score = host ? 4 * !!(f & VK_MEMORY_PROPERTY_HOST_COHERENT_BIT)
            + 2 * !!(f & VK_MEMORY_PROPERTY_HOST_CACHED_BIT) + !local : !visible;
        if (score >= best) { best = score; *out = i; }
    }
    return best >= 0;
}

static int native_range(VkDeviceSize size, VkDeviceSize offset, VkDeviceSize length) {
    return length && offset <= size && length <= size - offset;
}

static int extension(const VkExtensionProperties *extensions, uint32_t count, const char *name) {
    for (uint32_t i = 0; i < count; ++i)
        if (!strcmp(extensions[i].extensionName, name)) return 1;
    return 0;
}

static void native_destroy(Native *n) {
    /* The owner must drain all submissions and destroy children first. */
    if (n->timeline) n->vkDestroySemaphore(n->device, n->timeline, NULL);
    if (n->device) n->vkDestroyDevice(n->device, NULL);
    if (n->instance) n->destroy_instance(n->instance, NULL);
    if (n->library) dlclose(n->library);
    memset(n, 0, sizeof(*n));
}

static int native_create(Native *n) {
    const char *path = getenv("OGPU_VULKAN_LIBRARY");
    n->library = dlopen(path ? path : "libvulkan.so.1", RTLD_NOW | RTLD_LOCAL);
    if (!n->library) { fprintf(stderr, "Native loader: %s\n", dlerror()); return 0; }
    PFN_vkGetInstanceProcAddr get = (PFN_vkGetInstanceProcAddr)dlsym(n->library, "vkGetInstanceProcAddr");
    n->destroy_instance = (PFN_vkDestroyInstance)dlsym(n->library, "vkDestroyInstance");
    NEED(get && n->destroy_instance);
    PFN_vkEnumerateInstanceVersion version = (PFN_vkEnumerateInstanceVersion)get(NULL, "vkEnumerateInstanceVersion");
    PFN_vkCreateInstance create = (PFN_vkCreateInstance)get(NULL, "vkCreateInstance");
    NEED(version && create);
    uint32_t api = 0;
    VK_TRY(version(&api));
    NEED(api >= VK_API_VERSION_1_4);
    VkApplicationInfo app = {.sType=VK_STRUCTURE_TYPE_APPLICATION_INFO,
        .pApplicationName="learned-image-native", .apiVersion=VK_API_VERSION_1_4};
    VkInstanceCreateInfo instance = {.sType=VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo=&app};
    VK_TRY(create(&instance, NULL, &n->instance));
#define LOAD(name) n->name = (PFN_##name)get(n->instance, #name); NEED(n->name);
    NATIVE_FUNCTIONS(LOAD)
#undef LOAD
    /* A deliberately single-device benchmark: select one ICD explicitly. Refuse
     * ambiguity instead of silently comparing different physical devices. */
    VkPhysicalDevice devices[2];
    uint32_t count = 2;
    VK_TRY(n->vkEnumeratePhysicalDevices(n->instance, &count, devices));
    NEED(count == 1);
    n->physical = devices[0];
    VkExtensionProperties extensions[1024];
    count = 1024;
    VK_TRY(n->vkEnumerateDeviceExtensionProperties(n->physical, NULL, &count, extensions));
    const char *names[] = {VK_EXT_DESCRIPTOR_HEAP_EXTENSION_NAME,
        VK_KHR_DEVICE_ADDRESS_COMMANDS_EXTENSION_NAME, VK_KHR_SHADER_UNTYPED_POINTERS_EXTENSION_NAME,
        VK_KHR_UNIFIED_IMAGE_LAYOUTS_EXTENSION_NAME};
    for (unsigned i = 0; i < 3; ++i) {
        if (!extension(extensions, count, names[i])) {
            fprintf(stderr, "Native missing extension: %s\n", names[i]); return 0;
        }
    }
    int has_unified = extension(extensions, count, names[3]);
    VkPhysicalDeviceUnifiedImageLayoutsFeaturesKHR unified = {
        .sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_UNIFIED_IMAGE_LAYOUTS_FEATURES_KHR};
    VkPhysicalDeviceShaderUntypedPointersFeaturesKHR untyped = {
        .sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_UNTYPED_POINTERS_FEATURES_KHR,
        .pNext=has_unified ? &unified : NULL};
    VkPhysicalDeviceDeviceAddressCommandsFeaturesKHR addresses = {
        .sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_ADDRESS_COMMANDS_FEATURES_KHR, .pNext=&untyped};
    VkPhysicalDeviceDescriptorHeapFeaturesEXT heap = {
        .sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_FEATURES_EXT, .pNext=&addresses};
    VkPhysicalDeviceVulkan14Features v14 = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_4_FEATURES, .pNext=&heap};
    VkPhysicalDeviceVulkan13Features v13 = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES, .pNext=&v14};
    VkPhysicalDeviceVulkan12Features v12 = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES, .pNext=&v13};
    VkPhysicalDevice16BitStorageFeatures storage = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES, .pNext=&v12};
    VkPhysicalDeviceFeatures2 features = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2, .pNext=&storage};
    n->vkGetPhysicalDeviceFeatures2(n->physical, &features);
    NEED(v12.bufferDeviceAddress && v12.timelineSemaphore && v13.synchronization2
         && v13.dynamicRendering && v14.maintenance5 && storage.storageBuffer16BitAccess
         && heap.descriptorHeap && addresses.deviceAddressCommands && untyped.shaderUntypedPointers);
    int float16 = v12.shaderFloat16;
    has_unified = has_unified && unified.unifiedImageLayouts;
    /* Enable only the OGPU profile, never the entire query result. */
    unified.unifiedImageLayoutsVideo = VK_FALSE;
    untyped.pNext = has_unified ? &unified : NULL;
    heap = (VkPhysicalDeviceDescriptorHeapFeaturesEXT){.sType=heap.sType, .pNext=&addresses, .descriptorHeap=VK_TRUE};
    v14 = (VkPhysicalDeviceVulkan14Features){.sType=v14.sType, .pNext=&heap, .maintenance5=VK_TRUE};
    v13 = (VkPhysicalDeviceVulkan13Features){.sType=v13.sType, .pNext=&v14, .synchronization2=VK_TRUE, .dynamicRendering=VK_TRUE};
    v12 = (VkPhysicalDeviceVulkan12Features){.sType=v12.sType, .pNext=&v13, .bufferDeviceAddress=VK_TRUE,
        .timelineSemaphore=VK_TRUE, .shaderFloat16=float16};
    storage = (VkPhysicalDevice16BitStorageFeatures){.sType=storage.sType, .pNext=&v12, .storageBuffer16BitAccess=VK_TRUE};
    VkPhysicalDeviceDriverProperties driver = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DRIVER_PROPERTIES};
    VkPhysicalDeviceTimelineSemaphoreProperties timeline = {
        .sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_PROPERTIES, .pNext=&driver};
    VkPhysicalDeviceDescriptorHeapPropertiesEXT heap_properties = {
        .sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_PROPERTIES_EXT, .pNext=&timeline};
    VkPhysicalDeviceProperties2 properties = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2, .pNext=&heap_properties};
    n->vkGetPhysicalDeviceProperties2(n->physical, &properties);
    n->properties = properties.properties;
    NEED(n->properties.apiVersion >= VK_API_VERSION_1_4);
    n->max_push_data = heap_properties.maxPushDataSize;
    n->max_difference = timeline.maxTimelineSemaphoreValueDifference;
    NEED(n->max_difference > 0);
    n->vkGetPhysicalDeviceMemoryProperties(n->physical, &n->memory);
    VkQueueFamilyProperties families[128];
    count = 128;
    n->vkGetPhysicalDeviceQueueFamilyProperties(n->physical, &count, families);
    /* Query the count separately so a truncated list cannot silently select. */
    uint32_t total = 0;
    n->vkGetPhysicalDeviceQueueFamilyProperties(n->physical, &total, NULL);
    NEED(total <= 128 && count == total);
    n->family = UINT32_MAX;
    for (uint32_t i = 0; i < count; ++i) {
        VkQueueFlags required = VK_QUEUE_GRAPHICS_BIT | VK_QUEUE_COMPUTE_BIT;
        if (families[i].queueCount && (families[i].queueFlags & required) == required) { n->family = i; break; }
    }
    NEED(n->family != UINT32_MAX);
    n->timestamp_bits = families[n->family].timestampValidBits;
    float priority = 1.0f;
    VkDeviceQueueCreateInfo queue = {.sType=VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
        .queueFamilyIndex=n->family, .queueCount=1, .pQueuePriorities=&priority};
    VkDeviceCreateInfo device = {.sType=VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO, .pNext=&storage,
        .queueCreateInfoCount=1, .pQueueCreateInfos=&queue, .enabledExtensionCount=has_unified ? 4 : 3,
        .ppEnabledExtensionNames=names};
    VK_TRY(n->vkCreateDevice(n->physical, &device, NULL, &n->device));
    n->vkGetDeviceQueue(n->device, n->family, 0, &n->queue);
    VkSemaphoreTypeCreateInfo type = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO, .semaphoreType=VK_SEMAPHORE_TYPE_TIMELINE};
    VkSemaphoreCreateInfo semaphore = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO, .pNext=&type};
    VK_TRY(n->vkCreateSemaphore(n->device, &semaphore, NULL, &n->timeline));
    printf("Native device: %s; vendor=%u device=%u api=%u driver=%s %s queue=%u float16=%d unified=%d\n",
        n->properties.deviceName, n->properties.vendorID, n->properties.deviceID, n->properties.apiVersion,
        driver.driverName, driver.driverInfo, n->family, float16, has_unified);
    printf("DEVICE {\"vendor\":%u,\"device\":%u,\"api\":[%u,%u,%u]}\n",
        n->properties.vendorID, n->properties.deviceID, VK_API_VERSION_MAJOR(n->properties.apiVersion),
        VK_API_VERSION_MINOR(n->properties.apiVersion), VK_API_VERSION_PATCH(n->properties.apiVersion));
    return 1;
}

static void native_buffer_destroy(Native *n, NativeBuffer *b) {
    if (b->mapped) n->vkUnmapMemory(n->device, b->memory);
    if (b->buffer) n->vkDestroyBuffer(n->device, b->buffer, NULL);
    if (b->memory) n->vkFreeMemory(n->device, b->memory, NULL);
    memset(b, 0, sizeof(*b));
}

/* Caller owns partial construction immediately, and always destroys it. */
static int native_buffer_create(Native *n, NativeBuffer *b, VkDeviceSize size, int host) {
    NEED(size && size <= PTRDIFF_MAX && !b->buffer && !b->memory);
    b->size = size;
    VkBufferCreateInfo create = {.sType=VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size=size,
        .usage=VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT | VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT
             | VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT,
        .sharingMode=VK_SHARING_MODE_EXCLUSIVE};
    VK_TRY(n->vkCreateBuffer(n->device, &create, NULL, &b->buffer));
    VkMemoryRequirements requirements;
    n->vkGetBufferMemoryRequirements(n->device, b->buffer, &requirements);
    NEED(native_memory_type(&n->memory, requirements.memoryTypeBits, host, &b->type));
    b->allocated = requirements.size;
    b->coherent = !!(n->memory.memoryTypes[b->type].propertyFlags & VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
    VkMemoryDedicatedAllocateInfo dedicated = {.sType=VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO, .buffer=b->buffer};
    VkMemoryAllocateFlagsInfo flags = {.sType=VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
        .pNext=&dedicated, .flags=VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT};
    VkMemoryAllocateInfo allocate = {.sType=VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
        .pNext=&flags, .allocationSize=requirements.size, .memoryTypeIndex=b->type};
    VK_TRY(n->vkAllocateMemory(n->device, &allocate, NULL, &b->memory));
    VK_TRY(n->vkBindBufferMemory(n->device, b->buffer, b->memory, 0));
    if (host) VK_TRY(n->vkMapMemory(n->device, b->memory, 0, VK_WHOLE_SIZE, 0, &b->mapped));
    VkBufferDeviceAddressInfo info = {.sType=VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO, .buffer=b->buffer};
    b->address = n->vkGetBufferDeviceAddress(n->device, &info);
    NEED(b->address);
    printf("NATIVE_BUFFER {\"host\":%s,\"requested\":%" PRIu64 ",\"allocated\":%" PRIu64
           ",\"type\":%u,\"flags\":%u}\n", host ? "true" : "false", size, b->allocated,
           b->type, n->memory.memoryTypes[b->type].propertyFlags);
    return 1;
}

static int native_cache(Native *n, NativeBuffer *b, int flush) {
    NEED(b->mapped);
    if (b->coherent) return 1;
    VkMappedMemoryRange range = {.sType=VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
        .memory=b->memory, .offset=0, .size=VK_WHOLE_SIZE};
    return flush ? vk_ok(n->vkFlushMappedMemoryRanges(n->device, 1, &range), "flush")
                 : vk_ok(n->vkInvalidateMappedMemoryRanges(n->device, 1, &range), "invalidate");
}

static int native_write(Native *n, NativeBuffer *b, VkDeviceSize offset, const void *data, size_t size) {
    NEED(b->mapped && native_range(b->size, offset, size));
    memcpy((char *)b->mapped + offset, data, size);
    return native_cache(n, b, 1);
}

static int native_read(Native *n, NativeBuffer *b, VkDeviceSize offset, void *data, size_t size) {
    NEED(b->mapped && native_range(b->size, offset, size));
    if (!native_cache(n, b, 0)) return 0;
    memcpy(data, (char *)b->mapped + offset, size);
    return 1;
}

static void native_barrier(Native *n, VkCommandBuffer command, VkPipelineStageFlags2 source,
                           VkAccessFlags2 source_access, VkPipelineStageFlags2 dest, VkAccessFlags2 dest_access) {
    VkMemoryBarrier2 barrier = {.sType=VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
        .srcStageMask=source, .srcAccessMask=source_access, .dstStageMask=dest, .dstAccessMask=dest_access};
    VkDependencyInfo dependency = {.sType=VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
        .memoryBarrierCount=1, .pMemoryBarriers=&barrier};
    n->vkCmdPipelineBarrier2(command, &dependency);
}

static int native_timing_supported(Native *n) {
    return n->timestamp_bits >= 36 && n->timestamp_bits <= 64
        && isfinite(n->properties.limits.timestampPeriod) && n->properties.limits.timestampPeriod > 0;
}

static double native_timestamp_delta(uint64_t first, uint64_t last, uint32_t bits, double period) {
    return (double)((last - first) & (UINT64_MAX >> (64 - bits))) * period;
}

static int native_begin_timed(Native *n, NativeBatch *b, int timed) {
    NEED(!b->pool && !b->pending && !b->queries && !b->value);
    if (timed) {
        NEED(native_timing_supported(n));
        VkQueryPoolCreateInfo query = {.sType=VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,
            .queryType=VK_QUERY_TYPE_TIMESTAMP, .queryCount=2};
        VK_TRY(n->vkCreateQueryPool(n->device, &query, NULL, &b->queries));
    }
    VkCommandPoolCreateInfo pool = {.sType=VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex=n->family};
    VK_TRY(n->vkCreateCommandPool(n->device, &pool, NULL, &b->pool));
    VkCommandBufferAllocateInfo allocate = {.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        .commandPool=b->pool, .level=VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount=1};
    VK_TRY(n->vkAllocateCommandBuffers(n->device, &allocate, &b->command));
    VkCommandBufferBeginInfo begin = {.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
    VK_TRY(n->vkBeginCommandBuffer(b->command, &begin));
    if (timed) {
        n->vkCmdResetQueryPool(b->command, b->queries, 0, 2);
        n->vkCmdWriteTimestamp2(b->command, VK_PIPELINE_STAGE_2_TOP_OF_PIPE_BIT, b->queries, 0);
    }
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_WRITE_BIT,
        VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT);
    return 1;
}

static int native_begin(Native *n, NativeBatch *b) { return native_begin_timed(n, b, 0); }

static int native_copy(Native *n, NativeBatch *b, NativeBuffer *src, VkDeviceSize src_offset,
                       NativeBuffer *dst, VkDeviceSize dst_offset, VkDeviceSize size) {
    NEED(b->command && !b->pending && !b->value && native_range(src->size, src_offset, size)
         && native_range(dst->size, dst_offset, size));
    NEED(src_offset <= UINT64_MAX - src->address && dst_offset <= UINT64_MAX - dst->address);
    VkDeviceMemoryCopyKHR region = {.sType=VK_STRUCTURE_TYPE_DEVICE_MEMORY_COPY_KHR,
        .srcRange={src->address + src_offset, size}, .dstRange={dst->address + dst_offset, size},
        .srcFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR, .dstFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR};
    VkCopyDeviceMemoryInfoKHR copy = {.sType=VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_INFO_KHR,
        .regionCount=1, .pRegions=&region};
    n->vkCmdCopyMemoryKHR(b->command, &copy);
    return 1;
}

/* No successful path uses queue idle. Failure cleanup must not release GPU-owned
 * resources after a transient wait error, or wait on a rejected timeline value. */
static void native_drain(Native *n) {
    VkResult r;
    for (;;) {
        r = n->vkQueueWaitIdle(n->queue);
        if (r == VK_SUCCESS || r == VK_ERROR_DEVICE_LOST) return;
        sched_yield();
    }
}

static int native_submit(Native *n, NativeBatch *b) {
    NEED(b->command && !b->pending && !b->value && !n->in_flight && n->next_value < UINT64_MAX
         && n->next_value - n->observed_value < n->max_difference);
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        VK_ACCESS_2_MEMORY_WRITE_BIT, VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_READ_BIT);
    if (b->queries) n->vkCmdWriteTimestamp2(b->command, VK_PIPELINE_STAGE_2_BOTTOM_OF_PIPE_BIT, b->queries, 1);
    VK_TRY(n->vkEndCommandBuffer(b->command));
    b->value = ++n->next_value;
    VkCommandBufferSubmitInfo command = {.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO, .commandBuffer=b->command};
    VkSemaphoreSubmitInfo signal = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
        .semaphore=n->timeline, .value=b->value, .stageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT};
    VkSubmitInfo2 submit = {.sType=VK_STRUCTURE_TYPE_SUBMIT_INFO_2, .commandBufferInfoCount=1,
        .pCommandBufferInfos=&command, .signalSemaphoreInfoCount=1, .pSignalSemaphoreInfos=&signal};
    VkResult result = n->vkQueueSubmit2(n->queue, 1, &submit, VK_NULL_HANDLE);
    if (result == VK_SUCCESS) { b->pending = 1; n->in_flight = 1; }
    else if (result != VK_ERROR_OUT_OF_HOST_MEMORY && result != VK_ERROR_OUT_OF_DEVICE_MEMORY
             && result != VK_ERROR_DEVICE_LOST) native_drain(n);
    return vk_ok(result, "submit");
}

static int native_wait(Native *n, NativeBatch *b) {
    NEED(b->pending);
    VkSemaphoreWaitInfo wait = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,
        .semaphoreCount=1, .pSemaphores=&n->timeline, .pValues=&b->value};
    VkResult result = n->vkWaitSemaphores(n->device, &wait, UINT64_MAX);
    if (result == VK_SUCCESS || result == VK_ERROR_DEVICE_LOST) {
        b->pending = 0; n->in_flight = 0;
        /* Match OGPU: terminal wait retires command resources; the query pool
         * remains completion-owned until result retrieval or destruction. */
        if (b->pool) n->vkDestroyCommandPool(n->device, b->pool, NULL);
        b->pool = VK_NULL_HANDLE; b->command = VK_NULL_HANDLE;
    }
    b->succeeded = result == VK_SUCCESS;
    if (result == VK_SUCCESS) n->observed_value = b->value;
    return vk_ok(result, "wait");
}

static int native_elapsed(Native *n, NativeBatch *b, double *ns) {
    NEED(b->queries && b->succeeded && !b->pending && native_timing_supported(n));
    uint64_t ticks[2];
    VK_TRY(n->vkGetQueryPoolResults(n->device, b->queries, 0, 2, sizeof(ticks), ticks, 8, VK_QUERY_RESULT_64_BIT));
    *ns = native_timestamp_delta(ticks[0], ticks[1], n->timestamp_bits, n->properties.limits.timestampPeriod);
    n->vkDestroyQueryPool(n->device, b->queries, NULL); b->queries = VK_NULL_HANDLE;
    return 1;
}

static void native_batch_destroy(Native *n, NativeBatch *b) {
    if (b->pending && !native_wait(n, b) && b->pending) {
        native_drain(n);
        /* A failed explicit wait remains an error to the application. This is
         * terminal cleanup only, not permission to retry workload execution. */
        n->in_flight = 0;
    }
    if (b->pool) n->vkDestroyCommandPool(n->device, b->pool, NULL);
    if (b->queries) n->vkDestroyQueryPool(n->device, b->queries, NULL);
    memset(b, 0, sizeof(*b));
}

#include "native_workload.h"

int main(int argc, char **argv) {
    if (argc > 1 && (!strcmp(argv[1], "--validate") || !strcmp(argv[1], "--measure"))) return native_workload(argc, argv);
    if (argc != 2 || strcmp(argv[1], "--smoke")) {
        fprintf(stderr, "Usage: native --smoke | --validate/--measure resident|end-to-end weights w h ow oh output caseA caseB\n"); return 1;
    }
    Native n = {0};
    NativeBuffer upload = {0}, device = {0}, readback = {0};
    NativeBatch batch = {0};
    int result = 1;
    unsigned char input[260], output[260];
    if (!native_create(&n)
        || !native_buffer_create(&n, &upload, sizeof(input), 1)
        || !native_buffer_create(&n, &device, sizeof(input), 0)
        || !native_buffer_create(&n, &readback, sizeof(input), 1)) goto cleanup;
    /* A/B/A with exact full-buffer checks, fresh command pools, same allocations. */
    for (unsigned frame = 0; frame < 3; ++frame) {
        for (unsigned i = 0; i < sizeof(input); ++i) input[i] = (unsigned char)(i * 13 + (frame == 1 ? 71 : 0));
        if (!native_write(&n, &upload, 0, input, sizeof(input)) || !native_begin(&n, &batch)) goto cleanup;
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_TRANSFER_BIT,
            VK_ACCESS_2_TRANSFER_READ_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_READ_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT);
        if (!native_copy(&n, &batch, &upload, 0, &device, 0, sizeof(input))) goto cleanup;
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_READ_BIT);
        if (!native_copy(&n, &batch, &device, 0, &readback, 0, sizeof(input))
            || !native_submit(&n, &batch) || !native_wait(&n, &batch)) goto cleanup;
        native_batch_destroy(&n, &batch);
        if (!native_read(&n, &readback, 0, output, sizeof(output)) || memcmp(input, output, sizeof(input))) {
            fprintf(stderr, "Native smoke byte comparison failed\n"); goto cleanup;
        }
    }
    result = 0;
cleanup:
    native_batch_destroy(&n, &batch);
    native_buffer_destroy(&n, &readback);
    native_buffer_destroy(&n, &device);
    native_buffer_destroy(&n, &upload);
    native_destroy(&n);
    if (!result) puts("Native setup/address-copy A/B/A smoke PASS; no shader execution or timing comparison");
    return result;
}
