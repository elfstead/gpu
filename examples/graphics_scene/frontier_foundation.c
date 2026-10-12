/* Fixed/count/DrawIndex control with caller-owned recording storage. */
static unsigned frontier_records, frontier_strategy;
#define SCENE_DRAW_BYTES (132u+20u*frontier_records)
#define SCENE_FOUNDATION_FRONTIER
#define SCENE_REUSE_CONSUMER
#include "foundation.c"
#include <dlfcn.h>

static int mark(unsigned phase) {
    const char *path=getenv("OGPU_VULKAN_LIBRARY"); NEED(path && getenv("OGPU_SCENE_TRACE"));
    void *library=dlopen(path,RTLD_NOW|RTLD_LOCAL); NEED(library);
    void (*snapshot)(unsigned)=(void (*)(unsigned))dlsym(library,"scene_trace_snapshot");
    if(!snapshot) { dlclose(library); return 0; }
    snapshot(phase); dlclose(library); return 1;
}
static int write_frame(const char *directory,unsigned frame,const char *suffix,const void *bytes,size_t count) {
    char path[4096]; int n=snprintf(path,sizeof(path),"%s/frame-%u.%s",directory,frame,suffix); NEED(n>=0 && n<(int)sizeof(path));
    FILE *file=fopen(path,"wb"); if(!file) { perror(path); return 0; }
    int okay=fwrite(bytes,1,count,file)==count && !ferror(file);
    if(fclose(file)) okay=0;
    return okay;
}
static int run(Scene *s,const char *shaders,const char *output) {
    NEED(context(s,shaders));
    ogpu_next_graphics_limits limits;
    ogpu_next_query q=query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&limits,1,sizeof(limits));
    GPU(ogpu_next_device_query(s->device,&q)); NEED(limits.max_indirect_count>=frontier_records+7);
    /* Multi/count/DrawIndex are foundation raster baseline, not enabled by
     * inspecting SPIR-V at runtime. The runner checks compiler requirements. */
    printf("FRONTIER_FEATURES {\"multiDrawIndirect\":true,\"drawIndirectCount\":true,\"maxDrawIndirectCount\":%u}\n",limits.max_indirect_count);
    puts("DRAW_IDENTITY_FEATURES {\"shaderDrawParameters\":true}");
    NEED(storage(s) && resources(s) && buffer_create(s,CONTROL,144,OGPU_NEXT_USAGE_STORAGE,OGPU_NEXT_MEMORY_HOST_VISIBLE));
    uint64_t requested=2*s->pixels;
    for(unsigned i=0;i<BUFFER_COUNT;++i) requested+=s->sizes[i];
    printf("FRONTIER_STORAGE {\"requested_bytes\":%" PRIu64 ",\"arenas\":1}\n",requested);
    uint64_t address; GPU(ogpu_next_memory_address(span(s,CONTROL,64,16),&address));
    s->reuse_root[0]=s->root.vertices; s->reuse_root[1]=s->root.indices; s->reuse_root[2]=s->root.draws; s->reuse_root[3]=address;
    NEED(mark(0));
    unsigned counts[]={frontier_records,0,1,frontier_records/2,frontier_records+7,frontier_records,0,1};
    for(unsigned f=0;f<8;++f) {
        unsigned char input[144]; memset(input,0xa5,sizeof(input));
        uint32_t values[]={f%2,counts[f],frontier_records,frontier_strategy==2}; memcpy(input+64,values,sizeof(values));
        memcpy(s->maps[CONTROL].data,input,sizeof(input)); GPU(ogpu_next_memory_flush(span(s,CONTROL,0,sizeof(input))));
        NEED(begin(s));
        dependency(s,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_HOST_WRITE,OGPU_NEXT_ACCESS_SHADER_READ);
        commands(s,0); NEED(submit_wait(s));
        for(unsigned i=READBACK;i<=CONTROL;++i) GPU(ogpu_next_memory_invalidate(span(s,i,0,s->sizes[i])));
        NEED(write_frame(output,f,"images",s->maps[READBACK].data,s->sizes[READBACK]) &&
            write_frame(output,f,"geometry",s->maps[GEOMETRY].data,s->sizes[GEOMETRY]));
        NEED(!memcmp(input,s->maps[CONTROL].data,sizeof(input)));
        printf("FRONTIER_FRAME {\"frame\":%u,\"phase\":%u,\"active\":%u,\"capacity\":%u}\n",f,f%2,counts[f],frontier_records);
    }
    return mark(1);
}
int main(int argc,char **argv) {
    if(argc!=7) { fputs("Usage: frontier-foundation width height shaders output records single|multi|count\n",stderr); return 1; }
    Scene s={.index_bytes=4};
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")) { s.width=257; s.height=193; }
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")) { s.width=1280; s.height=720; }
    else return 1;
    if(!strcmp(argv[5],"1")) frontier_records=1;
    else if(!strcmp(argv[5],"64")) frontier_records=64;
    else if(!strcmp(argv[5],"512")) frontier_records=512;
    else return 1;
    if(!strcmp(argv[6],"single")) frontier_strategy=0;
    else if(!strcmp(argv[6],"multi")) frontier_strategy=1;
    else if(!strcmp(argv[6],"count")) frontier_strategy=2;
    else return 1;
    s.pixels=(size_t)s.width*s.height*4;
    int okay=run(&s,argv[3],argv[4]);
    destroy_resources(&s); destroy_context(&s);
    if(okay) puts("Public indexed frontier drained");
    return !okay;
}
