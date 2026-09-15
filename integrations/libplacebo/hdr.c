// Bounded upstream HDR reference; processing stays in libplacebo shader helpers.
#include "gpu.h"
#include "backend.h"
#include <libplacebo/vulkan.h>
#include <libplacebo/shaders/colorspace.h>
#include <libplacebo/shaders/sampling.h>
#include <libplacebo/dispatch.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr,"HDR check failed: %s:%d: %s\n",__FILE__,__LINE__,#x); exit(1); } } while (0)
static const char *directory;
static struct pl_gpu_fns original;
static unsigned creates, computes, rasters, depth, errors;
static unsigned alpha_failures;
static bool use_ogpu;
static FILE *manifest;
static void log_message(void *priv, enum pl_log_level level, const char *message)
{ if(level<=PL_LOG_WARN) fprintf(stderr,"libplacebo[%d]: %s\n",level,message); if(level<=PL_LOG_ERR) ++errors; }
static uint16_t half(float value)
{ _Static_assert(sizeof(_Float16)==2,"binary16 fixture conversion"); _Float16 h=value; uint16_t bits; memcpy(&bits,&h,2); return bits; }
static float wide(uint16_t bits)
{ _Float16 h; memcpy(&h,&bits,2); return (float)h; }
static void half_tests(void)
{
    CHECK(half(0)==0 && half(1)==0x3c00 && half(0.5f)==0x3800 && half(-2)==0xc000);
    CHECK(half(65504)==0x7bff && half(0x1p-24f)==1 && half(1+0x1p-11f)==0x3c00);
    CHECK(wide(0x3c00)==1 && wide(0xc000)==-2 && wide(1)==0x1p-24f);
}
static void save(const char *name,const void *data,size_t size)
{
    char path[4096]; CHECK(snprintf(path,sizeof(path),"%s/%s",directory,name)<(int)sizeof(path));
    FILE *f=fopen(path,"wb"); CHECK(f && fwrite(data,1,size,f)==size && !fclose(f));
}
static pl_pass capture_create(pl_gpu gpu,const struct pl_pass_params *p)
{
    unsigned id=creates++; char name[64];
    fprintf(manifest,"create=%u type=%s descriptors=%d constants=%d push=%zu target=%s\n",id,
        p->type==PL_PASS_COMPUTE?"compute":"raster",p->num_descriptors,p->num_constants,
        p->push_constants_size,p->target_format?p->target_format->name:"none");
    for(int i=0;i<p->num_constants;++i) {
        uint32_t bits; memcpy(&bits,(const char *)p->constant_data+p->constants[i].offset,4);
        fprintf(manifest," constant=%u bits=%08x\n",p->constants[i].id,bits);
    }
    snprintf(name,sizeof(name),"pass-%u.%s",id,p->type==PL_PASS_COMPUTE?"comp":"frag");
    save(name,p->glsl_shader,strlen(p->glsl_shader));
    if(p->vertex_shader) { snprintf(name,sizeof(name),"pass-%u.vert",id); save(name,p->vertex_shader,strlen(p->vertex_shader)); }
    return original.pass_create(gpu,p);
}
static void capture_run(pl_gpu gpu,const struct pl_pass_run_params *p)
{
    if(depth++) { original.pass_run(gpu,p); --depth; return; }
    const bool compute=p->pass->params.type==PL_PASS_COMPUTE;
    if(compute) ++computes; else ++rasters;
    if(p->pass->params.push_constants_size) {
        char name[80]; snprintf(name,sizeof(name),"run-%u.push",computes+rasters-1);
        CHECK(p->push_constants);
        save(name,p->push_constants,p->pass->params.push_constants_size);
    }
    fprintf(manifest,"run type=%s grid=%d,%d,%d\n",compute?"compute":"raster",p->compute_groups[0],p->compute_groups[1],p->compute_groups[2]);
    for(int i=0;i<p->pass->params.num_descriptors;++i) {
        const struct pl_desc *d=&p->pass->params.descriptors[i];
        CHECK(d->type==PL_DESC_SAMPLED_TEX || d->type==PL_DESC_STORAGE_IMG);
        pl_tex t=p->desc_bindings[i].object;
        fprintf(manifest," image binding=%d type=%d format=%s extent=%d,%d,%d texel=%zu sample=%d renderable=%d storable=%d\n",
            d->binding,d->type,t->params.format->name,t->params.w,t->params.h,t->params.d,
            t->params.format->texel_size,p->desc_bindings[i].sample_mode,
            t->params.renderable,t->params.storable);
    }
    original.pass_run(gpu,p); --depth;
}
static void process(pl_gpu gpu,pl_dispatch dp,pl_shader_obj *scale,pl_shader_obj *tone,
                    pl_tex src,pl_tex mid,pl_tex dst)
{
    pl_dispatch_reset_frame(dp);
    pl_shader sh=pl_dispatch_begin(dp);
    CHECK(pl_shader_sample_polar(sh,pl_sample_src(.tex=src,.new_w=mid->params.w,.new_h=mid->params.h),
        pl_sample_filter_params(.filter=pl_filter_ewa_lanczos,.lut=scale)));
    CHECK(pl_shader_is_compute(sh));
    CHECK(pl_dispatch_finish(dp,pl_dispatch_params(.shader=&sh,.target=mid)));
    sh=pl_dispatch_begin(dp);
    CHECK(pl_shader_sample_nearest(sh,pl_sample_src(.tex=mid)));
    struct pl_color_space source={.primaries=PL_COLOR_PRIM_BT_2020,.transfer=PL_COLOR_TRC_LINEAR,
        .hdr={.min_luma=0,.max_luma=1000}};
    struct pl_color_space dest=pl_color_space_srgb;
    dest.hdr.min_luma=0; dest.hdr.max_luma=PL_COLOR_SDR_WHITE;
    pl_shader_color_map_ex(sh,pl_color_map_params(.gamut_mapping=&pl_gamut_map_clip,
        .tone_mapping_function=&pl_tone_map_spline,.metadata=PL_HDR_METADATA_HDR10,
        .contrast_recovery=0),pl_color_map_args(.src=source,.dst=dest,.prelinearized=true,.state=tone));
    CHECK(!pl_shader_is_compute(sh));
    CHECK(pl_dispatch_finish(dp,pl_dispatch_params(.shader=&sh,.target=dst)));
    CHECK(!pl_gpu_is_failed(gpu));
}
static void run(pl_gpu gpu,int w,int h)
{
    int ow=2*w+1,oh=2*h+1;
    size_t in=(size_t)w*h*4,n=(size_t)ow*oh*4;
    uint16_t *input=malloc(in*2),*middle=malloc(n*2),*first_mid=malloc(n*2);
    uint8_t *output=malloc(n),*first=malloc(n); CHECK(input && middle && first_mid && output && first);
    pl_fmt f16=pl_find_named_fmt(gpu,"rgba16hf"),f8=pl_find_named_fmt(gpu,"rgba8");
    CHECK(f16 && f8 && f16->texel_size==8 && !f16->emulated);
    pl_tex src=pl_tex_create(gpu,pl_tex_params(.w=w,.h=h,.format=f16,.sampleable=true,.host_writable=true));
    // Pinned dispatch_finish checks renderable even for a compute-only target.
    pl_tex mid=pl_tex_create(gpu,pl_tex_params(.w=ow,.h=oh,.format=f16,.sampleable=true,.storable=true,.renderable=true,.host_readable=true));
    pl_tex dst=pl_tex_create(gpu,pl_tex_params(.w=ow,.h=oh,.format=f8,.renderable=true,.host_readable=true));
    CHECK(src && mid && dst);
    pl_buf staging=use_ogpu?NULL:pl_buf_create(gpu,pl_buf_params(.size=in*2,.host_writable=true,.memory_type=PL_BUF_MEM_HOST));
    CHECK(use_ogpu || staging);
    pl_dispatch dp=pl_dispatch_create(gpu->log,gpu); CHECK(dp);
    pl_shader_obj scale=NULL,tone=NULL; unsigned warm=0;
    for(unsigned frame=0;frame<3;++frame) {
        for(int y=0;y<h;++y) for(int x=0;x<w;++x) {
            size_t p=((size_t)y*w+x)*4;
            float ramp=4.5f*(frame==1?w-1-x:x)/(w-1);
            for(unsigned c=0;c<3;++c) input[p+c]=half(w==16?ramp:
                ramp*(0.25f+0.75f*((y+c*7)%h)/(h-1)));
            input[p+3]=half(1);
        }
        char name[80]; snprintf(name,sizeof(name),"%dx%d-frame%u-input.rgba16f",w,h,frame);
        save(name,input,in*2);
        if(use_ogpu) CHECK(pl_tex_upload(gpu,pl_tex_transfer_params(.tex=src,.ptr=input)));
        else {
            pl_buf_write(gpu,staging,0,input,in*2);
            CHECK(pl_tex_upload(gpu,pl_tex_transfer_params(.tex=src,.buf=staging)));
        }
        process(gpu,dp,&scale,&tone,src,mid,dst);
        CHECK(pl_tex_download(gpu,pl_tex_transfer_params(.tex=mid,.ptr=middle,.no_import=true)));
        CHECK(pl_tex_download(gpu,pl_tex_transfer_params(.tex=dst,.ptr=output,.no_import=true)));
        unsigned hdr=0, rounded_alpha=0; float peak=0,alpha_error=0;
        for(size_t i=0;i<n;++i) {
            float v=wide(middle[i]); CHECK(isfinite(v)); if(v>peak) peak=v;
            if(i%4==3) {
                CHECK(output[i]==255);
                rounded_alpha+=v!=1;
                alpha_error=fmaxf(alpha_error,fabsf(v-1));
            } else hdr+=v>1;
        }
        CHECK(hdr);
        if(w==16) for(int y=0;y<oh;++y) for(int x=1;x<ow;++x) {
            size_t p=((size_t)y*ow+x)*4;
            CHECK(abs((int)output[p]-(int)output[p+1])<=2 && abs((int)output[p]-(int)output[p+2])<=2);
            CHECK(frame==1 ? output[p]<=output[p-4]+2 : output[p]+2>=output[p-4]);
        }
        if(frame==0) { memcpy(first,output,n); memcpy(first_mid,middle,n*2); warm=creates; }
        else { CHECK(creates==warm); CHECK((memcmp(first,output,n)==0)==(frame==2));
            CHECK((memcmp(first_mid,middle,n*2)==0)==(frame==2)); }
        snprintf(name,sizeof(name),"%dx%d-frame%u.rgba",w,h,frame); save(name,output,n);
        snprintf(name,sizeof(name),"%dx%d-frame%u.rgba16f",w,h,frame); save(name,middle,n*2);
        alpha_failures+=alpha_error>0x1p-10f;
        printf("HDR input=%dx%d output=%dx%d frame=%u above_one=%u peak=%g rounded_alpha=%u max_alpha_error=%g gates=%s\n",
            w,h,ow,oh,frame,hdr,peak,rounded_alpha,alpha_error,alpha_error>0x1p-10f?"FAIL(alpha tolerance)":"PASS");
    }
    pl_dispatch_destroy(&dp); pl_shader_obj_destroy(&scale); pl_shader_obj_destroy(&tone);
    pl_tex_destroy(gpu,&src); pl_tex_destroy(gpu,&mid); pl_tex_destroy(gpu,&dst); pl_buf_destroy(gpu,&staging);
    free(input); free(middle); free(first_mid); free(output); free(first);
}
int main(int argc,char **argv)
{
    CHECK(argc==2 || (argc==3 && !strcmp(argv[2],"ogpu")));
    directory=argv[1]; use_ogpu=argc==3; half_tests();
    char path[4096]; CHECK(snprintf(path,sizeof(path),"%s/manifest.txt",directory)<(int)sizeof(path));
    manifest=fopen(path,"w"); CHECK(manifest);
    pl_log log=pl_log_create(PL_API_VER,pl_log_params(.log_cb=log_message,.log_level=PL_LOG_WARN)); CHECK(log);
    pl_vulkan vk=use_ogpu?NULL:pl_vulkan_create(log,pl_vulkan_params(.allow_software=true,.async_compute=false,.async_transfer=false));
    pl_gpu gpu=use_ogpu?ogpu_pl_create(log,0):vk?vk->gpu:NULL; CHECK(gpu);
    fprintf(manifest,"backend=%s max_push=%zu SDR_white=%g\n",use_ogpu?"ogpu":"native",gpu->limits.max_pushc_size,(double)PL_COLOR_SDR_WHITE);
    struct pl_gpu_fns *f=PL_PRIV(gpu); original=*f; f->pass_create=capture_create; f->pass_run=capture_run;
    run(gpu,16,16); run(gpu,31,17); run(gpu,64,33);
    pl_gpu_finish(gpu); *f=original;
    if(use_ogpu) {
        struct ogpu_stats s=ogpu_pl_stats(gpu);
        CHECK(!s.textures && !s.passes && !s.banks && !s.outstanding && !s.inflight &&
              !s.texture_bytes && !s.staging_bytes);
        CHECK(s.creates==6 && s.compute==9 && s.raster==9 && s.receipt_reuses==12);
        printf("HDR-ogpu live=0 receipt_reuses=%u PASS\n",s.receipt_reuses);
        ogpu_pl_destroy(&gpu);
    } else pl_vulkan_destroy(&vk);
    pl_log_destroy(&log); CHECK(!errors && !fclose(manifest));
    CHECK(computes==9 && rasters==9);
    printf("HDR-reference creates=%u compute=%u raster=%u alpha_failures=%u %s (%s)\n",
        creates,computes,rasters,alpha_failures,alpha_failures?"GATE FAILED":"PASS",use_ogpu?"ogpu":"native");
    return alpha_failures?2:0;
}
