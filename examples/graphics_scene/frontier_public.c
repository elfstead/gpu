/* Same frontier data/shaders through the public API; no Vulkan imports. */
static unsigned frontier_records, frontier_strategy;
#define SCENE_PUBLIC_FRONTIER
#define SCENE_REUSE_CONSUMER
#include "public.c"
#ifdef SCENE_ARGUMENT_REUSE
#include "argument_reuse.h"
#endif
#include <dlfcn.h>
#ifndef FRONTIER_REUSE
static int mark(unsigned phase) {
    const char *path=getenv("OGPU_VULKAN_LIBRARY");NEED(path && getenv("OGPU_SCENE_TRACE"));
    void *library=dlopen(path,RTLD_NOW|RTLD_LOCAL);NEED(library);
    void (*snapshot)(unsigned)=(void (*)(unsigned))dlsym(library,"scene_trace_snapshot");
    if(!snapshot) { dlclose(library);return 0; }
    snapshot(phase);dlclose(library);return 1;
}
#endif
static int frontier_draw(Scene *s,unsigned pipeline,const OgpuIndexRange *indices,unsigned first) {
    OgpuIndirectRange draws={.buffer=s->draws,.offset=64,.stride_bytes=20,.max_draw_count=frontier_records};
#ifdef SCENE_ARGUMENT_REUSE
    (void)first;NEED(frontier_strategy==0);
    for(unsigned i=0;i<frontier_records;++i) {
        draws.offset=64+20*argument_index(i,frontier_records);draws.max_draw_count=1;
        ARGUMENT_SUPPLIED();
        GPU(ogpu_batch_draw_indexed_indirect(s->batch,s->raster[pipeline],indices,&draws,
            s->argument_root,SCENE_RASTER_BYTES,&s->error));
    }
#elif defined(SCENE_RANGE_SCOPES)
    NEED(frontier_strategy==0 && first<frontier_records);
    draws.offset+=20*first;draws.max_draw_count=1;
    GPU(ogpu_batch_draw_indexed_indirect(s->batch,s->raster[pipeline],indices,&draws,&s->root.vertices,8,&s->error));
#else
    (void)first;
    if(frontier_strategy==2) { draws.count_buffer=s->draws;draws.count_offset=64+20*frontier_records; }
    if(frontier_strategy!=0) {
        GPU(ogpu_batch_draw_indexed_indirect(s->batch,s->raster[pipeline],indices,&draws,&s->root.vertices,8,&s->error));
    } else for(unsigned i=0;i<frontier_records;++i) {
        draws.offset=64+20*i;draws.max_draw_count=1;
        GPU(ogpu_batch_draw_indexed_indirect(s->batch,s->raster[pipeline],indices,&draws,&s->root.vertices,8,&s->error));
    }
#endif
    return 1;
}
#ifndef FRONTIER_REUSE
static int run(Scene *s,OgpuBuffer **control,const char *shaders,const char *output) {
    NEED(scene_create_context(s,shaders));
    OgpuCapabilities caps;OgpuDeviceLimits limits;
    GPU(ogpu_device_capabilities(s->device,&caps,&s->error));GPU(ogpu_device_limits(s->device,&limits,&s->error));
    NEED(caps.multi_draw_indirect && caps.draw_indirect_count && caps.shader_draw_parameters && limits.max_indirect_draw_count>=512);
    printf("FRONTIER_FEATURES {\"multiDrawIndirect\":true,\"drawIndirectCount\":true,\"maxDrawIndirectCount\":%u}\n",limits.max_indirect_draw_count);
    printf("DRAW_IDENTITY_FEATURES {\"shaderDrawParameters\":true}\n");
    NEED(scene_create_resources(s) && buffer_create(s,control,144,OGPU_MEMORY_HOST,0) && mark(0));
    uint64_t address;GPU(ogpu_buffer_device_address(*control,&address,&s->error));
    unsigned counts[]={frontier_records,0,1,frontier_records/2,frontier_records+7,frontier_records,0,1};
    for(unsigned f=0;f<8;++f) {
        unsigned char input[144];memset(input,0xa5,sizeof(input));
        uint32_t values[]={f%2,counts[f],frontier_records,frontier_strategy==2};memcpy(input+64,values,sizeof(values));
        uint64_t root[]={s->root.vertices,s->root.indices,s->root.draws,address+64};
        GPU(ogpu_buffer_write(*control,0,input,sizeof(input),&s->error));
        GPU(ogpu_batch_create(s->device,&s->batch,&s->error));
        NEED(scene_commands(s,0,root) && submit_wait(s));
        char name[128];snprintf(name,sizeof(name),"frame-%u.images",f);
        GPU(ogpu_buffer_read(s->readback,0,s->cpu,2*s->pixels+256,&s->error));
        NEED(write_file(output,name,s->cpu,2*s->pixels+256));
        snprintf(name,sizeof(name),"frame-%u.geometry",f);
        GPU(ogpu_buffer_read(s->geometry,0,s->cpu,416+SCENE_DRAW_BYTES,&s->error));
        NEED(write_file(output,name,s->cpu,416+SCENE_DRAW_BYTES));
        GPU(ogpu_buffer_read(*control,0,s->cpu,sizeof(input),&s->error));NEED(!memcmp(input,s->cpu,sizeof(input)));
        printf("FRONTIER_FRAME {\"frame\":%u,\"phase\":%u,\"active\":%u,\"capacity\":%u}\n",f,f%2,counts[f],frontier_records);
    }
    return mark(1);
}
int main(int argc,char **argv) {
    if(argc!=7) { fprintf(stderr,"Usage: frontier-public width height shaders output records single|multi|count\n");return 1; }
    Scene scene={.index_bytes=4};Scene *s=&scene;OgpuBuffer *control=NULL;
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")) { s->width=257;s->height=193; }
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")) { s->width=1280;s->height=720; }
    else return 1;
    if(!strcmp(argv[5],"1")) frontier_records=1;
    else if(!strcmp(argv[5],"64")) frontier_records=64;
    else if(!strcmp(argv[5],"512")) frontier_records=512;
    else return 1;
    if(!strcmp(argv[6],"single")) frontier_strategy=0;
    else if(!strcmp(argv[6],"multi")) frontier_strategy=1;
    else if(!strcmp(argv[6],"count")) frontier_strategy=2;
    else return 1;
    s->pixels=(size_t)s->width*s->height*4;
    int okay=run(s,&control,argv[3],argv[4]);
    /* Drain before releasing any root pointees. */
    ogpu_completion_destroy(s->completion);s->completion=NULL;
    ogpu_batch_destroy(s->batch);s->batch=NULL;
    ogpu_buffer_destroy(control);scene_destroy_resources(s);scene_destroy_context(s);
    if(okay) puts("Public indexed frontier drained");return !okay;
}
#endif
