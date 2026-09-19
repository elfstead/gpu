/* Injected native lifetime checks; no loader or GPU. */
#define FRONTIER_NATIVE
#define FRONTIER_NO_MAIN
#include "small.c"
#include <assert.h>

static VkResult submit_result, wait_result, reset_result;
static unsigned submits, waits, drains, destroys, resets;
static uint64_t waited_value;
static VkResult VKAPI_CALL submit_mock(VkQueue q, uint32_t count, const VkSubmitInfo2 *s, VkFence fence) {
    (void)q; assert(count == 1 && !fence && s->signalSemaphoreInfoCount == 1 && s->commandBufferInfoCount == 1);
    assert(s->pSignalSemaphoreInfos->value > 0); ++submits; return submit_result;
}
static VkResult VKAPI_CALL wait_mock(VkDevice d, const VkSemaphoreWaitInfo *w, uint64_t timeout) {
    (void)d; assert(w->semaphoreCount == 1 && timeout == UINT64_MAX);
    waited_value = w->pValues[0]; ++waits; return wait_result;
}
static VkResult VKAPI_CALL drain_mock(VkQueue q) { (void)q; ++drains; return VK_SUCCESS; }
static void VKAPI_CALL destroy_mock(VkDevice d, VkCommandPool p, const VkAllocationCallbacks *a) {
    (void)d; (void)a; assert(p); ++destroys;
}
static VkResult VKAPI_CALL reset_mock(VkDevice d, VkCommandPool p, VkCommandPoolResetFlags f) {
    (void)d; assert(p && !f); ++resets; return reset_result;
}
static Context fixture(const char *policy) {
    submit_result = wait_result = reset_result = VK_SUCCESS;
    submits = waits = drains = destroys = resets = 0; waited_value = 0;
    Context c = {.count=3, .dispatches=1, .policy=policy};
    c.native.max_difference = UINT64_MAX;
    c.native.vkQueueSubmit2 = submit_mock; c.native.vkWaitSemaphores = wait_mock;
    c.native.vkQueueWaitIdle = drain_mock; c.native.vkDestroyCommandPool = destroy_mock;
    c.reset_pool = reset_mock;
    for (unsigned i = 0; i < c.count; ++i) {
        c.slots[i].batch.pool = (VkCommandPool)(uintptr_t)(i + 1);
        c.slots[i].batch.command = (VkCommandBuffer)(uintptr_t)(i + 1);
    }
    return c;
}
int main(void) {
    /* These production paths are exercised by validated GPU controls. */
    (void)create; (void)destroy; (void)window;
    assert(selftest());
    Context c = fixture("replay");
    for (unsigned i = 0; i < 3; ++i) assert(submit_slot(&c, &c.slots[i]));
    assert(submits == 3 && !waits); /* multiple submissions, no hidden serial wait */
    assert(!record_slot(&c, &c.slots[0]) && !submit_slot(&c, &c.slots[0]));
    assert(wait_slot(&c, &c.slots[0]) && waited_value == 1 && !destroys);
    assert(record_slot(&c, &c.slots[0]) && submit_slot(&c, &c.slots[0]));
    assert(c.slots[0].submitted == 2);
    assert(wait_slot(&c, &c.slots[1]) && waited_value == 2);
    assert(wait_slot(&c, &c.slots[2]) && waited_value == 3);
    assert(wait_slot(&c, &c.slots[0]) && waited_value == 4 && !drains && !destroys);

    c = fixture("fresh");
    assert(submit_slot(&c, &c.slots[0]) && wait_slot(&c, &c.slots[0]));
    assert(destroys == 1 && !c.slots[0].batch.command && !c.slots[0].batch.pool);
    assert(wait_slot(&c, &c.slots[0]) && waits == 1); /* exactly once */

    c = fixture("reset"); reset_result = VK_ERROR_OUT_OF_HOST_MEMORY;
    assert(!record_slot(&c, &c.slots[0]) && resets == 1 && !submits && !waits);
    native_batch_destroy(&c.native, &c.slots[0].batch); assert(destroys == 1);

    c = fixture("replay"); submit_result = VK_ERROR_OUT_OF_HOST_MEMORY;
    assert(!submit_slot(&c, &c.slots[0]) && !c.slots[0].pending && !c.slots[0].submitted);
    assert(!waits && drains == 1 && c.slots[0].batch.value == 0);

    c = fixture("reset"); assert(submit_slot(&c, &c.slots[0]));
    wait_result = VK_ERROR_OUT_OF_HOST_MEMORY;
    assert(!wait_slot(&c, &c.slots[0]) && drains == 1 && !c.slots[0].pending);
    native_batch_destroy(&c.native, &c.slots[0].batch); assert(destroys == 1);

    c = fixture("fresh"); assert(submit_slot(&c, &c.slots[0]));
    wait_result = VK_ERROR_DEVICE_LOST;
    assert(!wait_slot(&c, &c.slots[0]) && !drains && destroys == 1 && !c.slots[0].pending);
    puts("Frontier injected slot/replay/reset/submission-failure/drain checks PASS");
    return 0;
}
