/* Block-copy matrix plus native BC1 sampling. Caller owns allocation, encoded
 * payloads and synchronization. Unsupported formats are reported, not emulated. */
static int compressed_formats(ogpu_next_device *device, uint32_t domain, const char *path) {
    ogpu_next_argument_layout *arguments = NULL;
    ogpu_next_descriptor_heap *bindings[2]={NULL,NULL};
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_format_info *formats = NULL;
    ogpu_next_image *images[2] = {NULL}; ogpu_next_view *views[2] = {NULL};
    ogpu_next_memory *backing[2] = {NULL}, *host = NULL, *heaps[2] = {NULL};
    ogpu_next_executable *pipeline = NULL; ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    void *barrier_scratch = NULL, *submit_scratch = NULL, *descriptor_scratch = NULL, *code = NULL;
    FILE *file = NULL;
    uint32_t executed = 0, excluded = 0, sampled = 0, rejected = 0;
    uint64_t value = 0;
    unsigned char expected[16384];
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FORMATS,NULL,0,sizeof(*formats));
    TRY(ogpu_next_device_query(device,&q)); formats = calloc(q.count,sizeof(*formats)); REQUIRE(formats);
    q.data = formats; q.capacity = q.count; TRY(ogpu_next_device_query(device,&q));
    REQUIRE(heap_memory(device,sizeof(expected),16,OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST | OGPU_NEXT_USAGE_STORAGE,
        OGPU_NEXT_MEMORY_HOST_VISIBLE,&host) == EXIT_SUCCESS);
    ogpu_next_span all = {host,0,sizeof(expected)}; ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map)); uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,0,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements br = {0}, sr = {0}, wr[2] = {{0}};
    TRY(ogpu_next_barrier_scratch_requirements(device,1,0,2,&br)); TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.wait_count=0,.signal_count=1},&sr));
    barrier_scratch = malloc(br.size); submit_scratch = malloc(sr.size); REQUIRE(barrier_scratch && submit_scratch);
    ogpu_next_sampler_desc sampler = {HEADER(ogpu_next_sampler_desc,OGPU_NEXT_SAMPLER_DESC),0,0,0,{2,2,2},0,0,0,0,0,2,0,1};
    ogpu_next_descriptor_shape sampler_shape; TRY(ogpu_next_sampler_descriptor_shape(device,&sampler,&sampler_shape));
    ogpu_next_heap_requirements heap_req[2];
    for(uint32_t i=0;i<2;++i) {
        TRY(heap_requirements_for(device,i+1,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,&heap_req[i]));
        TRY(ogpu_next_descriptor_scratch_requirements(device,i,1,&wr[i]));
    }
    uint64_t payloads[2]={0,sampler_shape.size}; ogpu_next_mapping maps[2]={{0}};
    uint64_t reserved=aligned(payloads[1],heap_req[1].reserved_alignment), heap_size=reserved+heap_req[1].reserved_size;
    REQUIRE(heap_memory(device,heap_size,heap_req[1].backing_alignment,OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,OGPU_NEXT_MEMORY_HOST_VISIBLE,&heaps[1])==EXIT_SUCCESS);
    TRY(ogpu_next_memory_map((ogpu_next_span){heaps[1],0,heap_size},&maps[1]));
    ogpu_next_heap_desc hd={{2,OGPU_NEXT_HEAP_MEMORY,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,0,heap_size},
        {heaps[1],0,heap_size},{maps[1].data,maps[1].size},reserved,heap_req[1].reserved_size};
    TRY(ogpu_next_heap_create(device,&hd,&bindings[1]));
    uint64_t scratch_size = wr[0].size > wr[1].size ? wr[0].size : wr[1].size;
    descriptor_scratch = malloc(scratch_size); REQUIRE(descriptor_scratch);
    ogpu_next_host_span scratch = {descriptor_scratch,scratch_size};

    ogpu_next_heap_location sampler_bytes = {bindings[1],0};
    TRY(ogpu_next_write_sampler_descriptors(device,1,&sampler,&sampler_bytes,scratch));
    TRY(ogpu_next_memory_flush((ogpu_next_span){heaps[1],0,payloads[1]}));
    file = fopen(path,"rb"); REQUIRE(file); REQUIRE(fseek(file,0,SEEK_END) == 0);
    long code_size = ftell(file); REQUIRE(code_size >= 20 && code_size % 4 == 0); REQUIRE(fseek(file,0,SEEK_SET) == 0);
    code = malloc((size_t)code_size); REQUIRE(code); REQUIRE(fread(code,1,(size_t)code_size,file) == (size_t)code_size);
    fclose(file); file = NULL;
    ogpu_next_argument_field argument_field = {0,32,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc argument_desc = {OGPU_NEXT_ARGUMENT_COMPUTE,32,1,0,&argument_field};
    TRY(ogpu_next_argument_layout_create(device,&argument_desc,&arguments));
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)code_size},"compressedMain",NULL,NULL};
    ogpu_next_shader_requirements requirements = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),FIXTURE_FEATURES(0),{64,1,1},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,1,&shader,NULL,0,&requirements.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device, &ed, arguments, &pipeline)); free(code); code = NULL;
    for (uint32_t i = 0; i < q.count; ++i) {
        const ogpu_next_format_info *f = &formats[i];
        if (f->copy_block_extent[0] == 1) continue;
        uint32_t bx = f->copy_block_extent[0], by = f->copy_block_extent[1], bytes = f->copy_block_bytes[0];
        REQUIRE(f->aspects == 1 && f->copy_block_extent[2] == 1 && (bytes == 8 || bytes == 16));
        REQUIRE(f->copy_block_bytes[1] == 0 && f->copy_block_bytes[2] == 0);
        ogpu_next_image_profile profile = {HEADER(ogpu_next_image_profile,OGPU_NEXT_IMAGE_PROFILE),f->format,OGPU_NEXT_IMAGE_2D,
            OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST | OGPU_NEXT_IMAGE_SAMPLED,OGPU_NEXT_IMAGE_MUTABLE_FORMAT,0};
        ogpu_next_image_limits image_limits;
        ogpu_next_status support = ogpu_next_image_limits_query(device,&profile,&image_limits);
        if (support == OGPU_NEXT_UNSUPPORTED) { printf("COMPRESSED_EXCLUDED id=%u\n",f->format); ++excluded; continue; }
        TRY(support);
        /* Adjacent catalog pairs share a native view class, not just byte size. */
        uint32_t pair = f->format + ((f->format - OGPU_NEXT_BC1_RGB_UNORM_BLOCK) % 2 ? -1 : 1);
        uint32_t view_formats[] = {f->format,pair};
        ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),f->format,OGPU_NEXT_IMAGE_2D,
            3,2,1,{2*bx-1,2*by-1,1},profile.usage,profile.image_flags,view_formats,2,0,NULL};
        for (uint32_t n = 0; n < 2; ++n) {
            uint32_t types[32]; ogpu_next_requirements req = {0}; req.compatible_types = types; req.compatible_type_capacity = 32;
            TRY(ogpu_next_image_requirements(device,&desc,&req)); REQUIRE(req.compatible_type_count);
            TRY(ogpu_next_image_create_unbound(device,&desc,&images[n]));
            TRY(ogpu_next_memory_create_dedicated_image(images[n],types[0],&backing[n]));
            TRY(ogpu_next_image_bind(images[n],(ogpu_next_span){backing[n],0,req.size}));
        }
        for (uint32_t n = 0; n < 2; ++n) {
            ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),view_formats[n],OGPU_NEXT_VIEW_2D_ARRAY,
                OGPU_NEXT_IMAGE_SAMPLED,{0},{OGPU_NEXT_ASPECT_COLOR,0,3,0,2}};
            TRY(ogpu_next_view_create(images[1],&vd,&views[n]));
        }
        int sample = f->format == OGPU_NEXT_BC1_RGBA_UNORM_BLOCK || f->format == OGPU_NEXT_BC1_RGBA_SRGB_BLOCK;
        for (uint32_t mip = 0; mip < 3; ++mip) {
            uint32_t width = desc.extent.x >> mip, height = desc.extent.y >> mip;
            if (!width) width = 1;
            if (!height) height = 1;
            uint32_t nx = (width+bx-1)/bx, ny = (height+by-1)/by;
            uint64_t row = (nx+1)*bytes, slice = (ny+1)*row, length = slice+(ny-1)*row+nx*bytes;
            const uint64_t src = 64, dst = 4096, out = 8192;
            if (sample) {
                ogpu_next_view_destroy(views[0]); views[0] = NULL;
                ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),f->format,OGPU_NEXT_VIEW_2D_ARRAY,
                    OGPU_NEXT_IMAGE_SAMPLED,{0},{1,mip,1,0,2}};
                TRY(ogpu_next_view_create(images[1],&vd,&views[0]));
                ogpu_next_resource_descriptor resource = {OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE,OGPU_NEXT_STATE_SHADER_READ,views[0],{NULL,0,0}};
                ogpu_next_descriptor_shape shape; TRY(ogpu_next_resource_descriptor_shape(device,&resource,&shape));
                if(!bindings[0]) {
                    payloads[0]=shape.size;
                    uint64_t offset=aligned(shape.size,heap_req[0].reserved_alignment), size=offset+heap_req[0].reserved_size;
                    REQUIRE(heap_memory(device,size,heap_req[0].backing_alignment,OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,OGPU_NEXT_MEMORY_HOST_VISIBLE,&heaps[0])==EXIT_SUCCESS);
                    TRY(ogpu_next_memory_map((ogpu_next_span){heaps[0],0,size},&maps[0]));
                    hd=(ogpu_next_heap_desc){{1,OGPU_NEXT_HEAP_MEMORY,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,0,size},
                        {heaps[0],0,size},{maps[0].data,maps[0].size},offset,heap_req[0].reserved_size};
                    TRY(ogpu_next_heap_create(device,&hd,&bindings[0]));
                }
                REQUIRE(shape.size<=payloads[0]);
                ogpu_next_heap_location destination = {bindings[0],0};
                TRY(ogpu_next_write_resource_descriptors(device,1,&resource,&destination,scratch));
                TRY(ogpu_next_memory_flush((ogpu_next_span){heaps[0],0,payloads[0]}));
            }
            memset(expected,0xa5,sizeof(expected));
            for (uint32_t layer = 0; layer < 2; ++layer) for (uint32_t y = 0; y < ny; ++y) for (uint32_t x = 0; x < nx; ++x) {
                unsigned char *p = expected+src+layer*slice+y*row+x*bytes;
                for (uint32_t b = 0; b < bytes; ++b) p[b] = (unsigned char)(i*11+mip*7+layer*29+y*3+x+b);
                if (sample) {
                    /* Valid BC1 endpoint-0 blocks: red / green, swapped by mip. */
                    memset(p,0,8); uint16_t color = (layer ^ (mip & 1)) ? 0x07e0 : 0xf800;
                    p[0] = (unsigned char)color; p[1] = (unsigned char)(color >> 8);
                }
            }
            memcpy(map.data,expected,sizeof(expected)); TRY(ogpu_next_memory_flush(all));
            ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,0,NULL};
            ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL;
            TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
            ogpu_next_image_barrier ib[2];
            for (uint32_t n = 0; n < 2; ++n) ib[n] = (ogpu_next_image_barrier){{0,0,0,OGPU_NEXT_ACCESS_COPY_WRITE},images[n],{1,mip,1,0,2},OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
            ogpu_next_barrier_scope dep_scope = {OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COPY,0,0};
    ogpu_next_host_span dep_scratch = {barrier_scratch,br.size};
    ogpu_next_dependency dep = {1,0,2,0,&dep_scope,NULL,ib}; common_barrier(encoder,&dep,&dep_scope,dep_scratch);
            ogpu_next_image_copy copy = {{1,mip,0,2,{0,0,0},{width,height,1}},row,slice,OGPU_NEXT_STATE_COPY_DST,0};
            ogpu_next_copy_to_image(encoder,images[0],(ogpu_next_span){host,src,length},&copy);
            ib[0].scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE; ib[0].scope.access_after = OGPU_NEXT_ACCESS_COPY_READ;
            ib[0].old_state = OGPU_NEXT_STATE_COPY_DST; ib[0].new_state = OGPU_NEXT_STATE_COPY_SRC; ib[0].discard = 0;
            dep.image_count = 1; common_barrier(encoder,&dep,&dep_scope,dep_scratch);
            ogpu_next_image_transfer transfer = {copy.region,copy.region,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_STATE_COPY_DST};
            if (width > bx) {
                transfer.source.extent.x = transfer.destination.extent.x = bx;
                ogpu_next_copy_image(encoder,images[1],images[0],&transfer);
                /* Deliberately serialize these fixture writes. This verifies
                 * edge-region lowering, not independent compressed-block writes. */
                ogpu_next_barrier_scope writes_scope = {OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_WRITE};
    max_align_t writes_bytes[16];
    ogpu_next_host_span writes_scratch = {writes_bytes,sizeof(writes_bytes)};
    ogpu_next_dependency writes = {1,0,0,0,&writes_scope,NULL,NULL};
                common_barrier(encoder,&writes,&writes_scope,writes_scratch);
                transfer.source.offset.x = transfer.destination.offset.x = (int32_t)bx;
                transfer.source.extent.x = transfer.destination.extent.x = width-bx;
            }
            ogpu_next_copy_image(encoder,images[1],images[0],&transfer);
            ib[1].scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE; ib[1].scope.access_after = OGPU_NEXT_ACCESS_COPY_READ;
            ib[1].old_state = OGPU_NEXT_STATE_COPY_DST; ib[1].new_state = OGPU_NEXT_STATE_COPY_SRC; ib[1].discard = 0;
            dep.images = &ib[1]; common_barrier(encoder,&dep,&dep_scope,dep_scratch);
            copy.state = OGPU_NEXT_STATE_COPY_SRC;
            ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,dst,length},images[1],&copy);
            if (sample) {
                dep_scope.stages_after = OGPU_NEXT_STAGE_COMPUTE;
                ib[1].scope.access_before = OGPU_NEXT_ACCESS_COPY_READ; ib[1].scope.access_after = OGPU_NEXT_ACCESS_SAMPLED_READ;
                ib[1].old_state = OGPU_NEXT_STATE_COPY_SRC; ib[1].new_state = OGPU_NEXT_STATE_SHADER_READ;
                common_barrier(encoder,&dep,&dep_scope,dep_scratch);
                for (uint32_t n = 0; n < 2; ++n) ogpu_next_bind_heap(encoder,bindings[n]);
                ogpu_next_bind_executable(encoder,pipeline);
                uint32_t indices[] = {0,0}, dimensions[] = {width,height,0,0};
                ogpu_next_set_arguments(encoder, arguments, 0, sizeof(indices), indices);
                ogpu_next_set_arguments(encoder, arguments, 8, 8, &(uint64_t){address+out});
                ogpu_next_set_arguments(encoder, arguments, 16, sizeof(dimensions), dimensions);
                ogpu_next_launch launch = {{(width*height*2+63)/64,1,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
            }
            dep_scope.stages_before = OGPU_NEXT_STAGE_COPY | (sample ? OGPU_NEXT_STAGE_COMPUTE : 0); dep_scope.stages_after = OGPU_NEXT_STAGE_HOST;
            dep_scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE | (sample ? OGPU_NEXT_ACCESS_STORAGE_WRITE : 0);
            dep_scope.access_after = OGPU_NEXT_ACCESS_HOST_READ; dep.image_count = 0; dep.images = NULL; common_barrier(encoder,&dep,&dep_scope,dep_scratch);
            TRY(ogpu_next_commands_end(encoder,&list));
            ogpu_next_sync_point signal = {{done,++value},OGPU_NEXT_STAGE_ALL};
            ogpu_next_submit_desc submit = {1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};
    ogpu_next_host_span submit_host = {submit_scratch,sr.size};
    ogpu_next_submit_outcome submit_outcome = {0};
            pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&submit_outcome,submit_host));
            TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
            for (uint32_t layer = 0; layer < 2; ++layer) for (uint32_t y = 0; y < ny; ++y)
                memcpy(expected+dst+layer*slice+y*row,expected+src+layer*slice+y*row,nx*bytes);
            if (sample) for (uint32_t layer = 0; layer < 2; ++layer) for (uint32_t p = 0; p < width*height; ++p) {
                uint32_t color = (layer ^ (mip & 1)) ? UINT32_C(0xff00ff00) : UINT32_C(0xff0000ff);
                memcpy(expected+out+4*(layer*width*height+p),&color,4);
            }
            if (memcmp(map.data,expected,sizeof(expected)) != 0) {
                for (size_t p = 0; p < sizeof(expected); ++p) if (((unsigned char *)map.data)[p] != expected[p]) {
                    fprintf(stderr,"Compressed mismatch format=%u mip=%u offset=%zu got=%u expected=%u\n",f->format,mip,p,((unsigned char *)map.data)[p],expected[p]); break;
                }
                REQUIRE(0);
            }
            TRY(ogpu_next_arena_reset(arena));
        }
        /* Invalid inputs poison recording before any invalid native command. */
        if (sample) for (uint32_t bad = 0; bad < 6; ++bad) {
            ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,0,NULL};
            ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
            ogpu_next_image_copy copy = {{1,0,0,2,{0,0,0},{7,7,1}},0,0,OGPU_NEXT_STATE_COPY_DST,0};
            ogpu_next_span input = {host,64,64};
            if (bad == 0) copy.region.offset.x = 1;
            if (bad == 1) copy.region.extent.x = 3;
            if (bad == 2) copy.row_pitch = 17;
            if (bad == 3) input.size = 63;
            if (bad == 4) input.offset++;
            if (bad == 5) {
                ogpu_next_subresources range = {1,0,1,0,2}; ogpu_next_clear_value clear = {0};
                ogpu_next_clear_image(encoder,images[0],OGPU_NEXT_STATE_COPY_DST,&range,&clear);
            } else ogpu_next_copy_to_image(encoder,images[0],input,&copy);
            REQUIRE(ogpu_next_commands_end(encoder,&list) == (bad == 5 ? OGPU_NEXT_UNSUPPORTED : OGPU_NEXT_INVALID));
            REQUIRE(list == NULL); TRY(ogpu_next_arena_reset(arena)); ++rejected;
        }
        executed++; if (sample) sampled++;
        for (uint32_t n = 0; n < 2; ++n) { ogpu_next_view_destroy(views[n]); views[n] = NULL; }
        for (uint32_t n = 0; n < 2; ++n) {
            ogpu_next_image_destroy(images[n]); images[n] = NULL; ogpu_next_memory_destroy(backing[n]); backing[n] = NULL;
        }
    }
    REQUIRE(executed > 0 && sampled == 2); /* Explicit local BC1 prerequisites. */
    printf("COMPRESSED_PASS formats=%u excluded=%u sampled=%u rejected=%u mips=3 layers=2 padded_copies=1 image_copies=1 guards=1\n",executed,excluded,sampled,rejected);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending compressed work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_executable_destroy(pipeline);
    for (uint32_t n = 0; n < 2; ++n) {
        ogpu_next_view_destroy(views[n]); ogpu_next_image_destroy(images[n]);
        ogpu_next_memory_destroy(backing[n]); ogpu_next_heap_destroy(bindings[n]); ogpu_next_memory_destroy(heaps[n]);
    }
    ogpu_next_memory_destroy(host); if (file) fclose(file);
    free(formats); free(code); free(barrier_scratch); free(submit_scratch); free(descriptor_scratch);
        ogpu_next_argument_layout_destroy(arguments);
return result;
}
