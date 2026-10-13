/* Independent Vulkan recording/state. Only shaders and frozen input equations
 * match the foundation consumer; no OGPU execution calls/linkage. The inherited
 * native loader fixture still includes its old generated host data declarations. */
#define SCENE_TEXTURE_NATIVE
#define SCENE_RASTER_BYTES 24u
#include "native.c"
#define TEXTURE_FUNCTIONS(X) X(vkGetPhysicalDeviceFormatProperties) X(vkWriteResourceDescriptorsEXT) \
    X(vkWriteSamplerDescriptorsEXT) X(vkCmdBindResourceHeapEXT) X(vkCmdBindSamplerHeapEXT) X(vkCmdCopyMemoryToImageKHR)
typedef struct {
    Scene scene;
    NativeImage texture;
    NativeBuffer upload,heaps[2];
    VkPhysicalDeviceDescriptorHeapPropertiesEXT limits;
    VkBindHeapInfoEXT bindings[2];
    void *heap_bytes[2];
#define DECLARE_TEXTURE(name) PFN_##name name;
    TEXTURE_FUNCTIONS(DECLARE_TEXTURE)
#undef DECLARE_TEXTURE
} TexturedNative;
typedef struct { uint64_t vertices; uint32_t image,sampler; float lod; uint32_t overlay; } TextureRoot;
_Static_assert(sizeof(TextureRoot)==24 && offsetof(TextureRoot,image)==8 && offsetof(TextureRoot,sampler)==12
    && offsetof(TextureRoot,lod)==16 && offsetof(TextureRoot,overlay)==20,"native texture root");
static uint64_t texture_align(uint64_t n,uint64_t a) { return (n+a-1)/a*a; }
static int texture_create(TexturedNative *t) {
    Native *n=t->scene.n;
    PFN_vkGetInstanceProcAddr get=(PFN_vkGetInstanceProcAddr)dlsym(n->library,"vkGetInstanceProcAddr");NEED(get);
#define LOAD_TEXTURE(name) t->name=(PFN_##name)get(n->instance,#name);NEED(t->name);
    TEXTURE_FUNCTIONS(LOAD_TEXTURE)
#undef LOAD_TEXTURE
    VkFormatProperties format;t->vkGetPhysicalDeviceFormatProperties(n->physical,VK_FORMAT_R8G8B8A8_UNORM,&format);
    NEED(format.optimalTilingFeatures&VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT);
    VkImageCreateInfo image={.sType=VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,.imageType=VK_IMAGE_TYPE_2D,
        .format=VK_FORMAT_R8G8B8A8_UNORM,.extent={8,8,1},.mipLevels=4,.arrayLayers=1,
        .samples=VK_SAMPLE_COUNT_1_BIT,.tiling=VK_IMAGE_TILING_OPTIMAL,.usage=VK_IMAGE_USAGE_SAMPLED_BIT|VK_IMAGE_USAGE_TRANSFER_DST_BIT};
    VK_TRY(n->vkCreateImage(n->device,&image,NULL,&t->texture.image));
    VkMemoryRequirements req;n->vkGetImageMemoryRequirements(n->device,t->texture.image,&req);
    NEED(native_image_type(&n->memory,req.memoryTypeBits,&t->texture.type));
    VkMemoryDedicatedAllocateInfo dedicated={.sType=VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,.image=t->texture.image};
    VkMemoryAllocateInfo allocate={.sType=VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,.pNext=&dedicated,.allocationSize=req.size,.memoryTypeIndex=t->texture.type};
    VK_TRY(n->vkAllocateMemory(n->device,&allocate,NULL,&t->texture.memory));
    VK_TRY(n->vkBindImageMemory(n->device,t->texture.image,t->texture.memory,0));
    NEED(native_buffer_create(n,&t->upload,512,1));
    t->limits.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_PROPERTIES_EXT;
    VkPhysicalDeviceProperties2 properties={.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,.pNext=&t->limits};
    n->vkGetPhysicalDeviceProperties2(n->physical,&properties);
    for(unsigned i=0;i<2;++i) {
        uint64_t alignment=i ? t->limits.samplerHeapAlignment : t->limits.resourceHeapAlignment;
        uint64_t descriptor=i ? t->limits.samplerDescriptorSize : t->limits.imageDescriptorSize;
        uint64_t tail=i ? t->limits.minSamplerHeapReservedRange : t->limits.minResourceHeapReservedRange;
        NEED(alignment && descriptor);
        uint64_t reserved_alignment=i ? t->limits.samplerDescriptorAlignment :
            t->limits.imageDescriptorAlignment>t->limits.bufferDescriptorAlignment ?
            t->limits.imageDescriptorAlignment : t->limits.bufferDescriptorAlignment;
        NEED(reserved_alignment);
        uint64_t reserved=texture_align(descriptor,reserved_alignment),size=reserved+tail;
        NEED(native_buffer_create_usage(n,&t->heaps[i],size+alignment,1,VK_BUFFER_USAGE_DESCRIPTOR_HEAP_BIT_EXT));
        uint64_t offset=texture_align(t->heaps[i].address,alignment)-t->heaps[i].address;
        t->heap_bytes[i]=(char *)t->heaps[i].mapped+offset;
        t->bindings[i]=(VkBindHeapInfoEXT){.sType=VK_STRUCTURE_TYPE_BIND_HEAP_INFO_EXT,
            .heapRange={t->heaps[i].address+offset,size},.reservedRangeOffset=reserved,.reservedRangeSize=tail};
    }
    return 1;
}
static int texture_update(TexturedNative *t,unsigned mode,unsigned phase) {
    Native *n=t->scene.n;NEED(!n->in_flight);
    unsigned first=mode==2 || mode==5,linear=mode==3 || mode==5;
    VkImageViewCreateInfo view={.sType=VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,.image=t->texture.image,
        .viewType=VK_IMAGE_VIEW_TYPE_2D,.format=VK_FORMAT_R8G8B8A8_UNORM,
        .subresourceRange={VK_IMAGE_ASPECT_COLOR_BIT,first,4-first,0,1}};
    VkImageDescriptorInfoEXT image={.sType=VK_STRUCTURE_TYPE_IMAGE_DESCRIPTOR_INFO_EXT,.pView=&view,.layout=VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL};
    VkResourceDescriptorInfoEXT resource={.sType=VK_STRUCTURE_TYPE_RESOURCE_DESCRIPTOR_INFO_EXT,.type=VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE,.data.pImage=&image};
    VkHostAddressRangeEXT out={t->heap_bytes[0],t->limits.imageDescriptorSize};
    VK_TRY(t->vkWriteResourceDescriptorsEXT(n->device,1,&resource,&out));
    VkSamplerCreateInfo sampler={.sType=VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,.magFilter=linear ? VK_FILTER_LINEAR : VK_FILTER_NEAREST,
        .minFilter=linear ? VK_FILTER_LINEAR : VK_FILTER_NEAREST,.mipmapMode=linear ? VK_SAMPLER_MIPMAP_MODE_LINEAR : VK_SAMPLER_MIPMAP_MODE_NEAREST,
        .addressModeU=VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,.addressModeV=VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
        .addressModeW=VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,.maxLod=3,.maxAnisotropy=1};
    out=(VkHostAddressRangeEXT){t->heap_bytes[1],t->limits.samplerDescriptorSize};
    VK_TRY(t->vkWriteSamplerDescriptorsEXT(n->device,1,&sampler,&out));
    NEED(native_cache(n,&t->heaps[0],1) && native_cache(n,&t->heaps[1],1));
    unsigned char data[512];memset(data,0xa5,sizeof(data));unsigned offset=64;
    for(unsigned mip=0;mip<4;++mip)for(unsigned y=0;y<(8u>>mip);++y)for(unsigned x=0;x<(8u>>mip);++x) {
        data[offset++]=(32+13*x+7*y+23*mip+17*phase)%192;
        data[offset++]=(48+5*x+19*y+31*mip+11*phase)%192;
        data[offset++]=(64+17*x+3*y+29*mip+13*phase)%192;data[offset++]=255;
    }
    return native_write(n,&t->upload,0,data,sizeof(data));
}
static int texture_pass(TexturedNative *t,unsigned mode,int overlay) {
    Scene *s=&t->scene;Native *n=s->n;VkCommandBuffer command=s->batch.command;
    VkRenderingAttachmentInfo color={.sType=VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO,.imageView=s->color.view,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL,.loadOp=overlay ? VK_ATTACHMENT_LOAD_OP_LOAD : VK_ATTACHMENT_LOAD_OP_CLEAR,
        .storeOp=VK_ATTACHMENT_STORE_OP_STORE,.clearValue.color.float32={0,0,0,1}};
    VkRenderingAttachmentInfo depth={.sType=VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO,.imageView=s->depth.view,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL,.loadOp=overlay ? VK_ATTACHMENT_LOAD_OP_LOAD : VK_ATTACHMENT_LOAD_OP_CLEAR,
        .storeOp=VK_ATTACHMENT_STORE_OP_STORE,.clearValue.depthStencil={1,0}};
    VkRenderingInfo render={.sType=VK_STRUCTURE_TYPE_RENDERING_INFO,.renderArea.extent={s->width,s->height},
        .layerCount=1,.colorAttachmentCount=1,.pColorAttachments=&color,.pDepthAttachment=&depth};
    n->vkCmdBeginRendering(command,&render);n->vkCmdBindPipeline(command,VK_PIPELINE_BIND_POINT_GRAPHICS,s->raster[overlay].pipeline);
    unsigned inset=mode>=4,vx=inset ? s->width/8 : 0,vy=inset ? s->height/8 : 0;
    VkViewport vp={(float)vx,(float)vy,(float)(s->width-2*vx),(float)(s->height-2*vy),0,1};
    VkRect2D scissor={.offset={overlay || inset ? (int32_t)(s->width/4) : 0,
        overlay ? (int32_t)(s->height/3) : inset ? (int32_t)(s->height/4) : 0},
        .extent={overlay || inset ? s->width/2 : s->width,overlay ? s->height/3 : inset ? s->height/2 : s->height}};
    n->vkCmdSetViewport(command,0,1,&vp);n->vkCmdSetScissor(command,0,1,&scissor);
    VkBindIndexBuffer3InfoKHR index={.sType=VK_STRUCTURE_TYPE_BIND_INDEX_BUFFER_3_INFO_KHR,.addressRange={s->indices.address+GUARD,32},
        .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,.indexType=VK_INDEX_TYPE_UINT32};s->bind_index(command,&index);
    TextureRoot root={s->vertices.address+GUARD,0,0,mode==1 ? 1.f : mode==3 || mode==5 ? .5f : 0.f,(uint32_t)overlay};
    NEED(native_push(n,&s->batch,&s->raster[overlay],&root,sizeof(root)));
    for(unsigned i=0;i<(overlay ? 1u : 2u);++i) {
        VkDrawIndirect2InfoKHR draw={.sType=VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,.addressRange={s->draws.address+GUARD+20*i,20,20},
            .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,.drawCount=1};s->draw_indexed(command,&draw);
    }
    n->vkCmdEndRendering(command);return 1;
}
static int texture_frame(TexturedNative *t,unsigned mode,unsigned frame,const char *directory) {
    Scene *s=&t->scene;Native *n=s->n;NativeBatch *batch=&s->batch;
    NEED(texture_update(t,mode,frame==1) && native_begin(n,batch));
    VkImageMemoryBarrier2 barrier={.sType=VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
        .srcStageMask=VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT,.srcAccessMask=VK_ACCESS_2_SHADER_SAMPLED_READ_BIT,
        .dstStageMask=VK_PIPELINE_STAGE_2_COPY_BIT,.dstAccessMask=VK_ACCESS_2_TRANSFER_WRITE_BIT,
        .oldLayout=VK_IMAGE_LAYOUT_UNDEFINED,.newLayout=VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
        .srcQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,.dstQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,
        .image=t->texture.image,.subresourceRange={VK_IMAGE_ASPECT_COLOR_BIT,0,4,0,1}};
    VkDependencyInfo dep={.sType=VK_STRUCTURE_TYPE_DEPENDENCY_INFO,.imageMemoryBarrierCount=1,.pImageMemoryBarriers=&barrier};
    n->vkCmdPipelineBarrier2(batch->command,&dep);unsigned offset=64;
    for(unsigned mip=0;mip<4;++mip) {
        unsigned width=8u>>mip,size=width*width*4;
        VkDeviceMemoryImageCopyKHR region={.sType=VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
            .addressRange={t->upload.address+offset,size},.addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
            .imageLayout=VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,.imageSubresource={VK_IMAGE_ASPECT_COLOR_BIT,mip,0,1},.imageExtent={width,width,1}};
        VkCopyDeviceMemoryImageInfoKHR copy={.sType=VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_IMAGE_INFO_KHR,.image=t->texture.image,.regionCount=1,.pRegions=&region};
        t->vkCmdCopyMemoryToImageKHR(batch->command,&copy);offset+=size;
    }
    barrier.srcStageMask=VK_PIPELINE_STAGE_2_COPY_BIT;barrier.srcAccessMask=VK_ACCESS_2_TRANSFER_WRITE_BIT;
    barrier.dstStageMask=VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT;barrier.dstAccessMask=VK_ACCESS_2_SHADER_SAMPLED_READ_BIT;
    barrier.oldLayout=VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;barrier.newLayout=VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
    n->vkCmdPipelineBarrier2(batch->command,&dep);
    native_barrier(n,batch->command,VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,VK_ACCESS_2_MEMORY_READ_BIT|VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,VK_ACCESS_2_SHADER_WRITE_BIT);
    SceneRoot root={s->vertices.address+GUARD,s->indices.address+GUARD,s->draws.address+GUARD,frame==1,0};
    NEED(native_dispatch(n,batch,&s->prepare,(Launch){.x=1,.y=1},&root,sizeof(root)));
    native_barrier(n,batch->command,VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_INDEX_INPUT_BIT|VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT|VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT,
        VK_ACCESS_2_INDEX_READ_BIT|VK_ACCESS_2_SHADER_READ_BIT|VK_ACCESS_2_INDIRECT_COMMAND_READ_BIT);
    scene_transition(s);t->vkCmdBindResourceHeapEXT(batch->command,&t->bindings[0]);t->vkCmdBindSamplerHeapEXT(batch->command,&t->bindings[1]);
    NEED(texture_pass(t,mode,0));
    native_barrier(n,batch->command,VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT|VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT|VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
        VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT|VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT|VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT|VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
        VK_ACCESS_2_COLOR_ATTACHMENT_READ_BIT|VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT|VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_READ_BIT);
    NEED(texture_pass(t,mode,1) && native_image_readback(n,batch,&s->color,&s->readback,GUARD));
    VkDeviceMemoryImageCopyKHR region={.sType=VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
        .addressRange={s->readback.address+s->pixels+3*GUARD,s->pixels},.addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL,.imageSubresource={VK_IMAGE_ASPECT_DEPTH_BIT,0,0,1},.imageExtent={s->width,s->height,1}};
    VkCopyDeviceMemoryImageInfoKHR copy={.sType=VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_IMAGE_INFO_KHR,.image=s->depth.image,.regionCount=1,.pRegions=&region};
    n->vkCmdCopyImageToMemoryKHR(batch->command,&copy);
    NEED(native_copy(n,batch,&s->vertices,0,&s->geometry,0,256) && native_copy(n,batch,&s->indices,0,&s->geometry,256,160)
        && native_copy(n,batch,&s->draws,0,&s->geometry,416,168) && native_submit(n,batch) && native_wait(n,batch));
    native_batch_destroy(n,batch);
    char path[4096],name[128];size_t size=2*s->pixels+4*GUARD;
    NEED(native_read(n,&s->readback,0,s->cpu,size));snprintf(name,sizeof(name),"mode-%u-frame-%u.images",mode,frame);
    NEED(native_path(path,directory,name) && native_file(path,s->cpu,size,1));
    NEED(native_read(n,&s->geometry,0,s->cpu,584));snprintf(name,sizeof(name),"mode-%u-frame-%u.geometry",mode,frame);
    NEED(native_path(path,directory,name) && native_file(path,s->cpu,584,1));
    NEED(native_read(n,&t->upload,0,s->cpu,512));snprintf(name,sizeof(name),"mode-%u-frame-%u.texture",mode,frame);
    NEED(native_path(path,directory,name) && native_file(path,s->cpu,512,1));
    printf("TEXTURED_FRAME {\"mode\":%u,\"frame\":%u,\"phase\":%u}\n",mode,frame,frame==1);return 1;
}
int main(int argc,char **argv) {
    if(argc!=5)return 1;
    TexturedNative t={0};Scene *s=&t.scene;s->n=&s->owned;s->index_bytes=4;int okay=0;
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")){s->width=257;s->height=193;}
    else if(!strcmp(argv[1],"640") && !strcmp(argv[2],"360")){s->width=640;s->height=360;}
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")){s->width=1280;s->height=720;}
    else return 1;
    s->pixels=(size_t)s->width*s->height*4;
    if(!scene_create_context(s,argv[3]) || !scene_create_resources(s) || !texture_create(&t))goto done;
    for(unsigned mode=0;mode<6;++mode)for(unsigned frame=0;frame<3;++frame)if(!texture_frame(&t,mode,frame,argv[4]))goto done;
    okay=1;
done:
    scene_destroy_resources(s);
    native_image_destroy(s->n,&t.texture);native_buffer_destroy(s->n,&t.upload);
    for(unsigned i=0;i<2;++i)native_buffer_destroy(s->n,&t.heaps[i]);
    scene_destroy_context(s);
    if(okay)puts("Native textured frames drained; independent oracle required");
    return !okay;
}
