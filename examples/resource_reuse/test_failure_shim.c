#define _POSIX_C_SOURCE 200809L
#include "failure_shim.c"
#include "diagnostics.h"
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

#define TEST(x) do { if (!(x)) { fprintf(stderr, "Shim test line %d: %s\n", __LINE__, #x); return 1; } } while (0)
static unsigned submits, waits, idles, frees;
static VkResult idle_result = VK_SUCCESS;
static VkResult VKAPI_CALL fake_submit(VkQueue q, uint32_t n, const VkSubmitInfo2 *s, VkFence f) {
    (void)q; (void)n; (void)s; (void)f; ++submits; return VK_SUCCESS;
}
static VkResult VKAPI_CALL fake_wait(VkDevice d, const VkSemaphoreWaitInfo *w, uint64_t t) {
    (void)d; (void)w; (void)t; ++waits; return VK_SUCCESS;
}
static VkResult VKAPI_CALL fake_idle(VkQueue q) { (void)q; ++idles; return idle_result; }
static void VKAPI_CALL fake_free(VkDevice d, VkDeviceMemory m, const VkAllocationCallbacks *a) {
    (void)d; (void)m; (void)a; ++frees;
}
static void early_free(void) { fault_free(VK_NULL_HANDLE, (VkDeviceMemory)(uintptr_t)1, NULL); }
static void unaccepted_wait(void) {
    VkSemaphoreWaitInfo w = {.semaphoreCount=1, .pSemaphores=&fault_timeline, .pValues=&rejected};
    (void)fault_wait(VK_NULL_HANDLE, &w, 0);
}
static void undrained_loss(void) {
    idle_result = VK_ERROR_OUT_OF_HOST_MEMORY;
    armed = FAULT_LOSS;
    VkSemaphoreWaitInfo w = {.semaphoreCount=1, .pSemaphores=&fault_timeline, .pValues=&accepted};
    (void)fault_wait(VK_NULL_HANDLE, &w, UINT64_MAX);
}
static int aborts(void (*action)(void)) {
    pid_t child = fork();
    if (child < 0) return 0;
    if (!child) {
        struct rlimit limit = {0,0};
        if (setrlimit(RLIMIT_CORE, &limit)) _exit(2);
        action(); _exit(0);
    }
    int status;
    return waitpid(child, &status, 0) == child && WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT;
}
int main(void) {
    real_submit = fake_submit; real_wait = fake_wait; real_idle = fake_idle; memory_free = fake_free;
    VkQueue queue = (VkQueue)(uintptr_t)1;
    VkSemaphoreSubmitInfo signal = {.semaphore=(VkSemaphore)(uintptr_t)2, .value=1};
    VkSubmitInfo2 submit = {.signalSemaphoreInfoCount=1, .pSignalSemaphoreInfos=&signal};
    VkSemaphoreWaitInfo wait = {.semaphoreCount=1, .pSemaphores=&signal.semaphore, .pValues=&signal.value};
    TEST(!reuse_fault_arm(7) && reuse_fault_arm(FAULT_REJECT) && !reuse_fault_arm(FAULT_POLL));
    TEST(fault_submit(queue,1,&submit,VK_NULL_HANDLE) == VK_ERROR_OUT_OF_HOST_MEMORY);
    TEST(!submits && !accepted && rejected == 1 && !armed && injections == 1 && aborts(unaccepted_wait));
    signal.value = 2;
    TEST(fault_submit(queue,1,&submit,VK_NULL_HANDLE) == VK_SUCCESS && submits == 1 && accepted == 2);
    TEST(aborts(early_free) && aborts(undrained_loss));
    TEST(reuse_fault_arm(FAULT_TIMEOUT));
    TEST(fault_wait(VK_NULL_HANDLE,&wait,0) == VK_TIMEOUT && !waits && !drained);
    TEST(reuse_fault_arm(FAULT_POLL));
    TEST(fault_wait(VK_NULL_HANDLE,&wait,0) == VK_ERROR_OUT_OF_HOST_MEMORY && !waits && !drained);
    TEST(reuse_fault_arm(FAULT_WAIT));
    TEST(fault_wait(VK_NULL_HANDLE,&wait,UINT64_MAX) == VK_ERROR_OUT_OF_HOST_MEMORY && !waits && !drained);
    TEST(fault_wait(VK_NULL_HANDLE,&wait,UINT64_MAX) == VK_SUCCESS && waits == 1 && drained == 2);
    signal.value = 3;
    TEST(reuse_fault_arm(FAULT_UNKNOWN));
    TEST(fault_submit(queue,1,&submit,VK_NULL_HANDLE) == VK_ERROR_UNKNOWN && submits == 2 && accepted == 3 && drained == 2);
    TEST(fault_idle(queue) == VK_SUCCESS && idles == 1 && drained == accepted);
    signal.value = 4;
    TEST(fault_submit(queue,1,&submit,VK_NULL_HANDLE) == VK_SUCCESS);
    TEST(reuse_fault_arm(FAULT_LOSS));
    TEST(fault_wait(VK_NULL_HANDLE,&wait,UINT64_MAX) == VK_ERROR_DEVICE_LOST && idles == 2 && drained == 4);
    fault_free(VK_NULL_HANDLE,(VkDeviceMemory)(uintptr_t)1,NULL);
    TEST(frees == 1 && injections == 6 && !armed);
    FILE *file = tmpfile(); TEST(file);
    reuse_json_string(file, "quote\"slash\\\n\t\001\377");
    rewind(file); char buffer[128] = {0}; TEST(fread(buffer,1,sizeof(buffer)-1,file) > 0);
    TEST(!fclose(file));
    TEST(!strcmp(buffer,"\"quote\\\"slash\\\\\\u000a\\u0009\\u0001\\u00ff\""));
    TEST(!strcmp(reuse_state_name(REUSE_FAILED_DRAINED),"failed-drained"));
    TEST(!strcmp(reuse_state_name((ReuseState)99),"invalid"));
    puts("Failure shim forwarding/drain/free guards and JSON escaping PASS (CPU fakes, no GPU)");
    return 0;
}
