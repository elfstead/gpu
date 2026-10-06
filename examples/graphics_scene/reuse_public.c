#define SCENE_REUSE_CONSUMER
#ifdef SCENE_CPU_TRACE
#include "reuse_cpu_trace.h"
#endif
#ifdef SCENE_RANGE_REUSE
#define FRONTIER_REUSE
#include "frontier_public.c"
#include "range_reuse.h"
#else
#include "public.c"
#include "reuse.h"
#endif

typedef struct {
    Scene scene; /* Context objects borrowed; attachments/buffers are owned. */
    OgpuBuffer *control;
    OgpuRecordingStorage *storage;
    OgpuCommandList *list;
    uint64_t root[4];
    unsigned frame;
} Slot;

static int slot_record(Slot *slot, Reuse *r) {
    Scene *s=&slot->scene;
    GPU(ogpu_batch_create_in(slot->storage,&s->batch,&s->error));
    NEED(scene_commands(s,r->mode,slot->root));++r->encodes;return 1;
}
static int slot_create(Slot *slot, Scene *context, Reuse *r) {
    Scene *s=&slot->scene;
    s->device=context->device;s->prepare=context->prepare;memcpy(s->raster,context->raster,sizeof(s->raster));
    s->width=r->width;s->height=r->height;s->pixels=(size_t)r->width*r->height*4;s->index_bytes=r->index_bytes;
    NEED(scene_create_resources(s) && buffer_create(s,&slot->control,REUSE_CONTROL_BYTES,OGPU_MEMORY_HOST,0));
    GPU(ogpu_recording_storage_create(s->device,&slot->storage,&s->error));
    slot->root[0]=s->root.vertices;slot->root[1]=s->root.indices;slot->root[2]=s->root.draws;
    GPU(ogpu_buffer_device_address(slot->control,&slot->root[3],&s->error));slot->root[3]+=64;
#ifdef SCENE_ARGUMENT_REUSE
    argument_prepare(s->argument_root,slot->root[0]);
#endif
    if(r->replay) {
        NEED(slot_record(slot,r));
        GPU(ogpu_batch_compile(s->batch,0,&slot->list,&s->error)); /* serial */
        ogpu_batch_destroy(s->batch);s->batch=NULL;
    }
    return 1;
}
static int slot_submit(Slot *slot, Reuse *r, unsigned frame) {
    Scene *s=&slot->scene;NEED(!s->completion);
    unsigned char control[REUSE_CONTROL_BYTES];reuse_control(control,reuse_variant(frame/r->slots));
    GPU(ogpu_buffer_write(slot->control,0,control,sizeof(control),&s->error));
#ifdef SCENE_ARGUMENT_REUSE
    argument_reverse=(frame/r->slots)%2;
#endif
    if(r->replay) GPU(ogpu_command_list_submit(slot->list,&s->completion,&s->error));
    else {
#ifdef SCENE_ARGUMENT_PROFILE
        double record_start=argument_clock();
#endif
        NEED(slot_record(slot,r));
#ifdef SCENE_ARGUMENT_PROFILE
        argument_record_ms=argument_clock()-record_start;double submit_start=argument_clock();
#endif
        GPU(ogpu_batch_submit(s->batch,&s->completion,&s->error));
#ifdef SCENE_ARGUMENT_PROFILE
        argument_submit_ms=argument_clock()-submit_start;
#endif
        ogpu_batch_destroy(s->batch);s->batch=NULL;
    }
    slot->frame=frame;reuse_submitted(r);return 1;
}
static int slot_observe(Slot *slot, Reuse *r) {
    Scene *s=&slot->scene;NEED(s->completion);
    GPU(ogpu_completion_wait(s->completion,&s->error));
    ogpu_completion_destroy(s->completion);s->completion=NULL;
    unsigned char mesh[REUSE_MESH_BYTES],control[REUSE_CONTROL_BYTES];
    GPU(ogpu_buffer_read(s->readback,0,s->cpu,r->image_bytes,&s->error));
    GPU(ogpu_buffer_read(s->geometry,0,mesh,sizeof(mesh),&s->error));
    GPU(ogpu_buffer_read(slot->control,0,control,sizeof(control),&s->error));
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
    /* All slots drain before any pointer-referenced backing or shared executable
       can disappear, including error paths and partially initialized slots. */
    for(unsigned i=0;i<2;++i) { ogpu_completion_destroy(slots[i].scene.completion);slots[i].scene.completion=NULL; }
    for(unsigned i=0;i<2;++i) {
        ogpu_command_list_destroy(slots[i].list);
        scene_destroy_resources(&slots[i].scene);
        ogpu_buffer_destroy(slots[i].control);ogpu_recording_storage_destroy(slots[i].storage);
    }
    scene_destroy_context(&context);reuse_free(&r);
#ifdef SCENE_CPU_TRACE
    scene_cpu_summary();
#endif
    if(okay) puts("Public scene reuse drained and checked");return !okay;
}
