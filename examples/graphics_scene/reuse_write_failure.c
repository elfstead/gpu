/* Test-only C-boundary injection: fail one HOST write before calling OGPU.
   No synthetic GPU success/loss and no change to real submission completion. */
#define _GNU_SOURCE
#include "ogpu.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
OgpuResult ogpu_buffer_write(OgpuBuffer *buffer, uint64_t offset, const void *data,
                             uint64_t bytes, OgpuError *error) {
    static unsigned calls;
    const char *mode=getenv("OGPU_SCENE_FAIL_WRITE");
    unsigned fail=mode ? (unsigned)strtoul(mode,NULL,10) : 0;
    if(++calls==fail) {
        if(error) { memset(error,0,sizeof(*error));strcpy(error->message,"test-only HOST write rejection"); }
        fprintf(stderr,"WRITE_FAILURE {\"call\":%u}\n",calls);return OGPU_ERROR_INTERNAL;
    }
    typedef OgpuResult (*Write)(OgpuBuffer *,uint64_t,const void *,uint64_t,OgpuError *);
    Write real=(Write)dlsym(RTLD_NEXT,"ogpu_buffer_write");
    if(!real) abort();return real(buffer,offset,data,bytes,error);
}
