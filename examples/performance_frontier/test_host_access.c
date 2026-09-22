#define FRONTIER_NATIVE
#define HOST_ACCESS_NO_MAIN
#include "host_access.c"

static unsigned flushes, invalidates;
static VkResult VKAPI_CALL flush_mock(VkDevice d, uint32_t count, const VkMappedMemoryRange *r) {
    (void)d; assert(count == 1 && r->offset == 512 && r->size == 512); ++flushes; return VK_SUCCESS;
}
static VkResult VKAPI_CALL invalidate_mock(VkDevice d, uint32_t count, const VkMappedMemoryRange *r) {
    (void)d; assert(count == 1 && r->offset == 0 && r->size == 512); ++invalidates; return VK_ERROR_OUT_OF_HOST_MEMORY;
}
static unsigned signals;
static VkResult VKAPI_CALL signal_mock(VkDevice d, const VkSemaphoreSignalInfo *s) {
    (void)d; assert(s->semaphore && s->value == 1); ++signals; return VK_SUCCESS;
}
static VkResult VKAPI_CALL reject_submit(VkQueue q, uint32_t count, const VkSubmitInfo2 *s, VkFence fence) {
    (void)q; (void)fence; assert(count == 1 && s->waitSemaphoreInfoCount == 1);
    return VK_ERROR_OUT_OF_HOST_MEMORY;
}
int main(void) {
    (void)create; (void)window; (void)selftest; (void)host_create; (void)host_destroy; (void)host_window; (void)gate_check;
    assert(host_selftest());
    HostAccess h = {.bytes=388, .stride=512, .shared=1};
    h.base.native.properties.limits.nonCoherentAtomSize = 256;
    h.base.slots[0].buffer.allocated = 1024;
    h.base.native.vkFlushMappedMemoryRanges = flush_mock;
    h.base.native.vkInvalidateMappedMemoryRanges = invalidate_mock;
    assert(sync_range(&h, 1, 1) && flushes == 1);
    assert(!sync_range(&h, 0, 0) && invalidates == 1);
    h.base.slots[0].buffer.coherent = 1;
    assert(sync_range(&h, 0, 0) && invalidates == 1);
    h.stride = 388;
    assert(!sync_range(&h, 1, 1) && flushes == 1); /* Shared atom rejected, even coherent. */
    h.gate = (VkSemaphore)(uintptr_t)1; h.signal = signal_mock; gated_host = &h;
    ungated_submit = reject_submit; VkSubmitInfo2 submit = {0};
    assert(gated_submit(VK_NULL_HANDLE, 1, &submit, VK_NULL_HANDLE) == VK_ERROR_OUT_OF_HOST_MEMORY);
    assert(h.opened && signals == 1); /* Unblock before submit_slot failure drain. */
    assert(open_gate(&h) && signals == 1);
    puts("HOST_ACCESS noncoherent range/error/coherent bypass/gate-failure cleanup PASS");
    return 0;
}
