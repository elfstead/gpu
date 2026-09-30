/* Consumer-side schedule and byte oracle only; no backend/runtime calls here. */
#ifndef SCENE_REUSE_H
#define SCENE_REUSE_H
#include <dlfcn.h>
static int reuse_mark(unsigned phase) {
    if(!getenv("OGPU_SCENE_TRACE")) return 1;
    const char *path=getenv("OGPU_VULKAN_LIBRARY");NEED(path);
    void *library=dlopen(path,RTLD_NOW|RTLD_LOCAL);NEED(library);
    void (*snapshot)(unsigned)=(void (*)(unsigned))dlsym(library,"scene_trace_snapshot");
    if(!snapshot) { dlclose(library);return 0; }
    snapshot(phase);dlclose(library);return 1;
}
typedef struct {
    unsigned width, height, frames, mode, slots, replay, index_bytes;
    const char *shaders, *reference;
    unsigned char *images[3], *mesh[3];
    size_t image_bytes;
    unsigned submitted, checked, pending, peak, encodes;
} Reuse;

static int reuse_number(const char *text, unsigned low, unsigned high, unsigned *out) {
    if(!text[0] || strspn(text,"0123456789")!=strlen(text)) return 0;
    unsigned value=0;
    for(const char *p=text;*p;++p) {
        unsigned digit=(unsigned)(*p-'0');
        if(value>high/10 || (value==high/10 && digit>high%10)) return 0;
        value=value*10+digit;
    }
    if(value<low) return 0;
    *out=value; return 1;
}
static int reuse_config(Reuse *r, int argc, char **argv) {
    if(argc!=10) { fprintf(stderr,"Usage: reuse width height shaders reference frames mode slots reset|replay 16|32\n"); return 0; }
    NEED(reuse_number(argv[1],1,1280,&r->width) && reuse_number(argv[2],1,720,&r->height));
    NEED((r->width==257 && r->height==193) || (r->width==640 && r->height==360) || (r->width==1280 && r->height==720));
    NEED(reuse_number(argv[5],8,1000,&r->frames) && reuse_number(argv[6],0,8,&r->mode)
        && reuse_number(argv[7],1,2,&r->slots) && r->frames%r->slots==0);
    NEED(r->mode==0 || (r->mode>=6 && r->mode<=8));
    NEED(!strcmp(argv[8],"reset") || !strcmp(argv[8],"replay")); r->replay=!strcmp(argv[8],"replay");
    NEED(!strcmp(argv[9],"16") || !strcmp(argv[9],"32")); r->index_bytes=!strcmp(argv[9],"16") ? 2 : 4;
    r->shaders=argv[3];r->reference=argv[4];r->image_bytes=(size_t)r->width*r->height*8+256; return 1;
}
static int reuse_file(const char *directory, unsigned mode, unsigned frame, const char *suffix, size_t bytes, unsigned char **out) {
    char path[4096]; int n=snprintf(path,sizeof(path),"%s/mode-%u-frame-%u.%s",directory,mode,frame,suffix);
    NEED(n>=0 && (size_t)n<sizeof(path));
    FILE *f=fopen(path,"rb"); if(!f) { perror(path); return 0; }
    *out=malloc(bytes);
    int okay=*out && fread(*out,1,bytes,f)==bytes && fgetc(f)==EOF && !ferror(f);
    if(fclose(f)) okay=0; return okay;
}
static int reuse_reference(Reuse *r) {
    for(unsigned i=0;i<3;++i) {
        NEED(reuse_file(r->reference,i==2 ? 9 : r->mode,i==1,"images",r->image_bytes,&r->images[i]));
        NEED(reuse_file(r->reference,i==2 ? 9 : r->mode,i==1,"geometry",584,&r->mesh[i]));
    }
    /* Mode 8 deliberately hides near-surface movement behind the far draw.
       Geometry must still change even when the final images are identical. */
    NEED(memcmp(r->mesh[0],r->mesh[1],584));
    return 1;
}
static unsigned reuse_variant(unsigned generation) { return generation%4==1 ? 1 : generation%4==2 ? 2 : 0; }
static void reuse_control(unsigned char bytes[136], unsigned variant) {
    memset(bytes,0xa5,136);
    uint32_t values[2]={variant==1,variant==2};memcpy(bytes+64,values,8);
}
static int reuse_check(Reuse *r, unsigned frame, const unsigned char *images, const unsigned char *mesh, const unsigned char *control) {
    unsigned generation=frame/r->slots,variant=reuse_variant(generation);
    unsigned char expected[136];reuse_control(expected,variant);
    NEED(!memcmp(images,r->images[variant],r->image_bytes) && !memcmp(mesh,r->mesh[variant],584)
        && !memcmp(control,expected,136));
    NEED(r->pending && frame==r->checked);
    --r->pending;++r->checked;
    printf("REUSE_FRAME {\"frame\":%u,\"slot\":%u,\"generation\":%u,\"phase\":%u,\"empty\":%u}\n",
        frame,frame%r->slots,generation,variant==1,variant==2);return 1;
}
static void reuse_submitted(Reuse *r) {
    ++r->submitted;++r->pending;if(r->pending>r->peak) r->peak=r->pending;
}
static int reuse_summary(Reuse *r) {
    NEED(r->submitted==r->frames && r->checked==r->frames && r->pending==0 && r->peak==r->slots);
    NEED(r->encodes==(r->replay ? r->slots : r->frames));
    printf("REUSE_SUMMARY {\"frames\":%u,\"slots\":%u,\"mode\":%u,\"index_bytes\":%u,\"replay\":%u,"
        "\"encodes\":%u,\"peak_unretired\":%u,\"requested_bytes\":%zu}\n",
        r->frames,r->slots,r->mode,r->index_bytes,r->replay,r->encodes,r->peak,
        r->slots*((size_t)r->width*r->height*16+1816));return 1;
}
static void reuse_free(Reuse *r) { for(unsigned i=0;i<3;++i) { free(r->images[i]);free(r->mesh[i]); } }
#endif
