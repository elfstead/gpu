/* Range/count variant of the existing slot scheduler. Same retained native and
 * public setup/submit/retire paths; independently checked serial reference bytes. */
#ifndef SCENE_RANGE_REUSE_H
#define SCENE_RANGE_REUSE_H
#include <dlfcn.h>
#define REUSE_CONTROL_BYTES 144u
#define REUSE_MESH_BYTES (416u+SCENE_DRAW_BYTES)
typedef struct {
    unsigned width,height,frames,mode,slots,replay,index_bytes;
    const char *shaders,*reference;
    size_t image_bytes;
    unsigned char *images[8],*mesh[8];
    unsigned submitted,checked,pending,peak,encodes;
} Reuse;
static int reuse_mark(unsigned phase) {
    if(!getenv("OGPU_SCENE_TRACE")) return 1;
    const char *path=getenv("OGPU_VULKAN_LIBRARY");NEED(path);
    void *library=dlopen(path,RTLD_NOW|RTLD_LOCAL);NEED(library);
    void (*snapshot)(unsigned)=(void (*)(unsigned))dlsym(library,"scene_trace_snapshot");
    if(!snapshot) { dlclose(library);return 0; }
    snapshot(phase);dlclose(library);return 1;
}
static int reuse_number(const char *text,unsigned low,unsigned high,unsigned *out) {
    if(!text[0] || strspn(text,"0123456789")!=strlen(text)) return 0;
    unsigned value=0;
    for(const char *p=text;*p;++p) {
        unsigned digit=(unsigned)(*p-'0');
        if(value>high/10 || (value==high/10 && digit>high%10)) return 0;
        value=value*10+digit;
    }
    if(value<low) return 0;
    *out=value;return 1;
}
static int reuse_config(Reuse *r,int argc,char **argv) {
    if(argc!=10) { fprintf(stderr,"Usage: range-reuse width height shaders reference frames slots reset|replay capacity single|multi|count\n");return 0; }
    NEED(reuse_number(argv[1],1,1280,&r->width) && reuse_number(argv[2],1,720,&r->height));
    NEED((r->width==257 && r->height==193) || (r->width==1280 && r->height==720));
    NEED(reuse_number(argv[5],8,1000,&r->frames) && reuse_number(argv[6],1,2,&r->slots) && r->frames%r->slots==0);
    NEED(!strcmp(argv[7],"reset") || !strcmp(argv[7],"replay"));r->replay=!strcmp(argv[7],"replay");
    NEED(reuse_number(argv[8],1,512,&frontier_records) && (frontier_records==1 || frontier_records==64 || frontier_records==512));
    if(!strcmp(argv[9],"single")) frontier_strategy=0;
    else if(!strcmp(argv[9],"multi")) frontier_strategy=1;
    else if(!strcmp(argv[9],"count")) frontier_strategy=2;
    else return 0;
    r->index_bytes=4;r->mode=0;r->shaders=argv[3];r->reference=argv[4];
    r->image_bytes=(size_t)r->width*r->height*8+256;return 1;
}
static int reuse_file(Reuse *r,unsigned frame,const char *suffix,size_t bytes,unsigned char **out) {
    char path[4096];int n=snprintf(path,sizeof(path),"%s/frame-%u.%s",r->reference,frame,suffix);
    NEED(n>=0 && (size_t)n<sizeof(path));FILE *f=fopen(path,"rb");if(!f) { perror(path);return 0; }
    *out=malloc(bytes);int okay=*out && fread(*out,1,bytes,f)==bytes && fgetc(f)==EOF && !ferror(f);
    if(fclose(f)) okay=0;return okay;
}
static int reuse_reference(Reuse *r) {
    for(unsigned i=0;i<8;++i)
        NEED(reuse_file(r,i,"images",r->image_bytes,&r->images[i]) && reuse_file(r,i,"geometry",REUSE_MESH_BYTES,&r->mesh[i]));
    NEED(memcmp(r->images[0],r->images[1],r->image_bytes));return 1;
}
static unsigned reuse_variant(unsigned generation) { return generation%8; }
static unsigned reuse_active(unsigned variant) {
    const unsigned counts[]={frontier_records,0,1,frontier_records/2,frontier_records+7,frontier_records,0,1};
    return counts[variant];
}
static void reuse_control(unsigned char bytes[REUSE_CONTROL_BYTES],unsigned variant) {
    memset(bytes,0xa5,REUSE_CONTROL_BYTES);
    uint32_t values[]={variant%2,reuse_active(variant),frontier_records,frontier_strategy==2};memcpy(bytes+64,values,sizeof(values));
}
static int reuse_verify(Reuse *r,unsigned frame,const unsigned char *images,const unsigned char *mesh,const unsigned char *control) {
    unsigned generation=frame/r->slots,variant=reuse_variant(generation);
    unsigned char expected[REUSE_CONTROL_BYTES];reuse_control(expected,variant);
    NEED(!memcmp(images,r->images[variant],r->image_bytes) && !memcmp(mesh,r->mesh[variant],REUSE_MESH_BYTES)
        && !memcmp(control,expected,sizeof(expected)));
    return 1;
}
static int reuse_check(Reuse *r,unsigned frame,const unsigned char *images,const unsigned char *mesh,const unsigned char *control) {
    unsigned generation=frame/r->slots,variant=reuse_variant(generation);
    NEED(reuse_verify(r,frame,images,mesh,control));
    NEED(r->pending && frame==r->checked);--r->pending;++r->checked;
    printf("RANGE_FRAME {\"frame\":%u,\"slot\":%u,\"generation\":%u,\"phase\":%u,\"active\":%u}\n",
        frame,frame%r->slots,generation,variant%2,reuse_active(variant));return 1;
}
static void reuse_submitted(Reuse *r) {
    ++r->submitted;++r->pending;if(r->pending>r->peak) r->peak=r->pending;
}
static int reuse_summary(Reuse *r) {
    NEED(r->submitted==r->frames && r->checked==r->frames && r->pending==0 && r->peak==r->slots);
    NEED(r->encodes==(r->replay ? r->slots : r->frames));
    size_t upload=SCENE_DRAW_BYTES>256 ? SCENE_DRAW_BYTES : 256;
    printf("RANGE_SUMMARY {\"frames\":%u,\"slots\":%u,\"capacity\":%u,\"strategy\":%u,\"replay\":%u,"
        "\"encodes\":%u,\"peak_unretired\":%u,\"requested_bytes\":%zu}\n",
        r->frames,r->slots,frontier_records,frontier_strategy,r->replay,r->encodes,r->peak,
        r->slots*((size_t)r->width*r->height*16+1232+2*SCENE_DRAW_BYTES+upload));return 1;
}
static void reuse_free(Reuse *r) { for(unsigned i=0;i<8;++i) { free(r->images[i]);free(r->mesh[i]); } }
#endif
