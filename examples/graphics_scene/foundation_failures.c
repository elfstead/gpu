/* Test-only link wrappers: real completion is never fabricated. Failure is
 * injected on the first scene frame, after the initial upload really completes. */
#define main scene_main
#ifdef SCENE_FOUNDATION_FRONTIER_FAILURES
#include "frontier_foundation.c"
#elif defined(SCENE_TEXTURE_FAILURES)
#include "textured.c"
#else
#include "foundation.c"
#endif
#undef main
static const char *failure;
static unsigned ends, submits, waits;
ogpu_next_status __real_ogpu_next_commands_end(ogpu_next_encoder *,ogpu_next_list **);
ogpu_next_status __wrap_ogpu_next_commands_end(ogpu_next_encoder *e,ogpu_next_list **out) {
    if(++ends==2 && !strcmp(failure,"end")) {
        ogpu_next_commands_cancel(e); *out=NULL;
        fputs("INJECT end\n",stderr); return OGPU_NEXT_INVALID;
    }
    return __real_ogpu_next_commands_end(e,out);
}
ogpu_next_status __real_ogpu_next_queue_submit(ogpu_next_queue *,const ogpu_next_submit_desc *);
ogpu_next_status __wrap_ogpu_next_queue_submit(ogpu_next_queue *q,const ogpu_next_submit_desc *d) {
    if(++submits==2 && (!strcmp(failure,"submit") || !strcmp(failure,"unknown"))) {
        fprintf(stderr,"INJECT %s\n",failure);
        return !strcmp(failure,"submit") ? OGPU_NEXT_INVALID : OGPU_NEXT_BACKEND_ERROR;
    }
    return __real_ogpu_next_queue_submit(q,d);
}
ogpu_next_status __real_ogpu_next_timeline_wait(ogpu_next_point,uint64_t);
ogpu_next_status __wrap_ogpu_next_timeline_wait(ogpu_next_point p,uint64_t timeout) {
    if(++waits==2 && !strcmp(failure,"wait")) {
        fputs("INJECT wait\n",stderr); return OGPU_NEXT_BACKEND_ERROR;
    }
    return __real_ogpu_next_timeline_wait(p,timeout);
}
void __real_ogpu_next_arena_destroy(ogpu_next_arena *);
void __wrap_ogpu_next_arena_destroy(ogpu_next_arena *a) {
    fputs("TEARDOWN arena\n",stderr); __real_ogpu_next_arena_destroy(a);
}
void __real_ogpu_next_memory_destroy(ogpu_next_memory *);
void __wrap_ogpu_next_memory_destroy(ogpu_next_memory *m) {
    fputs("TEARDOWN memory\n",stderr); __real_ogpu_next_memory_destroy(m);
}
void __real_ogpu_next_device_destroy(ogpu_next_device *);
void __wrap_ogpu_next_device_destroy(ogpu_next_device *d) {
    fputs("TEARDOWN device\n",stderr); __real_ogpu_next_device_destroy(d);
}
int main(int argc,char **argv) {
#ifdef SCENE_FOUNDATION_FRONTIER_FAILURES
    if(argc!=8) return 2;
#elif defined(SCENE_TEXTURE_FAILURES)
    if(argc!=5) return 2;
#else
    if(argc!=7) return 2;
#endif
    failure=argv[1]; return scene_main(argc-1,argv+1);
}
