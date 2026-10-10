/* All four secondary-source factors, independently in all four factor fields.
 * RGBA32_FLOAT and binary fractions avoid UNORM rounding policy in the oracle. */
static int dual_source_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                                 const char *vertex_path, const char *fragment_path) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code[2] = {NULL}, *bs = NULL, *rs = NULL, *ss = NULL;
    ogpu_next_executable *pipelines[16] = {NULL};
    ogpu_next_memory *memory = NULL, *backing = NULL;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); TRY(ogpu_next_device_query(device,&q));
    if (!(features.enabled & OGPU_NEXT_FEATURE_DUAL_SOURCE_BLEND)) {
        printf("Dual-source execution NOT exercised: feature unavailable.\n"); return EXIT_SUCCESS;
    }
    ogpu_next_graphics_limits limits = {0}; q = query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&limits,1,sizeof(limits));
    TRY(ogpu_next_device_query(device,&q)); REQUIRE(limits.max_dual_source_attachments >= 1);
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,16};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader shaders[2]; const char *paths[2] = {vertex_path,fragment_path};
    for (uint32_t i = 0; i < 2; ++i) {
        file = fopen(paths[i],"rb"); REQUIRE(file != NULL);
        REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file,0,SEEK_SET) == 0); code[i] = malloc((size_t)size); REQUIRE(code[i] != NULL);
        REQUIRE(fread(code[i],1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
        shaders[i] = (ogpu_next_shader){i ? OGPU_NEXT_STAGE_FRAGMENT : OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,
            {code[i],(size_t)size},i ? "fragmentMain" : "vertexMain",&abi.header,NULL,NULL};
    }
    ogpu_next_color_state color = {OGPU_NEXT_RGBA32_FLOAT,15,1,1,1,0,1,1,0};
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,1,&color,0,0,0,0,{0},0,{0},{0},NULL,NULL,0,0};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),
        OGPU_NEXT_FEATURE_RASTER | OGPU_NEXT_FEATURE_DUAL_SOURCE_BLEND,{0,0,0},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,
        2,shaders,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
    for (uint32_t i = 0; i < 16; ++i) {
        color.src_color = color.dst_color = color.src_alpha = color.dst_alpha = OGPU_NEXT_BLEND_ONE;
        uint32_t *factor_fields[4] = {&color.src_color,&color.dst_color,&color.src_alpha,&color.dst_alpha};
        *factor_fields[i/4] = OGPU_NEXT_BLEND_SRC1_COLOR+i%4;
        TRY(ogpu_next_executable_create(device,&ed,&pipelines[i]));
    }
    for (uint32_t i = 0; i < 2; ++i) { free(code[i]); code[i] = NULL; }
    md.size = 32768; TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA32_FLOAT,
        OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,0,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements requirements = {0};
    requirements.compatible_types = types; requirements.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&requirements)); REQUIRE(requirements.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,requirements.size}));
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA32_FLOAT,
        OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0,0,0,0},range};
    TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_host_requirements br = {0}, rr = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && rr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); rs = malloc((size_t)rr.size); ss = malloc((size_t)sr.size); REQUIRE(bs && rs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_image_barrier barrier = {image,range,0,OGPU_NEXT_ACCESS_COLOR_READ | OGPU_NEXT_ACCESS_COLOR_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),0,OGPU_NEXT_STAGE_COLOR,0,0,0,1,0,NULL,&barrier,bs,br.size};
    const float initial[4] = {0.5f,0.25f,0.125f,0.75f};
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0.5f,0.25f,0.125f,0.75f}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,8,8,1,0,1,0,1,&attachment,NULL,NULL,{rs,rr.size}};
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
    ogpu_next_draw_desc draw = {3,1,0,0,0};
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    for (uint32_t i = 0; i < 16; ++i) {
        if (i) {
            dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_COLOR;
            barrier.before = OGPU_NEXT_ACCESS_COPY_READ; barrier.after = OGPU_NEXT_ACCESS_COLOR_READ | OGPU_NEXT_ACCESS_COLOR_WRITE;
            barrier.old_state = OGPU_NEXT_STATE_COPY_SRC; barrier.new_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT;
        }
        ogpu_next_barrier(encoder,&dep);
        ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,pipelines[i]);
        ogpu_next_set_graphics_state(encoder,&vp.header); ogpu_next_set_root(encoder,slot.stages,0,address);
        ogpu_next_draw(encoder,&draw); ogpu_next_draw(encoder,&draw); ogpu_next_render_end(encoder);
        barrier.before = OGPU_NEXT_ACCESS_COLOR_WRITE; barrier.after = OGPU_NEXT_ACCESS_COPY_READ;
        barrier.old_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0;
        dep.before = OGPU_NEXT_STAGE_COLOR; dep.after = OGPU_NEXT_STAGE_COPY; ogpu_next_barrier(encoder,&dep);
        ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,512+i*1024,1024},image,&copy);
    }
    dep.image_count = 0; dep.images = NULL; dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ; ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data,0xa5,(size_t)md.size);
        float root[12] = {0.125f,0.25f,0.5f,0.75f,0.5f,1,0,0,0.25f,0.5f,0.75f,0.25f};
        if (replay) { root[0]=0.75f; root[1]=0.5f; root[2]=0.25f; root[3]=0.125f;
            root[8]=0.75f; root[9]=0.25f; root[10]=0.5f; root[11]=0.75f; }
        memcpy(map.data,root,sizeof(root)); TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t i = 0; i < 16; ++i) for (uint32_t c = 0; c < 4; ++c) {
            float sf=1, df=1, factor=root[8+(i%4 >= 2 ? 3 : c)], expected=initial[c];
            if (i%2) factor=1-factor;
            if ((i/4 == 0 && c < 3) || (i/4 == 2 && c == 3)) sf=factor;
            if ((i/4 == 1 && c < 3) || (i/4 == 3 && c == 3)) df=factor;
            for (uint32_t n = 0; n < 2; ++n) expected = root[c]*sf+expected*df;
            const float *pixels = (const float *)((char *)map.data+512+i*1024);
            for (uint32_t p = 0; p < 64; ++p) {
                if (pixels[p*4+c] != expected) fprintf(stderr,"dual-source case %u replay %u pixel %u channel %u: %g != %g\n",i,replay,p,c,pixels[p*4+c],expected);
                REQUIRE(pixels[p*4+c] == expected);
            }
        }
        REQUIRE(memcmp(map.data,root,sizeof(root)) == 0);
        for (uint64_t i = sizeof(root); i < md.size; ++i)
            if (!(i >= 512 && i < 512+16*1024)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Dual-source blending passes: 16 factor/field combinations, distinct source indices, two draws, float outputs, changed-input replay and guards.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending dual-source work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); for (uint32_t i = 0; i < 16; ++i) ogpu_next_executable_destroy(pipelines[i]);
    ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view); ogpu_next_image_destroy(image);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code[0]); free(code[1]); free(bs); free(rs); free(ss); return result;
}
