/* Sparse view masks are native draws, not a host loop. Check the entire array,
 * including layers outside an offset view, after replay with changed roots. */
static int multiview_execution(ogpu_next_device *device, ogpu_next_memory_desc md,
                              uint32_t domain, const char *vertex_path) {
    int result = EXIT_FAILURE, pending = 0;
    /* VVL 1.4.357 tracks only the first timestamp slot. The ordinary validated
     * profile resolves that slot; opt in to complete native slot readback with
     * OGPU_FOUNDATION_MULTIVIEW_ALL_TIMESTAMPS=1. See the multiview receipt.
     * Recording and API bounds still use the complete native mask in both modes. */
    const char *all_timestamps = getenv("OGPU_FOUNDATION_MULTIVIEW_ALL_TIMESTAMPS");
    uint32_t timestamp_array = all_timestamps && strcmp(all_timestamps,"1") == 0;
    FILE *file = NULL; void *code = NULL, *bs = NULL, *ss = NULL, *es = NULL;
    ogpu_next_argument_layout *arguments = NULL;
    ogpu_next_executable *pipelines[3] = {NULL};
    ogpu_next_memory *memory = NULL, *backing = NULL;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL, *short_view = NULL;
    ogpu_next_arena *arena = NULL, *children = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_query_pool *occlusion = NULL, *timestamps = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    if (!(fixture_mask(features.enabled) & FIXTURE_BIT(OGPU_NEXT_FEATURE_MULTIVIEW))) {
        puts("Multiview NOT exercised: feature unavailable."); return EXIT_SUCCESS;
    }
    ogpu_next_graphics_limits limits = {0}; q = query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&limits,1,sizeof(limits));
    TRY(ogpu_next_device_query(device,&q)); REQUIRE(limits.max_multiview_views >= 4);
    file = fopen(vertex_path,"rb"); REQUIRE(file);
    REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
    REQUIRE(fseek(file,0,SEEK_SET) == 0); code = malloc((size_t)size); REQUIRE(code);
    REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_argument_field field = {0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc layout = {OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};
    TRY(ogpu_next_argument_layout_create(device,&layout,&arguments));
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},"vertexMain",NULL,NULL};
    ogpu_next_graphics_state gs = {.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        .topology=OGPU_NEXT_TRIANGLES,.front_face=OGPU_NEXT_FRONT_CCW,.samples=1,
        .depth_format=OGPU_NEXT_D32_FLOAT,.depth_test=1,.depth_write=1,.depth_compare=OGPU_NEXT_COMPARE_ALWAYS};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),
        FIXTURE_FEATURES(FIXTURE_BIT(OGPU_NEXT_FEATURE_RASTER) | FIXTURE_BIT(OGPU_NEXT_FEATURE_MULTIVIEW)),{0,0,0},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),
        OGPU_NEXT_EXECUTABLE_GRAPHICS,1,&shader,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
    const uint32_t masks[3] = {5,10,8};
    for (uint32_t i = 0; i < 3; ++i) { gs.view_mask = masks[i]; TRY(ogpu_next_executable_create(device,&ed,arguments,&pipelines[i])); }
    if (limits.max_multiview_views < 32) {
        ogpu_next_executable *rejected = NULL; gs.view_mask = 1u << limits.max_multiview_views;
        REQUIRE(ogpu_next_executable_create(device,&ed,arguments,&rejected) == OGPU_NEXT_UNSUPPORTED && !rejected);
    }
    free(code); code = NULL;
    TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0}; uint64_t address = 0;
    TRY(ogpu_next_memory_map(all,&map)); TRY(ogpu_next_memory_address(all,&address)); REQUIRE(md.size >= 2048);
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_DEPTH,0,1,0,6};
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_D32_FLOAT,
        OGPU_NEXT_IMAGE_2D,1,6,1,{8,8,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST | OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT,0,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements ir = {.compatible_types=types,.compatible_type_capacity=32};
    TRY(ogpu_next_image_requirements(device,&id,&ir)); REQUIRE(ir.compatible_type_count);
    TRY(ogpu_next_image_create_unbound(device,&id,&image)); TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,ir.size}));
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_D32_FLOAT,
        OGPU_NEXT_VIEW_2D_ARRAY,OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT,{0,0,0,0},{OGPU_NEXT_ASPECT_DEPTH,0,1,1,4}};
    TRY(ogpu_next_view_create(image,&vd,&view)); vd.range.layer_count = 3; TRY(ogpu_next_view_create(image,&vd,&short_view));
    ogpu_next_host_requirements br = {0}, sr = {0}, er = {0};
    TRY(ogpu_next_barrier_scratch_requirements(device,1,0,1,&br));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    TRY(ogpu_next_execute_scratch_requirements(1,&er));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t) && er.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); ss = malloc((size_t)sr.size); es = malloc((size_t)er.size); REQUIRE(bs && ss && es);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); ad.level = OGPU_NEXT_SECONDARY; TRY(ogpu_next_arena_create(device,&ad,&children));
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_query_pool_desc qd = {HEADER(ogpu_next_query_pool_desc,OGPU_NEXT_QUERY_POOL_DESC),OGPU_NEXT_QUERY_OCCLUSION,2,0};
    TRY(ogpu_next_query_pool_create(device,&qd,&occlusion));
    qd.type = OGPU_NEXT_QUERY_TIMESTAMP; TRY(ogpu_next_query_pool_create(device,&qd,&timestamps));
    uint32_t precise = fixture_mask(features.enabled) & FIXTURE_BIT(OGPU_NEXT_FEATURE_PRECISE_OCCLUSION) ? OGPU_NEXT_QUERY_PRECISE : 0;
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={1,0}}};
    /* layers is ignored when a nonzero view mask selects layers. */
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,8,8,0,0,1,0,0,NULL,&attachment,NULL,{NULL,0},NULL};
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,4,8};
    ogpu_next_draw_desc draw = {3,1,0,0,0};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL, *child = NULL;
    uint64_t serial = 0;
    for (uint32_t mode = 0; mode < 6; ++mode) {
        uint32_t selected = mode % 3, secondary = mode >= 3, n = selected == 2 ? 1 : 2;
        uint32_t tn = timestamp_array ? n : 1;
        TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_arena_reset(children));
        render.view_mask = masks[selected]; render.flags = secondary ? OGPU_NEXT_RENDER_SECONDARY : 0;
        TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        ogpu_next_queries_reset(encoder,occlusion,0,2);
        ogpu_next_queries_reset(encoder,timestamps,0,2);
        ogpu_next_image_barrier barrier = {{0,0,0,OGPU_NEXT_ACCESS_COPY_WRITE},image,range,
            OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
        ogpu_next_barrier_scope scope = {0,OGPU_NEXT_STAGE_CLEAR,0,0};
        ogpu_next_dependency dep = {1,0,1,0,&scope,NULL,&barrier}; ogpu_next_host_span scratch = {bs,br.size};
        common_barrier(encoder,&dep,&scope,scratch);
        ogpu_next_clear_value clear = {.depth_stencil={0.875f,0}}; ogpu_next_clear_image(encoder,image,OGPU_NEXT_STATE_COPY_DST,&range,&clear);
        barrier.old_state = barrier.new_state; barrier.new_state = OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT; barrier.discard = 0;
        barrier.scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE; barrier.scope.access_after = OGPU_NEXT_ACCESS_DEPTH_READ | OGPU_NEXT_ACCESS_DEPTH_WRITE;
        scope.stages_before = OGPU_NEXT_STAGE_CLEAR; scope.stages_after = OGPU_NEXT_STAGE_DEPTH; common_barrier(encoder,&dep,&scope,scratch);
        if (!secondary) { /* Native dynamic-rendering begin preserves these bindings. */
            ogpu_next_bind_executable(encoder,pipelines[selected]); ogpu_next_set_graphics_state(encoder,&vp.header);
            ogpu_next_set_arguments(encoder,arguments,0,8,&address);
        }
        ogpu_next_render_begin(encoder,&render); ogpu_next_encoder *parent = encoder;
        ogpu_next_render_inheritance ri = {HEADER(ogpu_next_render_inheritance,OGPU_NEXT_RENDER_INHERITANCE),0,1,render.view_mask,0,OGPU_NEXT_D32_FLOAT,0,NULL,{NULL,0}};
        ogpu_next_inheritance inherit = {HEADER(ogpu_next_inheritance,OGPU_NEXT_INHERITANCE),0,0,0,&ri.header,NULL,NULL};
        ogpu_next_recording_desc cr = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_SECONDARY,&inherit.header};
        if (secondary) {
            TRY(ogpu_next_commands_begin(children,&cr,&encoder));
            ogpu_next_bind_executable(encoder,pipelines[selected]); ogpu_next_set_graphics_state(encoder,&vp.header);
            ogpu_next_set_arguments(encoder,arguments,0,8,&address);
        }
        ogpu_next_query_begin(encoder,occlusion,0,precise);
        if (selected == 0) ogpu_next_draw(encoder,&draw);
        else if (selected == 1) {
            ogpu_next_bind_indices(encoder,(ogpu_next_span){memory,32,12},OGPU_NEXT_INDEX_U32);
            ogpu_next_draw_indexed(encoder,&draw);
        } else {
            ogpu_next_indirect indirect = {{memory,64,16},16,1,{NULL,0,0}};
            ogpu_next_draw_indirect(encoder,&indirect);
        }
        ogpu_next_query_end(encoder,occlusion,0);
        ogpu_next_timestamp(encoder,timestamps,0,OGPU_NEXT_STAGE_VERTEX);
        if (secondary) { TRY(ogpu_next_commands_end(encoder,&child)); encoder = parent; ogpu_next_execute_lists(encoder,1,&child,(ogpu_next_host_span){es,er.size}); }
        ogpu_next_render_end(encoder);
        barrier.old_state = barrier.new_state; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC;
        barrier.scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE | OGPU_NEXT_ACCESS_DEPTH_WRITE; barrier.scope.access_after = OGPU_NEXT_ACCESS_COPY_READ;
        scope.stages_before = OGPU_NEXT_STAGE_CLEAR | OGPU_NEXT_STAGE_DEPTH; scope.stages_after = OGPU_NEXT_STAGE_COPY; common_barrier(encoder,&dep,&scope,scratch);
        ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_DEPTH,0,0,6,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
        ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,512,1536},image,&copy);
        ogpu_next_queries_resolve(encoder,occlusion,0,n,(ogpu_next_span){memory,128,n*8},8,OGPU_NEXT_QUERY_RESULT_64 | OGPU_NEXT_QUERY_RESULT_WAIT);
        ogpu_next_queries_resolve(encoder,timestamps,0,tn,(ogpu_next_span){memory,160,tn*16},16,
            OGPU_NEXT_QUERY_RESULT_64 | OGPU_NEXT_QUERY_RESULT_WAIT | OGPU_NEXT_QUERY_RESULT_AVAILABILITY);
        dep.image_count = 0; dep.images = NULL; scope = (ogpu_next_barrier_scope){OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};
        common_barrier(encoder,&dep,&scope,scratch);
        TRY(ogpu_next_commands_end(encoder,&list));
        for (uint32_t replay = 0; replay < 2; ++replay) {
            memset(map.data,0xa5,(size_t)md.size); float depth = 0.125f+replay*0.125f; memcpy(map.data,&depth,4);
            const uint32_t indices[3] = {0,1,2}, indirect[4] = {3,1,0,0};
            memcpy((char *)map.data+32,indices,12); memcpy((char *)map.data+64,indirect,16);
            TRY(ogpu_next_memory_flush(all)); ogpu_next_sync_point signal = {{done,++serial},OGPU_NEXT_STAGE_ALL};
            ogpu_next_submit_desc submit = {1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL}; ogpu_next_submit_outcome outcome = {0}; pending = 1;
            TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){ss,sr.size}));
            TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
            for (uint32_t layer = 0; layer < 6; ++layer) for (uint32_t y = 0; y < 8; ++y) for (uint32_t x = 0; x < 8; ++x) {
                float expected = 0.875f;
                if (layer >= 1 && layer <= 4 && (render.view_mask & (1u << (layer-1)))) expected = x < 4 ? depth+(layer-1)*0.125f : 1;
                REQUIRE(((float *)((char *)map.data+512))[layer*64+y*8+x] == expected);
            }
            const uint64_t *counts = (const uint64_t *)((const char *)map.data+128); uint64_t total = counts[0]+(n == 2 ? counts[1] : 0);
            const uint64_t *times = (const uint64_t *)((const char *)map.data+160);
            for (uint32_t i = 0; i < tn; ++i) REQUIRE(times[i*2+1] != 0);
            REQUIRE(total > 0); if (precise) REQUIRE(total == n*32);
            REQUIRE(memcmp(map.data,&depth,4) == 0 && memcmp((char *)map.data+32,indices,12) == 0 && memcmp((char *)map.data+64,indirect,16) == 0);
            for (uint64_t i = 4; i < md.size; ++i) if (!((i>=32 && i<44) || (i>=64 && i<80) || (i>=128 && i<128+n*8) || (i>=160 && i<160+tn*16) || (i>=512 && i<2048))) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
        }
    }
    /* Rejections must occur before issuing an invalid native command. */
    for (uint32_t bad = 0; bad < 7; ++bad) {
        TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        render.flags = 0; render.view_mask = 10; attachment.view = bad == 0 ? short_view : view;
        if (bad == 5) ogpu_next_query_begin(encoder,occlusion,0,precise);
        ogpu_next_render_begin(encoder,&render);
        ogpu_next_bind_executable(encoder,pipelines[bad == 1 ? 0 : 1]); ogpu_next_set_graphics_state(encoder,&vp.header);
        ogpu_next_set_arguments(encoder,arguments,0,8,&address);
        if (bad <= 1) ogpu_next_draw(encoder,&draw);
        if (bad == 2) ogpu_next_query_begin(encoder,occlusion,1,precise); /* popcount two */
        if (bad == 3) {
            ogpu_next_draw_desc invalid = {3,2,0,limits.max_multiview_instance_index,0}; ogpu_next_draw(encoder,&invalid);
        }
        if (bad == 4) { /* Last recorded child has mask 8; parent has mask 10. */
            ogpu_next_render_end(encoder); render.flags = OGPU_NEXT_RENDER_SECONDARY; ogpu_next_render_begin(encoder,&render);
            ogpu_next_execute_lists(encoder,1,&child,(ogpu_next_host_span){es,er.size});
        }
        if (bad == 6) ogpu_next_timestamp(encoder,timestamps,1,OGPU_NEXT_STAGE_VERTEX);
        if (bad == 2) ogpu_next_query_end(encoder,occlusion,1);
        ogpu_next_render_end(encoder);
        if (bad == 5) ogpu_next_query_end(encoder,occlusion,0);
        list = NULL; REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && !list);
    }
    puts("Multiview: 12 submissions; sparse/offset layers, direct/indexed/indirect, secondary inheritance, changed-root replay, query totals, timestamp slots and guards PASS.");
    printf("Multiview timestamp readback: %s.\n",timestamp_array ? "all active-view slots" : "first slot only (VVL tracking limitation)");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fputs("Pending multiview work after failure.\n",stderr); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_arena_destroy(children);
    for (uint32_t i = 0; i < 3; ++i) ogpu_next_executable_destroy(pipelines[i]);
    ogpu_next_argument_layout_destroy(arguments); ogpu_next_query_pool_destroy(occlusion); ogpu_next_query_pool_destroy(timestamps); ogpu_next_timeline_destroy(done);
    ogpu_next_view_destroy(short_view); ogpu_next_view_destroy(view); ogpu_next_image_destroy(image);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(bs); free(ss); free(es); return result;
}
