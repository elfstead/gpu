// Diagnostic executable only. Intercept the pinned backend table like reference.c;
// OGPU timing uses public ABI calls through link wrappers, never private runtime state.
#include "gpu.h"
#include "diagnostics.h"
#include "ogpu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "diagnostic check failed: %s:%d: %s\n", __FILE__, __LINE__, #x); abort(); } } while (0)
enum { OTHER, COMPUTE, RASTER, KINDS };
static const char *names[] = {"other-or-frame", "compute", "raster"};
static bool native, measured, timed;
static bool omit_compute, omit_raster;
static unsigned omitted[2];
static unsigned current, depth, creates, runs[KINDS], samples[KINDS];
static double nanoseconds[KINDS], poll_ms, wait_ms, destroy_ms, query_ms;
static pl_timer timers[KINDS];
static pl_pass (*original_create)(pl_gpu, const struct pl_pass_params *);
static void (*original_run)(pl_gpu, const struct pl_pass_run_params *);
static struct receipt { OgpuCompletion *handle; unsigned kind; bool queried; } receipts[32];
static double now(void)
{ struct timespec t; CHECK(!clock_gettime(CLOCK_MONOTONIC,&t)); return t.tv_sec*1000.0+t.tv_nsec/1e6; }
static void add(unsigned kind, double ns)
{ if (measured) { ++samples[kind]; nanoseconds[kind] += ns; } }
static void save(unsigned id, const char *suffix, const char *text)
{
    const char *dir = getenv("OGPU_DIAGNOSTIC_DIR");
    if (!dir || !text) return;
    CHECK(!measured);
    char path[4096];
    int n=snprintf(path,sizeof(path),"%s/%s-%u.%s",dir,native?"vulkan":"ogpu",id,suffix);
    CHECK(n>0 && (size_t)n<sizeof(path));
    FILE *f=fopen(path,"wb"); CHECK(f);
    CHECK(fwrite(text,1,strlen(text),f)==strlen(text)); CHECK(!fclose(f));
}
static pl_pass capture_create(pl_gpu gpu, const struct pl_pass_params *p)
{
    CHECK(!measured);
    printf("SHADER id=%u type=%s push=%zu constants=%d descriptors=%d\n",creates,
           p->type==PL_PASS_COMPUTE?"compute":"raster",p->push_constants_size,p->num_constants,p->num_descriptors);
    for (int i=0;i<p->num_constants;++i) {
        uint32_t bits; memcpy(&bits,(const char *)p->constant_data+p->constants[i].offset,4);
        printf("CONSTANT id=%u bits=%08x\n",p->constants[i].id,bits);
    }
    save(creates,p->type==PL_PASS_COMPUTE?"comp":"frag",p->glsl_shader);
    save(creates,"vert",p->vertex_shader); ++creates;
    return original_create(gpu,p);
}
static void capture_run(pl_gpu gpu, const struct pl_pass_run_params *p)
{
    // Native host vertex upload reenters this callback with an uploaded buffer.
    if (depth++) { original_run(gpu,p); --depth; return; }
    current=p->pass->params.type==PL_PASS_COMPUTE?COMPUTE:RASTER;
    if (!runs[current]++) printf("DISPATCH type=%s grid=%d,%d,%d\n",names[current],
        p->compute_groups[0],p->compute_groups[1],p->compute_groups[2]);
    struct pl_pass_run_params copy=*p;
    // Dispatch supplies its own internal native timer even without a callback.
    // Diagnostic mode substitutes our timer; control mode leaves it untouched.
    if (native && timed) copy.timer=timers[current];
    original_run(gpu,&copy); current=OTHER; --depth;
}
void diagnostic_attach(pl_gpu gpu, bool is_native)
{
    native=is_native; measured=false; current=depth=creates=0;
    memset(runs,0,sizeof(runs));
    const char *setting=getenv("OGPU_DIAGNOSTIC_TIMING");
    CHECK(setting && (!strcmp(setting,"0") || !strcmp(setting,"1")));
    timed=!strcmp(setting,"1");
    const char *omit=getenv("OGPU_DIAGNOSTIC_OMIT");
    if(!omit) omit="none";
    CHECK(!strcmp(omit,"none") || !strcmp(omit,"compute") || !strcmp(omit,"raster") || !strcmp(omit,"both"));
    omit_compute=!strcmp(omit,"compute") || !strcmp(omit,"both");
    omit_raster=!strcmp(omit,"raster") || !strcmp(omit,"both");
    CHECK(!native || (!omit_compute && !omit_raster));
    omitted[0]=omitted[1]=0;
    printf("OMISSION measured_only=%s (not workload throughput when non-none)\n",omit);
    printf("DIAGNOSTIC timing=%u native=%u threads=%u group=%u,%u,%u shared=%zu\n",
        timed,native,gpu->glsl.max_group_threads,gpu->glsl.max_group_size[0],
        gpu->glsl.max_group_size[1],gpu->glsl.max_group_size[2],gpu->glsl.max_shmem_size);
    if (native && timed) for(unsigned i=COMPUTE;i<KINDS;++i) { timers[i]=pl_timer_create(gpu); CHECK(timers[i]); }
    struct pl_gpu_fns *f=PL_PRIV(gpu);
    original_create=f->pass_create; original_run=f->pass_run;
    f->pass_create=capture_create; f->pass_run=capture_run;
}
void diagnostic_collect(pl_gpu gpu)
{
    if (native && timed) for(unsigned i=COMPUTE;i<KINDS;++i) {
        uint64_t ns; while ((ns=pl_timer_query(gpu,timers[i]))) add(i,(double)ns);
    }
}
void diagnostic_phase(pl_gpu gpu, bool active)
{
    diagnostic_collect(gpu);
    measured=active;
    memset(samples,0,sizeof(samples)); memset(nanoseconds,0,sizeof(nanoseconds));
    poll_ms=wait_ms=destroy_ms=query_ms=0;
}
void diagnostic_detach(pl_gpu gpu)
{
    diagnostic_collect(gpu);
    for(unsigned i=0;i<KINDS;++i) printf("DEVICE type=%s samples=%u mean_ms=%.6f\n",
        names[i],samples[i],samples[i]?nanoseconds[i]/samples[i]/1e6:0);
    printf("HOST totals_ms poll=%.6f wait=%.6f destroy=%.6f query=%.6f\n",poll_ms,wait_ms,destroy_ms,query_ms);
    printf("OMITTED compute=%u raster=%u\n",omitted[0],omitted[1]);
    for(unsigned i=0;i<32;++i) CHECK(!receipts[i].handle);
    if(native) for(unsigned i=COMPUTE;i<KINDS;++i) pl_timer_destroy(gpu,&timers[i]);
    struct pl_gpu_fns *f=PL_PRIV(gpu); f->pass_create=original_create; f->pass_run=original_run;
    measured=false;
}
OgpuResult __real_ogpu_batch_create(OgpuDevice *,OgpuBatch **,OgpuError *);
OgpuResult __wrap_ogpu_batch_create(OgpuDevice *d,OgpuBatch **b,OgpuError *e)
{
    OgpuResult r=__real_ogpu_batch_create(d,b,e);
    if(r==OGPU_SUCCESS && timed) CHECK(ogpu_batch_enable_timing(*b,e)==OGPU_SUCCESS);
    return r;
}
OgpuResult __real_ogpu_batch_submit(OgpuBatch *,OgpuCompletion **,OgpuError *);
OgpuResult __wrap_ogpu_batch_submit(OgpuBatch *b,OgpuCompletion **c,OgpuError *e)
{
    OgpuResult r=__real_ogpu_batch_submit(b,c,e);
    if(r==OGPU_SUCCESS && timed) {
        unsigned i=0; while(i<32 && receipts[i].handle) ++i; CHECK(i<32);
        receipts[i]=(struct receipt){.handle=*c,.kind=current};
    }
    return r;
}
static void observe(OgpuCompletion *c)
{
    if(!timed) return;
    for(unsigned i=0;i<32;++i) if(receipts[i].handle==c) {
        if(!receipts[i].queried) {
            double ns,start=now(); OgpuError e;
            CHECK(ogpu_completion_elapsed_ns(c,&ns,&e)==OGPU_SUCCESS);
            if(measured) query_ms+=now()-start;
            receipts[i].queried=true; add(receipts[i].kind,ns);
        }
        return;
    }
    CHECK(false);
}
OgpuResult __real_ogpu_completion_poll(OgpuCompletion *,uint32_t *,OgpuError *);
OgpuResult __wrap_ogpu_completion_poll(OgpuCompletion *c,uint32_t *ready,OgpuError *e)
{
    double start=now(); OgpuResult r=__real_ogpu_completion_poll(c,ready,e);
    if(measured) poll_ms+=now()-start;
    if(r==OGPU_SUCCESS && *ready) observe(c); return r;
}
OgpuResult __real_ogpu_completion_wait(OgpuCompletion *,OgpuError *);
OgpuResult __wrap_ogpu_completion_wait(OgpuCompletion *c,OgpuError *e)
{
    double start=now(); OgpuResult r=__real_ogpu_completion_wait(c,e);
    if(measured) wait_ms+=now()-start;
    if(r==OGPU_SUCCESS) observe(c); return r;
}
void __real_ogpu_completion_destroy(OgpuCompletion *);
void __wrap_ogpu_completion_destroy(OgpuCompletion *c)
{
    if(c) for(unsigned i=0;i<32;++i) if(receipts[i].handle==c) receipts[i]=(struct receipt){0};
    double start=now(); __real_ogpu_completion_destroy(c);
    if(measured) destroy_ms+=now()-start;
}
OgpuResult __real_ogpu_batch_dispatch(OgpuBatch *,OgpuKernel *,uint32_t,uint32_t,uint32_t,const void *,uint32_t,OgpuError *);
OgpuResult __wrap_ogpu_batch_dispatch(OgpuBatch *b,OgpuKernel *k,uint32_t x,uint32_t y,uint32_t z,const void *args,uint32_t size,OgpuError *e)
{
    if(measured && omit_compute) { ++omitted[0]; return OGPU_SUCCESS; }
    return __real_ogpu_batch_dispatch(b,k,x,y,z,args,size,e);
}
OgpuResult __real_ogpu_batch_draw_indirect(OgpuBatch *,OgpuRaster *,OgpuImage *,OgpuBuffer *,uint64_t,const void *,uint32_t,uint32_t,OgpuError *);
OgpuResult __wrap_ogpu_batch_draw_indirect(OgpuBatch *b,OgpuRaster *r,OgpuImage *t,OgpuBuffer *i,uint64_t offset,const void *args,uint32_t size,uint32_t load,OgpuError *e)
{
    if(measured && omit_raster) { ++omitted[1]; return OGPU_SUCCESS; }
    return __real_ogpu_batch_draw_indirect(b,r,t,i,offset,args,size,load,e);
}
