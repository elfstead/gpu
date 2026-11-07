/* Native VS/HS/DS/GS composition, adjacency and caller-owned changed-data replay. */
static int raster_has(ogpu_next_features features,uint32_t id) {
    for(uint32_t i=0;i<features.count;++i) if(features.ids[i]==id) return 1;
    return 0;
}
static int raster_stages_execution(ogpu_next_device *device,ogpu_next_memory_desc md,
                                   uint32_t domain,const char *const paths[7]) {
    int result=EXIT_FAILURE,pending=0; FILE *file=NULL;
    void *code[7]={NULL},*scratch=NULL,*render_scratch=NULL,*submit_scratch=NULL;
    ogpu_next_memory *memory=NULL,*backing=NULL; ogpu_next_image *image=NULL; ogpu_next_view *view=NULL;
    ogpu_next_argument_layout *arguments=NULL; ogpu_next_executable *pipeline=NULL;
    ogpu_next_arena *arena=NULL; ogpu_next_timeline *done=NULL; ogpu_next_query_pool *statistics=NULL;
    ogpu_next_feature_info features={0}; ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    uint32_t tess=raster_has(features.enabled,OGPU_NEXT_FEATURE_TESSELLATION),geometry=raster_has(features.enabled,OGPU_NEXT_FEATURE_GEOMETRY);
    if(!tess && !geometry) { puts("Raster stages UNVERIFIED: no native tessellation/geometry."); return EXIT_SUCCESS; }
    ogpu_next_raster_stage_limits limits={0}; q=query(OGPU_NEXT_QUERY_RASTER_STAGE_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    printf("Raster stage limits: patch=%u level=%u geometry vertices=%u invocations=%u.\n",limits.max_patch_size,limits.max_generation_level,limits.geometry_output_vertices,limits.geometry_invocations);
    uint64_t stages=OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT;
    if(tess) stages|=OGPU_NEXT_STAGE_TESS_CONTROL|OGPU_NEXT_STAGE_TESS_EVALUATION;
    if(geometry) stages|=OGPU_NEXT_STAGE_GEOMETRY;
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,stages,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field}; TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    const char *names[]={"vertexMain","adjacencyVertex","hullMain","domainMain","geometryMain","adjacencyMain","fragmentMain"};
    const uint32_t shader_stages[]={OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_TESS_CONTROL,OGPU_NEXT_STAGE_TESS_EVALUATION,OGPU_NEXT_STAGE_GEOMETRY,OGPU_NEXT_STAGE_GEOMETRY,OGPU_NEXT_STAGE_FRAGMENT};
    ogpu_next_shader shaders[7];
    for(uint32_t i=0;i<7;++i) {
        file=fopen(paths[i],"rb"); REQUIRE(file); REQUIRE(fseek(file,0,SEEK_END)==0); long n=ftell(file);
        REQUIRE(n>=20 && n%4==0); REQUIRE(fseek(file,0,SEEK_SET)==0); code[i]=malloc((size_t)n); REQUIRE(code[i]);
        REQUIRE(fread(code[i],1,(size_t)n,file)==(size_t)n); fclose(file);file=NULL;
        shaders[i]=(ogpu_next_shader){shader_stages[i],OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)n},names[i],NULL,NULL};
    }
    TRY(ogpu_next_memory_create(device,&md,&memory)); REQUIRE(md.size>=2048);
    ogpu_next_span all={memory,0,md.size};ogpu_next_mapping map={0};uint64_t address=0;
    TRY(ogpu_next_memory_map(all,&map));TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
    uint32_t types[32];ogpu_next_requirements ir={.compatible_types=types,.compatible_type_capacity=32};TRY(ogpu_next_image_requirements(device,&id,&ir));REQUIRE(ir.compatible_type_count);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,ir.size}));
    ogpu_next_subresources range={OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0},range};TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_host_requirements br={0},rr={0},sr={0};TRY(ogpu_next_barrier_scratch_requirements(device,1,0,1,&br));TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment<=_Alignof(max_align_t) && rr.alignment<=_Alignof(max_align_t) && sr.alignment<=_Alignof(max_align_t));
    scratch=malloc((size_t)br.size);render_scratch=malloc((size_t)rr.size);submit_scratch=malloc((size_t)sr.size);REQUIRE(scratch && render_scratch && submit_scratch);
    ogpu_next_host_span bs={scratch,br.size};
    ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0,checked_pixels=0; uint32_t rejected=0;
    for(uint32_t mode=0;mode<6;++mode) {
        uint32_t has_tess=mode>=2,has_geometry=mode<2 || mode>=4,adjacent=mode==1;
        if((has_tess&&!tess)||(has_geometry&&!geometry))continue;
        ogpu_next_shader selected[5];uint32_t count=0;selected[count++]=shaders[adjacent?1:0];
        if(has_tess){selected[count++]=shaders[2];selected[count++]=shaders[3];}
        if(has_geometry) { selected[count++]=shaders[adjacent?5:4]; }
        selected[count++]=shaders[6];
        uint32_t enables[3]={OGPU_NEXT_FEATURE_RASTER,0,0},enable_count=1;
        if(has_tess)enables[enable_count++]=OGPU_NEXT_FEATURE_TESSELLATION;
        if(has_geometry)enables[enable_count++]=OGPU_NEXT_FEATURE_GEOMETRY;
        ogpu_next_shader_requirements requirements={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),{enables,enable_count,0},{0},0};
        ogpu_next_color_state color={.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15};
        ogpu_next_tessellation_state ts={HEADER(ogpu_next_tessellation_state,OGPU_NEXT_TESSELLATION_STATE),3,mode&1};
        ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=has_tess?OGPU_NEXT_PATCHES:adjacent?OGPU_NEXT_TRIANGLES_ADJACENCY:OGPU_NEXT_TRIANGLES,.front_face=OGPU_NEXT_FRONT_CCW,.samples=1,.color_count=1,.colors=&color};
        gs.header.next=has_tess?&ts.header:NULL;
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,count,selected,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&requirements.header,NULL,0,0};
        if(mode==2) {
            for(uint32_t test=0;test<7;++test) {
                ts.patch_control_points=3;ts.domain_origin=0;ts.header.next=NULL;gs.header.next=&ts.header;gs.topology=OGPU_NEXT_PATCHES;ed.shader_count=count;
                ogpu_next_status expected=OGPU_NEXT_INVALID;
                if(test==0)ts.patch_control_points=0;
                if(test==1){ts.patch_control_points=limits.max_patch_size+1;expected=OGPU_NEXT_UNSUPPORTED;}
                if(test==2)ts.domain_origin=2;
                if(test==3)gs.header.next=NULL;
                if(test==4)gs.topology=OGPU_NEXT_TRIANGLES;
                if(test==5)ts.header.next=&ts.header;
                if(test==6){selected[2]=shaders[6];ed.shader_count=3;expected=OGPU_NEXT_UNSUPPORTED;}
                ogpu_next_executable *bad=NULL;ogpu_next_status status=ogpu_next_executable_create(device,&ed,arguments,&bad);
                if(status!=expected)fprintf(stderr,"raster rejection %u: %d expected %d\n",test,status,expected);
                REQUIRE(status==expected);REQUIRE(bad==NULL);++rejected;
            }
            ts.patch_control_points=3;ts.domain_origin=0;ts.header.next=NULL;gs.header.next=&ts.header;gs.topology=OGPU_NEXT_PATCHES;ed.shader_count=count;selected[2]=shaders[3];
        }
        TRY(ogpu_next_executable_create(device,&ed,arguments,&pipeline));
        uint64_t mask=(has_geometry?(OGPU_NEXT_STATISTIC_GEOMETRY_INVOCATIONS|OGPU_NEXT_STATISTIC_GEOMETRY_PRIMITIVES):0)|(has_tess?(OGPU_NEXT_STATISTIC_TESS_CONTROL_PATCHES|OGPU_NEXT_STATISTIC_TESS_EVALUATION_INVOCATIONS):0);
        uint32_t counters=2*(has_geometry+has_tess);
        if(raster_has(features.enabled,OGPU_NEXT_FEATURE_PIPELINE_STATISTICS)) {
            ogpu_next_query_pool_desc qp={HEADER(ogpu_next_query_pool_desc,OGPU_NEXT_QUERY_POOL_DESC),OGPU_NEXT_QUERY_PIPELINE_STATISTICS,1,mask};TRY(ogpu_next_query_pool_create(device,&qp,&statistics));
        } else counters=0;
        TRY(ogpu_next_arena_reset(arena));ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&e));
        if(statistics)ogpu_next_queries_reset(e,statistics,0,1);
        ogpu_next_image_barrier ib={{0,OGPU_NEXT_STAGE_COLOR,0,OGPU_NEXT_ACCESS_COLOR_WRITE},image,range,OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
        ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&ib};ogpu_next_barrier(e,&dep,bs);
        ogpu_next_attachment a={view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
        ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=1,.color_count=1,.colors=&a,.scratch={render_scratch,rr.size}};
        ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
        ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline);ogpu_next_set_graphics_state(e,&vp.header);ogpu_next_set_arguments(e,arguments,0,8,&address);
        if(statistics)ogpu_next_query_begin(e,statistics,0,0);
        ogpu_next_draw_desc draw={adjacent?6u:3u,1,0,0,0};
        if(mode%3==0)ogpu_next_draw(e,&draw);
        else if(mode%3==1){ogpu_next_bind_indices(e,(ogpu_next_span){memory,320,24},OGPU_NEXT_INDEX_U32);ogpu_next_draw_indexed(e,&draw);}
        else {ogpu_next_indirect indirect={{memory,256,16},16,1,{NULL,0,0}};ogpu_next_draw_indirect(e,&indirect);}
        if(statistics)ogpu_next_query_end(e,statistics,0);
        ogpu_next_render_end(e);
        ib=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_COPY_READ},image,range,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0,NULL};ogpu_next_barrier(e,&dep,bs);
        ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024,256},image,&copy);
        if(statistics)ogpu_next_queries_resolve(e,statistics,0,1,(ogpu_next_span){memory,512,counters*8},counters*8,OGPU_NEXT_QUERY_RESULT_64|OGPU_NEXT_QUERY_RESULT_WAIT);
        ogpu_next_barrier_scope host={OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};dep=(ogpu_next_dependency){1,0,0,0,&host,NULL,NULL};ogpu_next_barrier(e,&dep,bs);
        TRY(ogpu_next_commands_end(e,&list));uint64_t previous_evaluations=0;
        for(uint32_t replay=0;replay<2;++replay) {
            memset(map.data,0xa5,(size_t)md.size);
            struct {float v,h,d,g,level;uint32_t mode;} control={3.0f+replay,5,7,11,2.0f+2*replay,mode+1};memcpy(map.data,&control,sizeof(control));
            uint32_t indirect[4]={draw.count,1,0,0},indices[6]={0,1,2,3,4,5};memcpy((char*)map.data+256,indirect,16);memcpy((char*)map.data+320,indices,24);
            TRY(ogpu_next_memory_flush(all));ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;
            TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submit_scratch,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
            uint32_t expected=3+replay+has_tess*12+has_geometry*11;
            for(uint32_t i=0;i<64;++i) {unsigned char *p=(unsigned char*)map.data+1024+i*4;if(p[0]!=expected)fprintf(stderr,"raster mode %u replay %u pixel %u: %u expected %u\n",mode,replay,i,p[0],expected);REQUIRE(p[0]==expected && p[1]==mode+1 && p[2]==37 && p[3]==255);++checked_pixels;}
            if(statistics) {
                uint64_t *values=(uint64_t*)((char*)map.data+512);
                for(uint32_t i=0;i<counters;++i)REQUIRE(values[i]>0);
                if(has_tess){REQUIRE(values[counters-2]==1);REQUIRE(values[counters-1]>previous_evaluations);previous_evaluations=values[counters-1];}
                printf("Raster stages mode %u replay %u stats",mode,replay);for(uint32_t i=0;i<counters;++i)printf(" %" PRIu64,values[i]);puts("");
            }
            REQUIRE(memcmp(map.data,&control,sizeof(control))==0);
            REQUIRE(memcmp((char*)map.data+256,indirect,16)==0 && memcmp((char*)map.data+320,indices,24)==0);
            for(uint64_t i=24;i<md.size;++i)if(!(i>=256&&i<272)&&!(i>=320&&i<344)&&!(i>=512&&i<512+counters*8)&&!(i>=1024&&i<1280))REQUIRE(((unsigned char*)map.data)[i]==0xa5);
        }
        TRY(ogpu_next_arena_reset(arena));ogpu_next_executable_destroy(pipeline);pipeline=NULL;ogpu_next_query_pool_destroy(statistics);statistics=NULL;
    }
    printf("Raster stages PASS: %" PRIu64 " submissions, %" PRIu64 " pixels, %u rejected preparations, changed tessellation levels, direct/indexed/indirect and guards.\n",serial,checked_pixels,rejected);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Pending raster-stage work after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);ogpu_next_query_pool_destroy(statistics);ogpu_next_executable_destroy(pipeline);ogpu_next_argument_layout_destroy(arguments);
    ogpu_next_view_destroy(view);ogpu_next_image_destroy(image);ogpu_next_memory_destroy(backing);ogpu_next_memory_destroy(memory);
    if(file) { fclose(file); }
    for(uint32_t i=0;i<7;++i){free(code[i]);}
    free(scratch);free(render_scratch);free(submit_scratch);return result;
}
