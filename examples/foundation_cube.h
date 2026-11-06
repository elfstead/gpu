/* Native cube arrays: descriptor changes select caller-owned layer/mip windows.
 * Face interiors are uniform, so this is not a seam/filter precision test. */
static uint32_t cube_pixel(uint32_t layer, uint32_t mip, uint32_t replay) {
    return UINT32_C(0xff000000) | (17+layer*7+replay*3) | ((31+mip*47+replay*5)<<8) | ((layer+mip*19+replay*11)<<16);
}
static int cube_execution(ogpu_next_device *device, ogpu_next_memory_desc host_desc, uint32_t domain, const char *path) {
    ogpu_next_argument_layout *arguments = NULL;
    ogpu_next_descriptor_heap *bindings[2]={NULL,NULL};
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code = NULL, *scratch = NULL, *barrier_scratch = NULL, *submit_scratch = NULL;
    ogpu_next_memory *backing = NULL, *data = NULL, *heaps[2] = {NULL};
    ogpu_next_image *image = NULL; ogpu_next_view *views[3] = {NULL};
    ogpu_next_executable *pipeline = NULL; ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    if (!(fixture_mask(features.enabled) & FIXTURE_BIT(OGPU_NEXT_FEATURE_CUBE_ARRAY))) { printf("Cube-array execution NOT exercised: feature unavailable.\n"); return EXIT_SUCCESS; }
    REQUIRE(host_desc.size >= 3168);
    ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,
        3,18,1,{4,4,1},OGPU_NEXT_IMAGE_COPY_DST | OGPU_NEXT_IMAGE_SAMPLED,OGPU_NEXT_IMAGE_CUBE_COMPATIBLE,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements req = {0}; req.compatible_types = types; req.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&desc,&req)); REQUIRE(req.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device,&desc,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,req.size}));
    const uint32_t first_layers[] = {1,6,1}, first_mips[] = {0,1,1};
    for (uint32_t i = 0; i < 3; ++i) {
        ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,
            OGPU_NEXT_VIEW_CUBE_ARRAY,OGPU_NEXT_IMAGE_SAMPLED,{0,0,0,0},{OGPU_NEXT_ASPECT_COLOR,first_mips[i],2,first_layers[i],12}};
        TRY(ogpu_next_view_create(image,&vd,&views[i]));
    }
    /* Rejected shapes clear output; single-cube arrays remain valid. */
    const uint32_t counts[] = {0,1,5,7,11,13,18,UINT32_MAX,6};
    for (uint32_t i = 0; i < sizeof(counts)/sizeof(counts[0]); ++i) {
        ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,
            OGPU_NEXT_VIEW_CUBE_ARRAY,OGPU_NEXT_IMAGE_SAMPLED,{0,0,0,0},{OGPU_NEXT_ASPECT_COLOR,0,1,1,counts[i]}};
        ogpu_next_view *out = (ogpu_next_view *)(uintptr_t)1;
        ogpu_next_status status = ogpu_next_view_create(image,&vd,&out);
        if (counts[i] == 6) { REQUIRE(status == OGPU_NEXT_OK && out != NULL); ogpu_next_view_destroy(out); }
        else { REQUIRE(status == OGPU_NEXT_INVALID && out == NULL); }
    }
    TRY(ogpu_next_memory_create(device,&host_desc,&data)); ogpu_next_span all = {data,0,host_desc.size}; ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map)); uint64_t output_address = 0;
    TRY(ogpu_next_memory_address((ogpu_next_span){data,3072,96},&output_address));
    ogpu_next_sampler_desc sampler = {HEADER(ogpu_next_sampler_desc,OGPU_NEXT_SAMPLER_DESC),0,0,0,{2,2,2},0,0,0,0,0,1,0,1};
    ogpu_next_resource_descriptor shape_resource={OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE,OGPU_NEXT_STATE_SHADER_READ,views[0],{0}};
    ogpu_next_descriptor_shape image_shape={0}, sampler_shape={0};
    ogpu_next_heap_requirements resource_req={0}, sampler_req={0};
    TRY(ogpu_next_resource_descriptor_shape(device,&shape_resource,&image_shape));
    TRY(ogpu_next_sampler_descriptor_shape(device,&sampler,&sampler_shape));
    TRY(heap_requirements_for(device,1,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,&resource_req));
    TRY(heap_requirements_for(device,2,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,&sampler_req));
    uint64_t payloads[] = {image_shape.size,sampler_shape.size};
    uint64_t reserved[] = {aligned(payloads[0],resource_req.reserved_alignment),aligned(payloads[1],sampler_req.reserved_alignment)};
    uint64_t reserved_sizes[] = {resource_req.reserved_size,sampler_req.reserved_size};
    uint64_t alignments[] = {resource_req.backing_alignment,sampler_req.backing_alignment};
    ogpu_next_mapping maps[2] = {{0}};
    ogpu_next_host_requirements wr[2] = {{0}}, br = {0}, sr = {0};
    for (uint32_t i = 0; i < 2; ++i) {
        uint64_t size = reserved[i]+reserved_sizes[i];
        REQUIRE(heap_memory(device,size,alignments[i],OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,OGPU_NEXT_MEMORY_HOST_VISIBLE,&heaps[i]) == EXIT_SUCCESS);
        TRY(ogpu_next_memory_map((ogpu_next_span){heaps[i],0,size},&maps[i]));
        ogpu_next_heap_desc hd={{i+1,OGPU_NEXT_HEAP_MEMORY,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,0,size},
            {heaps[i],0,size},{maps[i].data,maps[i].size},reserved[i],reserved_sizes[i]};
        TRY(ogpu_next_heap_create(device,&hd,&bindings[i]));
        TRY(ogpu_next_descriptor_scratch_requirements(device,i,1,&wr[i]));
        REQUIRE(wr[i].alignment <= _Alignof(max_align_t));
    }
    uint64_t scratch_size = wr[0].size > wr[1].size ? wr[0].size : wr[1].size;
    scratch = malloc((size_t)scratch_size); REQUIRE(scratch != NULL);
    ogpu_next_host_span temporary = {scratch,scratch_size};

    ogpu_next_heap_location sampler_bytes = {bindings[1],0};
    TRY(ogpu_next_write_sampler_descriptors(device,1,&sampler,&sampler_bytes,temporary));
    TRY(ogpu_next_memory_flush((ogpu_next_span){heaps[1],0,payloads[1]}));
    file = fopen(path,"rb"); REQUIRE(file != NULL); REQUIRE(fseek(file,0,SEEK_END) == 0);
    long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0); REQUIRE(fseek(file,0,SEEK_SET) == 0);
    code = malloc((size_t)size); REQUIRE(code != NULL); REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_argument_field argument_field = {0,16,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc argument_desc = {OGPU_NEXT_ARGUMENT_COMPUTE,16,1,0,&argument_field};
    TRY(ogpu_next_argument_layout_create(device,&argument_desc,&arguments));
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},"cubeMain",NULL,NULL};
    ogpu_next_shader_requirements requirements = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),FIXTURE_FEATURES(FIXTURE_BIT(OGPU_NEXT_FEATURE_CUBE_ARRAY)),{32,1,1},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,1,&shader,NULL,0,&requirements.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device, &ed, arguments, &pipeline)); free(code); code = NULL;
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    TRY(ogpu_next_barrier_scratch_requirements(device,1,0,1,&br)); TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.wait_count=0,.signal_count=1},&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    barrier_scratch = malloc((size_t)br.size); submit_scratch = malloc((size_t)sr.size); REQUIRE(barrier_scratch && submit_scratch);
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_image_barrier barrier = {{0,0,OGPU_NEXT_ACCESS_SAMPLED_READ,OGPU_NEXT_ACCESS_COPY_WRITE},image,{OGPU_NEXT_ASPECT_COLOR,0,3,0,18},OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0};
    ogpu_next_barrier_scope dep_scope = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COPY,0,0};
    ogpu_next_host_span dep_scratch = {barrier_scratch,br.size};
    ogpu_next_dependency dep = {1,0,1,0,&dep_scope,NULL,&barrier}; common_barrier(encoder,&dep,&dep_scope,dep_scratch);
    const uint64_t offsets[] = {128,1536,2048};
    for (uint32_t mip = 0; mip < 3; ++mip) {
        uint32_t width = 4 >> mip;
        ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,mip,0,18,{0,0,0},{width,width,1}},0,0,OGPU_NEXT_STATE_COPY_DST,0};
        ogpu_next_copy_to_image(encoder,image,(ogpu_next_span){data,offsets[mip],width*width*4*18},&copy);
    }
    dep_scope.stages_before = OGPU_NEXT_STAGE_COPY; dep_scope.stages_after = OGPU_NEXT_STAGE_COMPUTE;
    barrier.scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE; barrier.scope.access_after = OGPU_NEXT_ACCESS_SAMPLED_READ;
    barrier.old_state = OGPU_NEXT_STATE_COPY_DST; barrier.new_state = OGPU_NEXT_STATE_SHADER_READ; barrier.discard = 0;
    common_barrier(encoder,&dep,&dep_scope,dep_scratch);
    for (uint32_t i = 0; i < 2; ++i) ogpu_next_bind_heap(encoder,bindings[i]);
    ogpu_next_bind_executable(encoder,pipeline); uint32_t indices[] = {0,0};
    ogpu_next_set_arguments(encoder, arguments, 0, sizeof(indices), indices);
    ogpu_next_set_arguments(encoder, arguments, 8, 8, &(uint64_t){output_address});
    ogpu_next_launch launch = {{1,1,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
    ogpu_next_barrier_scope host_scope = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_HOST_READ};
    max_align_t host_bytes[16];
    ogpu_next_host_span host_scratch = {host_bytes,sizeof(host_bytes)};
    ogpu_next_dependency host = {1,0,0,0,&host_scope,NULL,NULL}; common_barrier(encoder,&host,&host_scope,host_scratch);
    TRY(ogpu_next_commands_end(encoder,&list));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};
    ogpu_next_host_span submit_host = {submit_scratch,sr.size};
    ogpu_next_submit_outcome submit_outcome = {0};
    for (uint32_t replay = 0; replay < 3; ++replay) {
        memset(map.data,0xa5,(size_t)host_desc.size);
        for (uint32_t mip = 0; mip < 3; ++mip) for (uint32_t layer = 0; layer < 18; ++layer)
            for (uint32_t p = 0; p < (16u >> (2*mip)); ++p)
                ((uint32_t *)((char *)map.data+offsets[mip]))[layer*(16u >> (2*mip))+p] = cube_pixel(layer,mip,replay);
        ogpu_next_resource_descriptor resource = {OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE,OGPU_NEXT_STATE_SHADER_READ,views[replay],{NULL,0,0}};
        ogpu_next_heap_location destination = {bindings[0],0};
        TRY(ogpu_next_write_resource_descriptors(device,1,&resource,&destination,temporary));
        TRY(ogpu_next_memory_flush((ogpu_next_span){heaps[0],0,payloads[0]})); TRY(ogpu_next_memory_flush(all));
        signal.point.value = replay+1; pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&submit_outcome,submit_host));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t i = 0; i < 24; ++i)
            REQUIRE(((uint32_t *)((char *)map.data+3072))[i] == cube_pixel(first_layers[replay]+i%12,first_mips[replay]+i/12,replay));
        for (uint32_t mip = 0; mip < 3; ++mip) for (uint32_t i = 0; i < (18u*16 >> (2*mip)); ++i)
            REQUIRE(((uint32_t *)((char *)map.data+offsets[mip]))[i] == cube_pixel(i/(16u >> (2*mip)),mip,replay));
        for (uint64_t i = 0; i < host_desc.size; ++i)
            if (!((i >= 128 && i < 1280) || (i >= 1536 && i < 1824) || (i >= 2048 && i < 2120) || (i >= 3072 && i < 3168)))
                REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Cube arrays pass: both cubes/all faces, offset layer/mip windows, descriptor/data changes across three replays and guards.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending cube-array work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_executable_destroy(pipeline);
    for (uint32_t i = 0; i < 2; ++i) { ogpu_next_heap_destroy(bindings[i]); ogpu_next_memory_destroy(heaps[i]); }
    for (uint32_t i = 0; i < 3; ++i) ogpu_next_view_destroy(views[i]);
    ogpu_next_image_destroy(image); ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(data);
    if (file) fclose(file);
    free(code); free(scratch); free(barrier_scratch); free(submit_scratch);     ogpu_next_argument_layout_destroy(arguments);
return result;
}
