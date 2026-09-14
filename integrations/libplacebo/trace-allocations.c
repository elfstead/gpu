// Loader shim for the single-device, serialized diagnostic process.
// Select with OGPU_VULKAN_LIBRARY; point OGPU_TRACE_LOADER at the real loader.
// Default is read-only. OGPU_DIAGNOSTIC_IMAGE_LOCAL=1 tests non-host-visible
// DEVICE_LOCAL image placement, restricted to the actual memoryTypeBits.
// Never linked into the runtime or the normal benchmark.
#include <vulkan/vulkan.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static PFN_vkGetInstanceProcAddr get_instance;
static PFN_vkGetDeviceProcAddr get_device;
static PFN_vkGetPhysicalDeviceMemoryProperties memory_properties;
static PFN_vkAllocateMemory allocate;
static PFN_vkGetImageMemoryRequirements image_requirements;
static VkPhysicalDeviceMemoryProperties memory;
static struct { VkImage image; uint32_t mask; } requirements[64];
static void VKAPI_CALL trace_requirements(VkDevice d,VkImage image,VkMemoryRequirements *r)
{
    image_requirements(d,image,r);
    unsigned i=0; while(i<64 && requirements[i].image && requirements[i].image!=image) ++i;
    if(i==64) abort(); requirements[i].image=image; requirements[i].mask=r->memoryTypeBits;
}
VKAPI_ATTR void VKAPI_CALL vkDestroyInstance(VkInstance i,const VkAllocationCallbacks *a)
{
    if(!get_instance) abort();
    PFN_vkDestroyInstance destroy=(PFN_vkDestroyInstance)get_instance(i,"vkDestroyInstance");
    if(!destroy) abort();
    destroy(i,a);
}
static void VKAPI_CALL trace_properties(VkPhysicalDevice p,VkPhysicalDeviceMemoryProperties *m)
{
    memory_properties(p,m); memory=*m;
    for(uint32_t i=0;i<m->memoryTypeCount;++i) fprintf(stderr,"MEMORY type=%u flags=0x%x heap=%u heap_bytes=%llu\n",
        i,m->memoryTypes[i].propertyFlags,m->memoryTypes[i].heapIndex,
        (unsigned long long)m->memoryHeaps[m->memoryTypes[i].heapIndex].size);
}
static VkResult VKAPI_CALL trace_allocate(VkDevice d,const VkMemoryAllocateInfo *a,const VkAllocationCallbacks *c,VkDeviceMemory *m)
{
    VkImage image=VK_NULL_HANDLE;
    for(const VkBaseInStructure *p=a->pNext;p;p=p->pNext) if(p->sType==VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO)
        image=((const VkMemoryDedicatedAllocateInfo *)p)->image;
    VkMemoryAllocateInfo copy=*a;
    const char *control=getenv("OGPU_DIAGNOSTIC_IMAGE_LOCAL");
    if(control && strcmp(control,"1")) abort();
    if(image && control) {
        unsigned i=0; while(i<64 && requirements[i].image!=image) ++i; if(i==64) abort();
        uint32_t type=0;
        for(;type<memory.memoryTypeCount;++type) {
            VkMemoryPropertyFlags flags=memory.memoryTypes[type].propertyFlags;
            if((requirements[i].mask & (1u<<type)) && (flags & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT) &&
               !(flags & (VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD |
                          VK_MEMORY_PROPERTY_PROTECTED_BIT | VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT))) break;
        }
        if(type==memory.memoryTypeCount) abort(); // diagnostic unsupported, no fallback
        copy.memoryTypeIndex=type;
        fprintf(stderr,"IMAGE_LOCAL original=%u selected=%u mask=0x%x\n",a->memoryTypeIndex,type,requirements[i].mask);
    }
    fprintf(stderr,"ALLOC bytes=%llu type=%u flags=0x%x dedicated_image=%u\n",
        (unsigned long long)a->allocationSize,a->memoryTypeIndex,
        memory.memoryTypes[a->memoryTypeIndex].propertyFlags,image!=VK_NULL_HANDLE);
    return allocate(d,&copy,c,m);
}
static PFN_vkVoidFunction VKAPI_CALL trace_device(VkDevice d,const char *name)
{
    PFN_vkVoidFunction f=get_device(d,name);
    if(!strcmp(name,"vkAllocateMemory")) { allocate=(PFN_vkAllocateMemory)f; return (PFN_vkVoidFunction)trace_allocate; }
    if(!strcmp(name,"vkGetImageMemoryRequirements")) { image_requirements=(PFN_vkGetImageMemoryRequirements)f; return (PFN_vkVoidFunction)trace_requirements; }
    return f;
}
VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance i,const char *name)
{
    if(!get_instance) {
        const char *path=getenv("OGPU_TRACE_LOADER"); if(!path) abort();
        void *lib=dlopen(path,RTLD_NOW|RTLD_LOCAL); if(!lib) abort();
        get_instance=(PFN_vkGetInstanceProcAddr)dlsym(lib,"vkGetInstanceProcAddr");
        if(!get_instance || get_instance==vkGetInstanceProcAddr) abort();
    }
    PFN_vkVoidFunction f=get_instance(i,name);
    if(!strcmp(name,"vkAllocateMemory")) { allocate=(PFN_vkAllocateMemory)f; return (PFN_vkVoidFunction)trace_allocate; }
    if(!strcmp(name,"vkGetImageMemoryRequirements")) { image_requirements=(PFN_vkGetImageMemoryRequirements)f; return (PFN_vkVoidFunction)trace_requirements; }
    if(!strcmp(name,"vkGetDeviceProcAddr")) { get_device=(PFN_vkGetDeviceProcAddr)f; return (PFN_vkVoidFunction)trace_device; }
    if(!strcmp(name,"vkGetPhysicalDeviceMemoryProperties")) { memory_properties=(PFN_vkGetPhysicalDeviceMemoryProperties)f; return (PFN_vkVoidFunction)trace_properties; }
    return f;
}
