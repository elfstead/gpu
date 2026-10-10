/* Vertex-only rasterization, not discard. Modes: 0/depth rejection, 1/clamp,
 * 2/clipping control, 3/stored-depth bounds, 4/positive clamped bias,
 * 5/negative clamped bias, 6/unclamped bias. Every list replays changed roots. */
static int depth_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                           const char *vertex_path, uint32_t mode) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code = NULL, *bs = NULL, *ss = NULL;
    ogpu_next_executable *pipeline = NULL, *bounded = NULL;
    ogpu_next_memory *memory = NULL, *backing = NULL;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    uint64_t required = mode == 1 ? OGPU_NEXT_FEATURE_DEPTH_CLAMP : mode == 3 ? OGPU_NEXT_FEATURE_DEPTH_BOUNDS
        : (mode == 4 || mode == 5) ? OGPU_NEXT_FEATURE_DEPTH_BIAS_CLAMP : 0;
    if (required & ~features.enabled) { printf("Depth-only mode %u NOT exercised: feature unavailable.\n",mode); return EXIT_SUCCESS; }
    file = fopen(vertex_path,"rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
    REQUIRE(fseek(file,0,SEEK_SET) == 0); code = malloc((size_t)size); REQUIRE(code != NULL);
    REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_VERTEX,0,16};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},"vertexMain",&abi.header,NULL,NULL};
    ogpu_next_raster_options options = {HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),
        0,mode == 1,0,OGPU_NEXT_POLYGON_FILL,mode >= 4,0,0,0,1,0,0,1,UINT64_MAX,0,0,0,0};
    if (mode == 4 || mode == 5) {
        options.depth_bias_constant = mode == 4 ? 268435456.f : -268435456.f;
        options.depth_bias_clamp = mode == 4 ? 0.0625f : -0.0625f;
    }
    if (mode == 6) options.depth_bias_constant = 16;
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,0,NULL,
        OGPU_NEXT_D32_FLOAT,1,1,mode == 0 ? OGPU_NEXT_COMPARE_LESS : OGPU_NEXT_COMPARE_ALWAYS,{0},0,{0},{0},NULL,&options.header};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),OGPU_NEXT_FEATURE_RASTER,{0,0,0},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,
        1,&shader,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device,&ed,&pipeline));
    if (mode == 3) {
        options.depth_bounds = 1; options.max_depth_bounds = 0.5f;
        TRY(ogpu_next_executable_create(device,&ed,&bounded));
    }
    free(code); code = NULL;
    TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map)); uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_DEPTH,0,1,0,1};
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_D32_FLOAT,
        OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT,0,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements requirements = {0};
    requirements.compatible_types = types; requirements.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&requirements)); REQUIRE(requirements.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,requirements.size}));
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_D32_FLOAT,
        OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT,{0,0,0,0},range};
    TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_host_requirements br = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); ss = malloc((size_t)sr.size); REQUIRE(bs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_image_barrier barrier = {image,range,0,OGPU_NEXT_ACCESS_DEPTH_READ | OGPU_NEXT_ACCESS_DEPTH_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),0,OGPU_NEXT_STAGE_DEPTH,0,0,0,1,0,NULL,&barrier,bs,br.size};
    ogpu_next_barrier(encoder,&dep);
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={1,0}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,8,8,1,0,1,0,0,NULL,&attachment,NULL,{NULL,0}};
    ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,pipeline);
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,4,8};
    ogpu_next_draw_desc draw = {3,1,0,0,0};
    for (uint32_t half = 0; half < 2; ++half) {
        vp.scissor_x = half ? (mode == 0 ? 0 : 4) : 0; vp.scissor_width = half && mode == 0 ? 8 : 4;
        ogpu_next_set_graphics_state(encoder,&vp.header); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX,0,address+half*32);
        ogpu_next_draw(encoder,&draw);
    }
    if (mode == 3) {
        ogpu_next_bind_executable(encoder,bounded); vp.scissor_x = 0; vp.scissor_width = 8;
        ogpu_next_set_graphics_state(encoder,&vp.header); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX,0,address+64);
        ogpu_next_draw(encoder,&draw);
    }
    ogpu_next_render_end(encoder);
    barrier.before = barrier.after; barrier.after = OGPU_NEXT_ACCESS_COPY_READ;
    barrier.old_state = barrier.new_state; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0;
    dep.before = OGPU_NEXT_STAGE_DEPTH; dep.after = OGPU_NEXT_STAGE_COPY; ogpu_next_barrier(encoder,&dep);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_DEPTH,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,512,256},image,&copy);
    dep.image_count = 0; dep.images = NULL; dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ; ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data,0xa5,(size_t)md.size);
        float depths[3] = {0.25f+replay*0.125f,0.75f+replay*0.125f,0.875f-replay*0.375f};
        if (mode == 1 || mode == 2) { depths[0] = replay ? 1.5f : -0.5f; depths[1] = replay ? -0.5f : 1.5f; }
        if (mode == 3) { depths[0] = replay ? 0.75f : 0.25f; depths[1] = replay ? 0.25f : 0.75f; }
        float roots[24] = {0};
        for (uint32_t i = 0; i < 3; ++i) { roots[i*8+4] = depths[i]; roots[i*8+5] = 1; }
        memcpy(map.data,roots,sizeof(roots)); TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t y = 0; y < 8; ++y) for (uint32_t x = 0; x < 8; ++x) {
            float expected = depths[x >= 4], actual = ((float *)((char *)map.data+512))[y*8+x];
            if (mode == 1) expected = expected < 0 ? 0 : 1;
            if (mode == 2) expected = 1;
            if (mode == 3 && expected <= 0.5f) expected = depths[2];
            if (mode == 4) expected += 0.0625f;
            if (mode == 5) expected -= 0.0625f;
            if (mode == 6) REQUIRE(actual > expected && actual < expected+0.001f);
            else {
                if (actual != expected) fprintf(stderr,"depth mode %u replay %u (%u,%u): %g != %g\n",mode,replay,x,y,actual,expected);
                REQUIRE(actual == expected);
            }
        }
        REQUIRE(memcmp(map.data,roots,sizeof(roots)) == 0);
        for (uint64_t i = sizeof(roots); i < md.size; ++i)
            if (!(i >= 512 && i < 768)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Depth-only passes: mode %u, no fragment artifact, changed-root replay, D32 readback and guards.\n",mode);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending depth-only work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(pipeline); ogpu_next_executable_destroy(bounded);
    ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view); ogpu_next_image_destroy(image);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(bs); free(ss); return result;
}
