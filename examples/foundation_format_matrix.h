/* Broad format coverage through public C: explicit native capability exclusion,
 * padded address/image copies, ordinary views, depth/stencil clears and guards. */
static int format_matrix(ogpu_next_device *device, ogpu_next_memory_desc host_desc, uint32_t domain) {
    static const struct { uint32_t format, aspects, bytes, depth_bytes; } cases[] = {
        {OGPU_NEXT_R8_UNORM, 1, 1, 0},
        {OGPU_NEXT_RG8_UNORM, 1, 2, 0},
        {OGPU_NEXT_RGBA8_UNORM, 1, 4, 0},
        {OGPU_NEXT_RGBA8_SRGB, 1, 4, 0},
        {OGPU_NEXT_BGRA8_UNORM, 1, 4, 0},
        {OGPU_NEXT_BGRA8_SRGB, 1, 4, 0},
        {OGPU_NEXT_R16_FLOAT, 1, 2, 0},
        {OGPU_NEXT_RG16_FLOAT, 1, 4, 0},
        {OGPU_NEXT_RGBA16_FLOAT, 1, 8, 0},
        {OGPU_NEXT_R32_FLOAT, 1, 4, 0},
        {OGPU_NEXT_R32_UINT, 1, 4, 0},
        {OGPU_NEXT_RGBA32_FLOAT, 1, 16, 0},
        {OGPU_NEXT_D16_UNORM, 2, 0, 2},
        {OGPU_NEXT_D32_FLOAT, 2, 0, 4},
        {OGPU_NEXT_D24_UNORM_S8_UINT, 6, 0, 4},
        {OGPU_NEXT_D32_FLOAT_S8_UINT, 6, 0, 4},
        {OGPU_NEXT_RG32_FLOAT, 1, 8, 0},
        {OGPU_NEXT_RGB32_FLOAT, 1, 12, 0},
        {OGPU_NEXT_RGBA16_UNORM, 1, 8, 0},
        {OGPU_NEXT_R4G4_UNORM_PACK8, 1, 1, 0},
        {OGPU_NEXT_R4G4B4A4_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_B4G4R4A4_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_R5G6B5_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_B5G6R5_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_R5G5B5A1_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_B5G5R5A1_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_A1R5G5B5_UNORM_PACK16, 1, 2, 0},
        {OGPU_NEXT_R8_SNORM, 1, 1, 0},
        {OGPU_NEXT_R8_USCALED, 1, 1, 0},
        {OGPU_NEXT_R8_SSCALED, 1, 1, 0},
        {OGPU_NEXT_R8_UINT, 1, 1, 0},
        {OGPU_NEXT_R8_SINT, 1, 1, 0},
        {OGPU_NEXT_R8_SRGB, 1, 1, 0},
        {OGPU_NEXT_RG8_SNORM, 1, 2, 0},
        {OGPU_NEXT_RG8_USCALED, 1, 2, 0},
        {OGPU_NEXT_RG8_SSCALED, 1, 2, 0},
        {OGPU_NEXT_RG8_UINT, 1, 2, 0},
        {OGPU_NEXT_RG8_SINT, 1, 2, 0},
        {OGPU_NEXT_RG8_SRGB, 1, 2, 0},
        {OGPU_NEXT_RGB8_UNORM, 1, 3, 0},
        {OGPU_NEXT_RGB8_SNORM, 1, 3, 0},
        {OGPU_NEXT_RGB8_USCALED, 1, 3, 0},
        {OGPU_NEXT_RGB8_SSCALED, 1, 3, 0},
        {OGPU_NEXT_RGB8_UINT, 1, 3, 0},
        {OGPU_NEXT_RGB8_SINT, 1, 3, 0},
        {OGPU_NEXT_RGB8_SRGB, 1, 3, 0},
        {OGPU_NEXT_BGR8_UNORM, 1, 3, 0},
        {OGPU_NEXT_BGR8_SNORM, 1, 3, 0},
        {OGPU_NEXT_BGR8_USCALED, 1, 3, 0},
        {OGPU_NEXT_BGR8_SSCALED, 1, 3, 0},
        {OGPU_NEXT_BGR8_UINT, 1, 3, 0},
        {OGPU_NEXT_BGR8_SINT, 1, 3, 0},
        {OGPU_NEXT_BGR8_SRGB, 1, 3, 0},
        {OGPU_NEXT_RGBA8_SNORM, 1, 4, 0},
        {OGPU_NEXT_RGBA8_USCALED, 1, 4, 0},
        {OGPU_NEXT_RGBA8_SSCALED, 1, 4, 0},
        {OGPU_NEXT_RGBA8_UINT, 1, 4, 0},
        {OGPU_NEXT_RGBA8_SINT, 1, 4, 0},
        {OGPU_NEXT_BGRA8_SNORM, 1, 4, 0},
        {OGPU_NEXT_BGRA8_USCALED, 1, 4, 0},
        {OGPU_NEXT_BGRA8_SSCALED, 1, 4, 0},
        {OGPU_NEXT_BGRA8_UINT, 1, 4, 0},
        {OGPU_NEXT_BGRA8_SINT, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_UNORM_PACK32, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_SNORM_PACK32, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_USCALED_PACK32, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_SSCALED_PACK32, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_UINT_PACK32, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_SINT_PACK32, 1, 4, 0},
        {OGPU_NEXT_A8B8G8R8_SRGB_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2R10G10B10_UNORM_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2R10G10B10_SNORM_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2R10G10B10_USCALED_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2R10G10B10_SSCALED_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2R10G10B10_UINT_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2R10G10B10_SINT_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2B10G10R10_UNORM_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2B10G10R10_SNORM_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2B10G10R10_USCALED_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2B10G10R10_SSCALED_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2B10G10R10_UINT_PACK32, 1, 4, 0},
        {OGPU_NEXT_A2B10G10R10_SINT_PACK32, 1, 4, 0},
        {OGPU_NEXT_R16_UNORM, 1, 2, 0},
        {OGPU_NEXT_R16_SNORM, 1, 2, 0},
        {OGPU_NEXT_R16_USCALED, 1, 2, 0},
        {OGPU_NEXT_R16_SSCALED, 1, 2, 0},
        {OGPU_NEXT_R16_UINT, 1, 2, 0},
        {OGPU_NEXT_R16_SINT, 1, 2, 0},
        {OGPU_NEXT_RG16_UNORM, 1, 4, 0},
        {OGPU_NEXT_RG16_SNORM, 1, 4, 0},
        {OGPU_NEXT_RG16_USCALED, 1, 4, 0},
        {OGPU_NEXT_RG16_SSCALED, 1, 4, 0},
        {OGPU_NEXT_RG16_UINT, 1, 4, 0},
        {OGPU_NEXT_RG16_SINT, 1, 4, 0},
        {OGPU_NEXT_RGB16_UNORM, 1, 6, 0},
        {OGPU_NEXT_RGB16_SNORM, 1, 6, 0},
        {OGPU_NEXT_RGB16_USCALED, 1, 6, 0},
        {OGPU_NEXT_RGB16_SSCALED, 1, 6, 0},
        {OGPU_NEXT_RGB16_UINT, 1, 6, 0},
        {OGPU_NEXT_RGB16_SINT, 1, 6, 0},
        {OGPU_NEXT_RGB16_FLOAT, 1, 6, 0},
        {OGPU_NEXT_RGBA16_SNORM, 1, 8, 0},
        {OGPU_NEXT_RGBA16_USCALED, 1, 8, 0},
        {OGPU_NEXT_RGBA16_SSCALED, 1, 8, 0},
        {OGPU_NEXT_RGBA16_UINT, 1, 8, 0},
        {OGPU_NEXT_RGBA16_SINT, 1, 8, 0},
        {OGPU_NEXT_R32_SINT, 1, 4, 0},
        {OGPU_NEXT_RG32_UINT, 1, 8, 0},
        {OGPU_NEXT_RG32_SINT, 1, 8, 0},
        {OGPU_NEXT_RGB32_UINT, 1, 12, 0},
        {OGPU_NEXT_RGB32_SINT, 1, 12, 0},
        {OGPU_NEXT_RGBA32_UINT, 1, 16, 0},
        {OGPU_NEXT_RGBA32_SINT, 1, 16, 0},
        {OGPU_NEXT_B10G11R11_UFLOAT_PACK32, 1, 4, 0},
        {OGPU_NEXT_E5B9G9R9_UFLOAT_PACK32, 1, 4, 0},
        {OGPU_NEXT_X8_D24_UNORM_PACK32, 2, 0, 4},
        {OGPU_NEXT_S8_UINT, 4, 0, 0},
        {OGPU_NEXT_D16_UNORM_S8_UINT, 6, 0, 2},
    };
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_format_info *formats = NULL;
    ogpu_next_image *image = NULL;
    ogpu_next_view *view = NULL;
    ogpu_next_memory *backing = NULL, *host = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL;
    void *barrier_scratch = NULL, *submit_scratch = NULL;
    unsigned char expected[8192];
    uint32_t executed = 0, excluded = 0, unverified = 0, stencil = 0;
    const char *raster_only=getenv("OGPU_FOUNDATION_STENCIL_RASTER_ONLY");
    int omit_combined=raster_only && strcmp(raster_only,"1")==0;
    uint64_t value = 0;
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FORMATS, NULL, 0, sizeof(*formats));
    TRY(ogpu_next_device_query(device, &q));
    formats = calloc(q.count, sizeof(*formats)); REQUIRE(formats);
    q.data = formats; q.capacity = q.count; TRY(ogpu_next_device_query(device, &q));
    REQUIRE(q.count == sizeof(cases)/sizeof(cases[0]));
    host_desc.size = sizeof(expected); host_desc.alignment = 16;
    host_desc.usage = OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST;
    TRY(ogpu_next_memory_create(device, &host_desc, &host));
    ogpu_next_span all = {host,0,sizeof(expected)};
    ogpu_next_mapping mapping = {0}; TRY(ogpu_next_memory_map(all,&mapping));
    uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,0,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements br={0}, sr={0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br));
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    barrier_scratch=malloc(br.size); submit_scratch=malloc(sr.size);
    REQUIRE(barrier_scratch && submit_scratch);
    for(uint32_t i=0;i<q.count;++i) {
        uint32_t fmt=cases[i].format, aspects=cases[i].aspects;
        REQUIRE(formats[i].format == fmt);
        uint64_t features=formats[i].optimal_features;
        uint64_t view_usage=(features&OGPU_NEXT_FORMAT_SAMPLED) ? OGPU_NEXT_IMAGE_SAMPLED :
            (features&OGPU_NEXT_FORMAT_DEPTH_STENCIL) ? OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT :
            (features&OGPU_NEXT_FORMAT_COLOR) ? OGPU_NEXT_IMAGE_COLOR_ATTACHMENT : 0;
        ogpu_next_image_profile profile={HEADER(ogpu_next_image_profile,OGPU_NEXT_IMAGE_PROFILE),
            fmt,OGPU_NEXT_IMAGE_2D,OGPU_NEXT_IMAGE_COPY_SRC|OGPU_NEXT_IMAGE_COPY_DST|view_usage,0,0};
        ogpu_next_image_limits limits;
        ogpu_next_status support=ogpu_next_image_limits_query(device,&profile,&limits);
        if(support==OGPU_NEXT_UNSUPPORTED) {
            printf("FORMAT_EXCLUDED id=%u usage=%llu\n",fmt,(unsigned long long)profile.usage);
            ++excluded; continue;
        }
        TRY(support);
        if(aspects==6 && omit_combined) {
            printf("FORMAT_UNVERIFIED id=%u reason=explicit-combined-plane-readback-exclusion\n",fmt);
            ++unverified; continue;
        }
        ogpu_next_image_desc desc={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),
            fmt,OGPU_NEXT_IMAGE_2D,1,2,1,{5,3,1},profile.usage,0,NULL,0,0,NULL};
        uint32_t types[32]; ogpu_next_requirements req={0};
        req.compatible_types=types; req.compatible_type_capacity=32;
        TRY(ogpu_next_image_requirements(device,&desc,&req)); REQUIRE(req.compatible_type_count);
        TRY(ogpu_next_image_create_unbound(device,&desc,&image));
        TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
        TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,req.size}));
        if(view_usage) {
            ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),
                fmt,OGPU_NEXT_VIEW_2D_ARRAY,(uint32_t)view_usage,{0},{aspects==6 ? 2 : aspects,0,1,0,2}};
            TRY(ogpu_next_view_create(image,&vd,&view));
        }
        for(uint32_t aspect=1;aspect<=4;aspect<<=1) {
            if(!(aspects&aspect)) continue;
            uint64_t bytes=aspect==1 ? cases[i].bytes : aspect==2 ? cases[i].depth_bytes : 1;
            uint64_t alignment=aspect==1 ? bytes : 4;
            uint64_t src=64+(alignment-(address+64)%alignment)%alignment;
            uint64_t dst=4096+(alignment-(address+4096)%alignment)%alignment;
            uint64_t row=7*bytes, slice=4*row, length=slice+2*row+5*bytes;
            memset(expected,0xa5,sizeof(expected));
            for(unsigned layer=0;layer<2;++layer) for(unsigned y=0;y<3;++y) for(unsigned x=0;x<5*bytes;++x)
                expected[src+layer*slice+y*row+x]=(unsigned char)(17+(x+y*3+layer*5)%29);
            memcpy(mapping.data,expected,sizeof(expected)); TRY(ogpu_next_memory_flush(all));
            ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,0,NULL};
            ogpu_next_encoder *encoder=NULL; ogpu_next_list *list=NULL;
            TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
            ogpu_next_image_barrier ib={image,{aspects,0,1,0,2},0,OGPU_NEXT_ACCESS_COPY_WRITE,
                OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
            ogpu_next_dependency dep={HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),
                OGPU_NEXT_STAGE_COPY,aspect==1 ? OGPU_NEXT_STAGE_COPY : OGPU_NEXT_STAGE_CLEAR,0,0,0,1,0,NULL,&ib,barrier_scratch,br.size};
            ogpu_next_barrier(encoder,&dep);
            ogpu_next_image_copy copy={ {aspect,0,0,2,{0,0,0},{5,3,1}},row,slice,OGPU_NEXT_STATE_COPY_DST,0};
            if(aspect==1) ogpu_next_copy_to_image(encoder,image,(ogpu_next_span){host,src,length},&copy);
            else {
                ogpu_next_clear_value clear={0}; clear.depth_stencil.depth=0.0f; clear.depth_stencil.stencil=0x5a;
                ogpu_next_subresources range={aspect,0,1,0,2};
                ogpu_next_clear_image(encoder,image,OGPU_NEXT_STATE_COPY_DST,&range,&clear);
            }
            ib.before=OGPU_NEXT_ACCESS_COPY_WRITE; ib.after=OGPU_NEXT_ACCESS_COPY_READ;
            ib.old_state=OGPU_NEXT_STATE_COPY_DST; ib.new_state=OGPU_NEXT_STATE_COPY_SRC; ib.discard=0;
            dep.before=dep.after; dep.after=OGPU_NEXT_STAGE_COPY;
            ogpu_next_barrier(encoder,&dep);
            copy.state=OGPU_NEXT_STATE_COPY_SRC;
            ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,dst,length},image,&copy);
            dep.before=OGPU_NEXT_STAGE_COPY; dep.after=OGPU_NEXT_STAGE_HOST;
            dep.global_before=OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after=OGPU_NEXT_ACCESS_HOST_READ;
            dep.image_count=0; dep.images=NULL; ogpu_next_barrier(encoder,&dep);
            TRY(ogpu_next_commands_end(encoder,&list));
            ogpu_next_sync_point signal={{done,++value},OGPU_NEXT_STAGE_ALL};
            ogpu_next_submit_desc submit={HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),
                1,0,1,&list,NULL,&signal,submit_scratch,sr.size};
            pending=1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
            TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending=0;
            TRY(ogpu_next_memory_invalidate(all));
            for(unsigned layer=0;layer<2;++layer) for(unsigned y=0;y<3;++y) for(unsigned x=0;x<5*bytes;++x) {
                uint64_t at=layer*slice+y*row+x;
                expected[dst+at]=aspect==1 ? expected[src+at] : aspect==4 ? 0x5a : 0;
                /* Unused high byte of 24-bit depth is not defined by a copy. */
                if(aspect==2 && (fmt==OGPU_NEXT_X8_D24_UNORM_PACK32 || fmt==OGPU_NEXT_D24_UNORM_S8_UINT) && x%4==3)
                    expected[dst+at]=((unsigned char *)mapping.data)[dst+at];
            }
            if(memcmp(mapping.data,expected,sizeof(expected))!=0) {
                for(size_t at=0;at<sizeof(expected);++at) if(((unsigned char *)mapping.data)[at]!=expected[at]) {
                    fprintf(stderr,"Format mismatch id=%u aspect=%u offset=%zu got=%u expected=%u\n",
                        fmt,aspect,at,((unsigned char *)mapping.data)[at],expected[at]); break;
                }
                REQUIRE(0);
            }
            TRY(ogpu_next_arena_reset(arena));
        }
        executed++; if(fmt==OGPU_NEXT_S8_UINT) stencil++;
        ogpu_next_view_destroy(view); view=NULL; ogpu_next_image_destroy(image); image=NULL;
        ogpu_next_memory_destroy(backing); backing=NULL;
    }
    REQUIRE(executed>=40 && stencil==1); /* Explicit local fixture prerequisites. */
    printf("FORMAT_MATRIX_PASS executed=%u excluded=%u unverified=%u standalone_stencil=%u padded_copies=1 guards=1\n",executed,excluded,unverified,stencil);
    result=EXIT_SUCCESS;
cleanup:
    if(pending) { fprintf(stderr,"Pending format work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view);
    ogpu_next_image_destroy(image); ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(host);
    free(formats); free(barrier_scratch); free(submit_scratch);
    return result;
}
