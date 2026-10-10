/* Independently recorded split endpoints, explicit reset cycles and caller storage. */
static int split_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain, uint32_t replay_mode) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_split *splits[2] = {NULL};
    ogpu_next_arena *producer = NULL, *consumer = NULL;
    ogpu_next_memory *host = NULL, *backing = NULL;
    ogpu_next_image *image = NULL;
    ogpu_next_timeline *done = NULL, *gate = NULL;
    void *ranges[2] = {NULL}, *wait_scratch = NULL, *submit_scratch = NULL;
    ogpu_next_host_requirements br[2] = {{0}}, wr = {0}, sr = {0};
    TRY(ogpu_next_split_create(device, &splits[0])); TRY(ogpu_next_split_create(device, &splits[1]));
    TRY(ogpu_next_memory_create(device, &md, &host));
    ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map((ogpu_next_span){host,0,md.size}, &map));
    ogpu_next_image_desc image_desc = {HEADER(ogpu_next_image_desc, OGPU_NEXT_IMAGE_DESC),
        OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_IMAGE_2D, 1, 1, 1, {4,4,1},
        OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST, 0, NULL, 0, 0, NULL};
    uint32_t types[32];
    ogpu_next_requirements req = {0}; req.compatible_type_capacity = 32; req.compatible_types = types;
    TRY(ogpu_next_image_requirements(device, &image_desc, &req)); REQUIRE(req.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device, &image_desc, &image));
    TRY(ogpu_next_memory_create_dedicated_image(image, types[0], &backing));
    TRY(ogpu_next_image_bind(image, (ogpu_next_span){backing,0,req.size}));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1, OGPU_NEXT_PRIMARY, 0};
    TRY(ogpu_next_arena_create(device, &ad, &producer)); TRY(ogpu_next_arena_create(device, &ad, &consumer));
    TRY(ogpu_next_barrier_scratch_requirements(1,0,&br[0]));
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br[1]));
    TRY(ogpu_next_split_scratch_requirements(2,&wr));
    TRY(ogpu_next_submit_scratch_requirements(2,1,1,&sr));
    REQUIRE(br[0].alignment <= _Alignof(max_align_t) && br[1].alignment <= _Alignof(max_align_t));
    REQUIRE(wr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    ranges[0] = malloc((size_t)br[0].size); ranges[1] = malloc((size_t)br[1].size);
    wait_scratch = malloc((size_t)wr.size); submit_scratch = malloc((size_t)sr.size);
    REQUIRE(ranges[0] && ranges[1] && wait_scratch && submit_scratch);
    ogpu_next_memory_barrier memory_barrier = {{host,1024,128}, OGPU_NEXT_ACCESS_COPY_WRITE, OGPU_NEXT_ACCESS_COPY_READ,
        OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED};
    ogpu_next_subresources all_texels = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_image_barrier image_barrier = {image, all_texels, OGPU_NEXT_ACCESS_COPY_WRITE, OGPU_NEXT_ACCESS_COPY_READ,
        OGPU_NEXT_STATE_COPY_DST, OGPU_NEXT_STATE_COPY_SRC, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 0};
    ogpu_next_dependency deps[2] = {
        {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY), OGPU_NEXT_STAGE_COPY, OGPU_NEXT_STAGE_COPY,
            0,0,1,0,0,&memory_barrier,NULL,ranges[0],br[0].size},
        {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY), OGPU_NEXT_STAGE_CLEAR, OGPU_NEXT_STAGE_COPY,
            OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_READ,0,1,0,NULL,&image_barrier,ranges[1],br[1].size}
    };
    ogpu_next_split_dependency waits[2] = {{splits[0],&deps[0]},{splits[1],&deps[1]}};
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC), replay_mode, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *lists[2] = {NULL};
    /* Record acquire FIRST on the CPU: no runtime endpoint registry dictates
     * recording order. Producer still executes first on the same native queue. */
    TRY(ogpu_next_commands_begin(consumer,&rd,&encoder));
    ogpu_next_split_acquire(encoder,0,NULL,(ogpu_next_host_span){NULL,0});
    ogpu_next_split_acquire(encoder,2,waits,(ogpu_next_host_span){wait_scratch,wr.size});
    memset(wait_scratch,0xcc,(size_t)wr.size);
    for (uint32_t i = 0; i < 2; ++i) memset(ranges[i],0xcc,(size_t)br[i].size);
    ogpu_next_copy_memory(encoder,(ogpu_next_span){host,512,128},(ogpu_next_span){host,1024,128});
    ogpu_next_copy_memory(encoder,(ogpu_next_span){host,768,128},(ogpu_next_span){host,2048,128});
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{4,4,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,1536,64},image,&copy);
    ogpu_next_dependency visibility = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY), OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,
        OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&visibility);
    TRY(ogpu_next_commands_end(encoder,&lists[1]));
    TRY(ogpu_next_commands_begin(producer,&rd,&encoder));
    for (uint32_t i = 0; i < 2; ++i) ogpu_next_split_reset(encoder,splits[i],OGPU_NEXT_STAGE_ALL);
    ogpu_next_dependency reset_edge = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_ALL,
        0,0,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&reset_edge); /* Explicit reset -> future release ordering. */
    ogpu_next_image_barrier initial = image_barrier;
    initial.before = 0; initial.after = OGPU_NEXT_ACCESS_COPY_WRITE;
    initial.old_state = OGPU_NEXT_STATE_UNDEFINED; initial.new_state = OGPU_NEXT_STATE_COPY_DST; initial.discard = 1;
    ogpu_next_dependency transition = deps[1]; transition.before = 0; transition.global_before = 0;
    transition.after = OGPU_NEXT_STAGE_CLEAR; transition.global_after = 0; transition.images = &initial;
    ogpu_next_barrier(encoder,&transition);
    ogpu_next_copy_memory(encoder,(ogpu_next_span){host,1024,128},(ogpu_next_span){host,0,128});
    ogpu_next_split_release(encoder,splits[0],&deps[0]);
    /* Independent work between release A and the eventual acquire. */
    ogpu_next_fill_memory(encoder,(ogpu_next_span){host,2048,128},UINT32_C(0x10203040));
    ogpu_next_clear_value blue = {.f32 = {0,0,1,1}};
    ogpu_next_clear_image(encoder,image,OGPU_NEXT_STATE_COPY_DST,&all_texels,&blue);
    ogpu_next_split_release(encoder,splits[1],&deps[1]);
    for (uint32_t i = 0; i < 2; ++i) memset(ranges[i],0xdd,(size_t)br[i].size);
    TRY(ogpu_next_commands_end(encoder,&lists[0]));
    TRY(ogpu_next_timeline_create(device,0,&done)); TRY(ogpu_next_timeline_create(device,0,&gate));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL}, gate_wait = {{gate,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),
        2,1,1,lists,&gate_wait,&signal,submit_scratch,sr.size};
    uint32_t repetitions = replay_mode == OGPU_NEXT_ONE_SHOT ? 1 : 3;
    for (uint32_t replay = 0; replay < repetitions; ++replay) {
        memset(map.data,0xa5,(size_t)md.size);
        for (uint32_t i = 0; i < 32; ++i) ((uint32_t *)map.data)[i] = i*17+replay*79;
        TRY(ogpu_next_memory_flush((ogpu_next_span){host,0,md.size}));
        gate_wait.point.value = signal.point.value = replay+1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        REQUIRE(ogpu_next_timeline_wait(signal.point,0) == OGPU_NEXT_TIMEOUT);
        TRY(ogpu_next_timeline_signal_host(gate_wait.point));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate((ogpu_next_span){host,0,md.size}));
        for (uint32_t i = 0; i < 32; ++i) {
            REQUIRE(((uint32_t *)map.data)[128+i] == i*17+replay*79);
            REQUIRE(((uint32_t *)map.data)[192+i] == UINT32_C(0x10203040));
        }
        for (uint32_t i = 0; i < 16; ++i) REQUIRE(((uint32_t *)map.data)[384+i] == UINT32_C(0xffff0000));
        for (uint64_t i = 128; i < md.size; ++i)
            if (!(i >= 512 && i < 640) && !(i >= 768 && i < 896) && !(i >= 1024 && i < 1152)
                && !(i >= 1536 && i < 1600) && !(i >= 2048 && i < 2176)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    for (uint32_t bad = 0; bad < 5; ++bad) {
        TRY(ogpu_next_arena_reset(consumer)); TRY(ogpu_next_commands_begin(consumer,&rd,&encoder));
        if (bad == 0) ogpu_next_split_reset(encoder,splits[0],OGPU_NEXT_STAGE_HOST);
        if (bad == 1) ogpu_next_split_acquire(encoder,2,waits,(ogpu_next_host_span){wait_scratch,wr.size-1});
        if (bad == 2) { deps[1].header.version++; ogpu_next_split_acquire(encoder,2,waits,(ogpu_next_host_span){wait_scratch,wr.size}); deps[1].header.version--; }
        if (bad == 3) { deps[0].before |= OGPU_NEXT_STAGE_HOST; ogpu_next_split_release(encoder,splits[0],&deps[0]); deps[0].before &= ~OGPU_NEXT_STAGE_HOST; }
        if (bad == 4) ogpu_next_split_release(encoder,NULL,&deps[0]);
        REQUIRE(ogpu_next_commands_end(encoder,&lists[1]) == (bad == 1 ? OGPU_NEXT_CAPACITY : bad == 2 ? OGPU_NEXT_UNSUPPORTED : OGPU_NEXT_INVALID) && lists[1] == NULL);
    }
    printf("Split dependencies pass: independent recording order, one batched acquire, range + image transition, explicit reset, replay mode %u, host-gated execution and guards.\n",replay_mode);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending split work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(producer); ogpu_next_arena_destroy(consumer);
    ogpu_next_split_destroy(splits[0]); ogpu_next_split_destroy(splits[1]);
    ogpu_next_image_destroy(image); ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(host);
    ogpu_next_timeline_destroy(done); ogpu_next_timeline_destroy(gate);
    free(ranges[0]); free(ranges[1]); free(wait_scratch); free(submit_scratch);
    return result;
}
