/* Streaming application control. Reuse the tested slot submission/drain helpers,
 * not the small-compute workload. No runtime API changes. */
#define FRONTIER_NO_MAIN
#include "small.c"
#ifndef FRONTIER_NATIVE
#include "../learned_image/extent.h"
#include <application.generated.h>
#endif
#ifdef FRONTIER_REUSE
#include "../resource_reuse/ranges.h"
#include "../resource_reuse/slots.h"
#endif

enum { SI, SW, SH, SD, SC, SB_COUNT, SG = 64 };
#ifdef FRONTIER_NATIVE
#ifdef FRONTIER_REUSE
typedef struct {
    NativeBuffer native; /* Owning or borrowed parent metadata; owned decides teardown. */
    VkBuffer buffer;
    uint64_t offset, size;
    struct { void *data; uint64_t size_bytes, alignment, access_granularity; int coherent; } view;
    int owned;
} SBuffer;
#else
typedef NativeBuffer SBuffer;
#endif
typedef NativeImage SImage;
#else
#ifdef FRONTIER_REUSE
typedef struct {
    OgpuBuffer *buffer;
    uint64_t offset, size;
    OgpuHostView view;
    int owned;
} SBuffer;
#else
typedef OgpuBuffer *SBuffer;
#endif
typedef OgpuImage *SImage;
#endif
typedef struct {
    SBuffer data[SB_COUNT], upload, readback;
    SImage image;
    uint64_t address[SB_COUNT];
    unsigned input_index;
#ifdef FRONTIER_REUSE
    ReuseSlot reuse;
    uint64_t ticket;
#endif
} ImageSlot;
typedef struct {
    Context base;
    Slot diagnostic;
    ImageSlot slots[MAX_SLOTS];
    SBuffer draw;
#ifdef FRONTIER_NATIVE
    NativeProgram programs[3], raster;
#else
    OgpuKernel *kernels[3];
    OgpuRaster *raster;
#endif
    uint32_t w, h, ow, oh;
    size_t sizes[SB_COUNT], final_size;
    Launch grids[3];
    unsigned char *inputs[2], *weights, *reference[2], *pixels;
    unsigned maximum_delta;
#ifdef FRONTIER_REUSE
    SBuffer arenas[3]; /* DEVICE, HOST upload, HOST readback; never per-frame. */
    int arena;
    uint64_t requested_bytes;
    unsigned allocations;
    unsigned char *serial[2];
    const char *serial_paths[2];
    int write_serial;
#endif
} Stream;
typedef struct { Frame frame; double upload, read; } StreamFrame;

#ifndef FRONTIER_NATIVE
static OgpuBuffer *sb_handle(SBuffer *buffer) {
#ifdef FRONTIER_REUSE
    return buffer->buffer;
#else
    return *buffer;
#endif
}
static uint64_t sb_offset(SBuffer *buffer) {
#ifdef FRONTIER_REUSE
    return buffer->offset;
#else
    (void)buffer; return 0;
#endif
}
#endif

static int stream_file(const char *path, void *data, size_t bytes) {
    FILE *file = fopen(path, "rb");
    if (!file) { perror(path); return 0; }
    int okay = fread(data, 1, bytes, file) == bytes && fgetc(file) == EOF && !ferror(file);
    if (fclose(file)) okay = 0;
    return okay;
}
static int sb_create(Context *c, SBuffer *buffer, size_t bytes, int host, uint64_t *address) {
#ifdef FRONTIER_NATIVE
#ifdef FRONTIER_REUSE
    buffer->owned = 1; buffer->size = bytes;
    CHECK(native_buffer_create(&c->native, &buffer->native, bytes, host));
    buffer->buffer = buffer->native.buffer;
    if (host) {
        buffer->view.data = buffer->native.mapped;
        buffer->view.size_bytes = bytes;
        buffer->view.alignment = c->native.properties.limits.minMemoryMapAlignment;
        buffer->view.coherent = buffer->native.coherent;
        buffer->view.access_granularity = buffer->native.coherent ? 1 : c->native.properties.limits.nonCoherentAtomSize;
        CHECK(buffer->view.data && buffer->view.access_granularity);
    }
    if (address) *address = buffer->native.address;
#else
    CHECK(native_buffer_create(&c->native, buffer, bytes, host));
    if (address) *address = buffer->address;
#endif
#else
#ifdef FRONTIER_REUSE
    buffer->owned = 1; buffer->size = bytes;
    API(ogpu_buffer_create(c->device, &(OgpuBufferDesc){bytes, host ? OGPU_MEMORY_HOST : OGPU_MEMORY_DEVICE, 0}, &buffer->buffer, &c->error));
    if (host) {
        API(ogpu_buffer_host_view(buffer->buffer, &buffer->view, &c->error));
        CHECK(buffer->view.data && buffer->view.size_bytes == bytes && buffer->view.access_granularity
            && buffer->view.coherent <= 1);
    }
    if (address) API(ogpu_buffer_device_address(buffer->buffer, address, &c->error));
#else
    API(ogpu_buffer_create(c->device, &(OgpuBufferDesc){bytes, host ? OGPU_MEMORY_HOST : OGPU_MEMORY_DEVICE, 0}, buffer, &c->error));
    if (address) API(ogpu_buffer_device_address(*buffer, address, &c->error));
#endif
#endif
    return 1;
}
static void sb_destroy(Context *c, SBuffer *buffer) {
#ifdef FRONTIER_NATIVE
#ifdef FRONTIER_REUSE
    if (buffer->owned) native_buffer_destroy(&c->native, &buffer->native);
    memset(buffer, 0, sizeof(*buffer));
#else
    native_buffer_destroy(&c->native, buffer);
#endif
#else
#ifdef FRONTIER_REUSE
    (void)c; if (buffer->owned) ogpu_buffer_destroy(buffer->buffer);
    memset(buffer, 0, sizeof(*buffer));
#else
    (void)c; ogpu_buffer_destroy(*buffer); *buffer = NULL;
#endif
#endif
}
#ifdef FRONTIER_REUSE
/* Backend adapters only: the consumer range/cache/ownership policy is shared. */
static int sb_cache(Context *c, SBuffer *buffer, uint64_t offset, uint64_t bytes, int flush) {
    CHECK(buffer->view.data && offset <= buffer->view.size_bytes && bytes <= buffer->view.size_bytes-offset);
    if (!bytes || buffer->view.coherent) return 1;
#ifdef FRONTIER_NATIVE
    uint64_t atom = buffer->view.access_granularity, end;
    CHECK(atom && reuse_align(offset+bytes, atom, &end));
    uint64_t start = offset/atom*atom;
    VkMappedMemoryRange range = {.sType=VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
        .memory=buffer->native.memory, .offset=start,
        .size=end <= buffer->native.allocated ? end-start : VK_WHOLE_SIZE};
    Native *n = &c->native;
    return flush ? vk_ok(n->vkFlushMappedMemoryRanges(n->device, 1, &range), "range flush")
                 : vk_ok(n->vkInvalidateMappedMemoryRanges(n->device, 1, &range), "range invalidate");
#else
    if (flush) { API(ogpu_buffer_host_flush(buffer->buffer, offset, bytes, &c->error)); }
    else { API(ogpu_buffer_host_invalidate(buffer->buffer, offset, bytes, &c->error)); }
    return 1;
#endif
}
static int sb_address(Context *c, SBuffer *buffer, uint64_t *address) {
#ifdef FRONTIER_NATIVE
    (void)c; *address = buffer->native.address;
#else
    API(ogpu_buffer_device_address(buffer->buffer, address, &c->error));
#endif
    return 1;
}
#endif
static int sb_write(Context *c, SBuffer *buffer, const void *data, size_t bytes) {
#ifdef FRONTIER_REUSE
    CHECK(buffer->view.data && bytes <= buffer->size);
    memcpy((char *)buffer->view.data + buffer->offset, data, bytes);
    return sb_cache(c, buffer, buffer->offset, bytes, 1);
#elif defined(FRONTIER_NATIVE)
    return native_write(&c->native, buffer, 0, data, bytes);
#else
    API(ogpu_buffer_write(*buffer, 0, data, bytes, &c->error)); return 1;
#endif
}
static int sb_read(Context *c, SBuffer *buffer, void *data, size_t bytes) {
#ifdef FRONTIER_REUSE
    CHECK(buffer->view.data && bytes <= buffer->size);
    CHECK(sb_cache(c, buffer, buffer->offset, bytes, 0));
    memcpy(data, (char *)buffer->view.data + buffer->offset, bytes); return 1;
#elif defined(FRONTIER_NATIVE)
    return native_read(&c->native, buffer, 0, data, bytes);
#else
    API(ogpu_buffer_read(*buffer, 0, data, bytes, &c->error)); return 1;
#endif
}
#ifdef FRONTIER_REUSE
#include "../resource_reuse/stream_ranges.h"
#endif
static int stream_begin(Context *c, Slot *slot) {
    CHECK(!slot->pending);
#ifdef FRONTIER_NATIVE
    Native *n = &c->native; NativeBatch *b = &slot->batch;
    if (b->pool) {
        VK_TRY(c->reset_pool(n->device, b->pool, 0));
        VkCommandBufferBeginInfo begin = {.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
        VK_TRY(n->vkBeginCommandBuffer(b->command, &begin));
        native_barrier(n, b->command, VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_WRITE_BIT,
            VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT);
    } else CHECK(native_begin(n, b));
#else
    if (slot->storage) { API(ogpu_batch_create_in(slot->storage, &slot->batch, &c->error)); }
    else { API(ogpu_batch_create(c->device, &slot->batch, &c->error)); }
#endif
    return 1;
}
static int stream_end(Context *c, Slot *slot) {
#ifdef FRONTIER_NATIVE
    native_barrier(&c->native, slot->batch.command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        VK_ACCESS_2_MEMORY_WRITE_BIT, VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_READ_BIT);
    VK_TRY(c->native.vkEndCommandBuffer(slot->batch.command));
#else
    (void)c; (void)slot;
#endif
    return 1;
}
static int stream_copy_range(Context *c, Slot *slot, SBuffer *src, size_t src_offset,
                             SBuffer *dst, size_t dst_offset, size_t bytes) {
#ifdef FRONTIER_REUSE
    CHECK(src_offset <= src->size && bytes <= src->size-src_offset
        && dst_offset <= dst->size && bytes <= dst->size-dst_offset);
#endif
#ifdef FRONTIER_NATIVE
#ifdef FRONTIER_REUSE
    return native_copy(&c->native, &slot->batch, &src->native, src->offset+src_offset,
        &dst->native, dst->offset+dst_offset, bytes);
#else
    return native_copy(&c->native, &slot->batch, src, src_offset, dst, dst_offset, bytes);
#endif
#else
    API(ogpu_batch_copy_buffer(slot->batch, sb_handle(src), sb_offset(src)+src_offset,
        sb_handle(dst), sb_offset(dst)+dst_offset, bytes, &c->error)); return 1;
#endif
}
static int stream_copy(Context *c, Slot *slot, SBuffer *src, SBuffer *dst, size_t bytes) {
    return stream_copy_range(c, slot, src, 0, dst, 0, bytes);
}
static int stream_record(Stream *s, unsigned index) {
    Context *c = &s->base; Slot *slot = &c->slots[index]; ImageSlot *im = &s->slots[index];
    CHECK(!slot->pending);
#ifdef FRONTIER_NATIVE
    if (!strcmp(c->policy, "replay") && slot->batch.command) return 1;
#else
    if (slot->list) return 1;
#endif
    CHECK(stream_begin(c, slot));
    HiddenArguments hidden = {.arg_input_data=im->address[SI], .arg_weights=im->address[SW],
        .arg_output_data=im->address[SH], .arg_width=s->w, .arg_height=s->h, .arg_dispatch_width=s->grids[0].stride};
    DenoiseArguments denoise = {.arg_input_data=im->address[SI], .arg_hidden=im->address[SH],
        .arg_weights=im->address[SW], .arg_output_data=im->address[SD], .arg_count=s->w * s->h,
        .arg_dispatch_width=s->grids[1].stride};
    ProcessArguments process = {.arg_input_data=im->address[SD], .arg_output_data=im->address[SC],
        .arg_width=s->w, .arg_height=s->h, .arg_out_width=s->ow, .arg_out_height=s->oh,
        .arg_dispatch_width=s->grids[2].stride};
    DisplayArguments display = {.arg_pixels=im->address[SC], .arg_width=s->ow, .arg_height=s->oh};
#ifdef FRONTIER_NATIVE
    Native *n = &c->native; NativeBatch *b = &slot->batch;
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT);
    CHECK(stream_copy(c, slot, &im->upload, &im->data[SI], s->sizes[SI]));
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
    CHECK(native_dispatch(n, b, &s->programs[0], s->grids[0], &hidden, sizeof(hidden)));
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
    CHECK(native_dispatch(n, b, &s->programs[1], s->grids[1], &denoise, sizeof(denoise)));
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
    CHECK(native_dispatch(n, b, &s->programs[2], s->grids[2], &process, sizeof(process)));
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
#ifdef FRONTIER_REUSE
    CHECK(!s->draw.offset); /* Draw arguments deliberately remain dedicated. */
    CHECK(native_draw(n, b, &s->raster, &im->image, &s->draw.native, &display, sizeof(display)));
    CHECK(native_image_readback(n, b, &im->image, &im->readback.native, im->readback.offset+SG));
#else
    CHECK(native_draw(n, b, &s->raster, &im->image, &s->draw, &display, sizeof(display)));
    CHECK(native_image_readback(n, b, &im->image, &im->readback, SG));
#endif
#else
    API(ogpu_batch_barrier(slot->batch, OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE |
        OGPU_ACCESS_FRAGMENT_READ | OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE,
        OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE, &c->error));
    CHECK(stream_copy(c, slot, &im->upload, &im->data[SI], s->sizes[SI]));
    API(ogpu_batch_barrier(slot->batch, OGPU_ACCESS_TRANSFER_WRITE, OGPU_ACCESS_COMPUTE_READ, &c->error));
    API(ogpu_batch_dispatch(slot->batch, s->kernels[0], s->grids[0].x, s->grids[0].y, 1, &hidden, sizeof(hidden), &c->error));
    API(ogpu_batch_barrier(slot->batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &c->error));
    API(ogpu_batch_dispatch(slot->batch, s->kernels[1], s->grids[1].x, s->grids[1].y, 1, &denoise, sizeof(denoise), &c->error));
    API(ogpu_batch_barrier(slot->batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &c->error));
    API(ogpu_batch_dispatch(slot->batch, s->kernels[2], s->grids[2].x, s->grids[2].y, 1, &process, sizeof(process), &c->error));
    API(ogpu_batch_barrier(slot->batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_FRAGMENT_READ, &c->error));
    API(ogpu_batch_discard_image(slot->batch, im->image, &c->error));
    const OgpuRenderingDesc rendering_3 = {.color = {im->image, OGPU_ATTACHMENT_CLEAR, OGPU_STORE_STORE, {0, 0, 0, 1}}};
    API(ogpu_batch_begin_rendering(slot->batch, &rendering_3, &c->error));
    API(ogpu_batch_draw_indirect(slot->batch, s->raster, &(OgpuIndirectRange){.buffer=sb_handle(&s->draw),.offset=sb_offset(&s->draw),.stride_bytes=16,.max_draw_count=1}, &display, sizeof(display), &c->error));
    API(ogpu_batch_end_rendering(slot->batch, &c->error));
    API(ogpu_batch_copy_image_to_buffer(slot->batch, im->image, sb_handle(&im->readback), sb_offset(&im->readback)+SG, &c->error));
#endif
    CHECK(stream_end(c, slot)); return 1;
}
static int stream_verify(Stream *s, unsigned index) {
    ImageSlot *im = &s->slots[index];
    const unsigned char *want = s->reference[im->input_index];
    for (unsigned i = 0; i < SG; ++i) CHECK(s->pixels[i] == 0xa5 && s->pixels[s->final_size - SG + i] == 0xa5);
    unsigned maximum = 0, invalid_alpha = 0;
    for (size_t i = 0; i < s->final_size - 2 * SG; ++i) {
        unsigned char got = s->pixels[SG + i];
        unsigned delta = got > want[i] ? got - want[i] : want[i] - got;
        if (i % 4 == 3) invalid_alpha |= (got ^ 255u) | (want[i] ^ 255u);
        else if (delta > maximum) maximum = delta;
    }
    CHECK(!invalid_alpha && maximum <= 1);
    if (maximum > s->maximum_delta) s->maximum_delta = maximum;
#ifdef FRONTIER_REUSE
    if (!s->serial[im->input_index]) {
        CHECK(s->write_serial);
        s->serial[im->input_index] = malloc(s->final_size); CHECK(s->serial[im->input_index]);
        memcpy(s->serial[im->input_index], s->pixels, s->final_size);
        FILE *file = fopen(s->serial_paths[im->input_index], "wb"); CHECK(file);
        int okay = fwrite(s->pixels, 1, s->final_size, file) == s->final_size;
        if (fclose(file)) okay = 0;
        CHECK(okay);
    }
    CHECK(!memcmp(s->pixels, s->serial[im->input_index], s->final_size));
#endif
    return 1;
}
static int stream_retire(Stream *s, unsigned index, StreamFrame *frames, int validate) {
    Context *c = &s->base; Slot *slot = &c->slots[index];
    if (!slot->pending) return 1;
    StreamFrame *f = &frames[slot->sample_index]; double start = clock_ms();
    CHECK(wait_slot(c, slot)); double waited = clock_ms();
#ifdef FRONTIER_REUSE
    CHECK(reuse_completed(&s->slots[index].reuse, s->slots[index].ticket));
#endif
    CHECK(sb_read(c, &s->slots[index].readback, s->pixels, s->final_size)); double read = clock_ms();
    f->frame.wait = waited - start; f->read = read - waited; f->frame.latency = read - f->frame.start;
    if (validate) CHECK(stream_verify(s, index));
#ifdef FRONTIER_REUSE
    CHECK(reuse_release(&s->slots[index].reuse, s->slots[index].ticket));
#endif
    return 1;
}
static int stream_window(Stream *s, unsigned count, StreamFrame *frames, int validate, double *wall) {
    Context *c = &s->base; double start = clock_ms();
    for (unsigned frame = 0; frame < count; ++frame) {
        unsigned index = frame % c->count; Slot *slot = &c->slots[index]; ImageSlot *im = &s->slots[index];
        CHECK(stream_retire(s, index, frames, validate));
#ifdef FRONTIER_REUSE
        CHECK(reuse_acquire(&im->reuse, &im->ticket));
#endif
#ifdef FRONTIER_REUSE
        /* Alternate each slot's contents. Global ABAB alone leaves even-count
         * slots permanently on A or B and cannot detect a stale upload there. */
        im->input_index = stream_input_index(frame, c->count);
#else
        im->input_index = frame % 2;
#endif
        slot->sample_index = frame; frames[frame].frame.start = clock_ms();
        CHECK(sb_write(c, &im->upload, s->inputs[im->input_index], s->sizes[SI])); double uploaded = clock_ms();
        CHECK(stream_record(s, index)); double recorded = clock_ms();
#ifdef FRONTIER_REUSE
        CHECK(reuse_recorded(&im->reuse, im->ticket));
        if (!submit_slot(c, slot)) { (void)reuse_quarantine(&im->reuse, im->ticket); return 0; }
        CHECK(reuse_submitted(&im->reuse, im->ticket));
        uint64_t rejected = 0;
        CHECK(!reuse_acquire(&im->reuse, &rejected) && !rejected); /* No premature recycle. */
        double submitted = clock_ms();
#else
        CHECK(submit_slot(c, slot)); double submitted = clock_ms();
#endif
        frames[frame].upload = uploaded - frames[frame].frame.start;
        frames[frame].frame.record = recorded - uploaded; frames[frame].frame.submit = submitted - recorded;
    }
    unsigned first = count > c->count ? count - c->count : 0;
    for (unsigned frame = first; frame < count; ++frame) CHECK(stream_retire(s, frame % c->count, frames, validate));
    *wall = clock_ms() - start;
    /* Timing checks every slot's final image after the window, not just the last frame. */
    for (unsigned i = 0; i < c->count && i < count; ++i) {
        CHECK(sb_read(c, &s->slots[i].readback, s->pixels, s->final_size)); CHECK(stream_verify(s, i));
    }
    return 1;
}
static int stream_create(Stream *s, char **argv) {
    Context *c = &s->base; uint32_t limits[3];
#ifdef FRONTIER_NATIVE
    Native *n = &c->native; CHECK(native_create(n));
    PFN_vkGetInstanceProcAddr iget = (PFN_vkGetInstanceProcAddr)dlsym(n->library, "vkGetInstanceProcAddr"); CHECK(iget);
    PFN_vkGetDeviceProcAddr get = (PFN_vkGetDeviceProcAddr)iget(n->instance, "vkGetDeviceProcAddr"); CHECK(get);
    c->reset_pool = (PFN_vkResetCommandPool)get(n->device, "vkResetCommandPool"); CHECK(c->reset_pool);
    memcpy(limits, n->properties.limits.maxComputeWorkGroupCount, sizeof(limits));
#define SCOMPUTE(i, prefix) CHECK(native_compute(n, &s->programs[i], prefix##_code, sizeof(prefix##_code), prefix##_push_size, prefix##_local))
    SCOMPUTE(0, hidden); SCOMPUTE(1, denoise); SCOMPUTE(2, process);
#undef SCOMPUTE
    CHECK(native_raster(n, &s->raster));
#else
    API(ogpu_probe_create(OGPU_ABI_VERSION, &c->probe, &c->error));
    uint32_t devices; API(ogpu_probe_device_count(c->probe, &devices)); CHECK(devices == 1);
    API(ogpu_device_create_graphics(c->probe, 0, &c->device, &c->error));
    OgpuDeviceInfo info; API(ogpu_probe_device_info(c->probe, 0, &info));
    printf("DEVICE {\"vendor\":%u,\"device\":%u,\"api\":[%u,%u,%u]}\n", info.vendor_id, info.device_id,
        info.vulkan_api_major, info.vulkan_api_minor, info.vulkan_api_patch);
    OgpuDeviceLimits l; OgpuCapabilities caps;
    API(ogpu_device_limits(c->device, &l, &c->error)); API(ogpu_device_capabilities(c->device, &caps, &c->error));
    CHECK(hidden_compatible(&caps, &l) && denoise_compatible(&caps, &l) && process_compatible(&caps, &l)
        && fullscreen_compatible(&caps, &l) && display_compatible(&caps, &l));
    memcpy(limits, l.max_dispatch, sizeof(limits));
    OgpuShaderDesc shaders[] = {hidden_shader(), denoise_shader(), process_shader(), fullscreen_shader(), display_shader()};
    unsigned roots[] = {hidden_push_size, denoise_push_size, process_push_size};
    for (unsigned i = 0; i < 3; ++i) API(ogpu_kernel_create(c->device, &shaders[i], roots[i], &s->kernels[i], &c->error));
    API(ogpu_raster_create(c->device, &shaders[3], &shaders[4], &(OgpuRasterDesc){display_push_size, OGPU_TOPOLOGY_TRIANGLE_LIST, OGPU_FORMAT_RGBA8_UNORM, OGPU_FORMAT_NONE, 0, 0, OGPU_COMPARE_ALWAYS, 0}, &s->raster, &c->error));
#endif
    uint32_t pixels, out;
    CHECK(image_count(s->w, s->h, 8, &pixels) && image_count(s->ow, s->oh, 4, &out)
        && resize_axis(s->w, s->ow) && resize_axis(s->h, s->oh));
    CHECK(launch(pixels * 8, hidden_local, limits, &s->grids[0]) && launch(pixels, denoise_local, limits, &s->grids[1])
        && launch(out, process_local, limits, &s->grids[2]));
    size_t payload[] = {(size_t)pixels * 4, 89 * 4, (size_t)pixels * 32, (size_t)pixels * 4, (size_t)out * 16};
    for (unsigned i = 0; i < SB_COUNT; ++i) CHECK(add_size(payload[i], 2 * SG, &s->sizes[i]));
    CHECK(add_size((size_t)out * 4, 2 * SG, &s->final_size));
    s->weights = malloc(s->sizes[SW]); s->pixels = malloc(s->final_size); CHECK(s->weights && s->pixels);
    memset(s->weights, 0xa5, s->sizes[SW]); memset(s->pixels, 0xa5, s->final_size);
    CHECK(stream_file(argv[9], s->weights + SG, payload[SW]));
    for (unsigned i = 0; i < 2; ++i) {
        s->inputs[i] = malloc(s->sizes[SI]); s->reference[i] = malloc(s->final_size - 2 * SG);
        CHECK(s->inputs[i] && s->reference[i]); memset(s->inputs[i], 0xa5, s->sizes[SI]);
        CHECK(stream_file(argv[10 + i], s->inputs[i] + SG, payload[SI]));
        CHECK(stream_file(argv[12 + i], s->reference[i], s->final_size - 2 * SG));
    }
    CHECK(memcmp(s->reference[0], s->reference[1], s->final_size - 2 * SG));
    CHECK(sb_create(c, &s->draw, 16, 1, NULL)); uint32_t draw[] = {3, 1, 0, 0}; CHECK(sb_write(c, &s->draw, draw, sizeof(draw)));
#ifdef FRONTIER_REUSE
    CHECK(stream_allocate_ranges(s));
    if (!s->write_serial) for (unsigned i = 0; i < 2; ++i) {
        s->serial[i] = malloc(s->final_size); CHECK(s->serial[i]);
        CHECK(stream_file(s->serial_paths[i], s->serial[i], s->final_size));
    }
#endif
    for (unsigned i = 0; i < c->count; ++i) {
        ImageSlot *im = &s->slots[i]; Slot *slot = &c->slots[i];
#ifndef FRONTIER_REUSE
        for (unsigned j = 0; j < SB_COUNT; ++j) {
            CHECK(sb_create(c, &im->data[j], s->sizes[j], 0, &im->address[j])); im->address[j] += SG;
        }
        size_t upload = s->sizes[SI] > s->sizes[SW] ? s->sizes[SI] : s->sizes[SW];
        CHECK(sb_create(c, &im->upload, upload, 1, NULL) && sb_create(c, &im->readback, s->final_size, 1, NULL));
#else
#ifndef FRONTIER_NATIVE
        API(ogpu_recording_storage_create(c->device, &slot->storage, &c->error));
#endif
#endif
        CHECK(sb_write(c, &im->readback, s->pixels, s->final_size));
#ifdef FRONTIER_NATIVE
        CHECK(native_image_create(n, &im->image, s->ow, s->oh));
#else
        OgpuImageDesc image = {OGPU_IMAGE_2D, s->ow, s->oh, OGPU_FORMAT_RGBA8_UNORM,
            OGPU_IMAGE_USAGE_COLOR | OGPU_IMAGE_USAGE_COPY_SRC, 0};
        API(ogpu_image_create(c->device, &image, &im->image, &c->error));
#endif
        CHECK(sb_write(c, &im->upload, s->weights, s->sizes[SW]) && stream_begin(c, slot));
        CHECK(stream_copy(c, slot, &im->upload, &im->data[SW], s->sizes[SW]));
        for (unsigned j = SH; j < SB_COUNT; ++j) {
            CHECK(stream_copy_range(c, slot, &im->upload, 0, &im->data[j], 0, SG));
            CHECK(stream_copy_range(c, slot, &im->upload, 0, &im->data[j], s->sizes[j] - SG, SG));
        }
        CHECK(stream_end(c, slot) && submit_slot(c, slot) && wait_slot(c, slot));
#ifdef FRONTIER_NATIVE
        /* Discard setup recording even for replay; record the actual frame once. */
        native_batch_destroy(n, &slot->batch);
        if (!strcmp(c->policy, "replay")) CHECK(stream_record(s, i));
#else
        if (!strcmp(c->policy, "compiled")) {
            CHECK(stream_record(s, i));
            API(ogpu_batch_compile(slot->batch, 0, &slot->list, &c->error));
            ogpu_batch_destroy(slot->batch); slot->batch = NULL;
        }
#endif
    }
    return 1;
}
static int stream_check_buffers(Stream *s) {
    Context *c = &s->base; Slot *slot = &s->diagnostic;
    size_t bytes = s->sizes[SI] + s->sizes[SW] + 6 * SG;
    /* The selected upscales have sufficient existing readback space; no diagnostic
     * allocation or full intermediate copy enters ordinary frame timing. */
    CHECK(bytes <= s->final_size);
    for (unsigned i = 0; i < c->count; ++i) {
        ImageSlot *im = &s->slots[i]; CHECK(!c->slots[i].pending && stream_begin(c, slot));
#ifdef FRONTIER_NATIVE
        native_barrier(&c->native, slot->batch.command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
            VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT, VK_PIPELINE_STAGE_2_TRANSFER_BIT,
            VK_ACCESS_2_TRANSFER_READ_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT);
#else
        API(ogpu_batch_barrier(slot->batch, OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE |
            OGPU_ACCESS_FRAGMENT_READ | OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE,
            OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE, &c->error));
#endif
        CHECK(stream_copy(c, slot, &im->data[SI], &im->readback, s->sizes[SI]));
        CHECK(stream_copy_range(c, slot, &im->data[SW], 0, &im->readback, s->sizes[SI], s->sizes[SW]));
        size_t offset = s->sizes[SI] + s->sizes[SW];
        for (unsigned j = SH; j < SB_COUNT; ++j) {
            CHECK(stream_copy_range(c, slot, &im->data[j], 0, &im->readback, offset, SG)); offset += SG;
            CHECK(stream_copy_range(c, slot, &im->data[j], s->sizes[j] - SG, &im->readback, offset, SG)); offset += SG;
        }
        CHECK(stream_end(c, slot) && submit_slot(c, slot) && wait_slot(c, slot));
        CHECK(sb_read(c, &im->readback, s->pixels, bytes));
        CHECK(!memcmp(s->pixels, s->inputs[im->input_index], s->sizes[SI]));
        CHECK(!memcmp(s->pixels + s->sizes[SI], s->weights, s->sizes[SW]));
        for (size_t j = s->sizes[SI] + s->sizes[SW]; j < bytes; ++j) CHECK(s->pixels[j] == 0xa5);
    }
    puts("Streaming final input/weight integrity and all intermediate guards PASS");
    return 1;
}
static void stream_destroy(Stream *s) {
    Context *c = &s->base;
    (void)wait_slot(c, &s->diagnostic);
#ifdef FRONTIER_NATIVE
    native_batch_destroy(&c->native, &s->diagnostic.batch);
#else
    ogpu_batch_destroy(s->diagnostic.batch);
#endif
    for (unsigned i = 0; i < c->count; ++i) (void)wait_slot(c, &c->slots[i]);
    /* Destroy recordings before releasing any shader/image resources. */
    for (unsigned i = 0; i < c->count; ++i) {
#ifdef FRONTIER_NATIVE
        native_batch_destroy(&c->native, &c->slots[i].batch);
#else
        ogpu_batch_destroy(c->slots[i].batch); c->slots[i].batch = NULL;
        ogpu_command_list_destroy(c->slots[i].list); c->slots[i].list = NULL;
#endif
    }
    for (unsigned i = 0; i < c->count; ++i) {
        ImageSlot *im = &s->slots[i];
#ifdef FRONTIER_NATIVE
        native_image_destroy(&c->native, &im->image);
#else
        ogpu_image_destroy(im->image);
#endif
        for (unsigned j = 0; j < SB_COUNT; ++j) sb_destroy(c, &im->data[j]);
        sb_destroy(c, &im->upload); sb_destroy(c, &im->readback);
    }
#ifdef FRONTIER_REUSE
    for (unsigned i = 0; i < 3; ++i) sb_destroy(c, &s->arenas[i]);
    for (unsigned i = 0; i < 2; ++i) free(s->serial[i]);
#endif
    sb_destroy(c, &s->draw);
#ifdef FRONTIER_NATIVE
    for (unsigned i = 0; i < 3; ++i) native_program_destroy(&c->native, &s->programs[i]);
    native_program_destroy(&c->native, &s->raster);
#else
    for (unsigned i = 0; i < 3; ++i) ogpu_kernel_destroy(s->kernels[i]);
    ogpu_raster_destroy(s->raster);
#endif
    destroy(c);
    for (unsigned i = 0; i < 2; ++i) { free(s->inputs[i]); free(s->reference[i]); }
    free(s->weights); free(s->pixels);
}
#ifndef FRONTIER_STREAM_NO_MAIN
int main(int argc, char **argv) {
    (void)create; (void)window; (void)selftest;
    if (argc != 14) { fprintf(stderr, "Usage: stream policy slots w h ow oh frames validate|measure weights inputA inputB referenceA referenceB\n"); return 1; }
    Stream s = {0}; Context *c = &s.base; c->policy = argv[1]; c->count = number(argv[2]);
    s.w = number(argv[3]); s.h = number(argv[4]); s.ow = number(argv[5]); s.oh = number(argv[6]);
    unsigned frames = number(argv[7]); int validate = !strcmp(argv[8], "validate");
    if (!c->count || c->count > MAX_SLOTS || frames < c->count || (!validate && strcmp(argv[8], "measure"))) return 1;
#ifdef FRONTIER_NATIVE
    if (strcmp(c->policy, "fresh") && strcmp(c->policy, "reset") && strcmp(c->policy, "replay")) return 1;
#else
    if (strcmp(c->policy, "ogpu") && strcmp(c->policy, "compiled")) return 1;
#endif
#ifdef FRONTIER_REUSE
    const char *allocation = getenv("OGPU_STREAM_ALLOCATION");
    if (!allocation || (strcmp(allocation, "dedicated") && strcmp(allocation, "arena"))) return 1;
    s.arena = !strcmp(allocation, "arena");
    s.serial_paths[0] = getenv("OGPU_STREAM_SERIAL_A"); s.serial_paths[1] = getenv("OGPU_STREAM_SERIAL_B");
    if (!s.serial_paths[0] || !s.serial_paths[1] || !strcmp(s.serial_paths[0], s.serial_paths[1])) return 1;
    const char *serial_mode = getenv("OGPU_STREAM_SERIAL_MODE");
    if (!serial_mode || (strcmp(serial_mode, "write") && strcmp(serial_mode, "check"))) return 1;
    s.write_serial = !strcmp(serial_mode, "write");
    if (s.write_serial && (c->count != 1 || !validate)) return 1;
#endif
    StreamFrame *samples = calloc(frames > 100 ? frames : 100, sizeof(*samples)); if (!samples) return 1;
    int okay = 0; double setup = clock_ms(), wall = 0;
    if (!stream_create(&s, argv)) goto cleanup;
    setup = clock_ms() - setup;
#ifdef FRONTIER_REUSE
    /* Known application storage only. Runtime/driver command allocations are NOT zero. */
    printf("REUSE_CPU_STORAGE {\"stream_stack_bytes\":%zu,\"sample_bytes\":%zu,\"fixture_bytes\":%zu,"
        "\"runtime_command_bytes\":null,\"driver_command_bytes\":null}\n", sizeof(s),
        (size_t)(frames > 100 ? frames : 100)*sizeof(*samples),
        2*s.sizes[SI]+s.sizes[SW]+2*(s.final_size-2*SG)+3*s.final_size);
#endif
    if (!validate && !stream_window(&s, 100, samples, 0, &wall)) goto cleanup;
    memset(samples, 0, (frames > 100 ? frames : 100) * sizeof(*samples));
    if (!stream_window(&s, frames, samples, validate, &wall)) goto cleanup;
    if (!stream_check_buffers(&s)) goto cleanup;
#ifdef FRONTIER_REUSE
    if (!stream_check_range_padding(&s)) goto cleanup;
#endif
    printf("STREAM {\"policy\":\"%s\",\"slots\":%u,\"extent\":[%u,%u,%u,%u],\"frames\":%u,"
        "\"warmups\":%u,\"validation\":%s,\"setup_ms\":%.9f,\"wall_ms\":%.9f,\"max_rgb_delta\":%u}\n",
        c->policy, c->count, s.w, s.h, s.ow, s.oh, frames, validate ? 0 : 100, validate ? "true" : "false", setup, wall, s.maximum_delta);
    if (!validate) for (unsigned i = 0; i < frames; ++i) printf("FRAME {\"index\":%u,\"upload_ms\":%.9f,"
        "\"record_ms\":%.9f,\"submit_ms\":%.9f,\"wait_ms\":%.9f,\"read_ms\":%.9f,\"latency_ms\":%.9f}\n", i,
        samples[i].upload, samples[i].frame.record, samples[i].frame.submit, samples[i].frame.wait, samples[i].read, samples[i].frame.latency);
    okay = 1;
cleanup:
    stream_destroy(&s); free(samples);
    if (okay) puts("Streaming full-frame pixels/readback guards PASS; all slots drained");
    return !okay;
}
#endif
