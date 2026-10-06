#include <stdlib.h>
#include <stdio.h>
#include <assert.h>
#define SCENE_ARGUMENT_DIAGNOSTICS
#include "argument_reuse.h"
int main(void) {
    uint64_t root[32];memset(root,0xa5,sizeof(root));
    argument_prepare(root,UINT64_C(0x1122334455667788));
    assert(root[0]==UINT64_C(0x1122334455667788));
    for(unsigned i=0;i<(SCENE_RASTER_BYTES-8)/4;++i) {
        uint32_t value;memcpy(&value,(unsigned char *)root+8+4*i,4);assert(value==i+1);
    }
    for(unsigned i=SCENE_RASTER_BYTES;i<sizeof(root);++i) assert(((unsigned char *)root)[i]==0xa5);
    const unsigned capacities[]={1,64,512};
    for(unsigned c=0;c<3;++c) for(argument_reverse=0;argument_reverse<2;++argument_reverse) {
        unsigned char seen[512]={0};unsigned n=capacities[c];
        for(unsigned i=0;i<n;++i) {
            unsigned index=argument_index(i,n);assert(index<n && !seen[index]);seen[index]=1;
            assert(index==(argument_reverse ? n-1-i : i));
        }
    }
    ARGUMENT_SUPPLIED();ARGUMENT_SUPPLIED();
    assert(argument_calls==2 && argument_bytes==2*SCENE_RASTER_BYTES);
    puts("Argument root construction/order/accounting PASS");return 0;
}
