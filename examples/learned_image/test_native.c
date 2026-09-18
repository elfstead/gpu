/* Real helper implementation, injected Vulkan calls: no loader or GPU required. */
#define main native_smoke_main
#include "native.c"
#undef main
#include <assert.h>

static int fail_step, step, buffers, allocations, mappings, pools, drained, waited, transient_drains;
static char events[64];
static unsigned event_count;
static unsigned char mapping[512];
static VkResult submit_result, wait_result;
#define HANDLE(type, value) ((type)(uintptr_t)(value))
static VkResult next(void) { return ++step == fail_step ? VK_ERROR_OUT_OF_HOST_MEMORY : VK_SUCCESS; }
static void event(char c) { assert(event_count + 1 < sizeof(events)); events[event_count++] = c; }
static VkResult VKAPI_CALL create_buffer(VkDevice d, const VkBufferCreateInfo *info,
                                        const VkAllocationCallbacks *a, VkBuffer *out) {
    (void)d; (void)a;
    assert(info->size == 260 && info->sharingMode == VK_SHARING_MODE_EXCLUSIVE);
    assert(info->usage == (VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT | VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT
        | VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT));
    VkResult r = next(); if (r == VK_SUCCESS) { ++buffers; *out = HANDLE(VkBuffer, 1); } return r;
}
static void VKAPI_CALL buffer_requirements(VkDevice d, VkBuffer b, VkMemoryRequirements *out) {
    (void)d; assert(b); *out = (VkMemoryRequirements){.size=512, .alignment=64, .memoryTypeBits=1};
}
static VkResult VKAPI_CALL allocate(VkDevice d, const VkMemoryAllocateInfo *info,
                                    const VkAllocationCallbacks *a, VkDeviceMemory *out) {
    (void)d; (void)a;
    assert(info->allocationSize == 512 && info->memoryTypeIndex == 0);
    const VkMemoryAllocateFlagsInfo *flags = info->pNext;
    assert(flags && flags->flags == VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT);
    const VkMemoryDedicatedAllocateInfo *dedicated = flags->pNext;
    assert(dedicated && dedicated->buffer && !dedicated->image);
    VkResult r = next(); if (r == VK_SUCCESS) { ++allocations; *out = HANDLE(VkDeviceMemory, 2); } return r;
}
static VkResult VKAPI_CALL bind(VkDevice d, VkBuffer b, VkDeviceMemory m, VkDeviceSize offset) {
    (void)d; assert(b && m && !offset); return next();
}
static VkResult VKAPI_CALL map(VkDevice d, VkDeviceMemory m, VkDeviceSize offset, VkDeviceSize size,
                               VkMemoryMapFlags flags, void **out) {
    (void)d; assert(m && !offset && size == VK_WHOLE_SIZE && !flags);
    VkResult r = next(); if (r == VK_SUCCESS) { ++mappings; *out = mapping; } return r;
}
static VkDeviceAddress VKAPI_CALL address(VkDevice d, const VkBufferDeviceAddressInfo *info) {
    (void)d; assert(info->buffer); return fail_step == 5 ? 0 : 4096;
}
static void VKAPI_CALL unmap(VkDevice d, VkDeviceMemory m) {
    (void)d; assert(m && mappings == 1); --mappings; event('u');
}
static void VKAPI_CALL destroy_buffer(VkDevice d, VkBuffer b, const VkAllocationCallbacks *a) {
    (void)d; (void)a; assert(b && buffers == 1 && !mappings); --buffers; event('b');
}
static void VKAPI_CALL free_memory(VkDevice d, VkDeviceMemory m, const VkAllocationCallbacks *a) {
    (void)d; (void)a; assert(m && allocations == 1 && !buffers && !mappings); --allocations; event('m');
}
static VkResult VKAPI_CALL cache(VkDevice d, uint32_t count, const VkMappedMemoryRange *ranges) {
    (void)d; assert(count == 1 && ranges->memory && !ranges->offset && ranges->size == VK_WHOLE_SIZE);
    return next();
}
static VkResult VKAPI_CALL create_pool(VkDevice d, const VkCommandPoolCreateInfo *info,
                                      const VkAllocationCallbacks *a, VkCommandPool *out) {
    (void)d; (void)a; assert(!info->flags && info->queueFamilyIndex == 3);
    VkResult r = next(); if (r == VK_SUCCESS) { ++pools; *out = HANDLE(VkCommandPool, 3); } return r;
}
static VkResult VKAPI_CALL allocate_command(VkDevice d, const VkCommandBufferAllocateInfo *info, VkCommandBuffer *out) {
    (void)d; assert(info->commandPool && info->commandBufferCount == 1 && info->level == VK_COMMAND_BUFFER_LEVEL_PRIMARY);
    VkResult r = next(); if (r == VK_SUCCESS) *out = HANDLE(VkCommandBuffer, 4); return r;
}
static VkResult VKAPI_CALL begin_command(VkCommandBuffer c, const VkCommandBufferBeginInfo *info) {
    assert(c && !info->flags); return next();
}
static void VKAPI_CALL barrier(VkCommandBuffer c, const VkDependencyInfo *info) {
    assert(c && info->memoryBarrierCount == 1 && !info->bufferMemoryBarrierCount && !info->imageMemoryBarrierCount);
}
static VkResult VKAPI_CALL end_command(VkCommandBuffer c) { assert(c); return next(); }
static VkResult VKAPI_CALL submit(VkQueue q, uint32_t count, const VkSubmitInfo2 *info, VkFence f) {
    (void)q; assert(count == 1 && !f && info->commandBufferInfoCount == 1 && !info->waitSemaphoreInfoCount);
    assert(info->signalSemaphoreInfoCount == 1 && info->pSignalSemaphoreInfos->value == 1
        && info->pSignalSemaphoreInfos->stageMask == VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT);
    event('s'); return submit_result;
}
static VkResult VKAPI_CALL wait_semaphore(VkDevice d, const VkSemaphoreWaitInfo *info, uint64_t timeout) {
    (void)d; assert(info->semaphoreCount == 1 && *info->pValues == 1 && timeout == UINT64_MAX);
    ++waited; event('w'); return wait_result;
}
static VkResult VKAPI_CALL idle(VkQueue q) {
    (void)q; ++drained; event('d');
    return drained <= transient_drains ? VK_ERROR_OUT_OF_HOST_MEMORY : VK_SUCCESS;
}
static void VKAPI_CALL destroy_pool(VkDevice d, VkCommandPool p, const VkAllocationCallbacks *a) {
    (void)d; (void)a; assert(p && pools == 1); --pools; event('p');
}
static void VKAPI_CALL copy_memory(VkCommandBuffer c, const VkCopyDeviceMemoryInfoKHR *info) {
    assert(c && info->regionCount == 1);
    const VkDeviceMemoryCopyKHR *r = info->pRegions;
    assert(r->srcRange.address == 4160 && r->dstRange.address == 8196);
    assert(r->srcRange.size == 128 && r->dstRange.size == 128
        && r->srcFlags == VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR && r->dstFlags == r->srcFlags);
}

static Native fake(void) {
    assert(!buffers && !allocations && !mappings && !pools);
    step = fail_step = drained = waited = event_count = transient_drains = 0;
    memset(events, 0, sizeof(events));
    submit_result = wait_result = VK_SUCCESS;
    return (Native){.family=3, .max_difference=1, .memory={.memoryTypeCount=1,
        .memoryTypes={{.propertyFlags=VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT}}},
        .vkCreateBuffer=create_buffer, .vkGetBufferMemoryRequirements=buffer_requirements,
        .vkAllocateMemory=allocate, .vkBindBufferMemory=bind, .vkMapMemory=map, .vkGetBufferDeviceAddress=address,
        .vkUnmapMemory=unmap, .vkDestroyBuffer=destroy_buffer, .vkFreeMemory=free_memory,
        .vkFlushMappedMemoryRanges=cache, .vkInvalidateMappedMemoryRanges=cache,
        .vkCreateCommandPool=create_pool, .vkAllocateCommandBuffers=allocate_command,
        .vkBeginCommandBuffer=begin_command, .vkCmdPipelineBarrier2=barrier, .vkEndCommandBuffer=end_command,
        .vkQueueSubmit2=submit, .vkWaitSemaphores=wait_semaphore, .vkQueueWaitIdle=idle,
        .vkDestroyCommandPool=destroy_pool, .vkCmdCopyMemoryKHR=copy_memory};
}

static void test_memory_policy(void) {
    uint32_t chosen = UINT32_MAX;
    VkPhysicalDeviceMemoryProperties m = {.memoryTypeCount=6};
    m.memoryTypes[0].propertyFlags = VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT | VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
    m.memoryTypes[1].propertyFlags = VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
    m.memoryTypes[2].propertyFlags = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    m.memoryTypes[3].propertyFlags = m.memoryTypes[2].propertyFlags | VK_MEMORY_PROPERTY_HOST_CACHED_BIT;
    m.memoryTypes[4].propertyFlags = m.memoryTypes[3].propertyFlags | VK_MEMORY_PROPERTY_PROTECTED_BIT;
    m.memoryTypes[5].propertyFlags = m.memoryTypes[3].propertyFlags | VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD;
    assert(native_memory_type(&m, 63, 0, &chosen) && chosen == 1);
    assert(native_memory_type(&m, 63, 1, &chosen) && chosen == 3);
    assert(native_memory_type(&m, 1, 1, &chosen) && chosen == 0); /* noncoherent UMA */
    assert(native_memory_type(&m, 1, 0, &chosen) && chosen == 0);
    assert(!native_memory_type(&m, 0, 0, &chosen));
    assert(!native_memory_type(&m, 48, 1, &chosen));
    m.memoryTypes[5].propertyFlags = m.memoryTypes[3].propertyFlags;
    assert(native_memory_type(&m, 63, 1, &chosen) && chosen == 5); /* last equal score */
    m.memoryTypeCount = VK_MAX_MEMORY_TYPES + 1;
    assert(!native_memory_type(&m, UINT32_MAX, 1, &chosen));
    assert(native_range(260, 64, 196) && !native_range(260, 65, 196));
    assert(!native_range(260, 0, 0) && !native_range(260, UINT64_MAX, 2));
    assert(native_range(UINT64_MAX, UINT64_MAX - 1, 1));
}

static void test_buffer_failures(void) {
    for (int fault = 1; fault <= 5; ++fault) {
        Native n = fake(); NativeBuffer b = {0}; fail_step = fault;
        assert(!native_buffer_create(&n, &b, 260, 1));
        native_buffer_destroy(&n, &b);
        assert(!buffers && !allocations && !mappings && !b.buffer && !b.memory);
        native_buffer_destroy(&n, &b); /* idempotent partial cleanup */
    }
    Native n = fake(); NativeBuffer b = {0};
    assert(!native_buffer_create(&n, &b, 0, 1));
    assert(!native_buffer_create(&n, &b, UINT64_MAX, 1));
    assert(native_buffer_create(&n, &b, 260, 1));
    unsigned char input[260], output[260]; memset(input, 71, sizeof(input));
    assert(native_write(&n, &b, 0, input, sizeof(input)));
    assert(native_read(&n, &b, 0, output, sizeof(output)) && !memcmp(input, output, sizeof(input)));
    assert(!native_write(&n, &b, 1, input, sizeof(input)));
    fail_step = step + 1; assert(!native_write(&n, &b, 0, input, sizeof(input)));
    fail_step = step + 1; assert(!native_read(&n, &b, 0, output, sizeof(output)));
    b.coherent = 1; fail_step = step + 1;
    assert(native_write(&n, &b, 0, input, sizeof(input))); /* coherent: no flush */
    assert(step + 1 == fail_step);
    native_buffer_destroy(&n, &b); assert(!strcmp(events, "ubm"));
}

static void test_batch_failures(void) {
    for (int fault = 1; fault <= 3; ++fault) {
        Native n = fake(); NativeBatch b = {0}; fail_step = fault;
        assert(!native_begin(&n, &b)); native_batch_destroy(&n, &b);
        assert(!pools && !drained && !waited);
    }
    const VkResult failures[] = {VK_ERROR_OUT_OF_HOST_MEMORY, VK_ERROR_OUT_OF_DEVICE_MEMORY,
        VK_ERROR_DEVICE_LOST, VK_ERROR_UNKNOWN};
    for (unsigned i = 0; i < sizeof(failures) / sizeof(*failures); ++i) {
        Native n = fake(); NativeBatch b = {0};
        assert(native_begin(&n, &b)); submit_result = failures[i];
        assert(!native_submit(&n, &b) && !b.pending);
        native_batch_destroy(&n, &b);
        assert(!pools && !waited && drained == (failures[i] == VK_ERROR_UNKNOWN));
        assert(!strcmp(events, failures[i] == VK_ERROR_UNKNOWN ? "sdp" : "sp"));
    }
    for (unsigned mode = 0; mode < 3; ++mode) {
        Native n = fake(); NativeBatch b = {0};
        assert(native_begin(&n, &b) && native_submit(&n, &b) && b.pending);
        assert(!native_submit(&n, &b)); /* consumed, never submit twice */
        wait_result = mode == 0 ? VK_SUCCESS : mode == 1 ? VK_ERROR_DEVICE_LOST : VK_ERROR_OUT_OF_HOST_MEMORY;
        native_batch_destroy(&n, &b);
        assert(!pools && !n.in_flight && waited == 1 && drained == (mode == 2));
        assert(!strcmp(events, mode == 2 ? "swdp" : "swp"));
    }
    Native n = fake(); NativeBatch b = {0}; assert(native_begin(&n, &b));
    NativeBuffer a = {.size=260, .address=4096}, c = {.size=260, .address=8192};
    assert(native_copy(&n, &b, &a, 64, &c, 4, 128));
    assert(!native_copy(&n, &b, &a, 133, &c, 4, 128));
    n.next_value = UINT64_MAX; assert(!native_submit(&n, &b));
    n.next_value = 1; assert(!native_submit(&n, &b)); /* queried timeline distance */
    n.next_value = 0; fail_step = step + 1;
    assert(!native_submit(&n, &b)); /* vkEndCommandBuffer fails before submission */
    native_batch_destroy(&n, &b); assert(!waited && !drained);
    n = fake(); b = (NativeBatch){0};
    assert(native_begin(&n, &b) && native_submit(&n, &b));
    wait_result = VK_ERROR_OUT_OF_HOST_MEMORY; transient_drains = 2;
    native_batch_destroy(&n, &b);
    assert(!pools && !n.in_flight && !strcmp(events, "swdddp"));
}

int main(void) {
    test_memory_policy(); test_buffer_failures(); test_batch_failures();
    puts("Native policy, address bounds, partial allocation and submission cleanup tests PASS");
    return 0;
}
