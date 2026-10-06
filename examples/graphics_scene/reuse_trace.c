/* Diagnostic only: actual command/storage calls plus existing memory accounting. */
#define vkGetInstanceProcAddr memory_instance
#define vkDestroyInstance memory_destroy_instance
#include "../learned_image/trace_memory.c"
#undef vkGetInstanceProcAddr
#undef vkDestroyInstance

#define COMMANDS(X) X(vkCreateCommandPool) X(vkDestroyCommandPool) X(vkResetCommandPool) \
    X(vkBeginCommandBuffer) X(vkEndCommandBuffer) X(vkQueueSubmit2) X(vkQueueWaitIdle) \
    X(vkCmdBeginRendering) X(vkCmdEndRendering) X(vkCmdDrawIndexedIndirect2KHR) X(vkCmdDrawIndexedIndirectCount2KHR) \
    X(vkCmdBindPipeline) X(vkCmdPushDataEXT) X(vkCmdBindIndexBuffer3KHR) X(vkCmdPipelineBarrier2) \
    X(vkCmdSetViewport) X(vkCmdSetScissor)
#define COUNT_FIELD(name) uint64_t name;
static struct { COMMANDS(COUNT_FIELD) } counts;
#undef COUNT_FIELD
#define RESULT_HOOK(name,args,call) static PFN_##name real_##name; \
    static VkResult VKAPI_CALL traced_##name args { ++counts.name; return real_##name call; }
#define VOID_HOOK(name,args,call) static PFN_##name real_##name; \
    static void VKAPI_CALL traced_##name args { ++counts.name; real_##name call; }
RESULT_HOOK(vkCreateCommandPool,(VkDevice d,const VkCommandPoolCreateInfo *i,const VkAllocationCallbacks *a,VkCommandPool *o),(d,i,a,o))
VOID_HOOK(vkDestroyCommandPool,(VkDevice d,VkCommandPool p,const VkAllocationCallbacks *a),(d,p,a))
RESULT_HOOK(vkResetCommandPool,(VkDevice d,VkCommandPool p,VkCommandPoolResetFlags f),(d,p,f))
#ifdef SCENE_ARGUMENT_TRACE
static uint64_t push_bytes,indexed_forward_steps,indexed_backward_steps,indexed_other_steps;
static VkDeviceAddress previous_indexed_address;
static PFN_vkBeginCommandBuffer real_vkBeginCommandBuffer;
static VkResult VKAPI_CALL traced_vkBeginCommandBuffer(VkCommandBuffer c,const VkCommandBufferBeginInfo *i) {
    previous_indexed_address=0;++counts.vkBeginCommandBuffer;return real_vkBeginCommandBuffer(c,i);
}
#else
RESULT_HOOK(vkBeginCommandBuffer,(VkCommandBuffer c,const VkCommandBufferBeginInfo *i),(c,i))
#endif
RESULT_HOOK(vkEndCommandBuffer,(VkCommandBuffer c),(c))
RESULT_HOOK(vkQueueSubmit2,(VkQueue q,uint32_t n,const VkSubmitInfo2 *s,VkFence f),(q,n,s,f))
RESULT_HOOK(vkQueueWaitIdle,(VkQueue q),(q))
VOID_HOOK(vkCmdBeginRendering,(VkCommandBuffer c,const VkRenderingInfo *i),(c,i))
VOID_HOOK(vkCmdEndRendering,(VkCommandBuffer c),(c))
VOID_HOOK(vkCmdBindPipeline,(VkCommandBuffer c,VkPipelineBindPoint b,VkPipeline p),(c,b,p))
#ifdef SCENE_ARGUMENT_TRACE
static PFN_vkCmdPushDataEXT real_vkCmdPushDataEXT;
static void VKAPI_CALL traced_vkCmdPushDataEXT(VkCommandBuffer c,const VkPushDataInfoEXT *i) {
    ++counts.vkCmdPushDataEXT;push_bytes+=i->data.size;real_vkCmdPushDataEXT(c,i);
}
#else
VOID_HOOK(vkCmdPushDataEXT,(VkCommandBuffer c,const VkPushDataInfoEXT *i),(c,i))
#endif
VOID_HOOK(vkCmdBindIndexBuffer3KHR,(VkCommandBuffer c,const VkBindIndexBuffer3InfoKHR *i),(c,i))
VOID_HOOK(vkCmdPipelineBarrier2,(VkCommandBuffer c,const VkDependencyInfo *i),(c,i))
VOID_HOOK(vkCmdSetViewport,(VkCommandBuffer c,uint32_t f,uint32_t n,const VkViewport *p),(c,f,n,p))
VOID_HOOK(vkCmdSetScissor,(VkCommandBuffer c,uint32_t f,uint32_t n,const VkRect2D *p),(c,f,n,p))
static uint64_t indexed_records, counted_capacity;
static PFN_vkCmdDrawIndexedIndirect2KHR real_vkCmdDrawIndexedIndirect2KHR;
static void VKAPI_CALL traced_vkCmdDrawIndexedIndirect2KHR(VkCommandBuffer c,const VkDrawIndirect2InfoKHR *i) {
    ++counts.vkCmdDrawIndexedIndirect2KHR;indexed_records+=i->drawCount;
#ifdef SCENE_ARGUMENT_TRACE
    /* This probe records one command buffer at a time, with nonzero addresses.
     * Reset at begin, so slot addresses cannot masquerade as order changes. */
    if(previous_indexed_address) {
        if(i->addressRange.address==previous_indexed_address+20) ++indexed_forward_steps;
        else if(previous_indexed_address==i->addressRange.address+20) ++indexed_backward_steps;
        else ++indexed_other_steps;
    }
    previous_indexed_address=i->addressRange.address;
#endif
    real_vkCmdDrawIndexedIndirect2KHR(c,i);
}
static PFN_vkCmdDrawIndexedIndirectCount2KHR real_vkCmdDrawIndexedIndirectCount2KHR;
static void VKAPI_CALL traced_vkCmdDrawIndexedIndirectCount2KHR(VkCommandBuffer c,const VkDrawIndirectCount2InfoKHR *i) {
    ++counts.vkCmdDrawIndexedIndirectCount2KHR;counted_capacity+=i->maxDrawCount;
    real_vkCmdDrawIndexedIndirectCount2KHR(c,i);
}

void scene_trace_snapshot(unsigned phase) {
    fprintf(stderr,"COMMAND_COUNTS {\"phase\":%u",phase);
#define PRINT(name) fprintf(stderr,",\"" #name "\":%" PRIu64,counts.name);
    COMMANDS(PRINT)
#undef PRINT
    fprintf(stderr,",\"indexed_records\":%" PRIu64 ",\"counted_capacity\":%" PRIu64,indexed_records,counted_capacity);
    fputs("}\n",stderr);
#ifdef SCENE_ARGUMENT_TRACE
    fprintf(stderr,"ARGUMENT_COMMANDS {\"phase\":%u,\"push_bytes\":%" PRIu64 ",\"forward\":%" PRIu64
        ",\"backward\":%" PRIu64 ",\"other\":%" PRIu64 "}\n",phase,push_bytes,indexed_forward_steps,indexed_backward_steps,indexed_other_steps);
#endif
}
static PFN_vkGetDeviceProcAddr command_device_get;
static PFN_vkVoidFunction command_wrap(const char *name, PFN_vkVoidFunction f) {
    if(!f) return f;
#define HOOK(command) if(!strcmp(name,#command)) { real_##command=(PFN_##command)f;return (PFN_vkVoidFunction)traced_##command; }
    COMMANDS(HOOK)
#undef HOOK
    return f;
}
static PFN_vkVoidFunction VKAPI_CALL command_device(VkDevice d,const char *name) {
    return command_wrap(name,command_device_get(d,name));
}
VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance i,const char *name) {
    PFN_vkVoidFunction f=memory_instance(i,name);
    if(f && !strcmp(name,"vkGetDeviceProcAddr")) {
        command_device_get=(PFN_vkGetDeviceProcAddr)f;return (PFN_vkVoidFunction)command_device;
    }
    return command_wrap(name,f);
}
VKAPI_ATTR void VKAPI_CALL vkDestroyInstance(VkInstance i,const VkAllocationCallbacks *a) {
    memory_destroy_instance(i,a);scene_trace_snapshot(2);
}
