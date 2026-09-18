// Derived from the libplacebo diagnostic loader pattern. No allocation overrides.
// Select via OGPU_VULKAN_LIBRARY, with OGPU_TRACE_LOADER pointing at the real loader.
#include <vulkan/vulkan.h>
#include <dlfcn.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "allocation_tracker.h"

static PFN_vkGetInstanceProcAddr get_instance;
static PFN_vkGetDeviceProcAddr get_device;
static PFN_vkAllocateMemory allocate_memory;
static PFN_vkFreeMemory free_memory;
static AllocationTracker tracker;

static VkResult VKAPI_CALL trace_allocate(VkDevice d, const VkMemoryAllocateInfo *a,
                                          const VkAllocationCallbacks *c, VkDeviceMemory *m) {
    VkResult r = allocate_memory(d, a, c, m);
    if (r == VK_SUCCESS) {
        uint64_t handle = (uint64_t)(uintptr_t)*m;
        if (!track_allocate(&tracker, handle, a->allocationSize)) abort();
        fprintf(stderr, "ALLOCATE {\"id\":%" PRIu64 ",\"bytes\":%" PRIu64 ",\"type\":%u}\n",
            handle, (uint64_t)a->allocationSize, a->memoryTypeIndex);
    }
    return r;
}
static void VKAPI_CALL trace_free(VkDevice d, VkDeviceMemory m, const VkAllocationCallbacks *c) {
    if (m) {
        uint64_t handle = (uint64_t)(uintptr_t)m;
        if (!track_free(&tracker, handle)) abort();
        fprintf(stderr, "FREE {\"id\":%" PRIu64 "}\n", handle);
    }
    free_memory(d, m, c);
}
static PFN_vkVoidFunction wrap(const char *name, PFN_vkVoidFunction f) {
    if (!f) return f;
    if (!strcmp(name, "vkAllocateMemory")) {
        allocate_memory = (PFN_vkAllocateMemory)f; return (PFN_vkVoidFunction)trace_allocate;
    }
    if (!strcmp(name, "vkFreeMemory")) {
        free_memory = (PFN_vkFreeMemory)f; return (PFN_vkVoidFunction)trace_free;
    }
    return f;
}
static PFN_vkVoidFunction VKAPI_CALL trace_device(VkDevice d, const char *name) {
    return wrap(name, get_device(d, name));
}
VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance i, const char *name) {
    if (!get_instance) {
        const char *path = getenv("OGPU_TRACE_LOADER");
        if (!path) abort();
        void *library = dlopen(path, RTLD_NOW | RTLD_LOCAL);
        if (!library) abort();
        get_instance = (PFN_vkGetInstanceProcAddr)dlsym(library, "vkGetInstanceProcAddr");
        if (!get_instance || get_instance == vkGetInstanceProcAddr) abort();
    }
    PFN_vkVoidFunction f = get_instance(i, name);
    if (f && !strcmp(name, "vkGetDeviceProcAddr")) {
        get_device = (PFN_vkGetDeviceProcAddr)f; return (PFN_vkVoidFunction)trace_device;
    }
    return wrap(name, f);
}
VKAPI_ATTR void VKAPI_CALL vkDestroyInstance(VkInstance i, const VkAllocationCallbacks *a) {
    if (!get_instance) abort();
    PFN_vkDestroyInstance destroy = (PFN_vkDestroyInstance)get_instance(i, "vkDestroyInstance");
    if (!destroy) abort();
    destroy(i, a);
    fprintf(stderr, "MEMORY_SUMMARY {\"allocations\":%" PRIu64 ",\"frees\":%" PRIu64
        ",\"peak_bytes\":%" PRIu64 ",\"peak_count\":%" PRIu64 ",\"live_bytes\":%" PRIu64
        ",\"live_count\":%" PRIu64 "}\n", tracker.allocations, tracker.frees, tracker.peak_bytes,
        tracker.peak_count, tracker.live_bytes, tracker.live_count);
}
