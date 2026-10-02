/* Same validated slot encoders; CPU verification is outside timed windows.
 * GPU color/depth/geometry readback copies remain in every command sequence. */
#define _POSIX_C_SOURCE 200809L
#define SCENE_RANGE_REUSE
#define SCENE_REUSE_MAIN range_correctness_main
#ifdef RANGE_NATIVE
#define NATIVE_DRAW_IDENTITY
#include "reuse_native.c"
#else
#include "reuse_public.c"
#endif
#include <time.h>
typedef struct { double start,record_submit,wait,latency; } Sample;
static double clock_ms(void) {
    struct timespec now;if(clock_gettime(CLOCK_MONOTONIC,&now)) abort();
    return now.tv_sec*1000.0+now.tv_nsec/1000000.0;
}
static int timing_retire(Slot *slot,Reuse *r,Sample *samples) {
    Scene *s=&slot->scene;Sample *sample=&samples[slot->frame];double start=clock_ms();
#ifdef RANGE_NATIVE
    Native *n=s->n;NativeBatch *b=&s->batch;NEED(b->pending);
    VkSemaphoreWaitInfo wait={.sType=VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,.semaphoreCount=1,
        .pSemaphores=&n->timeline,.pValues=&b->value};
    VK_TRY(n->vkWaitSemaphores(n->device,&wait,UINT64_MAX));
    n->observed_value=b->value;b->pending=0;b->value=0;
#else
    NEED(s->completion);GPU(ogpu_completion_wait(s->completion,&s->error));
    ogpu_completion_destroy(s->completion);s->completion=NULL;
#endif
    double end=clock_ms();sample->wait=end-start;sample->latency=end-sample->start;
    NEED(r->pending && slot->frame==r->checked);--r->pending;++r->checked;return 1;
}
static int timing_verify(Slot *slot,Reuse *r) {
    Scene *s=&slot->scene;unsigned char mesh[REUSE_MESH_BYTES],control[REUSE_CONTROL_BYTES];
#ifdef RANGE_NATIVE
    NEED(native_read(s->n,&s->readback,0,s->cpu,r->image_bytes) && native_read(s->n,&s->geometry,0,mesh,sizeof(mesh))
        && native_read(s->n,&slot->control,0,control,sizeof(control)));
#else
    GPU(ogpu_buffer_read(s->readback,0,s->cpu,r->image_bytes,&s->error));
    GPU(ogpu_buffer_read(s->geometry,0,mesh,sizeof(mesh),&s->error));
    GPU(ogpu_buffer_read(slot->control,0,control,sizeof(control),&s->error));
#endif
    return reuse_verify(r,slot->frame,s->cpu,mesh,control);
}
static int window(Slot slots[2],Reuse *r,unsigned frames,Sample *samples,double *wall) {
    r->submitted=r->checked=r->pending=r->peak=0;r->encodes=r->replay ? r->slots : 0;
    double start=clock_ms();
    for(unsigned f=0;f<frames;++f) {
        Slot *slot=&slots[f%r->slots];
        if(f>=r->slots) NEED(timing_retire(slot,r,samples));
        samples[f].start=clock_ms();NEED(slot_submit(slot,r,f));
        samples[f].record_submit=clock_ms()-samples[f].start;
    }
    for(unsigned f=frames-r->slots;f<frames;++f) NEED(timing_retire(&slots[f%r->slots],r,samples));
    *wall=clock_ms()-start;
    NEED(r->submitted==frames && r->checked==frames && !r->pending && r->peak==r->slots
        && r->encodes==(r->replay ? r->slots : frames));
    /* Post-window full image/geometry/count/input checking of every final slot. */
    for(unsigned i=0;i<r->slots;++i) NEED(timing_verify(&slots[i],r));
    return 1;
}
int main(int argc,char **argv) {
    Reuse r={0};Scene context={0};Slot slots[2]={0};Sample *samples=NULL;int okay=0;
    if(!reuse_config(&r,argc,argv) || !reuse_reference(&r)) goto done;
    if(getenv("OGPU_SCENE_TRACE") || getenv("LD_PRELOAD") ||
        (getenv("VK_INSTANCE_LAYERS") && getenv("VK_INSTANCE_LAYERS")[0])) goto done;
    samples=calloc(r.frames>100 ? r.frames : 100,sizeof(*samples));if(!samples) goto done;
    double setup=clock_ms(),wall=0;context.index_bytes=r.index_bytes;
    if(!scene_create_context(&context,r.shaders)) goto done;
#ifdef RANGE_NATIVE
    PFN_vkGetInstanceProcAddr get=(PFN_vkGetInstanceProcAddr)dlsym(context.n->library,"vkGetInstanceProcAddr");
    reset_pool=(PFN_vkResetCommandPool)get(context.n->instance,"vkResetCommandPool");
    draw_count=(PFN_vkCmdDrawIndexedIndirectCount2KHR)get(context.n->instance,"vkCmdDrawIndexedIndirectCount2KHR");
    if(!reset_pool || !draw_count) goto done;
#endif
    for(unsigned i=0;i<r.slots;++i) if(!slot_create(&slots[i],&context,&r)) goto done;
    setup=clock_ms()-setup;
    if(!window(slots,&r,100,samples,&wall)) goto done;
    memset(samples,0,(r.frames>100 ? r.frames : 100)*sizeof(*samples));
    if(!window(slots,&r,r.frames,samples,&wall)) goto done;
    printf("RANGE_TIMING {\"width\":%u,\"height\":%u,\"frames\":%u,\"warmups\":100,\"slots\":%u,\"capacity\":%u,\"strategy\":%u,\"replay\":%u,"
        "\"setup_ms\":%.9f,\"wall_ms\":%.9f,\"peak_unretired\":%u,\"encodes\":%u}\n",
        r.width,r.height,r.frames,r.slots,frontier_records,frontier_strategy,r.replay,setup,wall,r.peak,r.encodes);
    for(unsigned i=0;i<r.frames;++i) printf("SAMPLE {\"index\":%u,\"record_submit_ms\":%.9f,\"wait_ms\":%.9f,\"retirement_ms\":%.9f}\n",
        i,samples[i].record_submit,samples[i].wait,samples[i].latency);
    okay=1;
done:
#ifdef RANGE_NATIVE
    for(unsigned i=0;i<2;++i) if(slots[i].scene.n) native_batch_destroy(slots[i].scene.n,&slots[i].scene.batch);
    for(unsigned i=0;i<2;++i) if(slots[i].scene.n) {
        native_buffer_destroy(slots[i].scene.n,&slots[i].control);scene_destroy_resources(&slots[i].scene);
    }
    if(context.n) scene_destroy_context(&context);
#else
    for(unsigned i=0;i<2;++i) { ogpu_completion_destroy(slots[i].scene.completion);slots[i].scene.completion=NULL; }
    for(unsigned i=0;i<2;++i) {
        ogpu_command_list_destroy(slots[i].list);scene_destroy_resources(&slots[i].scene);
        ogpu_buffer_destroy(slots[i].control);ogpu_recording_storage_destroy(slots[i].storage);
    }
    scene_destroy_context(&context);
#endif
    free(samples);reuse_free(&r);
    if(okay) puts("Range timing final-slot bytes checked and all work drained");return !okay;
}
