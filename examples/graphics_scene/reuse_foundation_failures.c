/* Test only. No successful execution/completion is synthesized. */
#include "reuse_cpu_trace.h"
#define main reuse_main
#include "reuse_foundation.c"
#undef main
static const char *fault;
static unsigned flushes,ends,submits,waits;
static int inject(const char *name,unsigned call,unsigned wanted) {
    if(strcmp(fault,name) || call!=wanted) return 0;
    fprintf(stderr,"INJECT %s\n",name); return 1;
}
ogpu_next_status __real_ogpu_next_memory_flush(ogpu_next_span);
ogpu_next_status __wrap_ogpu_next_memory_flush(ogpu_next_span s) {
    ++flushes;
    if(inject("flush1",flushes,1) || inject("flush3",flushes,3) || inject("flush6",flushes,6)) return OGPU_NEXT_INVALID;
    return __real_ogpu_next_memory_flush(s);
}
ogpu_next_status __real_ogpu_next_commands_end(ogpu_next_encoder *,ogpu_next_list **);
ogpu_next_status __wrap_ogpu_next_commands_end(ogpu_next_encoder *e,ogpu_next_list **out) {
    if(inject("end",++ends,4)) { ogpu_next_commands_cancel(e); *out=NULL; return OGPU_NEXT_INVALID; }
    return __real_ogpu_next_commands_end(e,out);
}
ogpu_next_status __real_ogpu_next_queue_submit(ogpu_next_queue *,const ogpu_next_submit_desc *);
ogpu_next_status __wrap_ogpu_next_queue_submit(ogpu_next_queue *q,const ogpu_next_submit_desc *d) {
    ++submits;
    if(inject("submit",submits,4)) return OGPU_NEXT_INVALID;
    if(inject("unknown",submits,4)) return OGPU_NEXT_BACKEND_ERROR;
    return __real_ogpu_next_queue_submit(q,d);
}
ogpu_next_status __real_ogpu_next_timeline_wait(ogpu_next_point,uint64_t);
ogpu_next_status __wrap_ogpu_next_timeline_wait(ogpu_next_point p,uint64_t timeout) {
    ++waits;
    if(inject("wait",waits,3) || inject("lost",waits,3) || (!strcmp(fault,"lost") && waits>3)) return OGPU_NEXT_BACKEND_ERROR;
    return __real_ogpu_next_timeline_wait(p,timeout);
}
void __real_ogpu_next_arena_destroy(ogpu_next_arena *);
void __wrap_ogpu_next_arena_destroy(ogpu_next_arena *a) {
    if(a) fputs("TEARDOWN arena\n",stderr);
    __real_ogpu_next_arena_destroy(a);
}
void __real_ogpu_next_memory_destroy(ogpu_next_memory *);
void __wrap_ogpu_next_memory_destroy(ogpu_next_memory *m) {
    if(m) fputs("TEARDOWN memory\n",stderr);
    __real_ogpu_next_memory_destroy(m);
}
int main(int argc,char **argv) {
    if(argc!=11) return 2;
    /* Preserve setup identity through the deliberately tested _Exit path. */
    if(setvbuf(stdout,NULL,_IONBF,0)) return 2;
    fault=argv[1]; int result=reuse_main(argc-1,argv+1);
    scene_cpu_summary(); return result;
}
