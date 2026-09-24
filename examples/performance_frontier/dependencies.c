/* P4: independent B between producer A and consumer C, native and public API.
 * Shared setup/oracle with prior controls; native execution never links OGPU. */
#define FRONTIER_NO_MAIN
#include "small.c"

typedef struct {
    Context base;
    unsigned elements[2], order;
    const char *strategy;
    uint32_t *host[2];
#ifdef FRONTIER_NATIVE
    VkEvent event;
    PFN_vkCreateEvent create_event;
    PFN_vkDestroyEvent destroy_event;
    PFN_vkCmdResetEvent2 reset_event;
    PFN_vkGetEventStatus event_status;
    PFN_vkCmdSetEvent2 set_event;
    PFN_vkCmdWaitEvents2 wait_event;
#endif
} DependencyControl;

/* Every topological order and barrier cut covering A->C. */
static const char *const orders[] = {"a-bc", "ab-c", "ba-c", "a-cb"};
static const unsigned nodes[4][3] = {{0,1,2},{0,1,2},{1,0,2},{0,2,1}};
static const unsigned cuts[] = {1,2,2,1};
static size_t data_bytes(DependencyControl *d, unsigned b) {
    return ((size_t)d->elements[b] + 2 * GUARD_WORDS) * sizeof(uint32_t);
}
static int check_data(DependencyControl *d) {
    Context *c = &d->base;
    CHECK(!c->slots[0].pending);
    for (unsigned b = 0; b < 2; ++b) {
#ifdef FRONTIER_NATIVE
        CHECK(native_read(&c->native, &c->slots[b].buffer, 0, d->host[b], data_bytes(d,b)));
#else
        API(ogpu_buffer_read(c->slots[b].buffer, 0, d->host[b], data_bytes(d,b), &c->error));
#endif
        for (unsigned j = 0; j < d->elements[b] + 2 * GUARD_WORDS; ++j) {
            uint32_t want = j < GUARD_WORDS || j >= GUARD_WORDS + d->elements[b] ? sentinel
                : expected(initial(b,j-GUARD_WORDS), (uint64_t)c->slots[0].submitted * (b ? 1 : 2));
            if (d->host[b][j] != want) {
                fprintf(stderr,"P4 mismatch buffer=%u word=%u got=%u want=%u\n",b,j,d->host[b][j],want);
                return 0;
            }
        }
    }
    return 1;
}
static int dispatch_node(DependencyControl *d, unsigned node) {
    Context *c = &d->base; unsigned b = node == 1;
    TransformArguments *root = &c->slots[b].root;
    unsigned groups = (root->arg_count + transform_local[0] - 1) / transform_local[0];
#ifdef FRONTIER_NATIVE
    CHECK(native_dispatch(&c->native, &c->slots[0].batch, &c->program,
                          (Launch){.x=groups,.y=1}, root, sizeof(*root)));
#else
    API(ogpu_batch_dispatch(c->slots[0].batch,c->kernel,groups,1,1,root,sizeof(*root),&c->error));
#endif
    return 1;
}
#ifdef FRONTIER_NATIVE
static VkMemoryBarrier2 memory_dependency(void) {
    return (VkMemoryBarrier2){.sType=VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
        .srcStageMask=VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
        .srcAccessMask=VK_ACCESS_2_SHADER_WRITE_BIT,
        .dstStageMask=VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
        .dstAccessMask=VK_ACCESS_2_SHADER_READ_BIT | VK_ACCESS_2_SHADER_WRITE_BIT};
}
#endif
static int global_dependency(DependencyControl *d) {
    Context *c = &d->base;
#ifdef FRONTIER_NATIVE
    native_barrier(&c->native,c->slots[0].batch.command,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,VK_ACCESS_2_SHADER_READ_BIT|VK_ACCESS_2_SHADER_WRITE_BIT);
#else
    API(ogpu_batch_barrier(c->slots[0].batch,OGPU_ACCESS_COMPUTE_WRITE,
        OGPU_ACCESS_COMPUTE_READ|OGPU_ACCESS_COMPUTE_WRITE,&c->error));
#endif
    return 1;
}
static int cut_dependency(DependencyControl *d) {
#ifdef FRONTIER_NATIVE
    if (!strcmp(d->strategy,"buffer")) {
        Context *c=&d->base; VkMemoryBarrier2 m=memory_dependency();
        VkBufferMemoryBarrier2 b={.sType=VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER_2,
            .srcStageMask=m.srcStageMask,.srcAccessMask=m.srcAccessMask,
            .dstStageMask=m.dstStageMask,.dstAccessMask=m.dstAccessMask,
            .srcQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,.dstQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,
            .buffer=c->slots[0].buffer.buffer,.offset=GUARD_WORDS*4,.size=d->elements[0]*4};
        VkDependencyInfo info={.sType=VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
            .bufferMemoryBarrierCount=1,.pBufferMemoryBarriers=&b};
        c->native.vkCmdPipelineBarrier2(c->slots[0].batch.command,&info); return 1;
    }
#endif
    return global_dependency(d);
}
static int encode_dependencies(DependencyControl *d) {
    Context *c=&d->base; Slot *s=&c->slots[0];
#ifdef FRONTIER_NATIVE
    Native *n=&c->native; CHECK(native_begin(n,&s->batch));
    if (d->event) {
        /* Serial replay only. Order prior event waits before reset, and reset
         * before subsequent set. This prefix is outside the A/B/C graph. */
        d->reset_event(s->batch.command,d->event,VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT);
        native_barrier(n,s->batch.command,VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,0,
            VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,0);
    }
#else
    API(ogpu_recording_storage_create(c->device,&s->storage,&c->error));
    API(ogpu_batch_create_in(s->storage,&s->batch,&c->error));
#endif
    /* Equal prior-execution visibility, outside this execution's A/B/C graph. */
    CHECK(global_dependency(d));
#ifdef FRONTIER_NATIVE
    if (!strcmp(d->strategy,"split")) {
        VkMemoryBarrier2 m=memory_dependency();
        VkDependencyInfo info={.sType=VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
            .memoryBarrierCount=1,.pMemoryBarriers=&m};
        CHECK(dispatch_node(d,0)); d->set_event(s->batch.command,d->event,&info);
        CHECK(dispatch_node(d,1)); d->wait_event(s->batch.command,1,&d->event,&info);
        CHECK(dispatch_node(d,2));
    } else
#else
    if (!strcmp(d->strategy,"ogpu-split")) {
        uint64_t point=0;
        CHECK(dispatch_node(d,0));
        API(ogpu_batch_dependency_begin(s->batch,OGPU_ACCESS_COMPUTE_WRITE,
            OGPU_ACCESS_COMPUTE_READ|OGPU_ACCESS_COMPUTE_WRITE,&point,&c->error));
        CHECK(dispatch_node(d,1));
        API(ogpu_batch_dependency_end(s->batch,point,&c->error));
        CHECK(dispatch_node(d,2));
    } else
#endif
    {
        for (unsigned i=0;i<3;++i) {
            if (i==cuts[d->order]) CHECK(cut_dependency(d));
            CHECK(dispatch_node(d,nodes[d->order][i]));
        }
    }
#ifdef FRONTIER_NATIVE
    native_barrier(n,s->batch.command,VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_HOST_BIT,VK_ACCESS_2_HOST_READ_BIT);
    VK_TRY(n->vkEndCommandBuffer(s->batch.command));
#else
    for (unsigned b=0;b<2;++b) API(ogpu_batch_retain_buffer(s->batch,c->slots[b].buffer,&c->error));
    API(ogpu_batch_compile(s->batch,0,&s->list,&c->error));
    ogpu_batch_destroy(s->batch); s->batch=NULL;
#endif
    return 1;
}
static int dependency_create(DependencyControl *d) {
    Context *c=&d->base;
    c->policy="replay"; /* Keep native command storage until final teardown. */
    CHECK(create(c)); /* count=0: initialize device/program, not the old workload. */
    c->count=2;
#ifdef FRONTIER_NATIVE
    if (!strcmp(d->strategy,"split")) {
        Native *n=&c->native;
        PFN_vkGetInstanceProcAddr ig=(PFN_vkGetInstanceProcAddr)dlsym(n->library,"vkGetInstanceProcAddr");
        PFN_vkGetDeviceProcAddr get=(PFN_vkGetDeviceProcAddr)ig(n->instance,"vkGetDeviceProcAddr");
#define EVENT_LOAD(field,name) d->field=(PFN_##name)get(n->device,#name); CHECK(d->field)
        EVENT_LOAD(create_event,vkCreateEvent); EVENT_LOAD(destroy_event,vkDestroyEvent);
        EVENT_LOAD(reset_event,vkCmdResetEvent2); EVENT_LOAD(event_status,vkGetEventStatus);
        EVENT_LOAD(set_event,vkCmdSetEvent2); EVENT_LOAD(wait_event,vkCmdWaitEvents2);
#undef EVENT_LOAD
        VkEventCreateInfo info={.sType=VK_STRUCTURE_TYPE_EVENT_CREATE_INFO};
        VK_TRY(d->create_event(n->device,&info,NULL,&d->event));
    }
#endif
    for (unsigned b=0;b<2;++b) {
        size_t bytes=data_bytes(d,b); uint64_t address;
        d->host[b]=malloc(bytes); CHECK(d->host[b]);
        for (unsigned j=0;j<d->elements[b]+2*GUARD_WORDS;++j)
            d->host[b][j]=j<GUARD_WORDS || j>=GUARD_WORDS+d->elements[b] ? sentinel : initial(b,j-GUARD_WORDS);
#ifdef FRONTIER_NATIVE
        CHECK(native_buffer_create(&c->native,&c->slots[b].buffer,bytes,1));
        address=c->slots[b].buffer.address;
        CHECK(native_write(&c->native,&c->slots[b].buffer,0,d->host[b],bytes));
#else
        API(ogpu_buffer_create(c->device,bytes,OGPU_MEMORY_HOST,&c->slots[b].buffer,&c->error));
        API(ogpu_buffer_device_address(c->slots[b].buffer,&address,&c->error));
        API(ogpu_buffer_write(c->slots[b].buffer,0,d->host[b],bytes,&c->error));
#endif
        c->slots[b].root=(TransformArguments){.arg_data=address+GUARD_WORDS*4,.arg_count=d->elements[b]};
    }
    return encode_dependencies(d);
}
static void dependency_destroy(DependencyControl *d) {
    Context *c=&d->base;
    (void)wait_slot(c,&c->slots[0]);
#ifdef FRONTIER_NATIVE
    /* Commands are no longer executable before destroying their event reference. */
    native_batch_destroy(&c->native,&c->slots[0].batch);
    if (d->event) d->destroy_event(c->native.device,d->event,NULL);
#endif
    destroy(c); free(d->host[0]); free(d->host[1]);
}
static int dependency_window(DependencyControl *d,unsigned count,int validate,Frame *frames,double *wall) {
    Context *c=&d->base; Slot *s=&c->slots[0]; double begin=clock_ms();
    for (unsigned i=0;i<count;++i) {
        Frame *f=&frames[i]; f->start=clock_ms();
        CHECK(!s->pending);
        CHECK(submit_slot(c,s)); double submitted=clock_ms();
        CHECK(wait_slot(c,s)); double end=clock_ms();
        f->submit=submitted-f->start; f->wait=end-submitted; f->latency=end-f->start;
        if (validate) {
#ifdef FRONTIER_NATIVE
            if (d->event) CHECK(d->event_status(c->native.device,d->event)==VK_EVENT_SET);
#endif
            CHECK(check_data(d));
        }
    }
    *wall=clock_ms()-begin;
    return check_data(d);
}
#ifndef DEPENDENCIES_NO_MAIN
int main(int argc,char **argv) {
    (void)window;
    if (argc==2 && !strcmp(argv[1],"--selftest")) return !selftest();
    if (argc!=6) { fprintf(stderr,"Usage: dependencies global|buffer|split|ogpu|ogpu-split order kib frames validate|measure\n"); return 1; }
    DependencyControl d={0}; d.strategy=argv[1];
    for (d.order=0;d.order<4 && strcmp(argv[2],orders[d.order]);++d.order) {}
    unsigned kib=number(argv[3]),count=number(argv[4]); int validate=!strcmp(argv[5],"validate");
    if (d.order==4 || (kib!=64 && kib!=4096) || !count || (!validate && strcmp(argv[5],"measure"))) return 1;
#ifdef FRONTIER_NATIVE
    if (strcmp(d.strategy,"global") && strcmp(d.strategy,"buffer") && strcmp(d.strategy,"split")) return 1;
    if (!strcmp(d.strategy,"split") && d.order) return 1;
#else
    if (strcmp(d.strategy,"ogpu") && strcmp(d.strategy,"ogpu-split")) return 1;
    if (!strcmp(d.strategy,"ogpu-split") && d.order) return 1;
#endif
    d.elements[0]=64*1024/4; d.elements[1]=kib*1024/4;
    Frame *frames=calloc(count>100?count:100,sizeof(*frames)); if (!frames) return 1;
    double start=clock_ms(),wall=0; int okay=0;
    if (!dependency_create(&d)) goto cleanup;
    double setup=clock_ms()-start;
    if (!validate && !dependency_window(&d,100,0,frames,&wall)) goto cleanup;
    if (!dependency_window(&d,count,validate,frames,&wall)) goto cleanup;
    printf("DEPENDENCY_RESULT {\"strategy\":\"%s\",\"order\":\"%s\",\"kib\":%u,\"frames\":%u,\"warmups\":%u,\"validation\":%s,\"setup_ms\":%.9f,\"wall_ms\":%.9f,\"event_count\":%u}\n",
        d.strategy,orders[d.order],kib,count,validate?0:100,validate?"true":"false",setup,wall,
        !strcmp(d.strategy,"split") || !strcmp(d.strategy,"ogpu-split"));
    if (!validate) for (unsigned i=0;i<count;++i)
        printf("FRAME {\"index\":%u,\"submit_ms\":%.9f,\"wait_ms\":%.9f,\"latency_ms\":%.9f}\n",i,frames[i].submit,frames[i].wait,frames[i].latency);
    puts("P4 exact X/Y outputs and guards PASS; all executions drained"); okay=1;
cleanup:
    dependency_destroy(&d); free(frames); return !okay;
}
#endif
