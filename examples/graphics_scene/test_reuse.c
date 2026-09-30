#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define NEED(x) do { if(!(x)) return 0; } while(0)
#include "reuse.h"

int main(void) {
    unsigned value=123;
    for(unsigned i=0;i<=1000;++i) {
        char text[16];snprintf(text,sizeof(text),"%u",i);
        assert(reuse_number(text,0,1000,&value) && value==i);
    }
    const char *bad[]={"","-1","+1","1x"," 1","1001","4294967296","9999999999999999999999999999999"};
    for(unsigned i=0;i<sizeof(bad)/sizeof(bad[0]);++i) { value=123;assert(!reuse_number(bad[i],0,1000,&value) && value==123); }
    unsigned char images[3][8],mesh[3][584];
    for(unsigned i=0;i<3;++i) { memset(images[i],i,8);memset(mesh[i],i,584); }
    for(unsigned slots=1;slots<=2;++slots) {
        Reuse r={.frames=1000,.slots=slots,.image_bytes=8};
        for(unsigned i=0;i<3;++i) { r.images[i]=images[i];r.mesh[i]=mesh[i]; }
        for(unsigned f=0;f<1000;++f) {
            unsigned v=reuse_variant(f/slots);assert(v==(f/slots%4==1 ? 1u : f/slots%4==2 ? 2u : 0u));
            unsigned char control[136],image[8],geometry[584];
            reuse_control(control,v);memcpy(image,images[v],8);memcpy(geometry,mesh[v],584);
            reuse_submitted(&r);unsigned checked=r.checked,pending=r.pending;
            image[0]^=1;assert(!reuse_check(&r,f,image,geometry,control));image[0]^=1;
            geometry[583]^=1;assert(!reuse_check(&r,f,image,geometry,control));geometry[583]^=1;
            for(unsigned k=0;k<136;++k) {
                control[k]^=1;assert(!reuse_check(&r,f,image,geometry,control));control[k]^=1;
            }
            assert(r.checked==checked && r.pending==pending);
            assert(reuse_check(&r,f,image,geometry,control));
        }
        assert(r.checked==1000 && r.pending==0);
    }
    puts("Reuse schedule/input guards CPU PASS");return 0;
}
