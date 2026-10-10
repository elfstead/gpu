/* Modes: 0/null defaults, 1/explicit, 2/zero mask, 3/high-only mask, 4/high+low,
 * 5/zero-alpha coverage, 6/alpha-to-one, 7/low-only mask, 8/partial MSAA mask,
 * 9/full sample shading, 10/attached discard, 11/undefined target, 12/null view,
 * 13/fewer pipeline outputs, 14/fewer scope attachments, 15..30/all logic operations.
 * No state normalization. */
static uint32_t logic_reference(uint32_t op, uint32_t s, uint32_t d) {
    switch (op) {
        case OGPU_NEXT_LOGIC_CLEAR: return 0;
        case OGPU_NEXT_LOGIC_AND: return s & d;
        case OGPU_NEXT_LOGIC_AND_REVERSE: return s & ~d;
        case OGPU_NEXT_LOGIC_COPY: return s;
        case OGPU_NEXT_LOGIC_AND_INVERTED: return ~s & d;
        case OGPU_NEXT_LOGIC_NO_OP: return d;
        case OGPU_NEXT_LOGIC_XOR: return s ^ d;
        case OGPU_NEXT_LOGIC_OR: return s | d;
        case OGPU_NEXT_LOGIC_NOR: return ~(s | d);
        case OGPU_NEXT_LOGIC_EQUIVALENT: return ~(s ^ d);
        case OGPU_NEXT_LOGIC_INVERT: return ~d;
        case OGPU_NEXT_LOGIC_OR_REVERSE: return s | ~d;
        case OGPU_NEXT_LOGIC_COPY_INVERTED: return ~s;
        case OGPU_NEXT_LOGIC_OR_INVERTED: return ~s | d;
        case OGPU_NEXT_LOGIC_NAND: return ~(s & d);
        case OGPU_NEXT_LOGIC_SET: return UINT32_MAX;
        default: abort();
    }
}
static int mrt_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                         const char *vertex_path, const char *fragment_path, uint32_t mode) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL;
    void *code[2] = {NULL}, *bs = NULL, *rs = NULL, *ss = NULL;
    ogpu_next_executable *pipeline = NULL;
    ogpu_next_memory *memory = NULL, *backing[4] = {NULL};
    ogpu_next_image *images[4] = {NULL}; ogpu_next_view *views[4] = {NULL};
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    if (!(features.enabled & OGPU_NEXT_FEATURE_INDEPENDENT_BLEND)) {
        printf("Independent blend execution NOT exercised: feature unavailable.\n"); return EXIT_SUCCESS;
    }
    uint64_t required = mode >= 15 ? OGPU_NEXT_FEATURE_LOGIC_OP : mode >= 10 ? OGPU_NEXT_FEATURE_UNUSED_ATTACHMENTS
        : mode == 6 ? OGPU_NEXT_FEATURE_ALPHA_TO_ONE : mode == 9 ? OGPU_NEXT_FEATURE_SAMPLE_SHADING : 0;
    if (required & ~features.enabled) {
        printf("Raster options mode %u NOT exercised: feature unavailable.\n",mode); return EXIT_SUCCESS;
    }
    uint32_t samples = mode >= 8 && mode != 10 ? 4 : 1, image_count = samples == 1 ? 2 : 4;
    ogpu_next_graphics_limits limits = {0}; q = query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&limits,1,sizeof(limits));
    TRY(ogpu_next_device_query(device,&q));
    if (!(limits.color_samples & samples)) {
        printf("Raster options mode %u NOT exercised: sample count unavailable.\n",mode); return EXIT_SUCCESS;
    }
    ogpu_next_root_slot root = {OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,16};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&root};
    const char *paths[2] = {vertex_path,fragment_path}; ogpu_next_shader shaders[2];
    for (uint32_t i = 0; i < 2; ++i) {
        file = fopen(paths[i],"rb"); REQUIRE(file != NULL);
        REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file,0,SEEK_SET) == 0); code[i] = malloc((size_t)size); REQUIRE(code[i] != NULL);
        REQUIRE(fread(code[i],1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
        shaders[i] = (ogpu_next_shader){i ? OGPU_NEXT_STAGE_FRAGMENT : OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,
            {code[i],(size_t)size},i ? "fragmentMain" : "vertexMain",&abi.header,NULL,NULL};
    }
    ogpu_next_color_state states[2] = {
        {OGPU_NEXT_RGBA8_UNORM,15,1,OGPU_NEXT_BLEND_ONE,OGPU_NEXT_BLEND_ONE,OGPU_NEXT_BLEND_ADD,OGPU_NEXT_BLEND_ONE,OGPU_NEXT_BLEND_ONE,OGPU_NEXT_BLEND_ADD},
        {OGPU_NEXT_RGBA8_UNORM,5,0,OGPU_NEXT_BLEND_ONE,OGPU_NEXT_BLEND_ZERO,OGPU_NEXT_BLEND_ADD,OGPU_NEXT_BLEND_ONE,OGPU_NEXT_BLEND_ZERO,OGPU_NEXT_BLEND_ADD}
    };
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,2,states,0,0,0,0,{0},0,{0},{0},NULL,NULL,0,0};
    if (mode >= 15) {
        gs.logic_op_enable = 1; gs.logic_op = mode-15;
        // Caller state is explicit. A separate diagnostic exercises native logic
        // precedence over blendEnable; do not rewrite this in the implementation.
        const char *both = getenv("OGPU_FOUNDATION_LOGIC_BLEND_BOTH");
        states[0].blend = both && strcmp(both,"1") == 0;
        if (mode == 15) printf("Logic operation execution: blendEnable=%u (explicit consumer state).\n",states[0].blend);
    }
    ogpu_next_raster_options options = {HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),
        0,0,0,OGPU_NEXT_POLYGON_FILL,0,0,0,0,1,0,0,1,UINT64_MAX,mode == 9,1,mode == 5,mode == 6};
    if (mode == 2) options.sample_mask = 0;
    if (mode == 3) options.sample_mask = UINT64_C(1) << 63;
    if (mode == 4) options.sample_mask = (UINT64_C(1) << 63) | 1;
    if (mode == 7) options.sample_mask = 1;
    if (mode == 8) options.sample_mask = 5;
    gs.samples = samples; gs.raster_options = mode ? &options.header : NULL;
    if (mode == 10) { options.discard = 1; gs.color_count = 0; }
    if (mode == 11) { states[1].format = 0; states[1].write_mask = 0; }
    if (mode == 13) gs.color_count = 1;
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),
        OGPU_NEXT_FEATURE_RASTER | OGPU_NEXT_FEATURE_INDEPENDENT_BLEND,{0,0,0},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,
        2,shaders,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
    if (options.discard) ed.shader_count = 1;
    TRY(ogpu_next_executable_create(device,&ed,&pipeline));
    for (uint32_t i = 0; i < 2; ++i) { free(code[i]); code[i] = NULL; }
    TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map)); uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    for (uint32_t i = 0; i < image_count; ++i) {
        ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,
            OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,0,NULL,0,0,NULL};
        desc.sample_count = i < 2 ? samples : 1;
        uint32_t types[32]; ogpu_next_requirements requirements = {0}; requirements.compatible_types = types; requirements.compatible_type_capacity = 32;
        TRY(ogpu_next_image_requirements(device,&desc,&requirements)); REQUIRE(requirements.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device,&desc,&images[i]));
        TRY(ogpu_next_memory_create_dedicated_image(images[i],types[0],&backing[i]));
        TRY(ogpu_next_image_bind(images[i],(ogpu_next_span){backing[i],0,requirements.size}));
        ogpu_next_view_desc view = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,
            OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0,0,0,0},range};
        TRY(ogpu_next_view_create(images[i],&view,&views[i]));
    }
    ogpu_next_host_requirements br = {0}, rr = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,image_count,&br)); TRY(ogpu_next_render_scratch_requirements(2,&rr));
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && rr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); rs = malloc((size_t)rr.size); ss = malloc((size_t)sr.size); REQUIRE(bs && rs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_image_barrier barriers[4]; ogpu_next_attachment attachments[2];
    for (uint32_t i = 0; i < image_count; ++i) {
        barriers[i] = (ogpu_next_image_barrier){images[i],range,0,OGPU_NEXT_ACCESS_COLOR_READ | OGPU_NEXT_ACCESS_COLOR_WRITE,
            OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
        if (i < 2) attachments[i] = (ogpu_next_attachment){views[i],samples == 1 ? NULL : views[i+2],OGPU_NEXT_STATE_COLOR_ATTACHMENT,
            samples == 1 ? 0 : OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,
            samples == 1 ? OGPU_NEXT_ATTACHMENT_RESOLVE_NONE : OGPU_NEXT_ATTACHMENT_RESOLVE_AVERAGE,{.f32={0,0,0,0}}};
    }
    attachments[1].clear = (ogpu_next_clear_value){.f32={16.f/255,34.f/255,48.f/255,68.f/255}};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),0,OGPU_NEXT_STAGE_COLOR,0,0,0,image_count,0,NULL,barriers,bs,br.size};
    ogpu_next_barrier(encoder,&dep);
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,8,8,1,0,samples,0,2,attachments,NULL,NULL,{rs,rr.size}};
    if (mode == 12 || mode == 14) {
        // Explicitly initialize both targets, including the target omitted below.
        ogpu_next_render_begin(encoder,&render); ogpu_next_render_end(encoder);
        dep.before = dep.after = OGPU_NEXT_STAGE_COLOR;
        dep.global_before = OGPU_NEXT_ACCESS_COLOR_WRITE;
        dep.global_after = OGPU_NEXT_ACCESS_COLOR_READ | OGPU_NEXT_ACCESS_COLOR_WRITE;
        dep.image_count = 0; dep.images = NULL; ogpu_next_barrier(encoder,&dep);
        dep.global_before = dep.global_after = 0; dep.image_count = image_count; dep.images = barriers;
        attachments[0].load_op = attachments[1].load_op = OGPU_NEXT_LOAD;
        if (mode == 12) attachments[1] = (ogpu_next_attachment){0};
        if (mode == 14) render.color_count = 1;
    }
    ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,pipeline);
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
    ogpu_next_set_graphics_state(encoder,&vp.header); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,address);
    ogpu_next_draw_desc draw = {3,1,0,0,0}; ogpu_next_draw(encoder,&draw);
    if (mode >= 15) ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,address+32);
    ogpu_next_draw(encoder,&draw);
    ogpu_next_render_end(encoder);
    for (uint32_t i = 0; i < image_count; ++i) {
        barriers[i].before = OGPU_NEXT_ACCESS_COLOR_WRITE; barriers[i].after = OGPU_NEXT_ACCESS_COPY_READ;
        barriers[i].old_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT; barriers[i].new_state = OGPU_NEXT_STATE_COPY_SRC; barriers[i].discard = 0;
    }
    dep.before = OGPU_NEXT_STAGE_COLOR; dep.after = OGPU_NEXT_STAGE_COPY; ogpu_next_barrier(encoder,&dep);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    for (uint32_t i = 0; i < 2; ++i) ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,512+i*512,256},images[i+(samples == 1 ? 0 : 2)],&copy);
    dep.image_count = 0; dep.images = NULL; dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ; ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data,0xa5,(size_t)md.size); uint32_t r = 32+replay*16, g = 64+replay*16, b = 96+replay*16;
        float root_data[16] = {r/255.f,g/255.f,b/255.f,(128+replay*16)/255.f,0.5f,1,0,0,
            (160-r)/255.f,32/255.f,(176+replay*16)/255.f,64/255.f,0.5f,1,0,0}; memcpy(map.data,root_data,sizeof(root_data));
        if (mode == 5 || mode == 6) { root_data[3] = 0; memcpy(map.data,root_data,sizeof(root_data)); }
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        uint32_t expected[2] = {(r*2) | ((g*2)<<8) | ((b*2)<<16) | UINT32_C(0xff000000),b | (34u<<8) | (r<<16) | (68u<<24)};
        if (mode == 2 || mode == 3 || mode == 5) { expected[0] = 0; expected[1] = 16 | (34u<<8) | (48u<<16) | (68u<<24); }
        if (mode == 8) { expected[0] = r | (g<<8) | (b<<16) | UINT32_C(0x80000000);
            expected[1] = (b+16)/2 | (34u<<8) | ((r+48)/2<<16) | (68u<<24); }
        if (mode >= 10) expected[1] = 16 | (34u<<8) | (48u<<16) | (68u<<24);
        if (mode == 10) expected[0] = 0;
        if (mode >= 15) {
            uint32_t source[2][2] = {
                {r | (g<<8) | (b<<16) | ((128+replay*16)<<24), b | (g<<8) | (r<<16) | ((128+replay*16)<<24)},
                {(160-r) | (32u<<8) | ((176+replay*16)<<16) | (64u<<24), (176+replay*16) | (32u<<8) | ((160-r)<<16) | (64u<<24)}
            };
            expected[0] = 0; expected[1] = 16 | (34u<<8) | (48u<<16) | (68u<<24);
            for (uint32_t draw_index = 0; draw_index < 2; ++draw_index) for (uint32_t i = 0; i < 2; ++i) {
                uint32_t mask = i ? UINT32_C(0x00ff00ff) : UINT32_MAX;
                expected[i] = (expected[i] & ~mask) | (logic_reference(gs.logic_op,source[draw_index][i],expected[i]) & mask);
            }
        }
        for (uint32_t i = 0; i < 2; ++i) for (uint32_t pixel = 0; pixel < 64; ++pixel) {
            if (((uint32_t *)((char *)map.data+512+i*512))[pixel] != expected[i])
                fprintf(stderr,"MRT mode %u replay %u target %u pixel %u: %08x != %08x\n",mode,replay,i,pixel,((uint32_t *)((char *)map.data+512+i*512))[pixel],expected[i]);
            REQUIRE(((uint32_t *)((char *)map.data+512+i*512))[pixel] == expected[i]);
        }
        REQUIRE(memcmp(map.data,root_data,sizeof(root_data)) == 0);
        for (uint64_t i = sizeof(root_data); i < md.size; ++i)
            if (!(i >= 512 && i < 768) && !(i >= 1024 && i < 1280)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Independent blend / raster options pass: mode %u, samples %u, two targets, channel masks, two draws, changed-input replay and guards.\n",mode,samples);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending MRT work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(pipeline); ogpu_next_timeline_destroy(done);
    for (uint32_t i = 0; i < 4; ++i) { ogpu_next_view_destroy(views[i]); ogpu_next_image_destroy(images[i]); ogpu_next_memory_destroy(backing[i]); }
    for (uint32_t i = 0; i < 2; ++i) free(code[i]);
    ogpu_next_memory_destroy(memory); if (file) fclose(file);
    free(bs); free(rs); free(ss); return result;
}
