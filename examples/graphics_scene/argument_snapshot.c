/* Diagnostic only: call-time snapshots, not a timing or reuse-strategy control.
 * Reuse scene resource/dependency code; keep every recorded result observable. */
static unsigned frontier_records=2;
#define SCENE_REUSE_CONSUMER
#define SCENE_ARGUMENT_ROOT
#ifdef ARGUMENT_NATIVE
#define NATIVE_INDEXED_FRONTIER
#define NATIVE_DRAW_IDENTITY
#define SCENE_NATIVE_FRONTIER
#include "native.c"
#else
#define SCENE_PUBLIC_FRONTIER
#include "public.c"
#endif

static unsigned snapshot;
static uint64_t ordinary_address, alternate_address;
static union { uint64_t aligned[32]; unsigned char bytes[256]; } arguments;
static void set_arguments(unsigned changed) {
#ifdef ARGUMENT_PATCH_PAYLOAD
    uint64_t address=ordinary_address;
#else
    uint64_t address=changed ? alternate_address : ordinary_address;
#endif
    memcpy(arguments.bytes,&address,8);
    for(unsigned i=0;i<(SCENE_RASTER_BYTES-8)/4;++i) {
        uint32_t delta=changed;
#ifdef ARGUMENT_PATCH_PAYLOAD
        if(i!=(SCENE_RASTER_BYTES-8)/4-1) delta=0;
#endif
        uint32_t value=i+1+delta;
        memcpy(arguments.bytes+8+4*i,&value,4);
    }
}

#ifdef ARGUMENT_NATIVE
static int frontier_draw(Scene *s,unsigned first) {
    (void)first;
#else
static int frontier_draw(Scene *s,unsigned pipeline,const OgpuIndexRange *indices,unsigned first) {
    (void)first;
#endif
    for(unsigned draw=0;draw<2;++draw) {
#ifndef ARGUMENT_NATIVE
        /* A rejected call must not append work or poison the successful retry.
         * Use a valid address with a deliberately wrong byte count. */
        set_arguments(1);
        OgpuIndirectRange range={.buffer=s->draws,.offset=64+20*draw,.stride_bytes=20,.max_draw_count=1};
        NEED(ogpu_batch_draw_indexed_indirect(s->batch,s->raster[pipeline],indices,&range,
            arguments.bytes,SCENE_RASTER_BYTES-4,&s->error)==OGPU_ERROR_INVALID_ARGUMENT);
#endif
        set_arguments(snapshot==2 && draw==1);
#ifdef ARGUMENT_NATIVE
#ifdef ARGUMENT_NATIVE_PARTIAL
        /* Compute has replaced the beginning of the bank at each scope.
         * Initialize the whole graphics value, then update only the changed word. */
        if(draw==0) NEED(native_push(s->n,&s->batch,&s->raster[0],arguments.bytes,SCENE_RASTER_BYTES));
        else if(snapshot==2) {
            VkPushDataInfoEXT update={.sType=VK_STRUCTURE_TYPE_PUSH_DATA_INFO_EXT,
                .offset=SCENE_RASTER_BYTES-4,.data={arguments.bytes+SCENE_RASTER_BYTES-4,4}};
            s->n->vkCmdPushDataEXT(s->batch.command,&update);
        }
#else
        NEED(native_push(s->n,&s->batch,&s->raster[0],arguments.bytes,SCENE_RASTER_BYTES));
#endif
        VkDrawIndirect2InfoKHR info={.sType=VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
            .addressRange={s->draws.address+64+20*draw,20,20},
            .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,.drawCount=1};
        s->draw_indexed(s->batch.command,&info);
#else
        GPU(ogpu_batch_draw_indexed_indirect(s->batch,s->raster[pipeline],indices,&range,
            arguments.bytes,SCENE_RASTER_BYTES,&s->error));
#endif
        /* Same address, immediately poisoned after every successful call. */
        memset(arguments.bytes,0xa5,sizeof(arguments.bytes));
    }
    return 1;
}

static int execute(Scene *s,const char *shaders) {
#ifdef ARGUMENT_PATCH_PAYLOAD
    _Static_assert(SCENE_RASTER_BYTES==64 || SCENE_RASTER_BYTES==256,"partial payload sizes");
    printf("ARGUMENT_PATCH_POLICY {\"single_word\":true,\"native_partial\":%s}\n",
#ifdef ARGUMENT_NATIVE_PARTIAL
        "true"
#else
        "false"
#endif
    );
#endif
    NEED(scene_create_context(s,shaders));
#ifdef ARGUMENT_NATIVE
    printf("ARGUMENT_LIMIT {\"max_push_data_bytes\":%" PRIu64 "}\n",(uint64_t)s->n->max_push_data);
#else
    OgpuDeviceLimits limits;GPU(ogpu_device_limits(s->device,&limits,&s->error));
    printf("ARGUMENT_LIMIT {\"max_push_data_bytes\":%" PRIu64 "}\n",limits.max_push_data_bytes);
#endif
    NEED(scene_create_resources(s));return 1;
}

int main(int argc,char **argv) {
    if(argc!=3 || (SCENE_RASTER_BYTES!=8 && SCENE_RASTER_BYTES!=64 && SCENE_RASTER_BYTES!=256)) return 1;
    Scene scene={.index_bytes=4,.width=257,.height=193,.pixels=257*193*4};Scene *s=&scene;
    const size_t image_bytes=2*s->pixels+256;
    unsigned char *outputs=NULL;int okay=0;
#ifdef ARGUMENT_NATIVE
    NativeBuffer alternate={0},archive={0};
#else
    OgpuBuffer *alternate=NULL,*archive=NULL;
#endif
    if(!execute(s,argv[1])) goto done;
    outputs=malloc(4*image_bytes);if(!outputs) goto done;memset(outputs,0xa5,4*image_bytes);
    /* A second valid pointee makes pointer mutation observable even at 8 bytes.
     * Same far quad; near quad moves right. Explicit table independent of shader. */
    const float vertices[]={-.75f,-.75f,.75f,1,.75f,-.75f,.75f,1,.75f,.75f,.75f,1,-.75f,.75f,.75f,1,
        -.125f,-.375f,.25f,1,.375f,-.375f,.25f,1,.375f,.375f,.25f,1,-.125f,.375f,.25f,1};
#ifdef ARGUMENT_NATIVE
    if(!native_buffer_create(s->n,&alternate,sizeof(vertices),1) || !native_buffer_create(s->n,&archive,4*image_bytes,1)
        || !native_write(s->n,&alternate,0,vertices,sizeof(vertices)) || !native_write(s->n,&archive,0,outputs,4*image_bytes)) goto done;
    ordinary_address=s->vertices.address+64;alternate_address=alternate.address;
    if(!native_begin(s->n,&s->batch)) goto done;
    SceneRoot root={s->vertices.address+64,s->indices.address+64,s->draws.address+64,0,0};
#else
    if(!buffer_create(s,&alternate,sizeof(vertices),OGPU_MEMORY_HOST,0)
        || !buffer_create(s,&archive,4*image_bytes,OGPU_MEMORY_HOST,0)) goto done;
    if(ogpu_buffer_write(alternate,0,vertices,sizeof(vertices),&s->error)!=OGPU_SUCCESS
        || ogpu_buffer_write(archive,0,outputs,4*image_bytes,&s->error)!=OGPU_SUCCESS
        || ogpu_buffer_device_address(alternate,&alternate_address,&s->error)!=OGPU_SUCCESS
        || ogpu_batch_create(s->device,&s->batch,&s->error)!=OGPU_SUCCESS) goto done;
    ordinary_address=s->root.vertices;SceneRoot root=s->root;
#endif
    for(snapshot=0;snapshot<4;++snapshot) {
        if(!scene_commands(s,0,&root)) goto done;
#ifdef ARGUMENT_NATIVE
        native_barrier(s->n,s->batch.command,VK_PIPELINE_STAGE_2_TRANSFER_BIT,VK_ACCESS_2_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_TRANSFER_BIT,VK_ACCESS_2_TRANSFER_READ_BIT);
        if(!native_copy(s->n,&s->batch,&s->readback,0,&archive,snapshot*image_bytes,image_bytes)) goto done;
        native_barrier(s->n,s->batch.command,VK_PIPELINE_STAGE_2_TRANSFER_BIT,VK_ACCESS_2_TRANSFER_READ_BIT,
            VK_PIPELINE_STAGE_2_TRANSFER_BIT,VK_ACCESS_2_TRANSFER_WRITE_BIT);
#else
        if(ogpu_batch_barrier(s->batch,OGPU_ACCESS_TRANSFER_WRITE,OGPU_ACCESS_TRANSFER_READ,&s->error)!=OGPU_SUCCESS
            || ogpu_batch_copy_buffer(s->batch,s->readback,0,archive,snapshot*image_bytes,image_bytes,&s->error)!=OGPU_SUCCESS
            || ogpu_batch_barrier(s->batch,OGPU_ACCESS_TRANSFER_READ,OGPU_ACCESS_TRANSFER_WRITE,&s->error)!=OGPU_SUCCESS) goto done;
#endif
    }
    memset(arguments.bytes,0,sizeof(arguments.bytes));
#ifdef ARGUMENT_NATIVE
    if(!native_submit(s->n,&s->batch) || !native_wait(s->n,&s->batch)
        || !native_read(s->n,&archive,0,outputs,4*image_bytes)) goto done;
    char path[4096];
    if(!native_path(path,argv[2],"snapshots.images") || !native_file(path,outputs,4*image_bytes,1)) goto done;
    if(!native_read(s->n,&s->geometry,0,s->cpu,416+SCENE_DRAW_BYTES)
        || !native_path(path,argv[2],"final.geometry") || !native_file(path,s->cpu,416+SCENE_DRAW_BYTES,1)) goto done;
#else
    if(!submit_wait(s) || ogpu_buffer_read(archive,0,outputs,4*image_bytes,&s->error)!=OGPU_SUCCESS
        || !write_file(argv[2],"snapshots.images",outputs,4*image_bytes)) goto done;
    if(ogpu_buffer_read(s->geometry,0,s->cpu,416+SCENE_DRAW_BYTES,&s->error)!=OGPU_SUCCESS
        || !write_file(argv[2],"final.geometry",s->cpu,416+SCENE_DRAW_BYTES)) goto done;
#endif
    printf("ARGUMENT_SNAPSHOTS {\"bytes\":%u,\"snapshots\":4,\"draws\":8,\"invalid_retries\":%u}\n",
        (unsigned)SCENE_RASTER_BYTES,
#ifdef ARGUMENT_NATIVE
        0u
#else
        8u
#endif
    );
    okay=1;
done:
#ifdef ARGUMENT_NATIVE
    if(s->n) {
        native_batch_destroy(s->n,&s->batch);native_buffer_destroy(s->n,&archive);native_buffer_destroy(s->n,&alternate);
        scene_destroy_resources(s);scene_destroy_context(s);
    }
#else
    ogpu_completion_destroy(s->completion);s->completion=NULL;ogpu_batch_destroy(s->batch);s->batch=NULL;
    ogpu_buffer_destroy(archive);ogpu_buffer_destroy(alternate);scene_destroy_resources(s);scene_destroy_context(s);
#endif
    free(outputs);if(okay) puts("Argument snapshot control drained");return !okay;
}
