/* Native ordered output, two streams, overflow, pause/resume and byte-count draw. */
static int capture_execution(ogpu_next_device *device, uint32_t domain,const char *const paths[3]) {
    int result=EXIT_FAILURE,pending=0;FILE *file=NULL;
    void *code[3]={NULL},*scratch=NULL,*barriers=NULL,*submits=NULL;
    ogpu_next_memory *memory=NULL;ogpu_next_executable *pipeline=NULL;ogpu_next_argument_layout *arguments=NULL;
    ogpu_next_arena *arena=NULL;ogpu_next_timeline *done=NULL;ogpu_next_query_pool *queries=NULL,*statistics=NULL;
    ogpu_next_feature_info features={0};ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));TRY(ogpu_next_device_query(device,&q));
    if(!raster_has(features.enabled,OGPU_NEXT_FEATURE_TRANSFORM_FEEDBACK)){puts("Capture UNVERIFIED: native feature unavailable.");return EXIT_SUCCESS;}
    ogpu_next_capture_limits limits={0};q=query(OGPU_NEXT_QUERY_CAPTURE_LIMITS,&limits,1,sizeof(limits));TRY(ogpu_next_device_query(device,&q));
    printf("Capture limits: streams=%u buffers=%u size=%" PRIu64 " stride=%u queries=%u select=%u draw=%u.\n",limits.max_streams,limits.max_buffers,limits.max_buffer_size,limits.max_stride,limits.queries,limits.rasterization_stream_select,limits.draw_byte_count);
    REQUIRE(limits.max_buffers>=1 && limits.max_buffer_size>=96 && limits.max_stride>=16);
    REQUIRE(heap_memory(device,4096,16,OGPU_NEXT_USAGE_STORAGE|OGPU_NEXT_USAGE_INDIRECT|OGPU_NEXT_USAGE_CAPTURE|OGPU_NEXT_USAGE_CAPTURE_COUNTER|OGPU_NEXT_USAGE_COPY_DST,
        OGPU_NEXT_MEMORY_HOST_VISIBLE,&memory)==EXIT_SUCCESS);
    ogpu_next_span all={memory,0,4096};ogpu_next_mapping map={0};uint64_t address=0;TRY(ogpu_next_memory_map(all,&map));TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    ogpu_next_shader shaders[3];
    for(uint32_t i=0;i<3;++i){file=fopen(paths[i],"rb");REQUIRE(file);REQUIRE(fseek(file,0,SEEK_END)==0);long size=ftell(file);REQUIRE(size>=20&&size%4==0);REQUIRE(fseek(file,0,SEEK_SET)==0);code[i]=malloc((size_t)size);REQUIRE(code[i]);REQUIRE(fread(code[i],1,(size_t)size,file)==(size_t)size);fclose(file);file=NULL;
        shaders[i]=(ogpu_next_shader){i==1?OGPU_NEXT_STAGE_GEOMETRY:OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)size},"main",NULL,NULL};}
    ogpu_next_host_requirements cr={0},br={0},sr={0};TRY(ogpu_next_capture_scratch_requirements(2,&cr));TRY(ogpu_next_barrier_scratch_requirements(device,1,0,0,&br));TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    scratch=malloc(cr.size);barriers=malloc(br.size);submits=malloc(sr.size);REQUIRE(scratch&&barriers&&submits);ogpu_next_host_span cs={scratch,cr.size},bs={barriers,br.size};
    ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,0,0};TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    if(limits.queries){ogpu_next_query_pool_desc qp={HEADER(ogpu_next_query_pool_desc,OGPU_NEXT_QUERY_POOL_DESC),OGPU_NEXT_QUERY_CAPTURE_STREAM,2,0};TRY(ogpu_next_query_pool_create(device,&qp,&queries));}
    if(limits.draw_byte_count&&raster_has(features.enabled,OGPU_NEXT_FEATURE_PIPELINE_STATISTICS)){ogpu_next_query_pool_desc qp={HEADER(ogpu_next_query_pool_desc,OGPU_NEXT_QUERY_POOL_DESC),OGPU_NEXT_QUERY_PIPELINE_STATISTICS,1,OGPU_NEXT_STATISTIC_INPUT_VERTICES};TRY(ogpu_next_query_pool_create(device,&qp,&statistics));}
    uint64_t serial=0;uint32_t profiles=0,vertices=0,rejected=0;
    for(uint32_t mode=0;mode<6;++mode){
        uint32_t geometry=mode>=3,paused=mode%3==1,overflow=mode%3==2,count=geometry?2:1;
        if(geometry&&(!raster_has(features.enabled,OGPU_NEXT_FEATURE_GEOMETRY_STREAMS)||limits.max_streams<2||limits.max_buffers<2))continue;
        uint32_t ids[]={OGPU_NEXT_FEATURE_RASTER,OGPU_NEXT_FEATURE_TRANSFORM_FEEDBACK,OGPU_NEXT_FEATURE_GEOMETRY,OGPU_NEXT_FEATURE_GEOMETRY_STREAMS};
        ogpu_next_shader_requirements req={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),{ids,geometry?4:2,0},{0},0};
        ogpu_next_raster_options ro={.header=HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),.discard=1,.line_width=1};
        ogpu_next_rasterization_stream stream={HEADER(ogpu_next_rasterization_stream,OGPU_NEXT_RASTERIZATION_STREAM_STATE),geometry&&limits.rasterization_stream_select?1:0,0};
        ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_POINTS,.samples=1,.raster_options=&ro.header};gs.header.next=&stream.header;
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,geometry?2:1,shaders,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&req.header,NULL,0,0};
        ogpu_next_shader selected[]={shaders[geometry?2:0],shaders[1]};ed.shaders=selected;
        TRY(ogpu_next_executable_create(device,&ed,arguments,&pipeline));
        TRY(ogpu_next_arena_reset(arena));ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,0,NULL};TRY(ogpu_next_commands_begin(arena,&rd,&e));
        uint32_t captured=overflow?4:6;ogpu_next_span buffers[]={{memory,64,captured*16},{memory,256,captured*16}},counters[]={{memory,512,4},{memory,516,4}};
        ogpu_next_bind_capture_buffers(e,0,count,buffers,cs);
        if(queries){ogpu_next_queries_reset(e,queries,0,count);for(uint32_t i=0;i<count;++i)ogpu_next_query_begin(e,queries,i,i,0);}
        if(statistics)ogpu_next_queries_reset(e,statistics,0,1);
        ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=1,.height=1,.layers=1,.samples=1};
        ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,1,1,0,1,0,0,1,1};
        for(uint32_t part=0;part<(paused?2u:1u);++part){
            ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline);ogpu_next_set_graphics_state(e,&vp.header);ogpu_next_set_arguments(e,arguments,0,8,&address);
            ogpu_next_capture_begin(e,0,part?count:0,part?counters:NULL,cs);
            ogpu_next_draw_desc draw={paused?3:6,1,part*3,0,0};ogpu_next_draw(e,&draw);
            ogpu_next_capture_end(e,0,count,counters,cs);ogpu_next_render_end(e);
            ogpu_next_barrier_scope scope={OGPU_NEXT_STAGE_CAPTURE,OGPU_NEXT_STAGE_CAPTURE|OGPU_NEXT_STAGE_INDIRECT,OGPU_NEXT_ACCESS_CAPTURE_WRITE|OGPU_NEXT_ACCESS_CAPTURE_COUNTER_WRITE,OGPU_NEXT_ACCESS_CAPTURE_WRITE|OGPU_NEXT_ACCESS_CAPTURE_COUNTER_READ};
            ogpu_next_dependency dep={1,0,0,0,&scope,NULL,NULL};ogpu_next_barrier(e,&dep,bs);
        }
        if(queries)for(uint32_t i=0;i<count;++i)ogpu_next_query_end(e,queries,i,i);
        if(statistics){ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline);ogpu_next_set_graphics_state(e,&vp.header);ogpu_next_query_begin(e,statistics,0,0,0);
            ogpu_next_draw_byte_count(e,counters[0],16,16,2,0);ogpu_next_query_end(e,statistics,0,0);ogpu_next_render_end(e);
            ogpu_next_queries_resolve(e,statistics,0,1,(ogpu_next_span){memory,608,8},8,OGPU_NEXT_QUERY_RESULT_64|OGPU_NEXT_QUERY_RESULT_WAIT);}
        if(queries)ogpu_next_queries_resolve(e,queries,0,count,(ogpu_next_span){memory,544,count*16},16,OGPU_NEXT_QUERY_RESULT_64|OGPU_NEXT_QUERY_RESULT_WAIT);
        ogpu_next_barrier_scope scope={OGPU_NEXT_STAGE_CAPTURE|OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_CAPTURE_WRITE|OGPU_NEXT_ACCESS_CAPTURE_COUNTER_WRITE|OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};ogpu_next_dependency dep={1,0,0,0,&scope,NULL,NULL};ogpu_next_barrier(e,&dep,bs);
        TRY(ogpu_next_commands_end(e,&list));
        for(uint32_t replay=0;replay<2;++replay){memset(map.data,0xa5,4096);uint32_t seed=13+47*replay;memcpy(map.data,&seed,4);TRY(ogpu_next_memory_flush(all));
            ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submits,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
            for(uint32_t b=0;b<count;++b){for(uint32_t v=0;v<captured;++v){float actual[4];memcpy(actual,(char*)map.data+buffers[b].offset+v*16,16);REQUIRE(actual[0]==(float)(seed+v+b*100)&&actual[1]==(float)(v*2)&&actual[2]==3&&actual[3]==4);++vertices;}
                uint32_t bytes;memcpy(&bytes,(char*)map.data+counters[b].offset,4);REQUIRE(bytes==captured*16);
                if(queries){uint64_t values[2];memcpy(values,(char*)map.data+544+b*16,16);REQUIRE(values[0]==captured&&values[1]==6);}}
            if(statistics){uint64_t value;memcpy(&value,(char*)map.data+608,8);REQUIRE(value==2*(captured-1));}
            for(uint32_t i=4;i<4096;++i){int written=(i>=64&&i<64+captured*16)||(count==2&&i>=256&&i<256+captured*16)||(i>=512&&i<512+count*4)||(queries&&i>=544&&i<544+count*16)||(statistics&&i>=608&&i<616);if(!written)REQUIRE(((unsigned char*)map.data)[i]==0xa5);}
        }
        if(mode==0) for(uint32_t bad=0;bad<7;++bad){
            TRY(ogpu_next_arena_reset(arena));TRY(ogpu_next_commands_begin(arena,&rd,&e));
            if(bad==0){ogpu_next_span invalid={memory,65,16};ogpu_next_bind_capture_buffers(e,0,1,&invalid,cs);}
            if(bad==1)ogpu_next_bind_capture_buffers(e,limits.max_buffers,0,NULL,cs);
            if(bad>=2){
                ogpu_next_bind_capture_buffers(e,0,1,buffers,cs);ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline);ogpu_next_set_graphics_state(e,&vp.header);
                if(bad==2)ogpu_next_capture_end(e,0,0,NULL,cs);
                else {ogpu_next_capture_begin(e,0,0,NULL,cs);
                    if(bad==3)ogpu_next_capture_begin(e,0,0,NULL,cs);
                    if(bad==4)ogpu_next_render_end(e);
                    if(bad==5)ogpu_next_bind_capture_buffers(e,0,1,buffers,cs);
                    if(bad==6)ogpu_next_bind_executable(e,pipeline);}
            }
            list=NULL;REQUIRE(ogpu_next_commands_end(e,&list)==OGPU_NEXT_INVALID && list==NULL);++rejected;
        }
        TRY(ogpu_next_arena_reset(arena));ogpu_next_executable_destroy(pipeline);pipeline=NULL;++profiles;
    }
    printf("Capture PASS: %u profiles, %" PRIu64 " submissions, %u ordered vertices, %u rejected recordings, counters/overflow, pause/resume, stream queries and byte-count draws.\n",profiles,serial,vertices,rejected);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Capture pending after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);ogpu_next_query_pool_destroy(queries);ogpu_next_query_pool_destroy(statistics);ogpu_next_executable_destroy(pipeline);ogpu_next_argument_layout_destroy(arguments);ogpu_next_memory_destroy(memory);
    if(file)fclose(file);
    for(uint32_t i=0;i<3;++i)free(code[i]);
    free(scratch);free(barriers);free(submits);return result;
}
