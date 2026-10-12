/* Call-time inline snapshots and explicit partial updates; no argument cache. */
#define SCENE_REUSE_CONSUMER
#define SCENE_ARGUMENT_SNAPSHOT
#define SCENE_DRAW_BYTES 172u
#include "foundation.c"
_Static_assert(SCENE_RASTER_BYTES==8 || SCENE_RASTER_BYTES==64 || SCENE_RASTER_BYTES==256,"snapshot root extent");
static unsigned snapshot;
static uint64_t alternate_address;
static union { uint64_t aligned[32]; unsigned char bytes[256]; } arguments;

static void snapshot_draw(Scene *s) {
    for(unsigned draw=0;draw<2;++draw) {
        unsigned changed=snapshot==2 && draw==1;
        uint64_t address=s->root.vertices;
#ifndef ARGUMENT_PATCH_PAYLOAD
        if(changed) address=alternate_address;
#endif
        memcpy(arguments.bytes,&address,8);
#if SCENE_RASTER_BYTES > 8
        for(unsigned i=0;i<(SCENE_RASTER_BYTES-8)/4;++i) {
            unsigned delta=changed;
#ifdef ARGUMENT_PATCH_PAYLOAD
            if(i!=(SCENE_RASTER_BYTES-8)/4-1) delta=0;
#endif
            uint32_t value=i+1+delta; memcpy(arguments.bytes+8+4*i,&value,4);
        }
#endif
#ifdef ARGUMENT_FOUNDATION_PARTIAL
        if(draw==0) ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,0,SCENE_RASTER_BYTES,arguments.bytes);
        else if(changed) ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,SCENE_RASTER_BYTES-4,4,arguments.bytes+SCENE_RASTER_BYTES-4);
#else
        ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,0,SCENE_RASTER_BYTES,arguments.bytes);
#endif
        /* Host storage can change immediately after set_inline, even before draw. */
        memset(arguments.bytes,0xa5,sizeof(arguments.bytes));
        ogpu_next_indirect indirect={span(s,DRAWS,GUARD+20*draw,20),20,1,{NULL,0,0}};
        ogpu_next_draw_indexed_indirect(s->encoder,&indirect);
    }
}
static int write_file(const char *directory,const char *name,const void *bytes,size_t size) {
    char path[4096]; int n=snprintf(path,sizeof(path),"%s/%s",directory,name); NEED(n>=0 && n<(int)sizeof(path));
    FILE *file=fopen(path,"wb"); if(!file) { perror(path); return 0; }
    int okay=fwrite(bytes,1,size,file)==size && !ferror(file);
    if(fclose(file)) okay=0;
    return okay;
}
static int run(Scene *s,const char *shaders,const char *output) {
#ifdef ARGUMENT_PATCH_PAYLOAD
    _Static_assert(SCENE_RASTER_BYTES>8,"partial update needs scalar payload");
    printf("ARGUMENT_PATCH_POLICY {\"single_word\":true,\"foundation_partial\":%s}\n",
#ifdef ARGUMENT_FOUNDATION_PARTIAL
        "true"
#else
        "false"
#endif
    );
#endif
    NEED(context(s,shaders) && storage(s) && resources(s));
    NEED(buffer_create(s,ALTERNATE,128,OGPU_NEXT_USAGE_STORAGE,OGPU_NEXT_MEMORY_HOST_VISIBLE) &&
        buffer_create(s,ARCHIVE,4*s->sizes[READBACK],OGPU_NEXT_USAGE_COPY_DST,OGPU_NEXT_MEMORY_HOST_VISIBLE));
    const float vertices[]={-.75f,-.75f,.75f,1,.75f,-.75f,.75f,1,.75f,.75f,.75f,1,-.75f,.75f,.75f,1,
        -.125f,-.375f,.25f,1,.375f,-.375f,.25f,1,.375f,.375f,.25f,1,-.125f,.375f,.25f,1};
    memcpy(s->maps[ALTERNATE].data,vertices,sizeof(vertices)); GPU(ogpu_next_memory_flush(span(s,ALTERNATE,0,128)));
    GPU(ogpu_next_memory_address(span(s,ALTERNATE,0,128),&alternate_address));
    memset(s->maps[ARCHIVE].data,0xa5,s->sizes[ARCHIVE]); GPU(ogpu_next_memory_flush(span(s,ARCHIVE,0,s->sizes[ARCHIVE])));
    /* Foundation encoding errors are sticky, not ABI20 recoverable draw retries.
     * Reject eight separate recordings, reset them, then build valid work. */
    for(unsigned i=0;i<8;++i) {
        NEED(begin(s)); uint32_t value=0; ogpu_next_list *invalid=NULL;
        ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX,1,sizeof(value),&value);
        ogpu_next_status status=ogpu_next_commands_end(s->encoder,&invalid); s->encoder=NULL;
        NEED(status==OGPU_NEXT_INVALID && invalid==NULL); GPU(ogpu_next_arena_reset(s->arena));
    }
    NEED(begin(s));
    dependency(s,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_ACCESS_HOST_WRITE,OGPU_NEXT_ACCESS_SHADER_READ);
    memcpy(s->reuse_root,&s->root,sizeof(s->reuse_root));
    for(snapshot=0;snapshot<4;++snapshot) {
        commands(s,0);
        dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_READ);
        ogpu_next_copy_memory(s->encoder,span(s,ARCHIVE,snapshot*s->sizes[READBACK],s->sizes[READBACK]),span(s,READBACK,0,s->sizes[READBACK]));
        dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COPY_READ,OGPU_NEXT_ACCESS_COPY_WRITE);
    }
    memset(arguments.bytes,0,sizeof(arguments.bytes));
    dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ);
    NEED(submit_wait(s));
    GPU(ogpu_next_memory_invalidate(span(s,ARCHIVE,0,s->sizes[ARCHIVE])));
    GPU(ogpu_next_memory_invalidate(span(s,GEOMETRY,0,s->sizes[GEOMETRY])));
    NEED(write_file(output,"snapshots.images",s->maps[ARCHIVE].data,s->sizes[ARCHIVE]) &&
         write_file(output,"final.geometry",s->maps[GEOMETRY].data,s->sizes[GEOMETRY]));
    printf("ARGUMENT_SNAPSHOTS {\"bytes\":%u,\"snapshots\":4,\"draws\":8,\"invalid_retries\":0,\"invalid_recordings\":8}\n",(unsigned)SCENE_RASTER_BYTES);
    return 1;
}
int main(int argc,char **argv) {
    if(argc!=3) return 1;
    Scene s={.index_bytes=4,.width=257,.height=193,.pixels=257*193*4};
    int okay=run(&s,argv[1],argv[2]);
    destroy_resources(&s); destroy_context(&s);
    if(okay) puts("Argument snapshot control drained");
    return !okay;
}
