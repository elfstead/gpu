/* Explicit native batch boundaries; all scratch and synchronization are caller-owned. */
static int submission_batches(ogpu_next_device *device, ogpu_next_memory_desc memory_desc, uint32_t domain) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_memory *memory = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *edge = NULL, *gate = NULL, *done = NULL;
    void *scratch[3] = {NULL};
    ogpu_next_queue *queue = ogpu_next_device_queue(device,domain,0);
    ogpu_next_host_requirements req[3] = {{0}};
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&req[0]));
    TRY(ogpu_next_submit_scratch_requirements(1,2,1,&req[1]));
    TRY(ogpu_next_submit_batch_scratch_requirements(3,&req[2]));
    for (uint32_t i = 0; i < 3; ++i) {
        REQUIRE(req[i].alignment <= _Alignof(max_align_t) && req[i].size <= SIZE_MAX-16);
        scratch[i] = malloc((size_t)req[i].size+16); REQUIRE(scratch[i] != NULL);
        memset(scratch[i],0xa5,(size_t)req[i].size+16);
    }
    ogpu_next_host_span batch_scratch = {scratch[2],req[2].size};
    ogpu_next_host_requirements zero = {1,1};
    TRY(ogpu_next_submit_batch_scratch_requirements(0,&zero)); REQUIRE(zero.size == 0 && zero.alignment == 1);
    TRY(ogpu_next_queue_submit_batch(queue,0,NULL,(ogpu_next_host_span){NULL,0}));
    TRY(ogpu_next_memory_create(device,&memory_desc,&memory));
    ogpu_next_span all = {memory,0,memory_desc.size};
    ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    memset(map.data,0xa5,(size_t)memory_desc.size);
    TRY(ogpu_next_memory_flush(all));
    TRY(ogpu_next_timeline_create(device,0,&edge));
    TRY(ogpu_next_timeline_create(device,0,&gate));
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,2};
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
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,
        OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&lists[1]));
    ogpu_next_sync_point producer = {{edge,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_sync_point waits[2] = {{{edge,1},OGPU_NEXT_STAGE_COPY},{{gate,1},OGPU_NEXT_STAGE_ALL}};
    ogpu_next_sync_point consumer = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc batches[3] = {
        {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&lists[0],NULL,&producer,scratch[0],req[0].size},
        {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),0,0,0,NULL,NULL,NULL,NULL,0},
        {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,2,1,&lists[1],waits,&consumer,scratch[1],req[1].size}
    };
    REQUIRE(ogpu_next_queue_submit_batch(queue,3,batches,(ogpu_next_host_span){scratch[2],req[2].size-1}) == OGPU_NEXT_CAPACITY);
    REQUIRE(ogpu_next_queue_submit_batch(queue,3,batches,(ogpu_next_host_span){NULL,req[2].size}) == OGPU_NEXT_INVALID);
    batches[2].scratch_size = 0;
    REQUIRE(ogpu_next_queue_submit_batch(queue,3,batches,batch_scratch) == OGPU_NEXT_CAPACITY);
    batches[2].scratch_size = req[1].size;
    batches[2].header.version = OGPU_NEXT_VERSION+1;
    REQUIRE(ogpu_next_queue_submit_batch(queue,3,batches,batch_scratch) == OGPU_NEXT_UNSUPPORTED);
    batches[2].header.version = OGPU_NEXT_VERSION;
    uint64_t value = UINT64_MAX;
    TRY(ogpu_next_timeline_poll(edge,&value)); REQUIRE(value == 0);
    pending = 1; TRY(ogpu_next_queue_submit_batch(queue,3,batches,batch_scratch));
    /* All translation scratch can be overwritten immediately while work is pending. */
    for (uint32_t i = 0; i < 3; ++i) {
        for (uint64_t j = req[i].size; j < req[i].size+16; ++j) REQUIRE(((unsigned char *)scratch[i])[j] == 0xa5);
        memset(scratch[i],0xcc,(size_t)req[i].size);
    }
    TRY(ogpu_next_timeline_poll(done,&value)); REQUIRE(value == 0);
    TRY(ogpu_next_timeline_signal_host((ogpu_next_point){gate,1}));
    TRY(ogpu_next_timeline_wait(consumer.point,UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t i = 0; i < memory_desc.size; ++i) {
        if (i < 256 || (i >= 512 && i < 768)) REQUIRE(((uint32_t *)map.data)[i/4] == UINT32_C(0x12345678));
        else REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    REQUIRE(ogpu_next_queue_submit_batch(queue,3,batches,batch_scratch) == OGPU_NEXT_INVALID); /* One-shot consumed. */
    /* The independent serial list remains replayable; keep its explicit waits. */
    consumer.point.value = 2;
    pending = 1; TRY(ogpu_next_queue_submit_batch(queue,1,&batches[2],batch_scratch));
    TRY(ogpu_next_timeline_wait(consumer.point,UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t i = 128; i < 192; ++i) REQUIRE(((uint32_t *)map.data)[i] == UINT32_C(0x12345678));
    printf("Submission batches pass: ordered producer/empty/consumer boundaries, timeline edge, host gate, scratch lifetime, atomic rejection and mixed replay modes.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending batch submission after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_memory_destroy(memory);
    ogpu_next_timeline_destroy(edge); ogpu_next_timeline_destroy(gate); ogpu_next_timeline_destroy(done);
    for (uint32_t i = 0; i < 3; ++i) free(scratch[i]);
    return result;
}
