/* Caller-managed A/B/A alias reuse. Every activation discards and fully initializes;
 * no claim that arbitrary different image interpretations preserve shared texels. */
static int alias_execution(ogpu_next_device *device, ogpu_next_memory_desc host_desc, uint32_t domain) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_image *images[2] = {NULL};
    ogpu_next_memory *backing = NULL, *host = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    void *barrier_scratch = NULL, *submit_scratch = NULL;
    REQUIRE(host_desc.size >= 3328);
    ogpu_next_image_desc desc[2] = {
        {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,
            1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST,OGPU_NEXT_IMAGE_ALIAS,NULL,0,0,NULL},
        {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_R32_UINT,OGPU_NEXT_IMAGE_2D,
            1,1,1,{16,4,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST,OGPU_NEXT_IMAGE_ALIAS,NULL,0,0,NULL}
    };
    uint32_t types[2][32]; ogpu_next_requirements req[2] = {{0}};
    for (uint32_t i = 0; i < 2; ++i) {
        req[i].compatible_types = types[i]; req[i].compatible_type_capacity = 32;
        TRY(ogpu_next_image_requirements(device,&desc[i],&req[i]));
        REQUIRE(!req[i].dedicated_required && req[i].size && req[i].alignment);
    }
    uint32_t common = UINT32_MAX;
    for (uint32_t i = 0; i < req[0].compatible_type_count; ++i)
        for (uint32_t j = 0; j < req[1].compatible_type_count; ++j)
            if (types[0][i] == types[1][j]) common = types[0][i];
    REQUIRE(common != UINT32_MAX); /* Explicit fixture prerequisite; no alternate allocation fallback. */
    uint64_t offset = req[0].alignment > req[1].alignment ? req[0].alignment : req[1].alignment;
    uint64_t size = req[0].size > req[1].size ? req[0].size : req[1].size;
    REQUIRE(offset % req[0].alignment == 0 && offset % req[1].alignment == 0 && size <= UINT64_MAX-offset);
    ogpu_next_memory_desc md = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),offset+size,1,0,common,OGPU_NEXT_MEMORY_OPAQUE,NULL,0,0};
    TRY(ogpu_next_memory_create(device,&md,&backing));
    for (uint32_t i = 0; i < 2; ++i)
        TRY(ogpu_next_image_create(device,&desc[i],(ogpu_next_span){backing,offset,req[i].size},&images[i]));
    TRY(ogpu_next_memory_create(device,&host_desc,&host));
    ogpu_next_span all = {host,0,host_desc.size}; ogpu_next_mapping mapping = {0};
    TRY(ogpu_next_memory_map(all,&mapping));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements br = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    barrier_scratch = malloc((size_t)br.size); submit_scratch = malloc((size_t)sr.size); REQUIRE(barrier_scratch && submit_scratch);
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    for (uint32_t step = 0; step < 3; ++step) {
        uint32_t which = step % 2;
        ogpu_next_image_barrier image = {images[which],{OGPU_NEXT_ASPECT_COLOR,0,1,0,1},0,OGPU_NEXT_ACCESS_COPY_WRITE,
            OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
        /* Global availability covers writes through the OTHER image alias. An
         * image-only barrier on the new alias would not cover those accesses.
         * Execution ordering also waits for prior readback before overwriting. */
        ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COPY,
            OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_WRITE,0,1,0,NULL,&image,barrier_scratch,br.size};
        ogpu_next_barrier(encoder,&dep);
        ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},desc[which].extent},0,0,OGPU_NEXT_STATE_COPY_DST,0};
        ogpu_next_copy_to_image(encoder,images[which],(ogpu_next_span){host,128+step*512,256},&copy);
        dep.global_before = dep.global_after = 0;
        image.before = OGPU_NEXT_ACCESS_COPY_WRITE; image.after = OGPU_NEXT_ACCESS_COPY_READ;
        image.old_state = OGPU_NEXT_STATE_COPY_DST; image.new_state = OGPU_NEXT_STATE_COPY_SRC; image.discard = 0;
        ogpu_next_barrier(encoder,&dep); copy.state = OGPU_NEXT_STATE_COPY_SRC;
        ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,2048+step*512,256},images[which],&copy);
    }
    ogpu_next_dependency host_dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,
        OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&host_dep); TRY(ogpu_next_commands_end(encoder,&list));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,submit_scratch,sr.size};
    for (uint32_t replay = 0; replay < 3; ++replay) {
        unsigned char *bytes = mapping.data; memset(bytes,0xa5,(size_t)host_desc.size);
        for (uint32_t step = 0; step < 3; ++step) for (uint32_t i = 0; i < 256; ++i)
            bytes[128+step*512+i] = (unsigned char)(i*13+step*47+replay*29);
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t step = 0; step < 3; ++step) for (uint32_t i = 0; i < 256; ++i) {
            unsigned char expected = (unsigned char)(i*13+step*47+replay*29);
            REQUIRE(bytes[128+step*512+i] == expected && bytes[2048+step*512+i] == expected);
        }
        for (uint64_t i = 0; i < host_desc.size; ++i) {
            uint32_t payload = 0;
            for (uint32_t step = 0; step < 3; ++step)
                payload |= (i >= 128+step*512 && i < 384+step*512) || (i >= 2048+step*512 && i < 2304+step*512);
            if (!payload) REQUIRE(bytes[i] == 0xa5);
        }
    }
    printf("Image alias reuse passes: differently shaped/formatted A/B/A at one nonzero placement, explicit global dependencies/discards, three changed-data replays and guards.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending alias work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done);
    for (uint32_t i = 0; i < 2; ++i) ogpu_next_image_destroy(images[i]);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(host);
    free(barrier_scratch); free(submit_scratch); return result;
}
