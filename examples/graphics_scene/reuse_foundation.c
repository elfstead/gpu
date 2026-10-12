/* One/two-slot reset/replay through explicit foundation lifetimes. */
#define SCENE_REUSE_CONSUMER
#ifdef SCENE_RANGE_REUSE
static unsigned frontier_records,frontier_strategy;
#define SCENE_DRAW_BYTES (132u+20u*frontier_records)
#define SCENE_FOUNDATION_FRONTIER
#endif
#include "foundation.c"
#ifdef SCENE_RANGE_REUSE
#include "range_reuse.h"
#else
#include "reuse.h"
#endif
typedef struct {
    Scene scene; /* Context/executables borrowed, all other objects owned. */
    ogpu_next_list *list;
    unsigned frame;
} Slot;
static int slot_record(Slot *slot,Reuse *r) {
    Scene *s=&slot->scene; NEED(!s->pending && begin(s));
    dependency(s,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_HOST_WRITE,OGPU_NEXT_ACCESS_SHADER_READ);
    commands(s,r->mode);
    ogpu_next_status status=ogpu_next_commands_end(s->encoder,&slot->list); s->encoder=NULL;
    GPU(status); ++r->encodes; return 1;
}
static int slot_create(Slot *slot,Scene *shared,Reuse *r) {
    Scene *s=&slot->scene; s->device=shared->device; s->queue=shared->queue; s->domain=shared->domain;
    s->prepare=shared->prepare; memcpy(s->raster,shared->raster,sizeof(s->raster));
    s->width=r->width; s->height=r->height; s->pixels=(size_t)r->width*r->height*4; s->index_bytes=r->index_bytes;
    NEED(storage(s) && resources(s) && buffer_create(s,CONTROL,REUSE_CONTROL_BYTES,OGPU_NEXT_USAGE_STORAGE,OGPU_NEXT_MEMORY_HOST_VISIBLE));
    s->reuse_root[0]=s->root.vertices; s->reuse_root[1]=s->root.indices; s->reuse_root[2]=s->root.draws;
    GPU(ogpu_next_memory_address(span(s,CONTROL,GUARD,REUSE_CONTROL_BYTES-2*GUARD),&s->reuse_root[3]));
    if(r->replay) NEED(slot_record(slot,r));
    return 1;
}
static int slot_submit(Slot *slot,Reuse *r,unsigned frame) {
    Scene *s=&slot->scene; NEED(!s->pending && s->serial<UINT64_MAX);
    reuse_control(s->maps[CONTROL].data,reuse_variant(frame/r->slots));
    GPU(ogpu_next_memory_flush(span(s,CONTROL,0,REUSE_CONTROL_BYTES)));
    if(!r->replay) NEED(slot_record(slot,r));
    ogpu_next_sync_point signal={{s->done,++s->serial},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit={HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&slot->list,NULL,&signal,s->ss,s->submit_scratch.size};
    ogpu_next_status status=ogpu_next_queue_submit(s->queue,&submit);
    s->pending=status==OGPU_NEXT_OK || status==OGPU_NEXT_BACKEND_ERROR || status==OGPU_NEXT_DEVICE_LOST;
    /* Unknown submission acceptance cannot be drained by waiting an unproven
     * signal. Fail-stop without releasing ANY slot or shared executable. */
    if(status==OGPU_NEXT_BACKEND_ERROR || status==OGPU_NEXT_DEVICE_LOST) {
        fputs("Unknown foundation scene submission; refusing unsafe teardown\n",stderr); _Exit(EXIT_FAILURE);
    }
    GPU(status); slot->frame=frame; reuse_submitted(r); return 1;
}
static int slot_wait(Slot *slot) {
    Scene *s=&slot->scene;
    if(!s->pending) return 1;
    GPU(ogpu_next_timeline_wait((ogpu_next_point){s->done,s->serial},UINT64_C(10000000000)));
    s->pending=0; return 1;
}
static int slot_observe(Slot *slot,Reuse *r) {
    Scene *s=&slot->scene; NEED(s->pending && slot_wait(slot));
    if(!r->replay) { GPU(ogpu_next_arena_reset(s->arena)); slot->list=NULL; }
    GPU(ogpu_next_memory_invalidate(span(s,READBACK,0,s->sizes[READBACK])));
    GPU(ogpu_next_memory_invalidate(span(s,GEOMETRY,0,REUSE_MESH_BYTES)));
    GPU(ogpu_next_memory_invalidate(span(s,CONTROL,0,REUSE_CONTROL_BYTES)));
    return reuse_check(r,slot->frame,s->maps[READBACK].data,s->maps[GEOMETRY].data,s->maps[CONTROL].data);
}
int main(int argc,char **argv) {
    Reuse r={0}; Scene shared={0}; Slot slots[2]={0}; int okay=0;
    if(!reuse_config(&r,argc,argv) || !reuse_reference(&r)) goto done;
    shared.index_bytes=r.index_bytes;
    if(!context(&shared,r.shaders)) goto done;
    for(unsigned i=0;i<r.slots;++i) if(!slot_create(&slots[i],&shared,&r)) goto done;
    if(!reuse_mark(0)) goto done;
    for(unsigned f=0;f<r.frames;++f) {
        Slot *slot=&slots[f%r.slots];
        if(f>=r.slots && !slot_observe(slot,&r)) goto done;
        if(!slot_submit(slot,&r,f)) goto done;
    }
    for(unsigned f=r.frames-r.slots;f<r.frames;++f) if(!slot_observe(&slots[f%r.slots],&r)) goto done;
    okay=reuse_mark(1) && reuse_summary(&r);
done:
    /* Known accepted work in every slot drains before any slot destruction.
     * Failed observation is not completion; retrying a real wait is allowed. */
    for(unsigned i=0;i<2;++i) if(!slot_wait(&slots[i])) {
        fputs("Unknown foundation scene completion; refusing unsafe teardown\n",stderr); _Exit(EXIT_FAILURE);
    }
    for(unsigned i=0;i<2;++i) destroy_resources(&slots[i].scene);
    destroy_context(&shared); reuse_free(&r);
    if(okay) puts("Public scene reuse drained and checked");
    return !okay;
}
