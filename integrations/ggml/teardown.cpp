// Minimal public-API lifecycle reproducer, deliberately independent of GGML.
#ifdef DIRECT_VULKAN
#include <cstdlib>
#include <dlfcn.h>
#include <vector>
#include <vulkan/vulkan.h>
#else
#include "ogpu.h"
#endif
#include <cstdio>
#include <cstdlib>
#include <cstring>

struct Runtime {
#ifdef DIRECT_VULKAN
    void *library = nullptr;
    VkInstance probe = nullptr;
    VkDevice device = nullptr;
    PFN_vkDestroyDevice destroy_device = nullptr;
    PFN_vkDestroyInstance destroy_instance = nullptr;
    bool create() {
        library = dlopen(std::getenv("OGPU_VULKAN_LIBRARY"), RTLD_NOW | RTLD_LOCAL);
        if (!library)
            return false;
        auto create_instance =
            reinterpret_cast<PFN_vkCreateInstance>(dlsym(library, "vkCreateInstance"));
        auto get =
            reinterpret_cast<PFN_vkGetInstanceProcAddr>(dlsym(library, "vkGetInstanceProcAddr"));
        VkApplicationInfo app{};
        app.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;
        app.apiVersion = VK_API_VERSION_1_2;
        VkInstanceCreateInfo instance{};
        instance.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
        instance.pApplicationInfo = &app;
        if (!create_instance || !get || create_instance(&instance, nullptr, &probe))
            return false;
        destroy_instance = reinterpret_cast<PFN_vkDestroyInstance>(get(probe, "vkDestroyInstance"));
        auto enumerate = reinterpret_cast<PFN_vkEnumeratePhysicalDevices>(
            get(probe, "vkEnumeratePhysicalDevices"));
        auto queues = reinterpret_cast<PFN_vkGetPhysicalDeviceQueueFamilyProperties>(
            get(probe, "vkGetPhysicalDeviceQueueFamilyProperties"));
        auto create_device = reinterpret_cast<PFN_vkCreateDevice>(get(probe, "vkCreateDevice"));
        uint32_t count = 0;
        if (enumerate(probe, &count, nullptr) || !count)
            return false;
        std::vector<VkPhysicalDevice> physical(count);
        if (enumerate(probe, &count, physical.data()))
            return false;
        queues(physical[0], &count, nullptr);
        std::vector<VkQueueFamilyProperties> families(count);
        queues(physical[0], &count, families.data());
        uint32_t family = 0;
        while (family < count && !(families[family].queueFlags & VK_QUEUE_COMPUTE_BIT))
            ++family;
        if (family == count)
            return false;
        float priority = 1.0f;
        VkDeviceQueueCreateInfo queue{};
        queue.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO;
        queue.queueFamilyIndex = family;
        queue.queueCount = 1;
        queue.pQueuePriorities = &priority;
        VkDeviceCreateInfo info{};
        info.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO;
        info.queueCreateInfoCount = 1;
        info.pQueueCreateInfos = &queue;
        if (create_device(physical[0], &info, nullptr, &device))
            return false;
        destroy_device = reinterpret_cast<PFN_vkDestroyDevice>(get(probe, "vkDestroyDevice"));
        return true;
    }
#else
    OgpuProbe *probe = nullptr;
    OgpuDevice *device = nullptr;
    bool create() {
        OgpuError error{};
        if (ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error) != OGPU_SUCCESS ||
            ogpu_device_create(probe, 0, &device, &error) != OGPU_SUCCESS) {
            std::fprintf(stderr, "%s\n", error.message);
            return false;
        }
        return true;
    }
#endif
    void clear() {
        std::fprintf(stderr, "destroy device\n");
#ifdef DIRECT_VULKAN
        if (device)
            destroy_device(device, nullptr);
#else
        ogpu_device_destroy(device);
#endif
        device = nullptr;
        std::fprintf(stderr, "destroy probe\n");
#ifdef DIRECT_VULKAN
        if (probe)
            destroy_instance(probe, nullptr);
        if (library)
            dlclose(library);
        library = nullptr;
#else
        ogpu_probe_destroy(probe);
#endif
        probe = nullptr;
    }
    ~Runtime() {
        if (probe || device)
            clear();
    }
};
static Runtime runtime;

int main(int argc, char **argv) {
    if (argc != 2 || (std::strcmp(argv[1], "explicit") && std::strcmp(argv[1], "static") &&
                      std::strcmp(argv[1], "atexit")))
        return 2;
    if (!runtime.create()) {
        runtime.clear();
        return 1;
    }
    if (!std::strcmp(argv[1], "explicit"))
        runtime.clear();
    if (!std::strcmp(argv[1], "atexit"))
        std::atexit([] { runtime.clear(); });
    std::fprintf(stderr, "return from main\n");
}
