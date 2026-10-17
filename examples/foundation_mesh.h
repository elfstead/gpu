/* Native mesh/task execution, never a conventional-draw fallback. */
static int mesh_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain, char **paths) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code[4] = {NULL}, *bs = NULL, *rs = NULL, *ss = NULL;
    ogpu_next_executable *pipelines[2] = {NULL};
    ogpu_next_memory *memory = NULL, *backing = NULL;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0}; ogpu_next_mesh_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_MESH_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    if (!(features.enabled & OGPU_NEXT_FEATURE_MESH_SHADER)) {
        printf("MESH_UNVERIFIED native mesh/task execution unavailable; no fallback.\n"); return EXIT_SUCCESS;
    }
    uint32_t variants = features.enabled & OGPU_NEXT_FEATURE_TASK_SHADER ? 2 : 1;
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),16,0,NULL};
    ogpu_next_shader shaders[4];
    const uint32_t stages[4] = {OGPU_NEXT_STAGE_MESH,OGPU_NEXT_STAGE_TASK,OGPU_NEXT_STAGE_MESH,OGPU_NEXT_STAGE_FRAGMENT};
    const char *entries[4] = {"meshMain","taskMain","payloadMain","fragmentMain"};
    for (uint32_t i = 0; i < 4; ++i) {
        file = fopen(paths[i],"rb"); REQUIRE(file);
        REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file,0,SEEK_SET) == 0); code[i] = malloc((size_t)size); REQUIRE(code[i]);
        REQUIRE(fread(code[i],1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
        shaders[i] = (ogpu_next_shader){stages[i],OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)size},entries[i],&abi.header,NULL,NULL};
    }
    ogpu_next_color_state color = {OGPU_NEXT_RGBA8_UNORM,15,0,0,0,0,0,0,0};
    /* Input topology is ignored by mesh pipelines; deliberately not a vertex enum. */
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        UINT32_MAX,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,1,&color,0,0,0,0,{0},0,{0},{0},NULL,NULL,0,0};
    REQUIRE(limits.output_vertex_granularity > 0);
    uint32_t padded_vertices = ((3+limits.output_vertex_granularity-1)/limits.output_vertex_granularity)*limits.output_vertex_granularity;
    ogpu_next_mesh_requirements req = {HEADER(ogpu_next_mesh_requirements,OGPU_NEXT_MESH_REQUIREMENTS),
        OGPU_NEXT_FEATURE_MESH_SHADER,{0,0,0},0,{1,1,1},0,0,32*padded_vertices,3,1,8,1};
    for (uint32_t variant = 0; variant < variants; ++variant) {
        ogpu_next_shader selected[3] = {variant ? shaders[1] : shaders[0],variant ? shaders[2] : shaders[3],shaders[3]};
        req.task_local[0] = req.task_local[1] = req.task_local[2] = variant;
        req.payload_size = variant ? 16 : 0;
        ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,
            variant ? 3 : 2,selected,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
        req.mesh_local[0] = 0;
        REQUIRE(ogpu_next_executable_create(device,&ed,&pipelines[variant]) == OGPU_NEXT_INVALID && !pipelines[variant]);
        req.mesh_local[0] = 1;
        TRY(ogpu_next_executable_create(device,&ed,&pipelines[variant]));
    }
    for (uint32_t i = 0; i < 4; ++i) { free(code[i]); code[i] = NULL; }
    md.size = 8192; TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    memset(map.data,0xa5,(size_t)md.size);
    const uint32_t wire[] = {1,1,1,0xa5a5a5a5,1,1,1,0xa5a5a5a5,1,0,2};
    memcpy(map.data,wire,sizeof(wire)); TRY(ogpu_next_memory_flush(all));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,
        OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,0,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements requirements = {0};
    requirements.compatible_types = types; requirements.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&requirements)); REQUIRE(requirements.compatible_type_count);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,requirements.size}));
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,
        OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0},range}; TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_host_requirements br = {0}, rr = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && rr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); rs = malloc((size_t)rr.size); ss = malloc((size_t)sr.size); REQUIRE(bs && rs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_image_barrier barrier = {image,range,0,OGPU_NEXT_ACCESS_COLOR_READ | OGPU_NEXT_ACCESS_COLOR_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),0,OGPU_NEXT_STAGE_COLOR,0,0,0,1,0,NULL,&barrier,bs,br.size};
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,1,1}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,8,8,1,0,1,0,1,&attachment,NULL,NULL,{rs,rr.size}};
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    const float red[4] = {1,0,0,1};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    for (uint32_t i = 0; i < variants*7; ++i) {
        uint32_t variant = i/7, mode = i%7;
        if (i) {
            dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_COLOR;
            barrier.before = OGPU_NEXT_ACCESS_COPY_READ; barrier.after = OGPU_NEXT_ACCESS_COLOR_READ | OGPU_NEXT_ACCESS_COLOR_WRITE;
            barrier.old_state = OGPU_NEXT_STATE_COPY_SRC; barrier.new_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT;
        }
        ogpu_next_barrier(encoder,&dep); ogpu_next_render_begin(encoder,&render);
        ogpu_next_bind_executable(encoder,pipelines[variant]); ogpu_next_set_graphics_state(encoder,&vp.header);
        ogpu_next_set_inline(encoder,variant ? OGPU_NEXT_STAGE_TASK : OGPU_NEXT_STAGE_MESH,0,sizeof(red),red);
        if (mode == 0 || mode == 4) ogpu_next_draw_mesh(encoder,(ogpu_next_extent){mode == 4 ? 0 : 1,1,1});
        else {
            ogpu_next_indirect indirect = {{memory,0,28},16,mode == 1 ? 2 : mode == 6 ? 0 : 1,{NULL,0,0}};
            if (mode != 1 && mode != 6) indirect.count = (ogpu_next_span){memory,mode == 3 ? 36 : mode == 5 ? 40 : 32,4};
            ogpu_next_draw_mesh_indirect(encoder,&indirect);
        }
        ogpu_next_render_end(encoder);
        barrier.before = OGPU_NEXT_ACCESS_COLOR_WRITE; barrier.after = OGPU_NEXT_ACCESS_COPY_READ;
        barrier.old_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0;
        dep.before = OGPU_NEXT_STAGE_COLOR; dep.after = OGPU_NEXT_STAGE_COPY; ogpu_next_barrier(encoder,&dep);
        ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,512+i*256,256},image,&copy);
    }
    dep.image_count = 0; dep.images = NULL; dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ; ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
    pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
    TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t i = 0; i < variants*7; ++i) {
        uint32_t mode = i%7;
        uint32_t expected = mode == 3 || mode == 4 || mode == 6 ? UINT32_C(0xffff0000) : i/7 ? UINT32_C(0xff00ff00) : UINT32_C(0xff0000ff);
        const uint32_t *pixels = (const uint32_t *)((const unsigned char *)map.data+512+i*256);
        for (uint32_t p = 0; p < 64; ++p) {
            if (pixels[p] != expected) fprintf(stderr,"Mesh variant=%u mode=%u pixel=%u got=%08x expected=%08x\n",i/7,mode,p,pixels[p],expected);
            REQUIRE(pixels[p] == expected);
        }
    }
    REQUIRE(memcmp(map.data,wire,sizeof(wire)) == 0);
    for (uint64_t i = sizeof(wire); i < md.size; ++i)
        if (!(i >= 512 && i < 512+variants*7*256)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    TRY(ogpu_next_arena_reset(arena));
    for (uint32_t bad = 0; bad < 5; ++bad) {
        TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        ogpu_next_bind_executable(encoder,pipelines[0]); ogpu_next_set_graphics_state(encoder,&vp.header);
        if (bad != 0) ogpu_next_render_begin(encoder,&render);
        if (bad == 0 || bad == 1) ogpu_next_draw_mesh(encoder,(ogpu_next_extent){bad == 1 ? limits.mesh.max_groups[0]+1 : 1,1,1});
        else if (bad == 2) { ogpu_next_draw_desc draw = {3,1,0,0,0}; ogpu_next_draw(encoder,&draw); }
        else { ogpu_next_indirect indirect = {{memory,0,bad == 3 ? 11 : 28},bad == 4 ? 8 : 16,1,{NULL,0,0}}; ogpu_next_draw_mesh_indirect(encoder,&indirect); }
        if (bad != 0) ogpu_next_render_end(encoder);
        REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && !list); TRY(ogpu_next_arena_reset(arena));
    }
    printf("MESH_PASS variants=%u modes=7 pixels=%u guards=1 rejected=5 task_payload=%u\n",variants,variants*7*64,variants == 2);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending mesh work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); for (uint32_t i = 0; i < 2; ++i) ogpu_next_executable_destroy(pipelines[i]);
    ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view); ogpu_next_image_destroy(image);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    for (uint32_t i = 0; i < 4; ++i) free(code[i]);
    free(bs); free(rs); free(ss); return result;
}
