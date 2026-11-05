/* Explicit native batch boundaries; all scratch and synchronization are caller-owned. */
static int submission_batches(ogpu_next_device *device, ogpu_next_memory_desc memory_desc, uint32_t domain) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_memory *memory = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *edge = NULL, *gate = NULL, *done = NULL;
    void *scratch = NULL;
    ogpu_next_queue *queue = ogpu_next_device_queue(device,domain,0);
    ogpu_next_submit_desc counts[3] = {{.list_count=1,.signal_count=1},{0},{.list_count=1,.wait_count=2,.signal_count=1}};
    ogpu_next_host_requirements req = {0};
    TRY(ogpu_next_submit_scratch_requirements(device,3,counts,&req));
    REQUIRE(req.alignment <= _Alignof(max_align_t) && req.size <= SIZE_MAX-16);
    scratch = malloc((size_t)req.size+16); REQUIRE(scratch != NULL);
    memset(scratch,0xa5,(size_t)req.size+16);
    ogpu_next_host_span batch_scratch = {scratch,req.size};
    ogpu_next_submit_outcome outcomes[3] = {{0}};
    ogpu_next_host_requirements zero = {1,1};
    TRY(ogpu_next_submit_scratch_requirements(device,0,NULL,&zero)); REQUIRE(zero.size == 0 && zero.alignment == 1);
    TRY(ogpu_next_queue_submit(queue,0,NULL,NULL,(ogpu_next_host_span){NULL,0}));
    zero.size = 777; zero.alignment = 32;
    REQUIRE(ogpu_next_submit_scratch_requirements(device,1,NULL,&zero) == OGPU_NEXT_INVALID);
    REQUIRE(zero.size == 777 && zero.alignment == 32);
    REQUIRE(ogpu_next_queue_submit(NULL,3,counts,outcomes,batch_scratch) == OGPU_NEXT_INVALID);
    for (unsigned i=0;i<3;++i) REQUIRE(outcomes[i].status==OGPU_NEXT_INVALID && outcomes[i].acceptance==OGPU_NEXT_SUBMIT_REJECTED);
    REQUIRE(ogpu_next_queue_submit(queue,3,NULL,outcomes,batch_scratch) == OGPU_NEXT_INVALID);
    for (unsigned i=0;i<3;++i) REQUIRE(outcomes[i].status==OGPU_NEXT_INVALID && outcomes[i].acceptance==OGPU_NEXT_SUBMIT_REJECTED);
    TRY(ogpu_next_memory_create(device,&memory_desc,&memory));
    ogpu_next_span all = {memory,0,memory_desc.size};
    ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    memset(map.data,0xa5,(size_t)memory_desc.size);
    TRY(ogpu_next_memory_flush(all));
    TRY(ogpu_next_timeline_create(device,0,&edge));
    TRY(ogpu_next_timeline_create(device,0,&gate));
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,2,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *lists[2] = {NULL};
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_fill_memory(encoder,(ogpu_next_span){memory,0,256},UINT32_C(0x12345678));
    TRY(ogpu_next_commands_end(encoder,&lists[0]));
    rd.replay_mode = OGPU_NEXT_SERIAL_REPLAY;
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_copy_memory(encoder,(ogpu_next_span){memory,512,256},(ogpu_next_span){memory,0,256});
    ogpu_next_barrier_scope dep_scope = {OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};
    max_align_t dep_bytes[16];
    ogpu_next_host_span dep_scratch = {dep_bytes,sizeof(dep_bytes)};
    ogpu_next_dependency dep = {1,0,0,0,&dep_scope,NULL,NULL};
    common_barrier(encoder,&dep,&dep_scope,dep_scratch);
    TRY(ogpu_next_commands_end(encoder,&lists[1]));
    ogpu_next_sync_point producer = {{edge,1},OGPU_NEXT_STAGE_CLEAR};
    ogpu_next_sync_point waits[2] = {{{edge,1},OGPU_NEXT_STAGE_COPY},{{gate,1},OGPU_NEXT_STAGE_ALL}};
    ogpu_next_sync_point consumer = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc batches[3] = {
        {1,0,1,0,0,0,&lists[0],NULL,&producer,NULL,NULL},
        {0},
        {1,2,1,0,0,0,&lists[1],waits,&consumer,NULL,NULL}
    };
    REQUIRE(ogpu_next_queue_submit(queue,3,batches,outcomes,(ogpu_next_host_span){scratch,req.size-1}) == OGPU_NEXT_CAPACITY);
    for (unsigned i=0;i<3;++i) REQUIRE(outcomes[i].status==OGPU_NEXT_CAPACITY && outcomes[i].acceptance==OGPU_NEXT_SUBMIT_REJECTED);
    REQUIRE(ogpu_next_queue_submit(queue,3,batches,outcomes,(ogpu_next_host_span){NULL,req.size}) == OGPU_NEXT_INVALID);
    REQUIRE(ogpu_next_queue_submit(queue,3,batches,NULL,batch_scratch) == OGPU_NEXT_INVALID);
    batches[2].flags = 1;
    REQUIRE(ogpu_next_queue_submit(queue,3,batches,outcomes,batch_scratch) == OGPU_NEXT_UNSUPPORTED);
    for (unsigned i=0;i<3;++i) REQUIRE(outcomes[i].status==OGPU_NEXT_UNSUPPORTED && outcomes[i].acceptance==OGPU_NEXT_SUBMIT_REJECTED);
    batches[2].flags = 0;
    uint64_t value = UINT64_MAX;
    TRY(ogpu_next_timeline_poll(edge,&value)); REQUIRE(value == 0);
    pending = 1; TRY(ogpu_next_queue_submit(queue,3,batches,outcomes,batch_scratch));
    for (unsigned i=0;i<3;++i) REQUIRE(outcomes[i].status==OGPU_NEXT_OK && outcomes[i].acceptance==OGPU_NEXT_SUBMIT_ACCEPTED);
    /* All translation scratch can be overwritten immediately while work is pending. */
    for (uint64_t j=req.size;j<req.size+16;++j) REQUIRE(((unsigned char *)scratch)[j] == 0xa5);
    memset(scratch,0xcc,(size_t)req.size);
    TRY(ogpu_next_timeline_poll(done,&value)); REQUIRE(value == 0);
    TRY(ogpu_next_timeline_signal_host((ogpu_next_point){gate,1}));
    TRY(ogpu_next_timeline_wait(consumer.point,UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t i = 0; i < memory_desc.size; ++i) {
        if (i < 256 || (i >= 512 && i < 768)) REQUIRE(((uint32_t *)map.data)[i/4] == UINT32_C(0x12345678));
        else REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    REQUIRE(ogpu_next_queue_submit(queue,3,batches,outcomes,batch_scratch) == OGPU_NEXT_INVALID); /* One-shot consumed. */
    /* The independent serial list remains replayable; keep its explicit waits. */
    consumer.point.value = 2;
    pending = 1; TRY(ogpu_next_queue_submit(queue,1,&batches[2],outcomes,batch_scratch));
    TRY(ogpu_next_timeline_wait(consumer.point,UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t i = 128; i < 192; ++i) REQUIRE(((uint32_t *)map.data)[i] == UINT32_C(0x12345678));
    printf("Submission batches pass: ordered producer/empty/consumer boundaries, timeline edge, host gate, scratch lifetime, atomic rejection and mixed replay modes.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending batch submission after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_memory_destroy(memory);
    ogpu_next_timeline_destroy(edge); ogpu_next_timeline_destroy(gate); ogpu_next_timeline_destroy(done);
    free(scratch);
    return result;
}
