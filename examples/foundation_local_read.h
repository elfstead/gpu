/* Two draws in one render instance: attachment and same-pixel address storage.
 * Locations change while attachment order/formats stay fixed. No render-pass break. */
static int local_read_execution(ogpu_next_device *device, ogpu_next_memory_desc md,
                                uint32_t domain, const char *const paths[3]) {
    int result=EXIT_FAILURE, pending=0;
    FILE *file=NULL; void *code[3]={NULL}, *scratch=NULL, *render_scratch=NULL, *submit_scratch=NULL;
    ogpu_next_memory *memory=NULL, *heap_memory=NULL, *backing[2]={NULL};
    ogpu_next_image *images[2]={NULL}; ogpu_next_view *views[2]={NULL};
    ogpu_next_executable *pipelines[2]={NULL}; ogpu_next_argument_layout *arguments=NULL;
    ogpu_next_descriptor_heap *heap=NULL; ogpu_next_arena *arena=NULL; ogpu_next_timeline *done=NULL;
    ogpu_next_feature_info features={0}; ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q)); uint32_t local=0;
    for (uint32_t i=0;i<features.enabled.count;++i) local |= features.enabled.ids[i]==OGPU_NEXT_FEATURE_LOCAL_READ;
    if (!local || !(fixture_mask(features.enabled)&FIXTURE_BIT(OGPU_NEXT_FEATURE_FRAGMENT_STORES_ATOMICS))) {
        puts("Local read NOT exercised: native local-read/fragment-store capability unavailable."); return EXIT_SUCCESS;
    }
    ogpu_next_local_read_limits limits={0}; q=query(OGPU_NEXT_QUERY_LOCAL_READ_LIMITS,&limits,1,sizeof(limits));
    TRY(ogpu_next_device_query(device,&q)); REQUIRE(limits.max_inputs>=1);
    printf("Local read native limits: inputs=%u depth/stencil=%u multisample=%u.\n",limits.max_inputs,limits.depth_stencil,limits.multisample);
    ogpu_next_argument_field field={0,16,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,16,1,0,&field};
    TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    ogpu_next_shader shaders[3];
    const char *names[3]={"vertexMain","produce","consume"};
    for (uint32_t i=0;i<3;++i) {
        file=fopen(paths[i],"rb"); REQUIRE(file); REQUIRE(fseek(file,0,SEEK_END)==0);
        long n=ftell(file); REQUIRE(n>=20 && n%4==0); REQUIRE(fseek(file,0,SEEK_SET)==0);
        code[i]=malloc((size_t)n); REQUIRE(code[i]); REQUIRE(fread(code[i],1,(size_t)n,file)==(size_t)n); fclose(file); file=NULL;
        shaders[i]=(ogpu_next_shader){i?OGPU_NEXT_STAGE_FRAGMENT:OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)n},names[i],NULL,NULL};
    }
    TRY(ogpu_next_memory_create(device,&md,&memory)); REQUIRE(md.size>=2048);
    ogpu_next_span all={memory,0,md.size}; ogpu_next_mapping map={0}; uint64_t address=0;
    TRY(ogpu_next_memory_map(all,&map)); TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,
        1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT|OGPU_NEXT_IMAGE_INPUT_ATTACHMENT|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements ir={.compatible_types=types,.compatible_type_capacity=32};
    TRY(ogpu_next_image_requirements(device,&id,&ir)); REQUIRE(ir.compatible_type_count);
    ogpu_next_subresources range={OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,
        OGPU_NEXT_IMAGE_COLOR_ATTACHMENT|OGPU_NEXT_IMAGE_INPUT_ATTACHMENT,{0},range};
    for (uint32_t i=0;i<2;++i) {
        TRY(ogpu_next_image_create_unbound(device,&id,&images[i])); TRY(ogpu_next_memory_create_dedicated_image(images[i],types[0],&backing[i]));
        TRY(ogpu_next_image_bind(images[i],(ogpu_next_span){backing[i],0,ir.size})); TRY(ogpu_next_view_create(images[i],&vd,&views[i]));
    }
    ogpu_next_resource_descriptor resource={OGPU_NEXT_DESCRIPTOR_INPUT_ATTACHMENT,OGPU_NEXT_STATE_GENERAL,views[0],{NULL,0,0}};
    ogpu_next_descriptor_shape shape={0}; ogpu_next_heap_requirements hr={0};
    TRY(ogpu_next_resource_descriptor_shape(device,&resource,&shape));
    TRY(heap_requirements_for(device,OGPU_NEXT_HEAP_RESOURCE,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,&hr));
    ogpu_next_memory_limits ml={0}; q=query(OGPU_NEXT_QUERY_MEMORY_LIMITS,&ml,1,sizeof(ml)); TRY(ogpu_next_device_query(device,&q));
    uint64_t stride=aligned(shape.size,shape.alignment), reserved_alignment=hr.reserved_alignment;
    if (reserved_alignment<ml.cache_atom_size) reserved_alignment=ml.cache_atom_size;
    uint64_t reserved_offset=aligned(stride*2,reserved_alignment);
    ogpu_next_memory_desc hm=md; hm.size=reserved_offset+hr.reserved_size; hm.usage=OGPU_NEXT_USAGE_DESCRIPTOR_HEAP; hm.kind=OGPU_NEXT_MEMORY_LINEAR;
    hm.alignment=hr.backing_alignment>ml.cache_atom_size?hr.backing_alignment:ml.cache_atom_size;
    ogpu_next_memory_type_info heap_types[32]; q=query(OGPU_NEXT_QUERY_MEMORY_TYPES,heap_types,32,sizeof(heap_types[0]));
    TRY(ogpu_next_device_query(device,&q)); uint32_t heap_type_count=q.count;
    ogpu_next_requirements heap_req={.compatible_types=types,.compatible_type_capacity=32};
    TRY(ogpu_next_memory_requirements(device,&hm,&heap_req)); hm.memory_type=UINT32_MAX;
    for (uint32_t j=0;j<heap_req.compatible_type_count;++j)
        for (uint32_t k=0;k<heap_type_count;++k)
            if (heap_types[k].id==types[j] && (heap_types[k].properties&OGPU_NEXT_MEMORY_HOST_VISIBLE)) hm.memory_type=types[j];
    REQUIRE(hm.memory_type!=UINT32_MAX);
    TRY(ogpu_next_memory_create(device,&hm,&heap_memory)); ogpu_next_mapping hmapped={0};
    TRY(ogpu_next_memory_map((ogpu_next_span){heap_memory,0,hm.size},&hmapped));
    ogpu_next_heap_desc hd={{OGPU_NEXT_HEAP_RESOURCE,OGPU_NEXT_HEAP_MEMORY,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,0,hm.size},
        {heap_memory,0,hm.size},{hmapped.data,hmapped.size},reserved_offset,hr.reserved_size};
    TRY(ogpu_next_heap_create(device,&hd,&heap));
    ogpu_next_host_requirements br={0},dr={0},rr={0},sr={0};
    TRY(ogpu_next_barrier_scratch_requirements(device,1,1,2,&br));
    TRY(ogpu_next_descriptor_scratch_requirements(device,OGPU_NEXT_DESCRIPTOR_WRITE_RESOURCE,1,&dr));
    TRY(ogpu_next_render_scratch_requirements(2,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    uint64_t scratch_size=br.size>dr.size?br.size:dr.size;
    REQUIRE(br.alignment<=_Alignof(max_align_t) && dr.alignment<=_Alignof(max_align_t) && rr.alignment<=_Alignof(max_align_t) && sr.alignment<=_Alignof(max_align_t));
    scratch=malloc((size_t)scratch_size); render_scratch=malloc((size_t)rr.size); submit_scratch=malloc((size_t)sr.size);
    REQUIRE(scratch && render_scratch && submit_scratch);
    ogpu_next_host_span bs={scratch,scratch_size};
    uint32_t locs[2][2]={{0,OGPU_NEXT_ATTACHMENT_UNUSED},{OGPU_NEXT_ATTACHMENT_UNUSED,0}};
    uint32_t input_indices[2]={0,OGPU_NEXT_ATTACHMENT_UNUSED}, unused=OGPU_NEXT_ATTACHMENT_UNUSED;
    ogpu_next_render_inputs inputs={2,0,input_indices,&unused,&unused};
    ogpu_next_input_attachment_binding binding={0,0,0,(uint32_t)stride}; REQUIRE(stride<=UINT32_MAX);
    ogpu_next_render_locations locations={2,0,locs[0]};
    ogpu_next_render_mapping mapping={HEADER(ogpu_next_render_mapping,OGPU_NEXT_RENDER_MAPPING),&locations,&inputs,0,0,NULL};
    ogpu_next_color_state colors[2]={{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15},{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15}};
    ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_TRIANGLES,
        .front_face=OGPU_NEXT_FRONT_CCW,.samples=1,.color_count=2,.colors=colors}; gs.header.next=&mapping.header;
    uint32_t enables[]={OGPU_NEXT_FEATURE_RASTER,OGPU_NEXT_FEATURE_LOCAL_READ,OGPU_NEXT_FEATURE_FRAGMENT_STORES_ATOMICS};
    ogpu_next_shader_requirements requirements={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),{enables,3,0},{0},0};
    for (uint32_t i=0;i<2;++i) {
        ogpu_next_shader pair[2]={shaders[0],shaders[i+1]}; locations.locations=locs[i]; mapping.binding_count=i; mapping.bindings=i?&binding:NULL;
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,pair,&gs.header,
            OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&requirements.header,NULL,0,0};
        if (i==1) {
            ogpu_next_input_attachment_binding bad[2]={binding,binding};
            mapping.bindings=bad;
            for (uint32_t test=0;test<4;++test) {
                bad[0]=binding; mapping.binding_count=1;
                if (test==0) bad[0].root_offset=2;
                if (test==1) bad[0].root_offset=16;
                if (test==2) bad[0].array_stride=1;
                if (test==3) mapping.binding_count=2;
                ogpu_next_executable *rejected=NULL;
                REQUIRE(ogpu_next_executable_create(device,&ed,arguments,&rejected)==OGPU_NEXT_INVALID);
                REQUIRE(rejected==NULL);
            }
            mapping.binding_count=1; mapping.bindings=&binding;
        }
        TRY(ogpu_next_executable_create(device,&ed,arguments,&pipelines[i]));
    }
    ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0;
    for (uint32_t mode=0;mode<4;++mode) {
        uint32_t state=mode&1?OGPU_NEXT_STATE_LOCAL_READ:OGPU_NEXT_STATE_GENERAL;
        resource.image_state=state; ogpu_next_heap_location output={heap,stride};
        TRY(ogpu_next_write_resource_descriptors(device,1,&resource,&output,bs));
        TRY(ogpu_next_memory_flush((ogpu_next_span){heap_memory,stride,shape.size}));
        TRY(ogpu_next_arena_reset(arena)); ogpu_next_encoder *e=NULL; ogpu_next_list *list=NULL;
        TRY(ogpu_next_commands_begin(arena,&rd,&e));
        ogpu_next_image_barrier ib[2];
        for (uint32_t i=0;i<2;++i) ib[i]=(ogpu_next_image_barrier){{0,OGPU_NEXT_STAGE_COLOR,0,OGPU_NEXT_ACCESS_COLOR_WRITE},images[i],range,
            OGPU_NEXT_STATE_UNDEFINED,state,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0};
        ogpu_next_dependency dep={0,0,2,0,NULL,NULL,ib}; ogpu_next_barrier(e,&dep,bs);
        ogpu_next_attachment a[2];
        for (uint32_t i=0;i<2;++i) a[i]=(ogpu_next_attachment){views[i],NULL,state,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
        ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=1,.color_count=2,.colors=a,.scratch={render_scratch,rr.size}};
        ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
        ogpu_next_draw_desc draw={3,1,0,0,0};
        ogpu_next_render_begin(e,&render); ogpu_next_bind_heap(e,heap); ogpu_next_set_graphics_state(e,&vp.header);
        ogpu_next_set_render_inputs(e,&inputs);
        locations.locations=locs[0]; ogpu_next_set_render_locations(e,&locations); ogpu_next_bind_executable(e,pipelines[0]);
        /* Each recorded list is replayed twice with the same roots and fresh host data. */
        struct {uint32_t offset,seed;uint64_t address;} roots={(uint32_t)stride,3+mode,address+512};
        ogpu_next_set_arguments(e,arguments,0,sizeof(roots),&roots); ogpu_next_draw(e,&draw);
        ogpu_next_barrier_scope global={OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_STAGE_FRAGMENT,
            OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ|OGPU_NEXT_ACCESS_STORAGE_READ};
        ogpu_next_memory_barrier mb={{OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_STORAGE_READ},
            {memory,512,256},OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED};
        ib[0]=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ},images[0],range,state,state,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0};
        dep=(ogpu_next_dependency){mode<2?1u:0u,mode<2?0u:1u,mode<2?0u:1u,OGPU_NEXT_DEPENDENCY_BY_REGION,mode<2?&global:NULL,mode<2?NULL:&mb,mode<2?NULL:ib};
        ogpu_next_barrier(e,&dep,bs);
        locations.locations=locs[1]; ogpu_next_set_render_locations(e,&locations); ogpu_next_bind_executable(e,pipelines[1]); ogpu_next_draw(e,&draw);
        ogpu_next_render_end(e);
        for (uint32_t i=0;i<2;++i) ib[i]=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_STAGE_COPY,
            OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ,OGPU_NEXT_ACCESS_COPY_READ},images[i],range,state,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0};
        dep=(ogpu_next_dependency){0,0,2,0,NULL,NULL,ib}; ogpu_next_barrier(e,&dep,bs);
        ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
        for (uint32_t i=0;i<2;++i) ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024+i*256,256},images[i],&copy);
        global=(ogpu_next_barrier_scope){OGPU_NEXT_STAGE_COPY|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE|OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_HOST_READ};
        dep=(ogpu_next_dependency){1,0,0,0,&global,NULL,NULL}; ogpu_next_barrier(e,&dep,bs);
        TRY(ogpu_next_commands_end(e,&list));
        ogpu_next_sync_point signal={{done,0},OGPU_NEXT_STAGE_ALL}; ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL}; ogpu_next_submit_outcome outcome={0};
        for (uint32_t replay=0;replay<2;++replay) {
            memset(map.data,0xa5,(size_t)md.size); TRY(ogpu_next_memory_flush(all)); signal.point.value=++serial; pending=1;
            TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submit_scratch,sr.size}));
            TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending=0; TRY(ogpu_next_memory_invalidate(all));
            for (uint32_t i=0;i<64;++i) {
                uint32_t x=i%8,y=i/8,seed=roots.seed;
                REQUIRE(((uint32_t*)((char*)map.data+512))[i]==seed+3*i);
                unsigned char *src=(unsigned char*)map.data+1024+i*4,*dst=src+256;
                REQUIRE(src[0]==x+seed && src[1]==y && src[2]==37 && src[3]==255);
                if (!(dst[0]==37 && dst[1]==((seed+3*i)&255) && dst[2]==x+seed && dst[3]==255))
                    fprintf(stderr,"local mode %u pixel %u: %u %u %u %u\n",mode,i,dst[0],dst[1],dst[2],dst[3]);
                REQUIRE(dst[0]==37 && dst[1]==((seed+3*i)&255) && dst[2]==x+seed && dst[3]==255);
            }
            for (uint64_t i=0;i<md.size;++i) if (!(i>=512 && i<768) && !(i>=1024 && i<1536)) REQUIRE(((unsigned char*)map.data)[i]==0xa5);
        }
    }
    puts("Local read PASS: 8 submissions, 1024 attachment pixels, 512 storage values, GENERAL/LOCAL_READ, global/resource region barriers and guards.");
    /* Malformed local commands must poison end without reaching native recording. */
    for (uint32_t test=0;test<16;++test) {
        TRY(ogpu_next_arena_reset(arena)); ogpu_next_encoder *e=NULL; ogpu_next_list *list=NULL;
        TRY(ogpu_next_commands_begin(arena,&rd,&e));
        ogpu_next_attachment a[2];
        for (uint32_t i=0;i<2;++i) a[i]=(ogpu_next_attachment){views[i],NULL,OGPU_NEXT_STATE_GENERAL,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
        ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=1,.color_count=2,.colors=a,.scratch={render_scratch,rr.size}};
        if (test==14) render.flags=OGPU_NEXT_RENDER_SECONDARY;
        if (test!=0 && test!=15) ogpu_next_render_begin(e,&render);
        uint32_t values[2]={0,OGPU_NEXT_ATTACHMENT_UNUSED};
        ogpu_next_render_locations l={2,0,values};
        ogpu_next_render_inputs in={2,0,values,&unused,&unused};
        ogpu_next_status expected=OGPU_NEXT_INVALID;
        if (test<=4 || test==14) {
            if (test==1) l.color_count=1;
            if (test==2) l.reserved=1;
            if (test==3) values[1]=0;
            if (test==4) { values[0]=UINT32_MAX-1; expected=OGPU_NEXT_UNSUPPORTED; }
            ogpu_next_set_render_locations(e,&l);
        } else if (test<=7) {
            if (test==5) values[1]=0;
            if (test==6) { values[0]=limits.max_inputs; expected=OGPU_NEXT_UNSUPPORTED; }
            if (test==7) in.depth=&values[0];
            ogpu_next_set_render_inputs(e,&in);
        } else {
            ogpu_next_barrier_scope scope={OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ};
            ogpu_next_image_barrier image={scope,images[0],range,OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0};
            ogpu_next_dependency dep={1,0,0,OGPU_NEXT_DEPENDENCY_BY_REGION,&scope,NULL,NULL};
            if (test==8) dep.flags=0;
            if (test==9) { scope.stages_before=OGPU_NEXT_STAGE_COMPUTE; scope.access_before=OGPU_NEXT_ACCESS_STORAGE_WRITE; }
            if (test>=10 && test<=12) { dep.global_count=0; dep.image_count=1; dep.images=&image; }
            if (test==10) image.new_state=OGPU_NEXT_STATE_LOCAL_READ;
            if (test==11) image.discard=1;
            if (test==12) image.source_domain=domain;
            if (test==13) { dep.flags=4; expected=OGPU_NEXT_UNSUPPORTED; }
            if (test==15) dep.flags=OGPU_NEXT_DEPENDENCY_VIEW_LOCAL;
            ogpu_next_barrier(e,&dep,bs);
        }
        REQUIRE(ogpu_next_commands_end(e,&list)==expected); REQUIRE(list==NULL);
    }
    puts("Local read rejection PASS: 4 executable mappings and 16 poisoned recordings; no malformed work submitted.");
    result=EXIT_SUCCESS;
cleanup:
    if (pending) { fputs("Pending local-read work after failure.\n",stderr); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done);
    for (uint32_t i=0;i<2;++i) ogpu_next_executable_destroy(pipelines[i]);
    ogpu_next_argument_layout_destroy(arguments); ogpu_next_heap_destroy(heap); ogpu_next_memory_destroy(heap_memory);
    for (uint32_t i=0;i<2;++i) {ogpu_next_view_destroy(views[i]);ogpu_next_image_destroy(images[i]);ogpu_next_memory_destroy(backing[i]);}
    ogpu_next_memory_destroy(memory); if(file)fclose(file);
    for(uint32_t i=0;i<3;++i) { free(code[i]); }
    free(scratch);free(render_scratch);free(submit_scratch);
    return result;
}
