/* Vertex-only rasterization, not discard. Modes: 0/depth rejection, 1/clamp,
 * 2/clipping control, 3/stored-depth bounds, 4/positive clamped bias,
 * 5/negative clamped bias, 6/unclamped bias, 7/shader-selected layers through an
 * offset array view; 8..11/four-sample depth resolves; 12..14/combined depth/stencil
 * resolves consumed by stencil-tested occlusion; 15..19/dynamic depth-test,
 * depth-write, compare, cull and front-face. Every list replays changed roots. */
static int depth_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                           const char *vertex_path, uint32_t mode) {
    int result = EXIT_FAILURE, pending = 0;
    uint32_t layered = mode == 7, layers = layered ? 3 : 1;
    uint32_t stencil_resolve = mode >= 12 && mode <= 14;
    uint32_t resolving = mode >= 8 && mode <= 14, resolve_mode = stencil_resolve ? (mode == 12 ? 1 : 1u << (mode-11)) : resolving ? 1u << (mode-8) : 0;
    FILE *file = NULL; void *code = NULL, *bs = NULL, *ss = NULL;
    ogpu_next_executable *pipeline = NULL, *bounded = NULL;
    ogpu_next_executable *probe = NULL; ogpu_next_query_pool *occlusion = NULL;
    ogpu_next_memory *memory = NULL, *backing = NULL;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL;
    ogpu_next_image *target = NULL; ogpu_next_view *target_view = NULL, *other_target_view = NULL; ogpu_next_memory *target_backing = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    uint64_t required = layered ? OGPU_NEXT_FEATURE_SHADER_OUTPUT_LAYER : mode == 1 ? OGPU_NEXT_FEATURE_DEPTH_CLAMP : mode == 3 ? OGPU_NEXT_FEATURE_DEPTH_BOUNDS
        : (mode == 4 || mode == 5) ? OGPU_NEXT_FEATURE_DEPTH_BIAS_CLAMP : 0;
    if (required & ~features.enabled) { printf("Depth-only mode %u NOT exercised: feature unavailable.\n",mode); return EXIT_SUCCESS; }
    ogpu_next_graphics_limits limits = {0}; q = query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&limits,1,sizeof(limits));
    TRY(ogpu_next_device_query(device,&q));
    if (resolving && (!(limits.depth_samples & 4) || !(limits.depth_resolve_modes & resolve_mode))) {
        printf("Depth-only mode %u NOT exercised: four samples/resolve mode unavailable.\n",mode); return EXIT_SUCCESS;
    }
    if (stencil_resolve && (!(limits.stencil_samples & 4) || !(limits.stencil_resolve_modes & resolve_mode))) {
        printf("Depth-only mode %u NOT exercised: stencil samples/resolve mode unavailable.\n",mode); return EXIT_SUCCESS;
    }
    file = fopen(vertex_path,"rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
    REQUIRE(fseek(file,0,SEEK_SET) == 0); code = malloc((size_t)size); REQUIRE(code != NULL);
    REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_VERTEX,0,16};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},"vertexMain",&abi.header,NULL,NULL};
    ogpu_next_raster_options options = {HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),
        0,mode == 1,0,OGPU_NEXT_POLYGON_FILL,mode >= 4 && mode <= 6,0,0,0,1,0,0,1,UINT64_MAX,0,0,0,0};
    if (mode == 4 || mode == 5) {
        options.depth_bias_constant = mode == 4 ? 268435456.f : -268435456.f;
        options.depth_bias_clamp = mode == 4 ? 0.0625f : -0.0625f;
    }
    if (mode == 6) options.depth_bias_constant = 16;
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,0,NULL,
        OGPU_NEXT_D32_FLOAT,1,1,mode == 0 ? OGPU_NEXT_COMPARE_LESS : OGPU_NEXT_COMPARE_ALWAYS,{0},0,{0},{0},NULL,&options.header,0,0};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),OGPU_NEXT_FEATURE_RASTER | (layered ? OGPU_NEXT_FEATURE_SHADER_OUTPUT_LAYER : 0),{0,0,0},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,
        1,&shader,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
    if (resolving) { gs.samples = 4; options.sample_mask = 3; }
    if (mode == 15) { ed.dynamic_state |= OGPU_NEXT_DYNAMIC_DEPTH_TEST; gs.depth_test = UINT32_MAX; }
    if (mode == 16) { ed.dynamic_state |= OGPU_NEXT_DYNAMIC_DEPTH_WRITE; gs.depth_write = UINT32_MAX; }
    if (mode == 17) { ed.dynamic_state |= OGPU_NEXT_DYNAMIC_DEPTH_COMPARE; gs.depth_compare = UINT32_MAX; }
    if (mode == 18) { ed.dynamic_state |= OGPU_NEXT_DYNAMIC_CULL; gs.cull = UINT32_MAX; }
    if (mode == 19) { ed.dynamic_state |= OGPU_NEXT_DYNAMIC_FRONT_FACE; gs.front_face = UINT32_MAX; gs.cull = OGPU_NEXT_CULL_FRONT; }
    if (stencil_resolve) {
        gs.depth_format = OGPU_NEXT_D32_FLOAT_S8_UINT; gs.stencil_test = 1;
        gs.stencil_front = (ogpu_next_stencil_state){OGPU_NEXT_STENCIL_KEEP,OGPU_NEXT_STENCIL_REPLACE,
            OGPU_NEXT_STENCIL_KEEP,OGPU_NEXT_COMPARE_ALWAYS,255,255,1}; gs.stencil_back = gs.stencil_front;
    }
    TRY(ogpu_next_executable_create(device,&ed,&pipeline));
    if (mode == 3) {
        options.depth_bounds = 1; options.max_depth_bounds = 0.5f;
        TRY(ogpu_next_executable_create(device,&ed,&bounded));
    }
    if (resolving) {
        options.sample_mask = 12; if (stencil_resolve) gs.stencil_front.reference = gs.stencil_back.reference = 3;
        TRY(ogpu_next_executable_create(device,&ed,&bounded));
    }
    if (stencil_resolve) {
        options.sample_mask = UINT64_MAX; gs.samples = 1; gs.depth_test = gs.depth_write = 0;
        gs.stencil_front = (ogpu_next_stencil_state){OGPU_NEXT_STENCIL_KEEP,OGPU_NEXT_STENCIL_KEEP,
            OGPU_NEXT_STENCIL_KEEP,OGPU_NEXT_COMPARE_EQUAL,255,0,0}; gs.stencil_back = gs.stencil_front;
        ed.dynamic_state |= OGPU_NEXT_DYNAMIC_STENCIL_REFERENCE;
        TRY(ogpu_next_executable_create(device,&ed,&probe));
        ogpu_next_query_pool_desc qd = {HEADER(ogpu_next_query_pool_desc,OGPU_NEXT_QUERY_POOL_DESC),OGPU_NEXT_QUERY_OCCLUSION,2,0};
        TRY(ogpu_next_query_pool_create(device,&qd,&occlusion));
    }
    free(code); code = NULL;
    TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map)); uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_DEPTH,0,1,0,layers};
    if (stencil_resolve) range.aspects |= OGPU_NEXT_ASPECT_STENCIL;
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_D32_FLOAT,
        OGPU_NEXT_IMAGE_2D,1,layers,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT | (layered ? OGPU_NEXT_IMAGE_COPY_DST : 0),0,NULL,0,0,NULL};
    if (resolving) id.sample_count = 4;
    if (stencil_resolve) id.format = OGPU_NEXT_D32_FLOAT_S8_UINT;
    uint32_t types[32]; ogpu_next_requirements requirements = {0};
    requirements.compatible_types = types; requirements.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&requirements)); REQUIRE(requirements.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,requirements.size}));
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_D32_FLOAT,
        OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT,{0,0,0,0},range};
    if (layered) { vd.dimension = OGPU_NEXT_VIEW_2D_ARRAY; vd.range.first_layer = 1; vd.range.layer_count = 2; }
    if (stencil_resolve) vd.format = id.format;
    TRY(ogpu_next_view_create(image,&vd,&view));
    if (resolving) {
        id.sample_count = 1;
        TRY(ogpu_next_image_requirements(device,&id,&requirements)); REQUIRE(requirements.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device,&id,&target));
        TRY(ogpu_next_memory_create_dedicated_image(target,types[0],&target_backing));
        TRY(ogpu_next_image_bind(target,(ogpu_next_span){target_backing,0,requirements.size}));
        TRY(ogpu_next_view_create(target,&vd,&target_view));
    }
    ogpu_next_host_requirements br = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); ss = malloc((size_t)sr.size); REQUIRE(bs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    if (stencil_resolve) ogpu_next_queries_reset(encoder,occlusion,0,2);
    ogpu_next_image_barrier barrier = {image,range,0,OGPU_NEXT_ACCESS_DEPTH_READ | OGPU_NEXT_ACCESS_DEPTH_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),0,OGPU_NEXT_STAGE_DEPTH,0,0,0,1,0,NULL,&barrier,bs,br.size};
    if (layered) {
        barrier.new_state = OGPU_NEXT_STATE_COPY_DST; barrier.after = OGPU_NEXT_ACCESS_COPY_WRITE; dep.after = OGPU_NEXT_STAGE_CLEAR;
        ogpu_next_barrier(encoder,&dep);
        ogpu_next_clear_value clear = {.depth_stencil={0.5f,0}};
        ogpu_next_clear_image(encoder,image,OGPU_NEXT_STATE_COPY_DST,&range,&clear);
        barrier.old_state = OGPU_NEXT_STATE_COPY_DST; barrier.new_state = OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT;
        barrier.before = OGPU_NEXT_ACCESS_COPY_WRITE; barrier.after = OGPU_NEXT_ACCESS_DEPTH_READ | OGPU_NEXT_ACCESS_DEPTH_WRITE; barrier.discard = 0;
        dep.before = OGPU_NEXT_STAGE_CLEAR; dep.after = OGPU_NEXT_STAGE_DEPTH;
    }
    ogpu_next_barrier(encoder,&dep);
    if (resolving) {
        barrier.image = target; barrier.after = OGPU_NEXT_ACCESS_COLOR_WRITE;
        dep.after = OGPU_NEXT_STAGE_COLOR; ogpu_next_barrier(encoder,&dep);
    }
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={1,0}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,8,8,1,0,1,0,0,NULL,&attachment,NULL,{NULL,0}};
    if (layered) render.layers = 2;
    if (resolving) {
        render.samples = 4; attachment.resolve_view = target_view;
        attachment.resolve_state = OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT;
        attachment.resolve_mode = resolve_mode; attachment.store_op = OGPU_NEXT_DISCARD;
    }
    ogpu_next_attachment stencil = attachment;
    if (stencil_resolve) render.stencil = &stencil;
    ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,pipeline);
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,4,8};
    ogpu_next_draw_desc draw = {3,1,0,0,0};
    for (uint32_t half = 0; half < 2; ++half) {
        vp.scissor_x = half ? (mode == 0 ? 0 : 4) : 0; vp.scissor_width = half && mode == 0 ? 8 : 4;
        if (resolving) { vp.scissor_x = 0; vp.scissor_width = 8; ogpu_next_bind_executable(encoder,half ? bounded : pipeline); }
        if (mode == 15) ogpu_next_set_depth_test(encoder,half);
        if (mode == 16) ogpu_next_set_depth_write(encoder,half);
        if (mode == 17) ogpu_next_set_depth_compare(encoder,half ? OGPU_NEXT_COMPARE_ALWAYS : OGPU_NEXT_COMPARE_NEVER);
        if (mode == 18) ogpu_next_set_cull(encoder,half ? OGPU_NEXT_CULL_NONE : OGPU_NEXT_CULL_BOTH);
        if (mode == 19) ogpu_next_set_front_face(encoder,half ? OGPU_NEXT_FRONT_CCW : OGPU_NEXT_FRONT_CW);
        ogpu_next_set_graphics_state(encoder,&vp.header); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX,0,address+half*32);
        ogpu_next_draw(encoder,&draw);
    }
    if (mode == 3) {
        ogpu_next_bind_executable(encoder,bounded); vp.scissor_x = 0; vp.scissor_width = 8;
        ogpu_next_set_graphics_state(encoder,&vp.header); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX,0,address+64);
        ogpu_next_draw(encoder,&draw);
    }
    ogpu_next_render_end(encoder);
    if (stencil_resolve) {
        barrier.before = OGPU_NEXT_ACCESS_COLOR_WRITE; barrier.after = OGPU_NEXT_ACCESS_DEPTH_READ;
        barrier.old_state = OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT; barrier.new_state = OGPU_NEXT_STATE_DEPTH_STENCIL_READ; barrier.discard = 0;
        dep.before = OGPU_NEXT_STAGE_COLOR; dep.after = OGPU_NEXT_STAGE_DEPTH; ogpu_next_barrier(encoder,&dep);
        attachment = (ogpu_next_attachment){target_view,NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_READ,0,OGPU_NEXT_LOAD,OGPU_NEXT_STORE,0,{.depth_stencil={0,0}}};
        stencil = attachment; render.samples = 1;
        ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,probe);
        for (uint32_t i = 0; i < 2; ++i) {
            ogpu_next_set_stencil_reference(encoder,OGPU_NEXT_FACE_BOTH,1+i*2);
            ogpu_next_query_begin(encoder,occlusion,i,features.enabled & OGPU_NEXT_FEATURE_PRECISE_OCCLUSION ? OGPU_NEXT_QUERY_PRECISE : 0);
            ogpu_next_draw(encoder,&draw); ogpu_next_query_end(encoder,occlusion,i);
        }
        ogpu_next_render_end(encoder);
        ogpu_next_queries_resolve(encoder,occlusion,0,2,(ogpu_next_span){memory,512,16},8,OGPU_NEXT_QUERY_RESULT_64 | OGPU_NEXT_QUERY_RESULT_WAIT);
    } else {
        barrier.before = barrier.after | (layered ? OGPU_NEXT_ACCESS_COPY_WRITE : 0); barrier.after = OGPU_NEXT_ACCESS_COPY_READ;
        barrier.old_state = barrier.new_state; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0;
        dep.before = resolving ? OGPU_NEXT_STAGE_COLOR : OGPU_NEXT_STAGE_DEPTH | (layered ? OGPU_NEXT_STAGE_CLEAR : 0);
        dep.after = OGPU_NEXT_STAGE_COPY; ogpu_next_barrier(encoder,&dep);
        ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_DEPTH,0,0,layers,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
        ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,512,256*layers},resolving ? target : image,&copy);
    }
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
        if (resolving && replay) { depths[0] = 0.875f; depths[1] = 0.375f; }
        float roots[24] = {0};
        for (uint32_t i = 0; i < 3; ++i) { roots[i*8+4] = depths[i]; roots[i*8+5] = 1; }
        if (layered) for (uint32_t i = 0; i < 2; ++i) { uint32_t layer = i ^ replay; memcpy(&roots[i*8+6],&layer,sizeof(layer)); }
        memcpy(map.data,roots,sizeof(roots)); TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        if (stencil_resolve) {
            const uint64_t *counts = (const uint64_t *)((const char *)map.data+512);
            uint32_t passing = resolve_mode == OGPU_NEXT_ATTACHMENT_RESOLVE_MAX;
            REQUIRE(counts[1-passing] == 0); REQUIRE(counts[passing] > 0);
            if (features.enabled & OGPU_NEXT_FEATURE_PRECISE_OCCLUSION) REQUIRE(counts[passing] == 64);
        } else for (uint32_t layer = 0; layer < layers; ++layer) for (uint32_t y = 0; y < 8; ++y) for (uint32_t x = 0; x < 8; ++x) {
            float expected = depths[x >= 4], actual = ((float *)((char *)map.data+512))[layer*64+y*8+x];
            if (layered) expected = layer == 0 ? 0.5f : layer-1 == ((x >= 4) ^ replay) ? expected : 1;
            if (resolving) expected = mode == 8 ? depths[0] : mode == 9 ? (depths[0]+depths[1])*0.5f
                : mode == 10 ? (depths[0] < depths[1] ? depths[0] : depths[1]) : (depths[0] > depths[1] ? depths[0] : depths[1]);
            if (mode == 1) expected = expected < 0 ? 0 : 1;
            if (mode == 2) expected = 1;
            if (mode == 3 && expected <= 0.5f) expected = depths[2];
            if (mode == 4) expected += 0.0625f;
            if (mode == 5) expected -= 0.0625f;
            if (mode >= 15 && mode <= 19 && x < 4) expected = 1;
            if (mode == 6) REQUIRE(actual > expected && actual < expected+0.001f);
            else {
                if (actual != expected) fprintf(stderr,"depth mode %u replay %u layer %u (%u,%u): %g != %g\n",mode,replay,layer,x,y,actual,expected);
                REQUIRE(actual == expected);
            }
        }
        REQUIRE(memcmp(map.data,roots,sizeof(roots)) == 0);
        for (uint64_t i = sizeof(roots); i < md.size; ++i)
            if (!(i >= 512 && i < 512+(stencil_resolve ? 16 : 256*layers))) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    if (mode == 8) for (uint32_t bad = 0; bad < 9; ++bad) {
        TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        ogpu_next_attachment invalid = attachment;
        ogpu_next_render_desc rejected = render; rejected.depth = &invalid;
        if (bad == 0) invalid.resolve_mode = 3;
        if (bad == 1) invalid.resolve_mode = 16;
        if (bad == 2) invalid.resolve_mode = OGPU_NEXT_ATTACHMENT_RESOLVE_NONE; /* Stray target. */
        if (bad == 3) invalid.resolve_view = NULL;
        if (bad == 4) invalid.resolve_view = view; /* Multisampled destination. */
        if (bad == 5) invalid.resolve_state = OGPU_NEXT_STATE_DEPTH_STENCIL_READ;
        if (bad == 6) invalid.resolve_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT;
        if (bad == 7) rejected.samples = 1;
        if (bad == 8) { invalid.view = NULL; invalid.state = OGPU_NEXT_STATE_UNDEFINED; }
        ogpu_next_render_begin(encoder,&rejected); list = NULL;
        REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL);
    }
    if (mode == 12) {
        /* Record-only matrix: capability rejection and paired native validation,
         * not an execution claim for every independent mode combination. */
        const uint32_t modes[] = {0,1,2,4,8};
        for (uint32_t di = 0; di < 5; ++di) for (uint32_t si = 0; si < 5; ++si) {
            uint32_t dm = modes[di], sm = modes[si];
            ogpu_next_attachment da = {view,dm ? target_view : NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,
                dm ? OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT : 0,OGPU_NEXT_CLEAR,OGPU_NEXT_DISCARD,dm,{.depth_stencil={1,0}}};
            ogpu_next_attachment sa = da; sa.resolve_view = sm ? target_view : NULL;
            sa.resolve_state = sm ? OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT : 0; sa.resolve_mode = sm;
            ogpu_next_render_desc candidate = render; candidate.samples = 4; candidate.depth = &da; candidate.stencil = &sa;
            int allowed = (!dm || (dm & limits.depth_resolve_modes)) && (!sm || (sm & limits.stencil_resolve_modes))
                && (dm == sm || ((!dm || !sm) ? limits.independent_resolve_none : limits.independent_resolve));
            TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
            ogpu_next_render_begin(encoder,&candidate); if (allowed) ogpu_next_render_end(encoder);
            list = NULL; ogpu_next_status status = ogpu_next_commands_end(encoder,&list);
            REQUIRE(status == (allowed ? OGPU_NEXT_OK : OGPU_NEXT_UNSUPPORTED)); REQUIRE((list != NULL) == allowed);
        }
        TRY(ogpu_next_view_create(target,&vd,&other_target_view));
        for (uint32_t bad = 0; bad < 2; ++bad) {
            ogpu_next_attachment da = {view,target_view,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,
                OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,OGPU_NEXT_CLEAR,OGPU_NEXT_DISCARD,1,{.depth_stencil={1,0}}};
            ogpu_next_attachment sa = da;
            if (bad == 0) sa.resolve_view = other_target_view;
            else sa.resolve_state = OGPU_NEXT_STATE_GENERAL;
            ogpu_next_render_desc rejected = render; rejected.samples = 4; rejected.depth = &da; rejected.stencil = &sa;
            TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
            ogpu_next_render_begin(encoder,&rejected); list = NULL;
            REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL);
        }
    }
    printf("Depth-only passes: mode %u, no fragment artifact, changed-root replay, %s and guards.\n",mode,stencil_resolve ? "resolved-stencil occlusion" : "D32 readback");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending depth-only work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(pipeline); ogpu_next_executable_destroy(bounded);
    ogpu_next_executable_destroy(probe); ogpu_next_query_pool_destroy(occlusion);
    ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view); ogpu_next_image_destroy(image);
    ogpu_next_view_destroy(other_target_view); ogpu_next_view_destroy(target_view); ogpu_next_image_destroy(target); ogpu_next_memory_destroy(target_backing);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(bs); free(ss); return result;
}
