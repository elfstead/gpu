/* Native stronger-strategy correctness probe, not a performance measurement. */
static unsigned frontier_records, frontier_strategy;
#define NATIVE_INDEXED_FRONTIER
#define SCENE_NATIVE_FRONTIER
#define SCENE_REUSE_CONSUMER
#include "native.c"
#ifdef SCENE_ARGUMENT_REUSE
#include "argument_reuse.h"
#endif
static PFN_vkCmdDrawIndexedIndirectCount2KHR draw_count;
#ifndef FRONTIER_REUSE
static int frontier_mark(unsigned phase) {
    const char *path=getenv("OGPU_VULKAN_LIBRARY");NEED(path && getenv("OGPU_SCENE_TRACE"));
    void *library=dlopen(path,RTLD_NOW|RTLD_LOCAL);NEED(library);
    void (*snapshot)(unsigned)=(void (*)(unsigned))dlsym(library,"scene_trace_snapshot");
    if(!snapshot) { dlclose(library);return 0; }
    snapshot(phase);dlclose(library);return 1;
}
#endif

static int frontier_draw(Scene *s,unsigned first) {
    VkDrawIndirect2InfoKHR info={.sType=VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
        .addressRange={s->draws.address+64,20*frontier_records,20},
        .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,.drawCount=frontier_records};
#ifdef SCENE_ARGUMENT_REUSE
    (void)first;NEED(frontier_strategy==0);
#ifndef SCENE_ARGUMENT_RESUPPLY
    ARGUMENT_SUPPLIED();
    NEED(native_push(s->n,&s->batch,&s->raster[0],s->argument_root,SCENE_RASTER_BYTES));
#endif
    for(unsigned i=0;i<frontier_records;++i) {
#ifdef SCENE_ARGUMENT_RESUPPLY
        ARGUMENT_SUPPLIED();
        NEED(native_push(s->n,&s->batch,&s->raster[0],s->argument_root,SCENE_RASTER_BYTES));
#endif
        info.addressRange.address=s->draws.address+64+20*argument_index(i,frontier_records);
        info.addressRange.size=20;info.drawCount=1;
        s->draw_indexed(s->batch.command,&info);
    }
#elif defined(SCENE_RANGE_SCOPES)
    NEED(frontier_strategy==0 && first<frontier_records);
    info.addressRange.address+=20*first;info.addressRange.size=20;info.drawCount=1;
    s->draw_indexed(s->batch.command,&info);
#else
    (void)first;
    if(frontier_strategy>=2) {
        VkDrawIndirectCount2InfoKHR count={.sType=VK_STRUCTURE_TYPE_DRAW_INDIRECT_COUNT_2_INFO_KHR,
            .addressRange=info.addressRange,.addressFlags=info.addressFlags,
            .countAddressRange={s->draws.address+64+20*frontier_records,4},
            .countAddressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,.maxDrawCount=frontier_records};
        draw_count(s->batch.command,&count);
        /* Diagnostic control: the fixed draw must not inherit a preceding
         * count word, even if that word is zero. Not a timing strategy. */
        if(frontier_strategy==3) s->draw_indexed(s->batch.command,&info);
    } else if(frontier_strategy==1) s->draw_indexed(s->batch.command,&info);
    else for(unsigned i=0;i<frontier_records;++i) {
        info.addressRange.address=s->draws.address+64+20*i;info.addressRange.size=20;info.drawCount=1;
        s->draw_indexed(s->batch.command,&info);
    }
#endif
    return 1;
}
#ifndef FRONTIER_REUSE
int main(int argc,char **argv) {
    Scene s={.index_bytes=4};NativeBuffer control={0};int okay=0;
    if(argc!=7) { fprintf(stderr,"Usage: frontier width height shaders output records single|multi|count|count-fixed\n");return 1; }
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")) { s.width=257;s.height=193; }
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")) { s.width=1280;s.height=720; }
    else return 1;
    if(!strcmp(argv[5],"1")) frontier_records=1;
    else if(!strcmp(argv[5],"64")) frontier_records=64;
    else if(!strcmp(argv[5],"512")) frontier_records=512;
    else return 1;
    if(!strcmp(argv[6],"single")) frontier_strategy=0;
    else if(!strcmp(argv[6],"multi")) frontier_strategy=1;
    else if(!strcmp(argv[6],"count")) frontier_strategy=2;
    else if(!strcmp(argv[6],"count-fixed")) frontier_strategy=3;
    else return 1;
    s.pixels=(size_t)s.width*s.height*4;
    if(!scene_create_context(&s,argv[3])) goto done;
    PFN_vkGetInstanceProcAddr get=(PFN_vkGetInstanceProcAddr)dlsym(s.n->library,"vkGetInstanceProcAddr");
    draw_count=(PFN_vkCmdDrawIndexedIndirectCount2KHR)get(s.n->instance,"vkCmdDrawIndexedIndirectCount2KHR");
    if(!draw_count || !scene_create_resources(&s) || !native_buffer_create(s.n,&control,144,1) || !frontier_mark(0)) goto done;
    unsigned counts[]={frontier_records,0,1,frontier_records/2,frontier_records+7,frontier_records,0,1};
    for(unsigned f=0;f<8;++f) {
        unsigned char input[144];memset(input,0xa5,sizeof(input));
        uint32_t values[]={f%2,counts[f],frontier_records,frontier_strategy>=2};memcpy(input+64,values,sizeof(values));
        uint64_t root[]={s.vertices.address+64,s.indices.address+64,s.draws.address+64,control.address+64};
        if(!native_write(s.n,&control,0,input,sizeof(input)) || !native_begin(s.n,&s.batch)
            || !scene_commands(&s,0,root) || !native_submit(s.n,&s.batch) || !native_wait(s.n,&s.batch)) goto done;
        native_batch_destroy(s.n,&s.batch);
        char path[4096],name[128];
        snprintf(name,sizeof(name),"frame-%u.images",f);
        if(!native_read(s.n,&s.readback,0,s.cpu,2*s.pixels+256) || !native_path(path,argv[4],name)
            || !native_file(path,s.cpu,2*s.pixels+256,1)) goto done;
        snprintf(name,sizeof(name),"frame-%u.geometry",f);
        if(!native_read(s.n,&s.geometry,0,s.cpu,416+SCENE_DRAW_BYTES) || !native_path(path,argv[4],name)
            || !native_file(path,s.cpu,416+SCENE_DRAW_BYTES,1)) goto done;
        if(!native_read(s.n,&control,0,s.cpu,sizeof(input)) || memcmp(input,s.cpu,sizeof(input))) goto done;
        printf("FRONTIER_FRAME {\"frame\":%u,\"phase\":%u,\"active\":%u,\"capacity\":%u}\n",f,f%2,counts[f],frontier_records);
    }
    okay=frontier_mark(1);
done:
    if(s.n) {
        native_batch_destroy(s.n,&s.batch);native_buffer_destroy(s.n,&control);
        scene_destroy_resources(&s);scene_destroy_context(&s);
    }
    if(okay) puts("Native indexed frontier drained");return !okay;
}
#endif
