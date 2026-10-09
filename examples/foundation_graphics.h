/* Offscreen public-C consumer. No old ABI helpers or implicit attachment policy. */
static int graphics_execution(ogpu_next_device *device, ogpu_next_memory_desc host_desc,
                              uint32_t domain, const char *vertex_path, const char *fragment_path) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL;
    void *code[2] = {NULL, NULL}, *scratch = NULL, *barrier_scratch = NULL, *submit_scratch = NULL;
    ogpu_next_memory *host = NULL, *backing[2] = {NULL, NULL};
    ogpu_next_image *images[2] = {NULL, NULL};
    ogpu_next_view *views[2] = {NULL, NULL};
    ogpu_next_executable *pipelines[2] = {NULL, NULL};
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL;
    ogpu_next_graphics_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS, &limits, 1, sizeof(limits));
    TRY(ogpu_next_device_query(device, &q)); REQUIRE(limits.max_colors > 0);
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT, 0, 16};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface, OGPU_NEXT_ARGUMENT_INTERFACE), 8, 1, &slot};
    ogpu_next_shader shaders[2];
    const char *paths[2] = {vertex_path, fragment_path};
    for (uint32_t i = 0; i < 2; ++i) {
        file = fopen(paths[i], "rb"); REQUIRE(file != NULL);
        REQUIRE(fseek(file, 0, SEEK_END) == 0);
        long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file, 0, SEEK_SET) == 0);
        code[i] = malloc((size_t)size); REQUIRE(code[i] != NULL);
        REQUIRE(fread(code[i], 1, (size_t)size, file) == (size_t)size);
        fclose(file); file = NULL;
        shaders[i] = (ogpu_next_shader){i ? OGPU_NEXT_STAGE_FRAGMENT : OGPU_NEXT_STAGE_VERTEX,
            OGPU_NEXT_SHADER_SPIRV, {code[i], (size_t)size}, i ? "fragmentMain" : "vertexMain", &abi.header, NULL};
    }
    ogpu_next_color_state color = {OGPU_NEXT_RGBA8_UNORM, 15, 0, 1, 0, 0, 1, 0, 0};
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state, OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES, OGPU_NEXT_CULL_NONE, OGPU_NEXT_FRONT_CCW, 1, 1, &color,
        OGPU_NEXT_D32_FLOAT, 1, 1, OGPU_NEXT_COMPARE_LESS, {0, 0, 0, 0}};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements, OGPU_NEXT_SHADER_REQUIREMENTS), OGPU_NEXT_FEATURE_RASTER, {0, 0, 0}, 0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc, OGPU_NEXT_EXECUTABLE_DESC),
        OGPU_NEXT_EXECUTABLE_GRAPHICS, 2, shaders, &gs.header, OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR, &req.header, {NULL, 0}};
    gs.samples = 4;
    REQUIRE(ogpu_next_executable_create(device, &ed, &pipelines[0]) == OGPU_NEXT_UNSUPPORTED && pipelines[0] == NULL);
    gs.samples = 1;
    TRY(ogpu_next_executable_create(device, &ed, &pipelines[0]));
    color.blend = 1; color.dst_color = 1; color.src_alpha = 0; color.dst_alpha = 1;
    gs.depth_write = 0;
    TRY(ogpu_next_executable_create(device, &ed, &pipelines[1]));
    free(code[0]); code[0] = NULL; free(code[1]); code[1] = NULL;
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR, 0, 1, 0, 1};
    for (uint32_t i = 0; i < 2; ++i) {
        ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc, OGPU_NEXT_IMAGE_DESC),
            i ? OGPU_NEXT_D32_FLOAT : OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_IMAGE_2D, 1, 1, 1, {16, 8, 1},
            OGPU_NEXT_IMAGE_COPY_SRC | (i ? OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT : OGPU_NEXT_IMAGE_COLOR_ATTACHMENT),
            0, NULL, 0, 0, NULL};
        uint32_t types[32];
        ogpu_next_requirements memory_req = {0};
        memory_req.compatible_type_capacity = 32; memory_req.compatible_types = types;
        TRY(ogpu_next_image_requirements(device, &desc, &memory_req)); REQUIRE(memory_req.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device, &desc, &images[i]));
        TRY(ogpu_next_memory_create_dedicated_image(images[i], types[0], &backing[i]));
        TRY(ogpu_next_image_bind(images[i], (ogpu_next_span){backing[i], 0, memory_req.size}));
        range.aspects = i ? OGPU_NEXT_ASPECT_DEPTH : OGPU_NEXT_ASPECT_COLOR;
        ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc, OGPU_NEXT_VIEW_DESC), desc.format,
            OGPU_NEXT_VIEW_2D, (uint32_t)(i ? OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT : OGPU_NEXT_IMAGE_COLOR_ATTACHMENT), {0,0,0,0}, range};
        TRY(ogpu_next_view_create(images[i], &vd, &views[i]));
    }
    TRY(ogpu_next_memory_create(device, &host_desc, &host));
    ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map((ogpu_next_span){host, 0, host_desc.size}, &map));
    uint64_t address = 0;
    TRY(ogpu_next_memory_address((ogpu_next_span){host, 0, host_desc.size}, &address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1};
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    ogpu_next_host_requirements rr = {0}, br = {0}, sr = {0};
    TRY(ogpu_next_render_scratch_requirements(1, &rr)); TRY(ogpu_next_barrier_scratch_requirements(0, 2, &br));
    TRY(ogpu_next_submit_scratch_requirements(1, 0, 1, &sr));
    REQUIRE(rr.alignment <= _Alignof(max_align_t) && br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    scratch = malloc((size_t)rr.size); barrier_scratch = malloc((size_t)br.size); submit_scratch = malloc((size_t)sr.size);
    REQUIRE(scratch && barrier_scratch && submit_scratch);
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_SERIAL_REPLAY, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_image_barrier barriers[2];
    for (uint32_t i = 0; i < 2; ++i) barriers[i] = (ogpu_next_image_barrier){images[i],
        {i ? OGPU_NEXT_ASPECT_DEPTH : OGPU_NEXT_ASPECT_COLOR, 0, 1, 0, 1}, OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE,
        i ? OGPU_NEXT_ACCESS_DEPTH_READ | OGPU_NEXT_ACCESS_DEPTH_WRITE : OGPU_NEXT_ACCESS_COLOR_WRITE,
        OGPU_NEXT_STATE_UNDEFINED, i ? OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT : OGPU_NEXT_STATE_COLOR_ATTACHMENT,
        OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY), OGPU_NEXT_STAGE_ALL,
        OGPU_NEXT_STAGE_COLOR | OGPU_NEXT_STAGE_DEPTH, 0, 0, 0, 2, 0, NULL, barriers, barrier_scratch, br.size};
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_attachment colors = {views[0], NULL, OGPU_NEXT_STATE_COLOR_ATTACHMENT, 0, OGPU_NEXT_CLEAR, OGPU_NEXT_STORE, 0, {.f32 = {0,0,0,1}}};
    ogpu_next_attachment depth = {views[1], NULL, OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT, 0, OGPU_NEXT_CLEAR, OGPU_NEXT_STORE, 0, {.depth_stencil = {1,0}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc, OGPU_NEXT_RENDER_DESC),
        0, 0, 16, 8, 1, 0, 1, 0, 1, &colors, &depth, NULL, {scratch, rr.size}};
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state, OGPU_NEXT_VIEWPORT_STATE),
        0, 0, 16, 8, 0, 1, 0, 0, 8, 8};
    ogpu_next_draw_desc draw = {3, 1, 0, 0, 0};
    ogpu_next_render_begin(encoder, &render);
    ogpu_next_bind_executable(encoder, pipelines[0]);
    ogpu_next_set_graphics_state(encoder, &vp.header);
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT, 0, address);
    ogpu_next_draw(encoder, &draw);
    ogpu_next_render_end(encoder);
    /* LOAD in a new scope is explicit, including inter-scope attachment hazards. */
    dep.before = dep.after;
    for (uint32_t i = 0; i < 2; ++i) { barriers[i].before = barriers[i].after; barriers[i].old_state = barriers[i].new_state; barriers[i].discard = 0; }
    barriers[0].after |= OGPU_NEXT_ACCESS_COLOR_READ;
    ogpu_next_barrier(encoder, &dep);
    colors.load_op = OGPU_NEXT_LOAD; depth.load_op = OGPU_NEXT_LOAD;
    ogpu_next_render_begin(encoder, &render);
    vp.scissor_width = 16;
    ogpu_next_set_graphics_state(encoder, &vp.header);
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT, 0, address + 32);
    ogpu_next_bind_indices(encoder, (ogpu_next_span){host,256,6}, OGPU_NEXT_INDEX_U16);
    draw.vertex_offset = -1; /* Indices 1,2,3 become vertices 0,1,2. */
    ogpu_next_draw_indexed(encoder, &draw); /* Fails depth on left; writes right. */
    draw.vertex_offset = 0;
    ogpu_next_bind_executable(encoder, pipelines[1]);
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT, 0, address + 64);
    /* Four disjoint stripes prove all address-indirect variants, not just no-ops. */
    for (uint32_t variant = 0; variant < 4; ++variant) {
        vp.scissor_x = 4 + (int32_t)variant * 2; vp.scissor_width = 2;
        ogpu_next_set_graphics_state(encoder, &vp.header);
        ogpu_next_indirect indirect = {{host,variant % 2 ? 320 : 272,64},32,2,
            {variant >= 2 ? host : NULL,variant >= 2 ? 384 : 0,variant >= 2 ? 4 : 0}};
        if (variant % 2) ogpu_next_draw_indexed_indirect(encoder, &indirect);
        else ogpu_next_draw_indirect(encoder, &indirect);
    }
    ogpu_next_render_end(encoder);
    for (uint32_t i = 0; i < 2; ++i) { barriers[i].before = barriers[i].after; barriers[i].after = OGPU_NEXT_ACCESS_COPY_READ; barriers[i].new_state = OGPU_NEXT_STATE_COPY_SRC; }
    dep.after = OGPU_NEXT_STAGE_COPY;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR, 0, 0, 1, {0,0,0}, {16,8,1}}, 0, 0, OGPU_NEXT_STATE_COPY_SRC, 0};
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host,512,512}, images[0], &copy);
    copy.region.aspect = OGPU_NEXT_ASPECT_DEPTH;
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host,1024,512}, images[1], &copy);
    dep.image_count = 0; dep.images = NULL; dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    TRY(ogpu_next_commands_end(encoder, &list));
    TRY(ogpu_next_timeline_create(device, 0, &done));
    ogpu_next_sync_point signal = {{done,1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC), 1,0,1,&list,NULL,&signal,submit_scratch,sr.size};
    struct Root { float color[4], depth; uint32_t width,height,pad; };
    _Static_assert(sizeof(struct Root) == 32, "graphics root ABI");
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data, 0xa5, (size_t)host_desc.size);
        struct Root roots[3] = {{{1,0,0,1},0.25f,16,8,0}, {{0,1,0,1},0.75f,16,8,0}, {{0,0,1,1},0.125f,16,8,0}};
        if (replay) { roots[0].color[0] = 0; roots[0].color[1] = 1; roots[1].color[0] = 1; roots[1].color[1] = 0; }
        memcpy(map.data, roots, sizeof(roots));
        const uint16_t indices[3] = {1,2,3};
        const ogpu_next_draw_arguments arguments[2] = {{3,1,0,0},{0,1,0,0}};
        const ogpu_next_draw_indexed_arguments indexed[2] = {{3,1,0,-1,0},{0,1,0,0,0}};
        _Static_assert(sizeof(ogpu_next_draw_arguments) == 16, "draw wire ABI");
        _Static_assert(sizeof(ogpu_next_draw_indexed_arguments) == 20, "indexed wire ABI");
        _Static_assert(sizeof(ogpu_next_indirect) == 56, "indirect ABI");
        memcpy((char *)map.data+256, indices, sizeof(indices));
        for (uint32_t i = 0; i < 2; ++i) {
            memcpy((char *)map.data+272+i*32, &arguments[i], sizeof(arguments[i]));
            memcpy((char *)map.data+320+i*32, &indexed[i], sizeof(indexed[i]));
        }
        ((uint32_t *)map.data)[96] = replay; /* Zero then one: visible count change on replay. */
        TRY(ogpu_next_memory_flush((ogpu_next_span){host,0,host_desc.size}));
        signal.point.value = replay + 1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0), &submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate((ogpu_next_span){host,0,host_desc.size}));
        for (uint32_t y = 0; y < 8; ++y) for (uint32_t x = 0; x < 16; ++x) {
            uint32_t expected = UINT32_C(0xff000000) | (((x < 8) != (replay != 0)) ? 255 : 65280) | ((x >= 4 && x < (replay ? 12u : 8u)) ? 16711680 : 0);
            if (((uint32_t *)map.data)[128+y*16+x] != expected)
                fprintf(stderr, "graphics replay %u pixel (%u,%u): %08x expected %08x\n", replay,x,y,((uint32_t *)map.data)[128+y*16+x],expected);
            REQUIRE(((uint32_t *)map.data)[128+y*16+x] == expected);
            REQUIRE(((float *)map.data)[256+y*16+x] == (x < 8 ? 0.25f : 0.75f));
        }
        for (uint32_t i = 96; i < host_desc.size; ++i)
            if ((i < 256 || (i >= 388 && i < 512) || i >= 1536)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    TRY(ogpu_next_arena_reset(arena));
    /* Attachmentless scopes need no bound executable. */
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    render.color_count = 0; render.colors = NULL; render.depth = NULL; render.scratch = (ogpu_next_host_span){NULL,0};
    ogpu_next_render_begin(encoder,&render); ogpu_next_render_end(encoder);
    TRY(ogpu_next_commands_end(encoder,&list));
    signal.point.value = 3;
    pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
    TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_render_begin(encoder,&render);
    ogpu_next_fill_memory(encoder,(ogpu_next_span){host,0,4},0);
    REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL);
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    render.width = 0;
    ogpu_next_render_begin(encoder,&render);
    REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL);
    render.width = 16;
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_render_begin(encoder,&render);
    REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL); /* Missing render_end. */
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_bind_executable(encoder,pipelines[0]);
    ogpu_next_draw(encoder,&draw);
    REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL); /* Outside scope. */
    for (uint32_t bad = 0; bad < 6; ++bad) {
        TRY(ogpu_next_arena_reset(arena));
        TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        ogpu_next_render_begin(encoder,&render);
        if (bad != 5) ogpu_next_bind_executable(encoder,pipelines[0]);
        if (bad != 4) ogpu_next_set_graphics_state(encoder,&vp.header);
        if (bad == 0) ogpu_next_draw_indexed(encoder,&draw); /* No index binding. */
        else if (bad == 1) {
            ogpu_next_bind_indices(encoder,(ogpu_next_span){host,256,6},OGPU_NEXT_INDEX_U16);
            draw.count = 4; ogpu_next_draw_indexed(encoder,&draw); draw.count = 3;
        } else if (bad < 4) {
            ogpu_next_indirect invalid = {{host,272,64},bad == 2 ? 4 : 32,2,
                {bad == 3 ? host : NULL,bad == 3 ? 384 : 0,bad == 3 ? 2 : 0}};
            ogpu_next_draw_indirect(encoder,&invalid); /* Bad stride or count span. */
        } else ogpu_next_draw(encoder,&draw); /* Missing viewport or executable. */
        REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL);
    }
    printf("Graphics passes: direct/indexed/all indirect variants, viewport/scissor, depth rejection, additive blending, LOAD scopes, changed-root/count replay and attachmentless scopes.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending graphics after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done);
    for (uint32_t i = 0; i < 2; ++i) { ogpu_next_executable_destroy(pipelines[i]); ogpu_next_view_destroy(views[i]); ogpu_next_image_destroy(images[i]); ogpu_next_memory_destroy(backing[i]); }
    ogpu_next_memory_destroy(host);
    if (file) fclose(file);
    free(code[0]);free(code[1]);free(scratch);free(barrier_scratch);free(submit_scratch);
    return result;
}
