/* Test-only, single-device/queue, externally serialized loader. Never use in
 * production or timing runs. Real work drains BEFORE synthetic device loss.
 * Reuse the allocation tracer; no runtime-private entry points or allocation override. */
#define vkGetInstanceProcAddr memory_instance
#define vkDestroyInstance memory_destroy_instance
#include "../learned_image/trace_memory.c"
#undef vkGetInstanceProcAddr
#undef vkDestroyInstance

enum { FAULT_NONE, FAULT_REJECT, FAULT_TIMEOUT, FAULT_POLL, FAULT_WAIT, FAULT_LOSS, FAULT_UNKNOWN };
static PFN_vkGetDeviceProcAddr fault_device_get;
static PFN_vkQueueSubmit2 real_submit;
static PFN_vkWaitSemaphores real_wait;
static PFN_vkQueueWaitIdle real_idle;
static PFN_vkFreeMemory memory_free;
static VkQueue fault_queue;
static VkSemaphore fault_timeline;
static uint64_t accepted, drained, rejected;
static unsigned armed, injections;

/* Consumer explicitly arms AFTER setup/warmup; no fragile native call ordinal. */
int reuse_fault_arm(unsigned mode) {
    if (mode > FAULT_UNKNOWN || (armed && mode)) return 0;
    armed = mode; return 1;
}
static void fault_event(const char *operation, int result) {
    fprintf(stderr, "FAULT {\"operation\":\"%s\",\"mode\":%u,\"result\":%d,"
        "\"accepted\":%" PRIu64 ",\"drained\":%" PRIu64 "}\n", operation, armed, result, accepted, drained);
}
static VkResult VKAPI_CALL fault_submit(VkQueue q, uint32_t n, const VkSubmitInfo2 *s, VkFence f) {
    if (n != 1 || s->signalSemaphoreInfoCount != 1 || (fault_queue && fault_queue != q)) abort();
    fault_queue = q;
    VkSemaphore timeline = s->pSignalSemaphoreInfos[0].semaphore;
    if (fault_timeline && fault_timeline != timeline) abort();
    fault_timeline = timeline;
    if (armed == FAULT_REJECT) {
        rejected = s->pSignalSemaphoreInfos[0].value;
        ++injections; fault_event("submit-rejected-before-driver", VK_ERROR_OUT_OF_HOST_MEMORY);
        armed = 0; return VK_ERROR_OUT_OF_HOST_MEMORY;
    }
    VkResult result = real_submit(q, n, s, f);
    if (result == VK_SUCCESS) accepted = s->pSignalSemaphoreInfos[0].value;
    if (armed == FAULT_UNKNOWN && result == VK_SUCCESS) {
        ++injections; fault_event("submit-accepted-then-error", VK_ERROR_UNKNOWN);
        armed = 0; return VK_ERROR_UNKNOWN; /* Runtime must drain the queue, not invent a receipt. */
    }
    return result;
}
static VkResult VKAPI_CALL fault_idle(VkQueue q) {
    VkResult result = real_idle(q);
    if (result == VK_SUCCESS && q == fault_queue) drained = accepted;
    fault_event("queue-drain", result);
    return result;
}
static VkResult VKAPI_CALL fault_wait(VkDevice d, const VkSemaphoreWaitInfo *w, uint64_t timeout) {
    if (w->semaphoreCount != 1 || w->pSemaphores[0] != fault_timeline) abort();
    if (rejected && w->pValues[0] == rejected) abort(); /* Never wait an unsubmitted timeline value. */
    if ((armed == FAULT_TIMEOUT || armed == FAULT_POLL) && timeout == 0) {
        VkResult result = armed == FAULT_TIMEOUT ? VK_TIMEOUT : VK_ERROR_OUT_OF_HOST_MEMORY;
        ++injections; fault_event("poll-no-observation", result); armed = 0; return result;
    }
    if (armed == FAULT_WAIT && timeout == UINT64_MAX) {
        ++injections; fault_event("wait-error-before-drain", VK_ERROR_OUT_OF_HOST_MEMORY);
        armed = 0; return VK_ERROR_OUT_OF_HOST_MEMORY;
    }
    if (armed == FAULT_LOSS) {
        /* There is only one real queue. Drain ALL its accepted work before telling
         * the runtime it may apply the device-wide terminal cleanup contract. */
        if (!fault_queue || !real_idle || fault_idle(fault_queue) != VK_SUCCESS) abort();
        ++injections; fault_event("synthetic-loss-after-real-drain", VK_ERROR_DEVICE_LOST);
        armed = 0; return VK_ERROR_DEVICE_LOST;
    }
    VkResult result = real_wait(d, w, timeout);
    if (result == VK_SUCCESS && w->pValues[0] > drained) drained = w->pValues[0];
    fault_event("real-wait", result);
    return result;
}
static void VKAPI_CALL fault_free(VkDevice d, VkDeviceMemory m, const VkAllocationCallbacks *a) {
    /* Stronger than necessary for arbitrary programs, appropriate to this fixed
     * stream: no buffer/image backing may be freed before ALL work drains. */
    if (m && accepted > drained) abort();
    memory_free(d, m, a);
}
static PFN_vkVoidFunction fault_wrap(const char *name, PFN_vkVoidFunction f) {
    if (!f) return f;
#define HOOK(name_, type_, saved_, hook_) if (!strcmp(name, name_)) { saved_ = (type_)f; return (PFN_vkVoidFunction)hook_; }
    HOOK("vkQueueSubmit2", PFN_vkQueueSubmit2, real_submit, fault_submit)
    HOOK("vkWaitSemaphores", PFN_vkWaitSemaphores, real_wait, fault_wait)
    HOOK("vkQueueWaitIdle", PFN_vkQueueWaitIdle, real_idle, fault_idle)
    HOOK("vkFreeMemory", PFN_vkFreeMemory, memory_free, fault_free)
#undef HOOK
    return f;
}
static PFN_vkVoidFunction VKAPI_CALL fault_device(VkDevice d, const char *name) {
    return fault_wrap(name, fault_device_get(d, name));
}
VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance i, const char *name) {
    PFN_vkVoidFunction f = memory_instance(i, name);
    if (f && !strcmp(name, "vkGetDeviceProcAddr")) {
        fault_device_get = (PFN_vkGetDeviceProcAddr)f; return (PFN_vkVoidFunction)fault_device;
    }
    return fault_wrap(name, f);
}
VKAPI_ATTR void VKAPI_CALL vkDestroyInstance(VkInstance i, const VkAllocationCallbacks *a) {
    memory_destroy_instance(i, a);
    fprintf(stderr, "FAULT_SUMMARY {\"injections\":%u,\"armed\":%u,\"accepted\":%" PRIu64
        ",\"drained\":%" PRIu64 ",\"rejected_value\":%" PRIu64 "}\n", injections, armed, accepted, drained, rejected);
}
