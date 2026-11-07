/* Bounded independent 4-sample oracle: distinct per-sample colors/depth are
 * reduced through native input attachments, then the output is resolved. */
typedef struct {
    ogpu_next_image *images[4];ogpu_next_view *views[4];ogpu_next_memory *backing[4];
    ogpu_next_executable *pipelines[2];
} LocalMS;
static int advanced_local_ms_run(Textured *t,LocalMS *m) {
    Scene *s=&t->scene;
    ogpu_next_local_read_limits limits;ogpu_next_query q=query(OGPU_NEXT_QUERY_LOCAL_READ_LIMITS,&limits,1,sizeof(limits));
    GPU(ogpu_next_device_query(s->device,&q));NEED(limits.multisample);
    for(unsigned i=0;i<4;++i) {
        uint32_t format=i==1?OGPU_NEXT_D32_FLOAT:OGPU_NEXT_RGBA8_UNORM;
        uint64_t usage=i==1?OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT:OGPU_NEXT_IMAGE_COLOR_ATTACHMENT;
        if(i<2)usage|=OGPU_NEXT_IMAGE_INPUT_ATTACHMENT;
        if(i==3)usage|=OGPU_NEXT_IMAGE_COPY_SRC;
        ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),format,OGPU_NEXT_IMAGE_2D,1,1,i==3?1:4,{8,8,1},usage,0,NULL,0,0,NULL};
        uint32_t types[32],type;ogpu_next_requirements req={.compatible_types=types,.compatible_type_capacity=32};
        GPU(ogpu_next_image_requirements(s->device,&id,&req));NEED(memory_type(s,&req,OGPU_NEXT_MEMORY_LOCAL,&type));
        GPU(ogpu_next_image_create_unbound(s->device,&id,&m->images[i]));GPU(ogpu_next_memory_create_dedicated_image(m->images[i],type,&m->backing[i]));
        GPU(ogpu_next_image_bind(m->images[i],(ogpu_next_span){m->backing[i],0,req.size}));
        ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),format,OGPU_NEXT_VIEW_2D,usage&~OGPU_NEXT_IMAGE_COPY_SRC,{0},{i==1?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1}};
        GPU(ogpu_next_view_create(m->images[i],&vd,&m->views[i]));
    }
    for(unsigned i=0;i<2;++i) {
        ogpu_next_resource_descriptor input={OGPU_NEXT_DESCRIPTOR_INPUT_ATTACHMENT,OGPU_NEXT_STATE_LOCAL_READ,m->views[i],{0}};
        ogpu_next_heap_location location={t->bindings[0],t->local_stride*(i+1)};
        GPU(ogpu_next_write_resource_descriptors(s->device,1,&input,&location,t->scratch));
    }
    GPU(ogpu_next_memory_flush((ogpu_next_span){t->heaps[0],t->local_stride,2*t->local_stride}));
    uint32_t output_indices[2][2]={{0,OGPU_NEXT_ATTACHMENT_UNUSED},{OGPU_NEXT_ATTACHMENT_UNUSED,0}},input_indices[2]={0,OGPU_NEXT_ATTACHMENT_UNUSED},depth=1,unused=OGPU_NEXT_ATTACHMENT_UNUSED;
    ogpu_next_render_locations locations={2,0,output_indices[0]};ogpu_next_render_inputs inputs={2,0,input_indices,&depth,&unused};
    ogpu_next_input_attachment_binding bindings[2]={{0,0,0,(uint32_t)t->local_stride},{0,1,4,(uint32_t)t->local_stride}};
    ogpu_next_render_mapping mapping={HEADER(ogpu_next_render_mapping,OGPU_NEXT_RENDER_MAPPING),&locations,&inputs,0,0,NULL};
    ogpu_next_color_state colors[2]={{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15},{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15}};
    ogpu_next_raster_options options={.header=HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),.line_width=1,.max_depth_bounds=1,.sample_mask=UINT64_MAX,.sample_shading=1,.min_sample_shading=1};
    ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_TRIANGLES,.front_face=OGPU_NEXT_FRONT_CCW,.samples=4,
        .color_count=2,.colors=colors,.depth_format=OGPU_NEXT_D32_FLOAT,.depth_compare=OGPU_NEXT_COMPARE_ALWAYS,.raster_options=&options.header};gs.header.next=&mapping.header;
    uint32_t features[]={OGPU_NEXT_FEATURE_RASTER,OGPU_NEXT_FEATURE_LOCAL_READ,OGPU_NEXT_FEATURE_SAMPLE_SHADING};
    ogpu_next_shader_requirements requirements={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),{features,3,0},{0},0};
    for(unsigned i=0;i<2;++i) {
        locations.locations=output_indices[i];mapping.binding_count=i?2:0;mapping.bindings=i?bindings:NULL;gs.depth_test=gs.depth_write=i==0;
        ogpu_next_shader shaders[2]={advanced_local_vertex_shader(),i?advanced_local_ms_fragment_shader():advanced_local_produce_shader()};
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,
            OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&requirements.header,NULL,0,0};
        GPU(ogpu_next_executable_create(s->device,&ed,t->local_arguments,&m->pipelines[i]));
    }
    NEED(begin(s));
    for(unsigned i=0;i<4;++i) {
        ogpu_next_image_barrier b={{0,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,0,OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE},m->images[i],
            {i==1?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1},OGPU_NEXT_STATE_UNDEFINED,i<2?OGPU_NEXT_STATE_LOCAL_READ:OGPU_NEXT_STATE_GENERAL,
            OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
        ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&b};ogpu_next_barrier(s->encoder,&dep,(ogpu_next_host_span){s->bs,s->barrier_scratch.size});
    }
    ogpu_next_attachment attachments[2]={
        {m->views[0],NULL,OGPU_NEXT_STATE_LOCAL_READ,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}},
        {m->views[2],m->views[3],OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,OGPU_NEXT_ATTACHMENT_RESOLVE_AVERAGE,{.f32={0,0,0,1}}}};
    ogpu_next_attachment depth_attachment={m->views[1],NULL,OGPU_NEXT_STATE_LOCAL_READ,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={1,0}}};
    ogpu_next_render_desc rd={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=4,.color_count=2,
        .colors=attachments,.depth=&depth_attachment,.scratch={s->rs,s->render_scratch.size}};
    ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
    ogpu_next_render_begin(s->encoder,&rd);ogpu_next_bind_heap(s->encoder,t->bindings[0]);ogpu_next_set_graphics_state(s->encoder,&vp.header);
    ogpu_next_set_render_inputs(s->encoder,&inputs);
    uint32_t root[2]={(uint32_t)t->local_stride,(uint32_t)(2*t->local_stride)};
    ogpu_next_bind_executable(s->encoder,m->pipelines[0]);ogpu_next_set_arguments(s->encoder,t->local_arguments,0,sizeof(root),root);
    ogpu_next_draw_desc draw={3,1,0,0,0};
    for(unsigned i=0;i<2;++i) {
        if(i) {
            ogpu_next_barrier_scope scope={OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_STAGE_FRAGMENT,
                OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE,OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ};
            ogpu_next_dependency dep={1,0,0,OGPU_NEXT_DEPENDENCY_BY_REGION,&scope,NULL,NULL};
            ogpu_next_barrier(s->encoder,&dep,(ogpu_next_host_span){s->bs,s->barrier_scratch.size});
        }
        locations.locations=output_indices[i];ogpu_next_set_render_locations(s->encoder,&locations);ogpu_next_bind_executable(s->encoder,m->pipelines[i]);ogpu_next_draw(s->encoder,&draw);
    }
    ogpu_next_render_end(s->encoder);
    ogpu_next_image_barrier b={{OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_COPY_READ},m->images[3],
        {OGPU_NEXT_ASPECT_COLOR,0,1,0,1},OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0,NULL};
    ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&b};ogpu_next_barrier(s->encoder,&dep,(ogpu_next_host_span){s->bs,s->barrier_scratch.size});
    ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    ogpu_next_copy_from_image(s->encoder,(ogpu_next_span){t->local_readback,GUARD,256},m->images[3],&copy);
    dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ);
    NEED(submit_wait(s));GPU(ogpu_next_memory_invalidate((ogpu_next_span){t->local_readback,0,s->pixels+2*GUARD}));
    const unsigned char *p=t->local_map.data;
    for(unsigned i=0;i<64;++i){unsigned expected[4]={(unsigned)((i%8)*255.f/16+.5f),(unsigned)((i/8)*255.f/16+.5f),64,96};
        for(unsigned c=0;c<4;++c)NEED(abs((int)p[GUARD+4*i+c]-(int)expected[c])<=1);}
    for(size_t i=0;i<s->pixels+2*GUARD;++i)if(i<GUARD || i>=GUARD+256)NEED(p[i]==0xa5);
    puts("LOCAL_MSAA {\"samples\":4,\"pixels\":64,\"distinct_sample_values\":true,\"color_and_depth\":true}");return 1;
}
static int advanced_local_ms(Textured *t) {
    LocalMS m={0};int okay=advanced_local_ms_run(t,&m);Scene *s=&t->scene;
    if(s->pending){fputs("Unknown MSAA completion; refusing unsafe teardown\n",stderr);_Exit(EXIT_FAILURE);}
    if(s->encoder){ogpu_next_commands_cancel(s->encoder);s->encoder=NULL;}
    for(unsigned i=0;i<2;++i)ogpu_next_executable_destroy(m.pipelines[i]);
    for(unsigned i=0;i<4;++i){ogpu_next_view_destroy(m.views[i]);ogpu_next_image_destroy(m.images[i]);ogpu_next_memory_destroy(m.backing[i]);}
    return okay;
}
