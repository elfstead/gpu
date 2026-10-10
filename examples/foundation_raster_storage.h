#define RASTER_STORAGE_FEATURES (OGPU_NEXT_FEATURE_VERTEX_STORES_ATOMICS | OGPU_NEXT_FEATURE_FRAGMENT_STORES_ATOMICS)
/* No attachment allocation, descriptor heap, readback staging or retained resource list. */
static int raster_storage_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                                    const char *const paths[4], uint32_t writers, uint32_t secondary, uint32_t discard) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL;
    void *code[2] = {NULL}, *scratch = NULL, *execute_scratch = NULL;
    ogpu_next_memory *memory = NULL;
    ogpu_next_executable *executable = NULL;
    ogpu_next_arena *arena = NULL, *parent = NULL;
    ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    uint64_t required = OGPU_NEXT_FEATURE_RASTER
        | (writers & 1 ? OGPU_NEXT_FEATURE_VERTEX_STORES_ATOMICS : 0)
        | (writers & 2 ? OGPU_NEXT_FEATURE_FRAGMENT_STORES_ATOMICS : 0);
    if (required & ~features.enabled) {
        printf("Raster storage NOT exercised: missing feature mask 0x%llx.\n",(unsigned long long)(required & ~features.enabled));
        return EXIT_SUCCESS;
    }
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,8};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader shaders[2];
    for (uint32_t i = 0; i < 2; ++i) {
        uint32_t writing = writers & (1u << i);
        file = fopen(paths[i+(writing ? 0 : 2)],"rb"); REQUIRE(file != NULL);
        REQUIRE(fseek(file,0,SEEK_END) == 0);
        long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file,0,SEEK_SET) == 0);
        code[i] = malloc((size_t)size); REQUIRE(code[i] != NULL);
        REQUIRE(fread(code[i],1,(size_t)size,file) == (size_t)size);
        fclose(file); file = NULL;
        shaders[i] = (ogpu_next_shader){i ? OGPU_NEXT_STAGE_FRAGMENT : OGPU_NEXT_STAGE_VERTEX,
            OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)size},i ? (writing ? "fragmentMain" : "fragmentRead") : (writing ? "vertexMain" : "vertexRead"),&abi.header,NULL,NULL};
    }
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),required,{0,0,0},0};
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,0,NULL,0,0,0,0,{0},0,{0},{0},NULL,NULL};
    ogpu_next_raster_options options = {HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),
        0,0,discard,OGPU_NEXT_POLYGON_FILL,0,0,0,0,1,0,0,1,UINT64_MAX,0,0,0,0};
    gs.raster_options = &options.header;
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),
        OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
    if (discard) ed.shader_count = 1;
    TRY(ogpu_next_executable_create(device,&ed,&executable));
    for (uint32_t i = 0; i < 2; ++i) { free(code[i]); code[i] = NULL; }
    TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size};
    ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,secondary ? OGPU_NEXT_SECONDARY : OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,ad.level,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),
        0,0,8,8,1,0,1,0,0,NULL,NULL,NULL,{NULL,0}};
    ogpu_next_render_begin(encoder,&render);
    ogpu_next_bind_executable(encoder,executable);
    ogpu_next_viewport_state viewport = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
    ogpu_next_set_graphics_state(encoder,&viewport.header);
    ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,address);
    ogpu_next_draw_desc draw = {3,1,0,0,0}; ogpu_next_draw(encoder,&draw);
    ogpu_next_render_end(encoder);
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),
        OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_STAGE_HOST,
        OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_HOST_READ,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list));
    if (secondary) {
        ad.level = OGPU_NEXT_PRIMARY; rd.level = OGPU_NEXT_PRIMARY;
        TRY(ogpu_next_arena_create(device,&ad,&parent));
        ogpu_next_host_requirements er = {0}; TRY(ogpu_next_execute_scratch_requirements(1,&er));
        REQUIRE(er.alignment <= _Alignof(max_align_t)); execute_scratch = malloc((size_t)er.size); REQUIRE(execute_scratch != NULL);
        TRY(ogpu_next_commands_begin(parent,&rd,&encoder));
        ogpu_next_execute_lists(encoder,1,&list,(ogpu_next_host_span){execute_scratch,er.size});
        TRY(ogpu_next_commands_end(encoder,&list));
    }
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements sr = {0}; TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(sr.alignment <= _Alignof(max_align_t)); scratch = malloc((size_t)sr.size); REQUIRE(scratch != NULL);
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,scratch,sr.size};
    for (uint32_t replay = 0; replay < 3; ++replay) {
        memset(map.data,0xa5,(size_t)md.size);
        uint64_t output_address = address+512; uint32_t seed = replay+1;
        memcpy(map.data,&output_address,8); memcpy((char *)map.data+8,&seed,4);
        uint32_t *output = (uint32_t *)((char *)map.data+512);
        output[0] = output[1] = output[4] = output[5] = output[6] = 0;
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(all));
        REQUIRE(output[0] == (writers & 1 ? 7u : 0u));
        for (uint32_t i = 0; i < 3; ++i) REQUIRE(output[4+i] == (writers & 1 ? (i+1) << seed : 0));
        REQUIRE(output[1] == ((writers & 2) && !discard ? 2080+64*seed*17 : 0));
        for (uint32_t i = 0; i < 64; ++i) REQUIRE(output[16+i] == ((writers & 2) && !discard ? i+1+seed*17 : UINT32_C(0xa5a5a5a5)));
        REQUIRE(memcmp(map.data,&output_address,8) == 0 && memcmp((char *)map.data+8,&seed,4) == 0);
        for (uint64_t i = 12; i < md.size; ++i)
            if (!(i >= 512 && i < 520) && !(i >= 528 && i < 540) && !(i >= 576 && i < 832)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Raster storage passes: writer mask %u, discard %u, attachmentless draw, vertex OR / fragment stores + sum, secondary %u, changed-seed replay and guards.\n",writers,discard,secondary);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending raster storage after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(parent); ogpu_next_arena_destroy(arena);
    ogpu_next_executable_destroy(executable); ogpu_next_memory_destroy(memory); ogpu_next_timeline_destroy(done);
    if (file) fclose(file);
    free(code[0]); free(code[1]); free(scratch); free(execute_scratch);
    return result;
}
