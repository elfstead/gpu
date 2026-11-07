/* Four-sample coverage, sloped depth, explicit transitions and LOAD preservation. */
static int sample_locations_execution(ogpu_next_device *device,ogpu_next_memory_desc md,
                                     uint32_t domain,const char *const paths[2]) {
    int result=EXIT_FAILURE,pending=0;FILE *file=NULL;
    void *code[2]={NULL},*scratch=NULL,*render_scratch=NULL,*submit_scratch=NULL,*split_scratch=NULL;
    ogpu_next_memory *memory=NULL,*backing[4]={NULL};ogpu_next_image *images[4]={NULL};ogpu_next_view *views[4]={NULL};
    ogpu_next_argument_layout *arguments=NULL;ogpu_next_executable *pipeline[2]={NULL};
    ogpu_next_arena *arena=NULL;ogpu_next_timeline *done=NULL;ogpu_next_split *split=NULL;
    ogpu_next_feature_info features={0};ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));TRY(ogpu_next_device_query(device,&q));
    ogpu_next_sample_location_limits limits={0};q=query(OGPU_NEXT_QUERY_SAMPLE_LOCATION_LIMITS,&limits,1,sizeof(limits));TRY(ogpu_next_device_query(device,&q));
    ogpu_next_graphics_limits graphics={0};q=query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&graphics,1,sizeof(graphics));TRY(ogpu_next_device_query(device,&q));
    printf("Sample locations: counts=0x%x grid=%ux%u coordinates=%g..%g precision=%u variable=%u four-sample-grid=%ux%u.\n",limits.samples,limits.max_grid[0],limits.max_grid[1],limits.coordinate_range[0],limits.coordinate_range[1],limits.subpixel_bits,limits.variable,limits.grids[2][0],limits.grids[2][1]);
    if(!raster_has(features.enabled,OGPU_NEXT_FEATURE_SAMPLE_LOCATIONS)||!(limits.samples&4)||!(graphics.depth_resolve_modes&OGPU_NEXT_ATTACHMENT_RESOLVE_SAMPLE_ZERO)) {
        puts("Sample locations UNVERIFIED: four-sample positions/depth resolve unavailable.");return EXIT_SUCCESS;
    }
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    const char *names[]={"vertexMain","fragmentMain"};const uint32_t stages[]={OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_FRAGMENT};ogpu_next_shader shaders[2];
    for(uint32_t i=0;i<2;++i) {
        file=fopen(paths[i],"rb");REQUIRE(file);REQUIRE(fseek(file,0,SEEK_END)==0);long n=ftell(file);REQUIRE(n>=20&&n%4==0);REQUIRE(fseek(file,0,SEEK_SET)==0);
        code[i]=malloc((size_t)n);REQUIRE(code[i]);REQUIRE(fread(code[i],1,(size_t)n,file)==(size_t)n);fclose(file);file=NULL;
        shaders[i]=(ogpu_next_shader){stages[i],OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)n},names[i],NULL,NULL};
    }
    TRY(ogpu_next_memory_create(device,&md,&memory));REQUIRE(md.size>=2048);ogpu_next_span all={memory,0,md.size};ogpu_next_mapping map={0};uint64_t address=0;
    TRY(ogpu_next_memory_map(all,&map));TRY(ogpu_next_memory_address(all,&address));
    for(uint32_t i=0;i<4;++i) {
        uint32_t depth=i>=2;ogpu_next_format format=depth?OGPU_NEXT_D32_FLOAT:OGPU_NEXT_RGBA8_UNORM;
        uint64_t usage=depth?OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT:OGPU_NEXT_IMAGE_COLOR_ATTACHMENT;
        ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),format,OGPU_NEXT_IMAGE_2D,1,1,(i&1)?1:4,{8,8,1},usage|OGPU_NEXT_IMAGE_COPY_SRC,i==2?OGPU_NEXT_IMAGE_SAMPLE_LOCATIONS_DEPTH:0,NULL,0,0,NULL};
        uint32_t types[32];ogpu_next_requirements ir={.compatible_types=types,.compatible_type_capacity=32};TRY(ogpu_next_image_requirements(device,&id,&ir));REQUIRE(ir.compatible_type_count);
        TRY(ogpu_next_image_create_unbound(device,&id,&images[i]));TRY(ogpu_next_memory_create_dedicated_image(images[i],types[0],&backing[i]));TRY(ogpu_next_image_bind(images[i],(ogpu_next_span){backing[i],0,ir.size}));
        ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),format,OGPU_NEXT_VIEW_2D,usage,{0},{depth?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1}};
        TRY(ogpu_next_view_create(images[i],&vd,&views[i]));
    }
    ogpu_next_host_requirements br={0},rr={0},sr={0},sp={0};TRY(ogpu_next_barrier_scratch_requirements(device,1,0,4,&br));TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment<=_Alignof(max_align_t)&&rr.alignment<=_Alignof(max_align_t)&&sr.alignment<=_Alignof(max_align_t));
    scratch=malloc((size_t)br.size);render_scratch=malloc((size_t)rr.size);submit_scratch=malloc((size_t)sr.size);REQUIRE(scratch&&render_scratch&&submit_scratch);
    TRY(ogpu_next_split_create(device,&split));ogpu_next_dependency split_shape={.image_count=1};ogpu_next_split_dependency shape={split,&split_shape};
    TRY(ogpu_next_split_scratch_requirements(device,1,&shape,&sp));REQUIRE(sp.alignment<=_Alignof(max_align_t));split_scratch=malloc((size_t)sp.size);REQUIRE(split_scratch);
    ogpu_next_host_span bs={scratch,br.size};ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0,pixels_checked=0;uint32_t tested=0,rejected=0,valid[9]={0};unsigned char reference[9][2][1024];
    for(uint32_t mode=0;mode<9;++mode) {
        uint32_t dp=mode==2||mode==3||mode==5,de=mode==4||mode==5,wide=mode==6,right=mode==1||mode==3||mode==5;
        if((de&&!raster_has(features.enabled,OGPU_NEXT_FEATURE_DYNAMIC_SAMPLE_LOCATIONS_ENABLE)) || (wide&&(limits.grids[2][0]%2||limits.grids[2][1]%2))) {
            printf("Sample locations mode %u UNVERIFIED: unsupported native profile.\n",mode);continue;
        }
        ogpu_next_sample_location coords[16];uint32_t cells=wide?4:1;
        for(uint32_t cell=0;cell<cells;++cell)for(uint32_t sample=0;sample<4;++sample) {
            uint32_t choose=wide?((cell%2+cell/2)%2):mode==7?sample%2:right;
            coords[cell*4+sample]=(ogpu_next_sample_location){choose?.75f:.25f,.5f};
        }
        ogpu_next_sample_locations positions={HEADER(ogpu_next_sample_locations,OGPU_NEXT_SAMPLE_LOCATIONS),4,{wide?2:1,wide?2:1},4*cells,coords};
        ogpu_next_sample_location_state state={HEADER(ogpu_next_sample_location_state,OGPU_NEXT_SAMPLE_LOCATION_STATE),de?UINT32_MAX:1,0,dp?NULL:&positions};
        ogpu_next_raster_options options={.header=HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),.line_width=1,.max_depth_bounds=1,.sample_mask=UINT64_MAX};options.header.next=&state.header;
        ogpu_next_color_state color={.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15};
        ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_TRIANGLES,.front_face=OGPU_NEXT_FRONT_CCW,.samples=4,.color_count=1,.colors=&color,
            .depth_format=OGPU_NEXT_D32_FLOAT,.depth_test=1,.depth_write=1,.depth_compare=OGPU_NEXT_COMPARE_LESS,.raster_options=&options.header};
        ogpu_next_shader_requirements req={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),features.enabled,{0},0};
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,
            OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR|(dp?OGPU_NEXT_DYNAMIC_SAMPLE_LOCATIONS:0)|(de?OGPU_NEXT_DYNAMIC_SAMPLE_LOCATIONS_ENABLE:0),&req.header,NULL,0,0};
        if(mode==0)for(uint32_t bad=0;bad<5;++bad) {
            if(bad==0){positions.count=3;}if(bad==1){positions.grid[0]=0;}if(bad==2){state.enable=2;}if(bad==3){state.reserved=1;}if(bad==4){positions.header.next=&positions.header;}
            ogpu_next_executable *invalid=NULL;REQUIRE(ogpu_next_executable_create(device,&ed,arguments,&invalid)!=OGPU_NEXT_OK);REQUIRE(!invalid);++rejected;
            positions.count=4;positions.grid[0]=1;state.enable=1;state.reserved=0;positions.header.next=NULL;
        }
        for(uint32_t phase=0;phase<2;++phase){gs.depth_write=!phase;TRY(ogpu_next_executable_create(device,&ed,arguments,&pipeline[phase]));}
        ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&e));
        ogpu_next_image_barrier ib[4];
        for(uint32_t phase=0;phase<2;++phase) {
            if(phase) {
                ib[0]=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_DEPTH|OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_ACCESS_DEPTH_WRITE|OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_DEPTH_READ},images[2],{OGPU_NEXT_ASPECT_DEPTH,0,1,0,1},OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0,&positions.header};
                ogpu_next_dependency transition={0,0,1,0,NULL,NULL,ib};
                if(mode==8) {
                    /* Reset is a separate event operation: explicitly order the next signal. */
                    ogpu_next_split_reset(e,split,OGPU_NEXT_STAGE_ALL);
                    ogpu_next_barrier_scope reset_order={OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_DEPTH|OGPU_NEXT_STAGE_COLOR,0,0};
                    ogpu_next_dependency reset_dep={1,0,0,0,&reset_order,NULL,NULL};ogpu_next_barrier(e,&reset_dep,bs);
                    ogpu_next_split_release(e,split,&transition,bs);ogpu_next_split_dependency wait={split,&transition};
                    ogpu_next_split_acquire(e,1,&wait,(ogpu_next_host_span){split_scratch,sp.size});
                }
                else ogpu_next_barrier(e,&transition,bs);
                ib[0].old_state=OGPU_NEXT_STATE_GENERAL;ib[0].new_state=OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT;ib[0].scope.access_before=OGPU_NEXT_ACCESS_WRITE;ogpu_next_barrier(e,&transition,bs);
            }
            for(uint32_t i=0;i<4;++i) {
                uint32_t depth=i>=2;uint32_t target=depth?OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT:OGPU_NEXT_STATE_COLOR_ATTACHMENT;
                ib[i]=(ogpu_next_image_barrier){{phase?((i&1)?OGPU_NEXT_STAGE_COPY:OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH):0,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,
                    phase?((i&1)?OGPU_NEXT_ACCESS_COPY_READ:OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE):0,OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_READ|OGPU_NEXT_ACCESS_DEPTH_WRITE},
                    images[i],{depth?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1},phase?((i&1)?OGPU_NEXT_STATE_COPY_SRC:target):OGPU_NEXT_STATE_UNDEFINED,target,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,!phase,0,i==2&&phase?&positions.header:NULL};
            }
            ogpu_next_dependency dep={0,0,4,0,NULL,NULL,ib};ogpu_next_barrier(e,&dep,bs);
            ogpu_next_attachment ca={views[0],views[1],OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,OGPU_NEXT_ATTACHMENT_RESOLVE_AVERAGE,{.f32={0,0,0,1}}};
            ogpu_next_attachment da={views[2],views[3],OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,phase?OGPU_NEXT_LOAD:OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,OGPU_NEXT_ATTACHMENT_RESOLVE_SAMPLE_ZERO,{.depth_stencil={1,0}}};
            ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=4,.color_count=1,.colors=&ca,.depth=&da,.scratch={render_scratch,rr.size}};
            ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};uint64_t root=address+32*phase;
            ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline[phase]);ogpu_next_set_graphics_state(e,&vp.header);ogpu_next_set_arguments(e,arguments,0,8,&root);
            if(dp)ogpu_next_set_sample_locations(e,&positions);
            if(de){ogpu_next_set_sample_locations_enable(e,0);ogpu_next_set_sample_locations_enable(e,1);}
            ogpu_next_draw_desc draw={phase?3:6,1,0,0,0};ogpu_next_draw(e,&draw);ogpu_next_render_end(e);
            for(uint32_t n=0;n<2;++n) {
                uint32_t i=1+2*n;ib[n]=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE,OGPU_NEXT_ACCESS_COPY_READ},images[i],
                    {n?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1},n?OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT:OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0,NULL};
            }
            dep.image_count=2;ogpu_next_barrier(e,&dep,bs);
            for(uint32_t n=0;n<2;++n){ogpu_next_image_copy copy={{n?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024+512*phase+256*n,256},images[1+2*n],&copy);}
        }
        ogpu_next_barrier_scope host={OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};ogpu_next_dependency host_dep={1,0,0,0,&host,NULL,NULL};ogpu_next_barrier(e,&host_dep,bs);TRY(ogpu_next_commands_end(e,&list));
        for(uint32_t replay=0;replay<2;++replay) {
            memset(map.data,0xa5,(size_t)md.size);struct {float depth,slope;uint32_t shape,unused;} control={replay?.125f:.25f,1.0f/32,0,0},later={.75f,0,1,0};
            memcpy(map.data,&control,sizeof(control));memcpy((char*)map.data+32,&later,sizeof(later));unsigned char before[1024];memcpy(before,map.data,1024);
            TRY(ogpu_next_memory_flush(all));ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;
            TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submit_scratch,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
            for(uint32_t phase=0;phase<2;++phase)for(uint32_t y=0;y<8;++y)for(uint32_t x=0;x<8;++x) {
                uint32_t cell=wide?(x%2+2*(y%2)):0,covered=0;for(uint32_t sample=0;sample<4;++sample)covered+=x+coords[4*cell+sample].x<4.5f;
                uint32_t visible=phase?4-covered:covered,expected=(255*visible+2)/4;unsigned char *pixel=(unsigned char*)map.data+1024+512*phase+4*(y*8+x);
                if(pixel[0]!=expected)fprintf(stderr,"Sample mode %u phase %u replay %u pixel %u,%u got %u expected %u\n",mode,phase,replay,x,y,pixel[0],expected);
                REQUIRE(pixel[0]==expected&&pixel[1]==expected&&pixel[2]==expected&&pixel[3]==255);
                float sx=coords[4*cell].x,want=x+sx<4.5f?control.depth+control.slope*(x+sx):1,actual;memcpy(&actual,(char*)map.data+1280+512*phase+4*(y*8+x),4);
                if(fabsf(actual-want)>1e-6f)fprintf(stderr,"Sample depth mode %u phase %u at %u,%u got %g expected %g\n",mode,phase,x,y,actual,want);
                REQUIRE(fabsf(actual-want)<=1e-6f);++pixels_checked;
            }
            REQUIRE(!memcmp((char*)map.data+1280,(char*)map.data+1792,256));
            memcpy(reference[mode][replay],(char*)map.data+1024,1024);uint32_t prior=mode==2||mode==4||mode==8?0:mode==3||mode==5?1:UINT32_MAX;
            if(prior!=UINT32_MAX&&valid[prior])REQUIRE(!memcmp(reference[prior][replay],(char*)map.data+1024,1024));
            REQUIRE(!memcmp(before,map.data,1024));for(uint64_t i=2048;i<md.size;++i)REQUIRE(((unsigned char*)map.data)[i]==0xa5);
        }
        printf("Sample locations mode %u PASS: dynamic-pattern=%u dynamic-enable=%u grid=%ux%u.\n",mode,dp,de,positions.grid[0],positions.grid[1]);valid[mode]=1;++tested;
        TRY(ogpu_next_arena_reset(arena));for(uint32_t i=0;i<2;++i){ogpu_next_executable_destroy(pipeline[i]);pipeline[i]=NULL;}
    }
    for(uint32_t bad=0;bad<5;++bad) {
        ogpu_next_sample_location coords[4]={{.25f,.5f},{.25f,.5f},{.25f,.5f},{.25f,.5f}};
        ogpu_next_sample_locations p={HEADER(ogpu_next_sample_locations,OGPU_NEXT_SAMPLE_LOCATIONS),4,{1,1},4,coords};
        ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_arena_reset(arena));TRY(ogpu_next_commands_begin(arena,&rd,&e));
        if(bad==0){p.count=3;ogpu_next_set_sample_locations(e,&p);}
        else if(bad==1)ogpu_next_set_sample_locations_enable(e,2);
        else {
            if(bad==3){p.samples=2;p.count=2;}if(bad==4)p.header.next=&p.header;
            ogpu_next_image_barrier b={{0,OGPU_NEXT_STAGE_COPY,0,OGPU_NEXT_ACCESS_COPY_READ},images[bad==2?0:2],{bad==2?OGPU_NEXT_ASPECT_COLOR:OGPU_NEXT_ASPECT_DEPTH,0,1,0,1},OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0,&p.header};
            ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&b};ogpu_next_barrier(e,&dep,bs);
        }
        REQUIRE(ogpu_next_commands_end(e,&list)!=OGPU_NEXT_OK&&list==NULL);++rejected;
    }
    printf("Sample locations PASS: %u profiles, %" PRIu64 " submissions, %" PRIu64 " color/depth pixels, %u rejected preparations/recordings and guards.\n",tested,serial,pixels_checked,rejected);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Pending sample-location work after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);ogpu_next_split_destroy(split);for(uint32_t i=0;i<2;++i)ogpu_next_executable_destroy(pipeline[i]);ogpu_next_argument_layout_destroy(arguments);
    for(uint32_t i=0;i<4;++i){ogpu_next_view_destroy(views[i]);ogpu_next_image_destroy(images[i]);ogpu_next_memory_destroy(backing[i]);}ogpu_next_memory_destroy(memory);
    if(file){fclose(file);}for(uint32_t i=0;i<2;++i)free(code[i]);free(scratch);free(render_scratch);free(submit_scratch);free(split_scratch);return result;
}
