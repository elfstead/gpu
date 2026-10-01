#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define NEED(x) do { if(!(x)) return 0; } while(0)
static unsigned frontier_records,frontier_strategy;
#define SCENE_DRAW_BYTES (132u+20u*frontier_records)
#include "range_reuse.h"
int main(void) {
    unsigned number=42;
    const char *bad[]={"","-1","+1","1x"," 1","1001","4294967296","999999999999999999999"};
    for(unsigned i=0;i<sizeof(bad)/sizeof(bad[0]);++i) assert(!reuse_number(bad[i],0,1000,&number) && number==42);
    for(unsigned c=0;c<3;++c) {
        frontier_records=(unsigned[]){1,64,512}[c];
        for(frontier_strategy=0;frontier_strategy<3;++frontier_strategy)
        for(unsigned slots=1;slots<=2;++slots) {
            Reuse r={.frames=32,.slots=slots,.image_bytes=8};
            for(unsigned i=0;i<8;++i) {
                r.images[i]=malloc(8);r.mesh[i]=malloc(REUSE_MESH_BYTES);assert(r.images[i] && r.mesh[i]);
                memset(r.images[i],i,8);memset(r.mesh[i],i,REUSE_MESH_BYTES);
            }
            for(unsigned f=0;f<32;++f) {
                unsigned variant=reuse_variant(f/slots);assert(variant==(f/slots)%8);
                unsigned char control[REUSE_CONTROL_BYTES],image[8],mesh[REUSE_MESH_BYTES];
                reuse_control(control,variant);memcpy(image,r.images[variant],8);memcpy(mesh,r.mesh[variant],sizeof(mesh));
                uint32_t words[4];memcpy(words,control+64,sizeof(words));
                assert(words[0]==variant%2 && words[1]==reuse_active(variant) && words[2]==frontier_records && words[3]==(frontier_strategy==2));
                reuse_submitted(&r);unsigned checked=r.checked,pending=r.pending;
                image[7]^=1;assert(!reuse_check(&r,f,image,mesh,control));image[7]^=1;
                for(size_t i=0;i<sizeof(mesh);++i) { mesh[i]^=1;assert(!reuse_check(&r,f,image,mesh,control));mesh[i]^=1; }
                for(size_t i=0;i<sizeof(control);++i) { control[i]^=1;assert(!reuse_check(&r,f,image,mesh,control));control[i]^=1; }
                assert(r.checked==checked && r.pending==pending);
                assert(reuse_check(&r,f,image,mesh,control));
            }
            assert(r.checked==32 && !r.pending);reuse_free(&r);
        }
    }
    puts("Range reuse schedule/count/guard CPU PASS");return 0;
}
