/* Failure/ownership tests for the event control; no GPU or Vulkan loader. */
#define FRONTIER_NATIVE
#define DEPENDENCIES_NO_MAIN
#include "dependencies.c"
#include <assert.h>

static VkResult reset_result, submit_result, wait_result;
static unsigned resets, submits, waits, drains, pools, events;
static VkResult VKAPI_CALL reset_event_mock(VkDevice d,VkEvent event) {
    (void)d; assert(event); ++resets; return reset_result;
}
static VkResult VKAPI_CALL submit_mock(VkQueue q,uint32_t count,const VkSubmitInfo2 *info,VkFence fence) {
    (void)q; (void)fence; assert(count==1 && info->commandBufferInfoCount==1); ++submits; return submit_result;
}
static VkResult VKAPI_CALL wait_mock(VkDevice d,const VkSemaphoreWaitInfo *info,uint64_t timeout) {
    (void)d; assert(info->semaphoreCount==1 && timeout==UINT64_MAX); ++waits; return wait_result;
}
static VkResult VKAPI_CALL drain_mock(VkQueue q) { (void)q; ++drains; return VK_SUCCESS; }
static void VKAPI_CALL pool_mock(VkDevice d,VkCommandPool pool,const VkAllocationCallbacks *a) {
    (void)d; (void)a; assert(pool); ++pools;
}
static void VKAPI_CALL event_mock(VkDevice d,VkEvent event,const VkAllocationCallbacks *a) {
    (void)d; (void)a; assert(event && pools==1); ++events;
}
static DependencyControl fixture(void) {
    reset_result=submit_result=wait_result=VK_SUCCESS;
    resets=submits=waits=drains=pools=events=0;
    DependencyControl d={0}; d.strategy="split"; d.base.policy="replay"; d.base.count=2;
    d.event=(VkEvent)(uintptr_t)1; d.reset_event=reset_event_mock; d.destroy_event=event_mock;
    Native *n=&d.base.native; n->max_difference=UINT64_MAX;
    n->vkQueueSubmit2=submit_mock; n->vkWaitSemaphores=wait_mock; n->vkQueueWaitIdle=drain_mock;
    n->vkDestroyCommandPool=pool_mock;
    d.base.slots[0].batch.pool=(VkCommandPool)(uintptr_t)1;
    d.base.slots[0].batch.command=(VkCommandBuffer)(uintptr_t)1;
    return d;
}
int main(void) {
    (void)dependency_create; (void)window; (void)orders; assert(selftest());
    Frame frame; double wall;
    DependencyControl d=fixture(); reset_result=VK_ERROR_OUT_OF_HOST_MEMORY;
    assert(!dependency_window(&d,1,0,&frame,&wall));
    assert(resets==1 && !submits && !waits && !drains);
    dependency_destroy(&d); assert(pools==1 && events==1);

    d=fixture(); d.base.slots[0].pending=1; d.base.slots[0].batch.value=1;
    assert(!dependency_window(&d,1,0,&frame,&wall) && !resets && !submits);
    dependency_destroy(&d); assert(waits==1 && pools==1 && events==1);

    VkResult failures[]={VK_ERROR_OUT_OF_HOST_MEMORY,VK_ERROR_UNKNOWN,VK_ERROR_DEVICE_LOST};
    for (unsigned i=0;i<3;++i) {
        d=fixture(); submit_result=failures[i];
        assert(!dependency_window(&d,1,0,&frame,&wall));
        assert(resets==1 && submits==1 && !waits && drains==1 && !d.base.slots[0].pending);
        dependency_destroy(&d); assert(pools==1 && events==1 && !waits);

        d=fixture(); wait_result=failures[i];
        assert(!dependency_window(&d,1,0,&frame,&wall));
        assert(resets==1 && submits==1 && waits==1 && !d.base.slots[0].pending);
        assert(drains==(failures[i]!=VK_ERROR_DEVICE_LOST));
        dependency_destroy(&d); assert(pools==1 && events==1 && waits==1);
    }
    puts("P4 reset/pending/submit/wait/loss failures drain before command/event release PASS");
    return 0;
}
