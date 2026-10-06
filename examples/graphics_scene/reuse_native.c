#define SCENE_REUSE_CONSUMER
#ifdef SCENE_RANGE_REUSE
#define FRONTIER_REUSE
#include "frontier.c"
#include "range_reuse.h"
#else
#include "native.c"
#include "reuse.h"
#endif

typedef struct {
    Scene scene; /* Borrowed context/executables, owned attachments/buffers/pool. */
    NativeBuffer control;
    uint64_t root[4];
    unsigned frame;
} Slot;
static PFN_vkResetCommandPool reset_pool;

static int slot_record(Slot *slot, Reuse *r) {
    Scene *s=&slot->scene;Native *n=s->n;NativeBatch *b=&s->batch;
    NEED(!b->pending && !b->value);
    if(!b->pool) NEED(native_begin(n,b));
    else {
        VK_TRY(reset_pool(n->device,b->pool,0));
        VkCommandBufferBeginInfo begin={.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
        VK_TRY(n->vkBeginCommandBuffer(b->command,&begin));
        native_barrier(n,b->command,VK_PIPELINE_STAGE_2_HOST_BIT,VK_ACCESS_2_HOST_WRITE_BIT,
            VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,VK_ACCESS_2_MEMORY_READ_BIT|VK_ACCESS_2_MEMORY_WRITE_BIT);
    }
    NEED(scene_commands(s,r->mode,slot->root));
    native_barrier(n,b->command,VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_HOST_BIT,VK_ACCESS_2_HOST_READ_BIT);
    VK_TRY(n->vkEndCommandBuffer(b->command));++r->encodes;return 1;
}
static int slot_create(Slot *slot, Scene *context, Reuse *r) {
    Scene *s=&slot->scene;s->n=context->n;s->prepare=context->prepare;
    memcpy(s->raster,context->raster,sizeof(s->raster));
    s->bind_index=context->bind_index;s->draw_indexed=context->draw_indexed;
    s->width=r->width;s->height=r->height;s->pixels=(size_t)r->width*r->height*4;s->index_bytes=r->index_bytes;
    NEED(scene_create_resources(s) && native_buffer_create(s->n,&slot->control,REUSE_CONTROL_BYTES,1));
    slot->root[0]=s->vertices.address+64;slot->root[1]=s->indices.address+64;
    slot->root[2]=s->draws.address+64;slot->root[3]=slot->control.address+64;
#ifdef SCENE_ARGUMENT_REUSE
    argument_prepare(s->argument_root,slot->root[0]);
#endif
    if(r->replay) NEED(slot_record(slot,r));
    return 1;
}
static int slot_submit(Slot *slot, Reuse *r, unsigned frame) {
    Scene *s=&slot->scene;Native *n=s->n;NativeBatch *b=&s->batch;
    NEED(!b->pending && !b->value && n->next_value<UINT64_MAX && n->next_value-n->observed_value<n->max_difference);
    unsigned char control[REUSE_CONTROL_BYTES];reuse_control(control,reuse_variant(frame/r->slots));
    NEED(native_write(n,&slot->control,0,control,sizeof(control)));
#ifdef SCENE_ARGUMENT_REUSE
    argument_reverse=(frame/r->slots)%2;
#endif
#ifdef SCENE_ARGUMENT_PROFILE
    double record_start=argument_clock();
#endif
    if(!r->replay) NEED(slot_record(slot,r));
#ifdef SCENE_ARGUMENT_PROFILE
    argument_record_ms=argument_clock()-record_start;double submit_start=argument_clock();
#endif
    b->value=++n->next_value;
    VkCommandBufferSubmitInfo command={.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO,.commandBuffer=b->command};
    VkSemaphoreSubmitInfo signal={.sType=VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,.semaphore=n->timeline,
        .value=b->value,.stageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT};
    VkSubmitInfo2 submit={.sType=VK_STRUCTURE_TYPE_SUBMIT_INFO_2,.commandBufferInfoCount=1,.pCommandBufferInfos=&command,
        .signalSemaphoreInfoCount=1,.pSignalSemaphoreInfos=&signal};
    VkResult result=n->vkQueueSubmit2(n->queue,1,&submit,VK_NULL_HANDLE);
    if(result!=VK_SUCCESS) {
        if(result!=VK_ERROR_OUT_OF_HOST_MEMORY && result!=VK_ERROR_OUT_OF_DEVICE_MEMORY && result!=VK_ERROR_DEVICE_LOST) native_drain(n);
        return vk_ok(result,"reuse submit");
    }
#ifdef SCENE_ARGUMENT_PROFILE
    argument_submit_ms=argument_clock()-submit_start;
#endif
    b->pending=1;slot->frame=frame;reuse_submitted(r);return 1;
}
static int slot_observe(Slot *slot, Reuse *r) {
    Scene *s=&slot->scene;Native *n=s->n;NativeBatch *b=&s->batch;NEED(b->pending);
    VkSemaphoreWaitInfo wait={.sType=VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,.semaphoreCount=1,
        .pSemaphores=&n->timeline,.pValues=&b->value};
    VK_TRY(n->vkWaitSemaphores(n->device,&wait,UINT64_MAX));
    n->observed_value=b->value;b->pending=0;b->value=0;
    unsigned char mesh[REUSE_MESH_BYTES],control[REUSE_CONTROL_BYTES];
    NEED(native_read(n,&s->readback,0,s->cpu,r->image_bytes) && native_read(n,&s->geometry,0,mesh,sizeof(mesh))
        && native_read(n,&slot->control,0,control,sizeof(control)));
    return reuse_check(r,slot->frame,s->cpu,mesh,control);
}
#ifndef SCENE_REUSE_MAIN
#define SCENE_REUSE_MAIN main
#endif
int SCENE_REUSE_MAIN(int argc, char **argv) {
    Reuse r={0};Scene context={0};Slot slots[2]={0};int okay=0;
    if(!reuse_config(&r,argc,argv) || !reuse_reference(&r)) goto done;
    if(!r.slots || r.slots>2 || r.frames<r.slots) goto done;
    context.index_bytes=r.index_bytes;
    if(!scene_create_context(&context,r.shaders)) goto done;
    PFN_vkGetInstanceProcAddr get=(PFN_vkGetInstanceProcAddr)dlsym(context.n->library,"vkGetInstanceProcAddr");
#ifdef SCENE_RANGE_REUSE
    draw_count=(PFN_vkCmdDrawIndexedIndirectCount2KHR)get(context.n->instance,"vkCmdDrawIndexedIndirectCount2KHR");
    if(!draw_count) goto done;
#endif
    reset_pool=(PFN_vkResetCommandPool)get(context.n->instance,"vkResetCommandPool");if(!reset_pool) goto done;
    for(unsigned i=0;i<r.slots;++i) if(!slot_create(&slots[i],&context,&r)) goto done;
    if(!reuse_mark(0)) goto done;
    for(unsigned f=0;f<r.frames;++f) {
        Slot *slot=&slots[f%r.slots];
        if(f>=r.slots && !slot_observe(slot,&r)) goto done;
        if(!slot_submit(slot,&r,f)) goto done;
    }
    for(unsigned f=r.frames-r.slots;f<r.frames;++f) if(!slot_observe(&slots[f%r.slots],&r)) goto done;
    okay=reuse_mark(1) && reuse_summary(&r);
done:
    /* Wait/drain every slot BEFORE freeing any shared or pointer-referenced data.
       The old one-shot helper is used only for teardown, not hot-path retirement. */
    for(unsigned i=0;i<2;++i) if(slots[i].scene.n) native_batch_destroy(slots[i].scene.n,&slots[i].scene.batch);
    for(unsigned i=0;i<2;++i) if(slots[i].scene.n) {
        native_buffer_destroy(slots[i].scene.n,&slots[i].control);scene_destroy_resources(&slots[i].scene);
    }
    if(context.n) scene_destroy_context(&context);reuse_free(&r);
    if(okay) puts("Native scene reuse drained and checked");return !okay;
}
