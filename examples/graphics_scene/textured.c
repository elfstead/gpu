/* Consumer-owned textured-scene policy; share only the existing fixture's
 * discovery, placed allocations, generated compute and fail-stop lifetime code. */
#define SCENE_GENERATED
#define SCENE_TEXTURE_CONSUMER
#include "foundation.c"
/* The linker intentionally never repacks stage banks. Reject future changes
 * that would make these two generated views disagree about shared inline bytes. */
#define SAME_FIELD(name) _Static_assert(offsetof(Textured_scene_vertexArguments,name)==offsetof(Textured_scene_fragmentArguments,name),"shared texture root " #name)
SAME_FIELD(arg_vertices);SAME_FIELD(arg_image);SAME_FIELD(arg_samplerIndex);SAME_FIELD(arg_lod);SAME_FIELD(arg_overlay);
_Static_assert(sizeof(Textured_scene_vertexArguments)==sizeof(Textured_scene_fragmentArguments),"shared texture root extent");
#undef SAME_FIELD
typedef struct {
    Scene scene;
    ogpu_next_image *texture;
    ogpu_next_memory *backing,*heaps[2];
    ogpu_next_view *views[2];
    ogpu_next_mapping maps[2];
    ogpu_next_heap_binding bindings[2];
    ogpu_next_descriptor_limits limits;
    ogpu_next_host_span scratch;
} Textured;
static uint64_t align_up(uint64_t n,uint64_t a) { return (n+a-1)/a*a; }
static int texture_resources(Textured *t) {
    Scene *s=&t->scene;
    ogpu_next_format_info formats[64]; ogpu_next_query q=query(OGPU_NEXT_QUERY_FORMATS,formats,64,sizeof(formats[0]));
    GPU(ogpu_next_device_query(s->device,&q)); int filter=0;
    for(uint32_t i=0;i<q.count;++i) if(formats[i].format==OGPU_NEXT_RGBA8_UNORM)
        filter=(formats[i].optimal_features&OGPU_NEXT_FORMAT_FILTER_LINEAR)!=0;
    NEED(filter);
    ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,
        OGPU_NEXT_IMAGE_2D,4,1,1,{8,8,1},OGPU_NEXT_IMAGE_SAMPLED|OGPU_NEXT_IMAGE_COPY_DST,0,NULL,0,0,NULL};
    uint32_t types[32],type; ogpu_next_requirements req={.compatible_types=types,.compatible_type_capacity=32};
    GPU(ogpu_next_image_requirements(s->device,&id,&req)); NEED(memory_type(s,&req,OGPU_NEXT_MEMORY_LOCAL,&type));
    GPU(ogpu_next_image_create_unbound(s->device,&id,&t->texture));
    GPU(ogpu_next_memory_create_dedicated_image(t->texture,type,&t->backing));
    GPU(ogpu_next_image_bind(t->texture,(ogpu_next_span){t->backing,0,req.size}));
    for(unsigned i=0;i<2;++i) {
        ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,
            OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_SAMPLED,{0},{OGPU_NEXT_ASPECT_COLOR,i,4-i,0,1}};
        GPU(ogpu_next_view_create(t->texture,&vd,&t->views[i]));
    }
    NEED(buffer_create(s,CONTROL,512,OGPU_NEXT_USAGE_COPY_SRC,OGPU_NEXT_MEMORY_HOST_VISIBLE));
    q=query(OGPU_NEXT_QUERY_DESCRIPTOR_LIMITS,&t->limits,1,sizeof(t->limits)); GPU(ogpu_next_device_query(s->device,&q));
    for(unsigned i=0;i<2;++i) {
        uint64_t payload=i ? t->limits.sampler_size : t->limits.image_size;
        uint64_t reserved=align_up(payload,i ? t->limits.sampler_reserved_alignment : t->limits.resource_reserved_alignment);
        uint64_t tail=i ? t->limits.sampler_reserved_size : t->limits.resource_reserved_size;
        ogpu_next_memory_desc md={HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),reserved+tail,
            i ? t->limits.sampler_heap_alignment : t->limits.resource_heap_alignment,
            OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
        GPU(ogpu_next_memory_requirements(s->device,&md,&req)); NEED(memory_type(s,&req,OGPU_NEXT_MEMORY_HOST_VISIBLE,&md.memory_type));
        GPU(ogpu_next_memory_create(s->device,&md,&t->heaps[i]));
        GPU(ogpu_next_memory_map((ogpu_next_span){t->heaps[i],0,md.size},&t->maps[i]));
        t->bindings[i]=(ogpu_next_heap_binding){HEADER(ogpu_next_heap_binding,OGPU_NEXT_HEAP_BINDING),
            i ? OGPU_NEXT_HEAP_SAMPLER : OGPU_NEXT_HEAP_RESOURCE,0,{t->heaps[i],0,md.size},reserved,tail};
        ogpu_next_host_requirements scratch; GPU(ogpu_next_descriptor_scratch_requirements(t->bindings[i].kind,1,&scratch));
        NEED(scratch.alignment<=_Alignof(max_align_t)); if(scratch.size>t->scratch.size)t->scratch.size=scratch.size;
    }
    t->scratch.data=malloc(t->scratch.size); NEED(t->scratch.data); return 1;
}
static int texture_update(Textured *t,unsigned mode,unsigned phase) {
    Scene *s=&t->scene; NEED(!s->pending);
    unsigned first=mode==2 || mode==5,linear=mode==3 || mode==5;
    ogpu_next_resource_descriptor image={OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE,OGPU_NEXT_STATE_SHADER_READ,t->views[first],{0}};
    ogpu_next_host_span output={t->maps[0].data,t->limits.image_size};
    GPU(ogpu_next_write_resource_descriptors(s->device,1,&image,&output,t->scratch));
    ogpu_next_sampler_desc sampler={HEADER(ogpu_next_sampler_desc,OGPU_NEXT_SAMPLER_DESC),linear,linear,linear,{2,2,2},0,0,0,0,0,3,0,1};
    output=(ogpu_next_host_span){t->maps[1].data,t->limits.sampler_size};
    GPU(ogpu_next_write_sampler_descriptors(s->device,1,&sampler,&output,t->scratch));
    GPU(ogpu_next_memory_flush((ogpu_next_span){t->heaps[0],0,t->limits.image_size}));
    GPU(ogpu_next_memory_flush((ogpu_next_span){t->heaps[1],0,t->limits.sampler_size}));
    unsigned char *bytes=s->maps[CONTROL].data; memset(bytes,0xa5,512); unsigned offset=64;
    for(unsigned mip=0;mip<4;++mip) for(unsigned y=0;y<(8u>>mip);++y) for(unsigned x=0;x<(8u>>mip);++x) {
        bytes[offset++]=(32+13*x+7*y+23*mip+17*phase)%192;
        bytes[offset++]=(48+5*x+19*y+31*mip+11*phase)%192;
        bytes[offset++]=(64+17*x+3*y+29*mip+13*phase)%192; bytes[offset++]=255;
    }
    GPU(ogpu_next_memory_flush(span(s,CONTROL,0,512))); return 1;
}
static void textured_pass(Textured *t,unsigned mode,int overlay) {
    Scene *s=&t->scene;
    ogpu_next_attachment color={s->views[0],NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,overlay ? OGPU_NEXT_LOAD : OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
    ogpu_next_attachment depth={s->views[1],NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,0,overlay ? OGPU_NEXT_LOAD : OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={1,0}}};
    ogpu_next_render_desc rd={HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,s->width,s->height,1,0,1,0,1,&color,&depth,NULL,{s->rs,s->render_scratch.size}};
    ogpu_next_render_begin(s->encoder,&rd); ogpu_next_bind_executable(s->encoder,s->raster[overlay ? 1 : 0]);
    unsigned inset=mode>=4,vx=inset ? s->width/8 : 0,vy=inset ? s->height/8 : 0;
    ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),
        (float)vx,(float)vy,(float)(s->width-2*vx),(float)(s->height-2*vy),0,1,
        overlay || inset ? s->width/4 : 0,overlay ? s->height/3 : inset ? s->height/4 : 0,
        overlay || inset ? s->width/2 : s->width,overlay ? s->height/3 : inset ? s->height/2 : s->height};
    ogpu_next_set_graphics_state(s->encoder,&vp.header);
    ogpu_next_bind_indices(s->encoder,span(s,INDICES,GUARD,32),OGPU_NEXT_INDEX_U32);
    Textured_scene_vertexArguments root={.arg_vertices=s->root.vertices,.arg_image=0,.arg_samplerIndex=0,
        .arg_lod=mode==1 ? 1.f : mode==3 || mode==5 ? .5f : 0.f,.arg_overlay=(uint32_t)overlay};
    ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,0,sizeof(root),&root);
    for(unsigned i=0;i<(overlay ? 1u : 2u);++i) {
        ogpu_next_indirect draw={span(s,DRAWS,GUARD+20*i,20),20,1,{0}}; ogpu_next_draw_indexed_indirect(s->encoder,&draw);
    }
    ogpu_next_render_end(s->encoder);
}
static int textured_frame(Textured *t,unsigned mode,unsigned frame,const char *directory) {
    Scene *s=&t->scene; NEED(texture_update(t,mode,frame==1) && begin(s));
    ogpu_next_image_barrier b={t->texture,{OGPU_NEXT_ASPECT_COLOR,0,4,0,1},OGPU_NEXT_ACCESS_SAMPLED_READ,OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
    ogpu_next_dependency dep={HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_STAGE_COPY,0,0,0,1,0,NULL,&b,s->bs,s->barrier_scratch.size};
    ogpu_next_barrier(s->encoder,&dep); unsigned offset=64;
    for(unsigned mip=0;mip<4;++mip) {
        unsigned width=8u>>mip,size=width*width*4;
        ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,mip,0,1,{0,0,0},{width,width,1}},0,0,OGPU_NEXT_STATE_COPY_DST,0};
        ogpu_next_copy_to_image(s->encoder,t->texture,span(s,CONTROL,offset,size),&copy); offset+=size;
    }
    b.before=OGPU_NEXT_ACCESS_COPY_WRITE; b.after=OGPU_NEXT_ACCESS_SAMPLED_READ; b.old_state=OGPU_NEXT_STATE_COPY_DST; b.new_state=OGPU_NEXT_STATE_SHADER_READ; b.discard=0;
    dep.before=OGPU_NEXT_STAGE_COPY;dep.after=OGPU_NEXT_STAGE_FRAGMENT;ogpu_next_barrier(s->encoder,&dep);
    dependency(s,OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_READ|OGPU_NEXT_ACCESS_WRITE,OGPU_NEXT_ACCESS_SHADER_WRITE);
    ogpu_next_bind_executable(s->encoder,s->prepare);
    Scene_prepareArguments root={.arg_vertices=s->root.vertices,.arg_indices=s->root.indices,.arg_draws=s->root.draws,.arg_phase=frame==1,.arg_empty=0};
    ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(root),&root);
    ogpu_next_launch launch={{1,1,1},0,NULL}; ogpu_next_dispatch(s->encoder,&launch);
    dependency(s,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_INDEX_INPUT|OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_INDIRECT,
        OGPU_NEXT_ACCESS_SHADER_WRITE,OGPU_NEXT_ACCESS_INDEX_READ|OGPU_NEXT_ACCESS_SHADER_READ|OGPU_NEXT_ACCESS_INDIRECT_READ);
    transition(s,0,1);transition(s,1,1);
    for(unsigned i=0;i<2;++i)ogpu_next_bind_heap(s->encoder,&t->bindings[i]);
    textured_pass(t,mode,0);
    dependency(s,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,
        OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE,
        OGPU_NEXT_ACCESS_COLOR_READ|OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_READ|OGPU_NEXT_ACCESS_DEPTH_WRITE);
    textured_pass(t,mode,1);
    for(unsigned i=0;i<2;++i) {
        transition(s,i,0);
        ogpu_next_image_copy copy={{i ? OGPU_NEXT_ASPECT_DEPTH : OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{s->width,s->height,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
        ogpu_next_copy_from_image(s->encoder,span(s,READBACK,i ? s->pixels+3*GUARD : GUARD,s->pixels),s->images[i],&copy);
    }
    dependency(s,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_SHADER_WRITE,OGPU_NEXT_ACCESS_COPY_READ);
    uint64_t at=0; for(unsigned i=VERTICES;i<=DRAWS;++i) {ogpu_next_copy_memory(s->encoder,span(s,GEOMETRY,at,s->sizes[i]),span(s,i,0,s->sizes[i]));at+=s->sizes[i];}
    dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ);
    NEED(submit_wait(s));
    GPU(ogpu_next_memory_invalidate(span(s,READBACK,0,s->sizes[READBACK])));GPU(ogpu_next_memory_invalidate(span(s,GEOMETRY,0,584)));
    NEED(write_output(directory,mode,frame,"images",s->maps[READBACK].data,s->sizes[READBACK]) && write_output(directory,mode,frame,"geometry",s->maps[GEOMETRY].data,584));
    NEED(write_output(directory,mode,frame,"texture",s->maps[CONTROL].data,512));
    printf("TEXTURED_FRAME {\"mode\":%u,\"frame\":%u,\"phase\":%u}\n",mode,frame,frame==1);return 1;
}
int main(int argc,char **argv) {
    if(argc!=4)return 1;
    Textured t={0};Scene *s=&t.scene;s->index_bytes=4;int okay=0;
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")){s->width=257;s->height=193;}
    else if(!strcmp(argv[1],"640") && !strcmp(argv[2],"360")){s->width=640;s->height=360;}
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")){s->width=1280;s->height=720;}
    else return 1;
    s->pixels=(size_t)s->width*s->height*4;
    if(!context(s,NULL) || !storage(s) || !resources(s) || !texture_resources(&t))goto done;
    for(unsigned mode=0;mode<6;++mode)for(unsigned f=0;f<3;++f)if(!textured_frame(&t,mode,f,argv[3]))goto done;
    okay=1;
done:
    destroy_resources(s); /* Retire recordings before descriptor/image backing. */
    for(unsigned i=0;i<2;++i){ogpu_next_view_destroy(t.views[i]);ogpu_next_memory_destroy(t.heaps[i]);}
    ogpu_next_image_destroy(t.texture);ogpu_next_memory_destroy(t.backing);free(t.scratch.data);destroy_context(s);
    if(okay)puts("Foundation textured frames drained; independent oracle required");
    return !okay;
}
