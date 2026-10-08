/* Public-C integration fixture, included after foundation.c's test helpers.
 * Allocation policy and all synchronization deliberately live in this caller. */
static int heap_memory(ogpu_next_device *device, uint64_t size, uint64_t alignment,
                       uint64_t usage, uint32_t properties, ogpu_next_memory **out) {
    int result = EXIT_FAILURE;
    ogpu_next_memory_type_info types[32];
    ogpu_next_query q = query(OGPU_NEXT_QUERY_MEMORY_TYPES, types, 32, sizeof(types[0]));
    TRY(ogpu_next_device_query(device, &q));
    uint32_t compatible[32];
    ogpu_next_requirements req = {0};
    req.compatible_type_capacity = 32; req.compatible_types = compatible;
    ogpu_next_memory_desc desc = {HEADER(ogpu_next_memory_desc, OGPU_NEXT_MEMORY_DESC),
        size, alignment, usage, UINT32_MAX, OGPU_NEXT_MEMORY_LINEAR, NULL, 0, 0};
    TRY(ogpu_next_memory_requirements(device, &desc, &req));
    for (uint32_t i = 0; i < req.compatible_type_count && desc.memory_type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < q.count; ++j)
            if (types[j].id == compatible[i] && (types[j].properties & properties) == properties) {
                desc.memory_type = types[j].id; break;
            }
    REQUIRE(desc.memory_type != UINT32_MAX);
    TRY(ogpu_next_memory_create(device, &desc, out));
    result = EXIT_SUCCESS;
cleanup:
    return result;
}

static int heap_execution(ogpu_next_device *device, ogpu_next_memory_desc host_desc,
                          uint32_t domain, const char *shader_path) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL;
    void *code = NULL, *scratch = NULL, *barrier_scratch = NULL, *submit_scratch = NULL;
    ogpu_next_memory *data = NULL, *staging = NULL, *resource_heap = NULL, *sampler_heap = NULL;
    ogpu_next_memory *backing[2] = {NULL, NULL};
    ogpu_next_image *images[2] = {NULL, NULL};
    ogpu_next_view *views[2] = {NULL, NULL};
    ogpu_next_executable *executable = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL, *gate = NULL;
    ogpu_next_descriptor_limits limits = {0};
    ogpu_next_memory_limits memory_limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_DESCRIPTOR_LIMITS, &limits, 1, sizeof(limits));
    TRY(ogpu_next_device_query(device, &q));
    q = query(OGPU_NEXT_QUERY_MEMORY_LIMITS, &memory_limits, 1, sizeof(memory_limits));
    TRY(ogpu_next_device_query(device, &q));
    const uint32_t width = 8, height = 4;
    const uint64_t pixels_size = width * height * 4;
    ogpu_next_subresources all_texels = {OGPU_NEXT_ASPECT_COLOR, 0, 1, 0, 1};
    for (uint32_t i = 0; i < 2; ++i) {
        uint64_t usage = i ? OGPU_NEXT_IMAGE_STORAGE | OGPU_NEXT_IMAGE_COPY_SRC
                           : OGPU_NEXT_IMAGE_SAMPLED | OGPU_NEXT_IMAGE_COPY_DST;
        ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc, OGPU_NEXT_IMAGE_DESC),
            OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_IMAGE_2D, 1, 1, 1, {width, height, 1}, usage,
            0, NULL, 0, 0, NULL};
        uint32_t types[32];
        ogpu_next_requirements req = {0};
        req.compatible_type_capacity = 32; req.compatible_types = types;
        TRY(ogpu_next_image_requirements(device, &desc, &req));
        REQUIRE(req.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device, &desc, &images[i]));
        TRY(ogpu_next_memory_create_dedicated_image(images[i], types[0], &backing[i]));
        TRY(ogpu_next_image_bind(images[i], (ogpu_next_span){backing[i], 0, req.size}));
        ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc, OGPU_NEXT_VIEW_DESC),
            OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_VIEW_2D, (uint32_t)(i ? OGPU_NEXT_IMAGE_STORAGE : OGPU_NEXT_IMAGE_SAMPLED),
            {0, 0, 0, 0}, all_texels};
        TRY(ogpu_next_view_create(images[i], &vd, &views[i]));
    }
    TRY(ogpu_next_memory_create(device, &host_desc, &data));
    ogpu_next_mapping data_map = {0}, stage_map = {0}, sampler_map = {0};
    TRY(ogpu_next_memory_map((ogpu_next_span){data, 0, host_desc.size}, &data_map));
    /* Mixed descriptor layout chosen by the caller. The compiled shader indexes
     * each typed heap array in units of that descriptor's native size. */
    uint64_t image_stride = aligned(limits.image_size, limits.image_alignment);
    uint64_t buffer_stride = aligned(limits.buffer_size, limits.buffer_alignment);
    uint64_t resource_stride = image_stride > buffer_stride ? image_stride : buffer_stride;
    uint64_t payload = resource_stride * 3;
    REQUIRE(resource_stride % image_stride == 0 && resource_stride % buffer_stride == 0);
    REQUIRE(resource_stride / image_stride <= UINT32_MAX && 2 * resource_stride / buffer_stride <= UINT32_MAX);
    uint64_t resource_reserved = aligned(payload, limits.resource_reserved_alignment);
    uint64_t resource_size = resource_reserved + limits.resource_reserved_size;
    REQUIRE(heap_memory(device, resource_size, limits.resource_heap_alignment,
        OGPU_NEXT_USAGE_DESCRIPTOR_HEAP | OGPU_NEXT_USAGE_COPY_DST, OGPU_NEXT_MEMORY_LOCAL, &resource_heap) == EXIT_SUCCESS);
    REQUIRE(heap_memory(device, payload, 1, OGPU_NEXT_USAGE_COPY_SRC,
        OGPU_NEXT_MEMORY_HOST_VISIBLE, &staging) == EXIT_SUCCESS);
    TRY(ogpu_next_memory_map((ogpu_next_span){staging, 0, payload}, &stage_map));
    uint64_t sampler_stride = limits.sampler_size;
    if (sampler_stride < memory_limits.cache_atom_size) sampler_stride = memory_limits.cache_atom_size;
    sampler_stride = aligned(sampler_stride, limits.sampler_alignment);
    uint64_t sampler_reserved_alignment = limits.sampler_reserved_alignment;
    if (sampler_reserved_alignment < memory_limits.cache_atom_size) sampler_reserved_alignment = memory_limits.cache_atom_size;
    uint64_t sampler_reserved = aligned(sampler_stride * 2, sampler_reserved_alignment);
    uint64_t sampler_size = sampler_reserved + limits.sampler_reserved_size;
    uint64_t sampler_alignment = limits.sampler_heap_alignment;
    if (sampler_alignment < memory_limits.cache_atom_size) sampler_alignment = memory_limits.cache_atom_size;
    REQUIRE(heap_memory(device, sampler_size, sampler_alignment, OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,
        OGPU_NEXT_MEMORY_HOST_VISIBLE, &sampler_heap) == EXIT_SUCCESS);
    TRY(ogpu_next_memory_map((ogpu_next_span){sampler_heap, 0, sampler_size}, &sampler_map));
    REQUIRE(sampler_map.cache_offset == 0); /* Fixture isolates pending slot/cache atoms. */
    ogpu_next_heap_binding bindings[2] = {
        {HEADER(ogpu_next_heap_binding, OGPU_NEXT_HEAP_BINDING), OGPU_NEXT_HEAP_RESOURCE, 0,
            {resource_heap, 0, resource_size}, resource_reserved, limits.resource_reserved_size},
        {HEADER(ogpu_next_heap_binding, OGPU_NEXT_HEAP_BINDING), OGPU_NEXT_HEAP_SAMPLER, 0,
            {sampler_heap, 0, sampler_size}, sampler_reserved, limits.sampler_reserved_size}
    };
    ogpu_next_host_requirements wr = {0}, ws = {0}, br = {0}, sr = {0};
    TRY(ogpu_next_descriptor_scratch_requirements(OGPU_NEXT_HEAP_RESOURCE, 3, &wr));
    TRY(ogpu_next_descriptor_scratch_requirements(OGPU_NEXT_HEAP_SAMPLER, 1, &ws));
    uint64_t scratch_size = wr.size > ws.size ? wr.size : ws.size;
    REQUIRE(wr.alignment <= _Alignof(max_align_t) && ws.alignment <= _Alignof(max_align_t));
    scratch = malloc((size_t)scratch_size); REQUIRE(scratch != NULL);
    ogpu_next_host_span temporary = {scratch, scratch_size};
    ogpu_next_host_span destinations[3] = {
        {stage_map.data, limits.image_size},
        {(unsigned char *)stage_map.data + resource_stride, limits.image_size},
        {(unsigned char *)stage_map.data + resource_stride * 2, limits.buffer_size}
    };
    ogpu_next_resource_descriptor resources[3] = {
        {OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE, OGPU_NEXT_STATE_GENERAL, views[0], {NULL, 0, 0}},
        {OGPU_NEXT_DESCRIPTOR_STORAGE_IMAGE, OGPU_NEXT_STATE_GENERAL, views[1], {NULL, 0, 0}},
        {OGPU_NEXT_DESCRIPTOR_STORAGE_BUFFER, 0, NULL, {data, 1024, pixels_size}}
    };
    ogpu_next_sampler_desc sampler = {HEADER(ogpu_next_sampler_desc, OGPU_NEXT_SAMPLER_DESC),
        0, 0, 0, {2, 2, 2}, 0, 0, 0, 0, 0, 0, 0, 1};
    ogpu_next_host_span sampler_output = {sampler_map.data, limits.sampler_size};
    TRY(ogpu_next_write_sampler_descriptors(device, 1, &sampler, &sampler_output, temporary));
    TRY(ogpu_next_memory_flush((ogpu_next_span){sampler_heap, 0, limits.sampler_size}));
    file = fopen(shader_path, "rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file, 0, SEEK_END) == 0);
    long code_size = ftell(file); REQUIRE(code_size >= 20 && code_size % 4 == 0);
    REQUIRE(fseek(file, 0, SEEK_SET) == 0);
    code = malloc((size_t)code_size); REQUIRE(code != NULL);
    REQUIRE(fread(code, 1, (size_t)code_size, file) == (size_t)code_size);
    fclose(file); file = NULL;
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface, OGPU_NEXT_ARGUMENT_INTERFACE), 24, 0, NULL};
    ogpu_next_shader_requirements requirements = {HEADER(ogpu_next_shader_requirements, OGPU_NEXT_SHADER_REQUIREMENTS), 0, {64, 1, 1}, 0};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE, OGPU_NEXT_SHADER_SPIRV,
        {code, (size_t)code_size}, "consume", &abi.header, NULL};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc, OGPU_NEXT_EXECUTABLE_DESC),
        OGPU_NEXT_EXECUTABLE_COMPUTE, 1, &shader, NULL, 0, &requirements.header, {NULL, 0}};
    TRY(ogpu_next_executable_create(device, &ed, &executable));
    free(code); code = NULL;
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1};
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    TRY(ogpu_next_barrier_scratch_requirements(0, 2, &br));
    TRY(ogpu_next_submit_scratch_requirements(1, 1, 1, &sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    barrier_scratch = malloc((size_t)br.size); submit_scratch = malloc((size_t)sr.size);
    REQUIRE(barrier_scratch && submit_scratch);
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_SERIAL_REPLAY, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_image_barrier barriers[2];
    for (uint32_t i = 0; i < 2; ++i) barriers[i] = (ogpu_next_image_barrier){images[i], all_texels,
        OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE, i ? OGPU_NEXT_ACCESS_SHADER_WRITE : OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_UNDEFINED, OGPU_NEXT_STATE_GENERAL, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY),
        OGPU_NEXT_STAGE_ALL, OGPU_NEXT_STAGE_COPY | OGPU_NEXT_STAGE_COMPUTE,
        OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE, OGPU_NEXT_ACCESS_COPY_WRITE | OGPU_NEXT_ACCESS_SHADER_WRITE,
        0, 2, 0, NULL, barriers, barrier_scratch, br.size};
    ogpu_next_barrier(encoder, &dep);
    /* Only payload bytes are copied: reserved heap bytes are never touched. */
    ogpu_next_copy_memory(encoder, (ogpu_next_span){resource_heap, 0, payload}, (ogpu_next_span){staging, 0, payload});
    ogpu_next_image_copy image_copy = {{OGPU_NEXT_ASPECT_COLOR, 0, 0, 1, {0, 0, 0}, {width, height, 1}}, 0, 0, OGPU_NEXT_STATE_GENERAL, 0};
    ogpu_next_copy_to_image(encoder, images[0], (ogpu_next_span){data, 0, pixels_size}, &image_copy);
    barriers[0].before = OGPU_NEXT_ACCESS_COPY_WRITE; barriers[0].after = OGPU_NEXT_ACCESS_SHADER_READ;
    barriers[0].old_state = OGPU_NEXT_STATE_GENERAL; barriers[0].discard = 0;
    dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_COMPUTE;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_RESOURCE_HEAP_READ;
    dep.image_count = 1;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_bind_heap(encoder, &bindings[0]); ogpu_next_bind_heap(encoder, &bindings[1]);
    ogpu_next_bind_executable(encoder, executable);
    uint32_t control[6] = {0, (uint32_t)(resource_stride / image_stride), 0,
        (uint32_t)(resource_stride * 2 / buffer_stride), width, height};
    ogpu_next_set_inline(encoder, OGPU_NEXT_STAGE_COMPUTE, 0, sizeof(control), control);
    ogpu_next_launch launch = {{1, 1, 1}, 0, NULL};
    ogpu_next_dispatch(encoder, &launch);
    barriers[1].before = OGPU_NEXT_ACCESS_SHADER_WRITE; barriers[1].after = OGPU_NEXT_ACCESS_COPY_READ;
    barriers[1].old_state = OGPU_NEXT_STATE_GENERAL; barriers[1].discard = 0;
    dep.images = &barriers[1]; dep.before = OGPU_NEXT_STAGE_COMPUTE; dep.after = OGPU_NEXT_STAGE_COPY | OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_SHADER_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){data, 512, pixels_size}, images[1], &image_copy);
    dep.image_count = 0; dep.images = NULL; dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    TRY(ogpu_next_commands_end(encoder, &list));
    TRY(ogpu_next_timeline_create(device, 0, &done)); TRY(ogpu_next_timeline_create(device, 0, &gate));
    ogpu_next_sync_point wait = {{gate, 1}, OGPU_NEXT_STAGE_ALL}, signal = {{done, 1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC),
        1, 1, 1, &list, &wait, &signal, submit_scratch, sr.size};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(data_map.data, 0xa5, (size_t)host_desc.size);
        for (uint32_t i = 0; i < width * height; ++i)
            ((uint32_t *)data_map.data)[i] = UINT32_C(0xff000000) | (17 + i + replay) | ((31 + i * 3) << 8) | ((220 - i - replay) << 16);
        TRY(ogpu_next_memory_flush((ogpu_next_span){data, 0, host_desc.size}));
        resources[2].buffer.offset = 1024 + replay * 256;
        memset(stage_map.data, 0xa5, (size_t)payload);
        TRY(ogpu_next_write_resource_descriptors(device, 3, resources, destinations, temporary));
        TRY(ogpu_next_memory_flush((ogpu_next_span){staging, 0, payload}));
        wait.point.value = replay + 1; signal.point.value = replay + 1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device, domain, 0), &submit));
        REQUIRE(ogpu_next_timeline_wait(signal.point, 0) == OGPU_NEXT_TIMEOUT);
        /* An unused sampler slot in a separate cache atom stays mutable while a
         * list that WILL consume slot zero is pending. This is not GPU-overlap timing. */
        ogpu_next_host_span unused = {(unsigned char *)sampler_map.data + sampler_stride, limits.sampler_size};
        sampler.min_filter = replay; sampler.mag_filter = replay;
        TRY(ogpu_next_write_sampler_descriptors(device, 1, &sampler, &unused, temporary));
        TRY(ogpu_next_memory_flush((ogpu_next_span){sampler_heap, sampler_stride, limits.sampler_size}));
        TRY(ogpu_next_timeline_signal_host(wait.point));
        TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate((ogpu_next_span){data, 0, host_desc.size}));
        for (uint32_t i = 0; i < width * height; ++i) {
            uint32_t expected = UINT32_C(0xff000000) | (220 - i - replay) | ((255 - 31 - i * 3) << 8) | ((17 + i + replay) << 16);
            REQUIRE(((uint32_t *)data_map.data)[128 + i] == expected);
            REQUIRE(((uint32_t *)data_map.data)[resources[2].buffer.offset / 4 + i] == expected);
        }
        for (uint64_t i = pixels_size; i < host_desc.size; ++i)
            if (!(i >= 512 && i < 512 + pixels_size) &&
                !(i >= resources[2].buffer.offset && i < resources[2].buffer.offset + pixels_size))
                REQUIRE(((unsigned char *)data_map.data)[i] == 0xa5);
    }
    TRY(ogpu_next_arena_reset(arena)); /* Reservation release, not just GPU completion. */
    printf("Heap execution passes: sampled image + sampler, storage image + buffer, explicit local descriptor upload, changed-descriptor replay and pending disjoint-slot writes.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr, "Pending heap execution after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(executable);
    ogpu_next_timeline_destroy(done); ogpu_next_timeline_destroy(gate);
    for (uint32_t i = 0; i < 2; ++i) { ogpu_next_view_destroy(views[i]); ogpu_next_image_destroy(images[i]); ogpu_next_memory_destroy(backing[i]); }
    ogpu_next_memory_destroy(resource_heap); ogpu_next_memory_destroy(sampler_heap);
    ogpu_next_memory_destroy(staging); ogpu_next_memory_destroy(data);
    if (file) fclose(file);
    free(code); free(scratch); free(barrier_scratch); free(submit_scratch);
    return result;
}
