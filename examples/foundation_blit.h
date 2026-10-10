/* Consumer-owned image processing: no automatic mip chain or hidden kernels. */
static int blit_execution(ogpu_next_device *device, ogpu_next_memory_desc host_desc, uint32_t domain) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_image *images[3] = {NULL};
    ogpu_next_memory *backing[3] = {NULL}, *host = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL;
    void *barrier_scratch = NULL, *submit_scratch = NULL;
    ogpu_next_host_requirements br = {0}, sr = {0};
    for (uint32_t i = 0; i < 3; ++i) {
        ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc, OGPU_NEXT_IMAGE_DESC),
            i == 1 ? OGPU_NEXT_BGRA8_UNORM : i == 2 ? OGPU_NEXT_RGBA32_FLOAT : OGPU_NEXT_RGBA8_UNORM,
            OGPU_NEXT_IMAGE_2D, i == 0 ? 2u : 1u, 2, 1, {i == 1 ? 16u : 8u, i == 1 ? 8u : 4u, 1},
            OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST, 0, NULL, 0, 0, NULL};
        uint32_t types[32];
        ogpu_next_requirements req = {0};
        req.compatible_type_capacity = 32; req.compatible_types = types;
        TRY(ogpu_next_image_requirements(device, &desc, &req));
        REQUIRE(req.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device, &desc, &images[i]));
        TRY(ogpu_next_memory_create_dedicated_image(images[i], types[0], &backing[i]));
        TRY(ogpu_next_image_bind(images[i], (ogpu_next_span){backing[i], 0, req.size}));
    }
    REQUIRE(host_desc.size >= 3072);
    TRY(ogpu_next_memory_create(device, &host_desc, &host));
    ogpu_next_span all = {host, 0, host_desc.size};
    ogpu_next_mapping mapping = {0};
    TRY(ogpu_next_memory_map(all, &mapping));
    TRY(ogpu_next_timeline_create(device, 0, &done));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1, OGPU_NEXT_PRIMARY, 0};
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    TRY(ogpu_next_barrier_scratch_requirements(0, 3, &br));
    TRY(ogpu_next_submit_scratch_requirements(1, 0, 1, &sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    barrier_scratch = malloc((size_t)br.size); submit_scratch = malloc((size_t)sr.size);
    REQUIRE(barrier_scratch && submit_scratch);
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_SERIAL_REPLAY, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_image_barrier barriers[3];
    for (uint32_t i = 0; i < 3; ++i) barriers[i] = (ogpu_next_image_barrier){images[i],
        {OGPU_NEXT_ASPECT_COLOR, 0, i == 0 ? 2u : 1u, 0, 2}, OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE,
        OGPU_NEXT_ACCESS_COPY_READ | OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_UNDEFINED, OGPU_NEXT_STATE_GENERAL, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY), OGPU_NEXT_STAGE_ALL,
        OGPU_NEXT_STAGE_COPY | OGPU_NEXT_STAGE_BLIT, 0, 0, 0, 3, 0, NULL, barriers, barrier_scratch, br.size};
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR, 0, 0, 2, {0,0,0}, {8,4,1}}, 0, 0, OGPU_NEXT_STATE_GENERAL, 0};
    ogpu_next_copy_to_image(encoder, images[0], (ogpu_next_span){host,0,256}, &copy);
    dep.image_count = 0; dep.images = NULL;
    dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_BLIT;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_COPY_READ;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_image_blit blit = {
        {OGPU_NEXT_ASPECT_COLOR, 0, 0, 2, {{8,0,0},{0,4,1}}},
        {OGPU_NEXT_ASPECT_COLOR, 0, 0, 2, {{0,0,0},{16,8,1}}},
        OGPU_NEXT_STATE_GENERAL, OGPU_NEXT_STATE_GENERAL, OGPU_NEXT_FILTER_NEAREST, 0};
    ogpu_next_blit_image(encoder, images[1], images[0], &blit); /* upscale + X reversal + BGRA conversion */
    blit.source.offsets[0].x = 0; blit.source.offsets[1].x = 8;
    blit.destination.offsets[1] = (ogpu_next_offset){8,4,1};
    ogpu_next_blit_image(encoder, images[2], images[0], &blit); /* UNORM -> four float channels */
    blit.destination.mip = 1;
    blit.destination.offsets[1] = (ogpu_next_offset){4,2,1};
    blit.filter = OGPU_NEXT_FILTER_LINEAR;
    ogpu_next_blit_image(encoder, images[0], images[0], &blit); /* explicitly chosen next mip */
    dep.before = OGPU_NEXT_STAGE_BLIT; dep.after = OGPU_NEXT_STAGE_COPY;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_COPY_READ;
    ogpu_next_barrier(encoder, &dep);
    copy.region.mip = 1; copy.region.extent = (ogpu_next_extent){4,2,1};
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host,256,64}, images[0], &copy);
    copy.region.mip = 0; copy.region.extent = (ogpu_next_extent){16,8,1};
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host,512,1024}, images[1], &copy);
    copy.region.extent = (ogpu_next_extent){8,4,1};
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host,2048,1024}, images[2], &copy);
    dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    TRY(ogpu_next_commands_end(encoder, &list));
    ogpu_next_sync_point signal = {{done,1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC),
        1, 0, 1, &list, NULL, &signal, submit_scratch, sr.size};
    for (uint32_t replay = 0; replay < 3; ++replay) {
        unsigned char *bytes = mapping.data;
        memset(bytes, 0xa5, (size_t)all.size);
        for (uint32_t layer = 0; layer < 2; ++layer) for (uint32_t y = 0; y < 4; ++y) for (uint32_t x = 0; x < 8; ++x) {
            uint32_t offset = (layer*32 + y*8 + x)*4;
            bytes[offset] = (unsigned char)(12 + x*20 + replay);
            bytes[offset+1] = (unsigned char)(20 + y*40 + layer*13);
            bytes[offset+2] = (unsigned char)(220 - x*14 - replay*3);
            bytes[offset+3] = 255;
        }
        TRY(ogpu_next_memory_flush(all));
        signal.point.value = replay + 1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device, domain, 0), &submit));
        TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t layer = 0; layer < 2; ++layer) {
            for (uint32_t y = 0; y < 2; ++y) for (uint32_t x = 0; x < 4; ++x) {
                unsigned char *dst = bytes+256+(layer*8+y*4+x)*4;
                unsigned char *src = bytes+(layer*32+y*16+x*2)*4;
                int expected[4] = {src[0]+10, src[1]+20, src[2]-7, 255};
                /* Linear midpoint differs from every nearest sample. Allow one
                 * UNORM code of native filter rounding, not an arbitrary error. */
                for (uint32_t c = 0; c < 4; ++c)
                    REQUIRE((int)dst[c] >= expected[c]-1 && (int)dst[c] <= expected[c]+1);
            }
            for (uint32_t y = 0; y < 8; ++y) for (uint32_t x = 0; x < 16; ++x) {
                unsigned char *dst = bytes+512+(layer*128+y*16+x)*4;
                unsigned char *src = bytes+(layer*32+(y/2)*8+7-x/2)*4;
                REQUIRE(dst[0] == src[2] && dst[1] == src[1] && dst[2] == src[0] && dst[3] == src[3]);
            }
            for (uint32_t i = 0; i < 128; ++i) {
                float expected = (float)bytes[layer*128+i]/255.f;
                float actual = ((float *)(bytes+2048))[layer*128+i];
                REQUIRE(actual >= expected-0.000001f && actual <= expected+0.000001f);
            }
        }
        for (uint64_t i = 320; i < all.size; ++i)
            if (!(i >= 512 && i < 1536) && !(i >= 2048 && i < 3072)) REQUIRE(bytes[i] == 0xa5);
    }
    for (uint32_t bad = 0; bad < 8; ++bad) {
        TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
        ogpu_next_image_blit invalid = blit;
        if (bad == 0) invalid.destination.offsets[1].x = 0;
        if (bad == 1) invalid.source.offsets[0].x = -1;
        if (bad == 2) invalid.source.mip = 2;
        if (bad == 3) invalid.destination.layer_count = 1;
        if (bad == 4) invalid.source.offsets[1].z = 0;
        if (bad == 5) invalid.filter = 99;
        if (bad == 6) invalid.reserved = 1;
        if (bad == 7) invalid.destination.aspect = OGPU_NEXT_ASPECT_DEPTH;
        ogpu_next_blit_image(encoder, images[0], images[0], &invalid);
        REQUIRE(ogpu_next_commands_end(encoder, &list) == (bad == 5 || bad == 6 ? OGPU_NEXT_UNSUPPORTED : OGPU_NEXT_INVALID) && list == NULL);
    }
    printf("Blits pass: reversed/scaled layered BGRA conversion, UNORM-to-float conversion, explicit same-image linear mip generation, narrow barriers, changed-input replay and guards.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending blit work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done);
    for (uint32_t i = 0; i < 3; ++i) { ogpu_next_image_destroy(images[i]); ogpu_next_memory_destroy(backing[i]); }
    ogpu_next_memory_destroy(host); free(barrier_scratch); free(submit_scratch);
    return result;
}
