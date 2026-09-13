// Integration glue only: upstream owns generation/math/dispatch; OGPU owns execution.
// Single-threaded and synchronous per operation. Unsupported requests fail closed.
#include "gpu.h"
#include "backend.h"
#include "compiler.h"
#include "ogpu.h"
#include <stdio.h>

#define SLOTS 8
#define CONSTANTS 16
struct backend {
    struct pl_gpu_fns fns; // pinned private ABI: must remain first
    OgpuDevice *device;
    struct ogpu_stats stats;
    bool failed;
};
struct texture { OgpuImage *image; OgpuBuffer *staging; size_t size; bool initialized; };
struct program {
    struct shader_binary primary, vertex;
    OgpuKernel *kernel;
    OgpuRaster *raster;
    OgpuImageHeap *images;
    OgpuSamplerHeap *samplers;
    OgpuBuffer *vertices, *indirect;
    uint64_t vertex_address;
    OgpuSpecializationConstant values[CONSTANTS];
};

static bool reject(pl_gpu gpu, const char *why)
{
    struct backend *b = PL_PRIV(gpu);
    fprintf(stderr, "OGPU libplacebo: %s\n", why);
    b->failed = true;
    return false;
}

// Every failure path below releases recordings/completions before their resources.
#define TRY(call) do { if ((call) != OGPU_SUCCESS) { reject(gpu, error.message); goto fail; } } while (0)
#define REQUIRE(test, why) do { if (!(test)) { reject(gpu, why); goto fail; } } while (0)

static bool finish(pl_gpu gpu, OgpuBatch *batch)
{
    OgpuError error;
    OgpuCompletion *done = NULL;
    bool ok = false;
    TRY(ogpu_batch_submit(batch, &done, &error));
    TRY(ogpu_completion_wait(done, &error));
    ok = true;
fail:
    // Destruction drains submitted work even after a wait error.
    ogpu_completion_destroy(done);
    return ok;
}

static void tex_destroy(pl_gpu gpu, pl_tex tex)
{
    struct backend *b = PL_PRIV(gpu);
    struct texture *t = PL_PRIV(tex);
    ogpu_image_destroy(t->image);
    ogpu_buffer_destroy(t->staging);
    free((void *) tex);
    --b->stats.textures;
}

static bool transfer(pl_gpu gpu, pl_tex tex, void *ptr, bool upload)
{
    struct backend *b = PL_PRIV(gpu);
    struct texture *t = PL_PRIV(tex);
    OgpuError error;
    OgpuBatch *batch = NULL;
    bool ok = false;
    REQUIRE(!b->failed, "device already failed");
    if (!t->staging)
        TRY(ogpu_buffer_create(b->device, t->size, OGPU_MEMORY_HOST, &t->staging, &error));
    TRY(ogpu_batch_create(b->device, &batch, &error));
    if (upload) {
        TRY(ogpu_buffer_write(t->staging, 0, ptr, t->size, &error));
        if (!t->initialized)
            TRY(ogpu_batch_discard_image(batch, t->image, &error));
        TRY(ogpu_batch_copy_buffer_to_image(batch, t->staging, 0, t->image, &error));
    } else {
        REQUIRE(t->initialized, "readback of unwritten image");
        TRY(ogpu_batch_copy_image_to_buffer(batch, t->image, t->staging, 0, &error));
    }
    if (!finish(gpu, batch)) goto fail;
    t->initialized = true;
    if (upload) ++b->stats.uploads;
    else {
        TRY(ogpu_buffer_read(t->staging, 0, ptr, t->size, &error));
        ++b->stats.downloads;
    }
    ok = true;
fail:
    ogpu_batch_destroy(batch);
    return ok;
}

static pl_tex tex_create(pl_gpu gpu, const struct pl_tex_params *p)
{
    struct backend *b = PL_PRIV(gpu);
    OgpuError error;
    struct pl_tex_t *tex = NULL;
    const bool rgba = !strcmp(p->format->name, "rgba8");
    REQUIRE(!b->failed && (rgba || !strcmp(p->format->name, "r32f")), "unsupported image format");
    REQUIRE(!p->d && !p->blit_src && !p->blit_dst && !p->import_handle && !p->export_handle,
            "unsupported image dimension/interop/blit");
    REQUIRE(!p->renderable || (rgba && p->h), "unsupported color attachment");
    tex = calloc(1, PL_ALIGN_MEM(sizeof(*tex)) + sizeof(struct texture));
    REQUIRE(tex, "image allocation failed");
    ++b->stats.textures;
    tex->params = *p;
    tex->params.initial_data = NULL;
    struct texture *t = PL_PRIV(tex);
    t->size = (size_t) p->w * (p->h ? p->h : 1) * 4;
    const OgpuImageDesc desc = {
        .dimension = p->h ? OGPU_IMAGE_2D : OGPU_IMAGE_1D,
        .width = p->w, .height = p->h ? p->h : 1,
        .format = rgba ? OGPU_FORMAT_RGBA8_UNORM : OGPU_FORMAT_R32_FLOAT,
        .usage = (p->sampleable ? OGPU_IMAGE_USAGE_SAMPLED : 0) |
                 (p->storable ? OGPU_IMAGE_USAGE_STORAGE : 0) |
                 (p->renderable ? OGPU_IMAGE_USAGE_COLOR : 0) |
                 (p->host_readable ? OGPU_IMAGE_USAGE_COPY_SRC : 0) |
                 (p->host_writable || p->initial_data ? OGPU_IMAGE_USAGE_COPY_DST : 0),
    };
    TRY(ogpu_image_create(b->device, &desc, &t->image, &error));
    if (p->initial_data && !transfer(gpu, tex, (void *) p->initial_data, true)) goto fail;
    return tex;
fail:
    if (tex) tex_destroy(gpu, tex);
    return NULL;
}

static bool tex_transfer(pl_gpu gpu, const struct pl_tex_transfer_params *p, bool upload)
{
    const struct pl_tex_params *t = &p->tex->params;
    if (!p->ptr || p->buf || p->callback || p->timer || p->rc.x0 || p->rc.x1 != t->w ||
        (t->h && (p->rc.y0 || p->rc.y1 != t->h || p->row_pitch != (size_t) t->w * 4)))
        return reject(gpu, "only full packed synchronous host image transfers are supported");
    return transfer(gpu, p->tex, p->ptr, upload);
}
static bool tex_upload(pl_gpu gpu, const struct pl_tex_transfer_params *p) { return tex_transfer(gpu, p, true); }
static bool tex_download(pl_gpu gpu, const struct pl_tex_transfer_params *p) { return tex_transfer(gpu, p, false); }

static void pass_destroy(pl_gpu gpu, pl_pass pass)
{
    struct backend *b = PL_PRIV(gpu);
    struct program *p = PL_PRIV(pass);
    ogpu_kernel_destroy(p->kernel);
    ogpu_raster_destroy(p->raster);
    ogpu_image_heap_destroy(p->images);
    ogpu_sampler_heap_destroy(p->samplers);
    ogpu_buffer_destroy(p->vertices);
    ogpu_buffer_destroy(p->indirect);
    free(p->primary.words); free(p->vertex.words);
    for (int i = 0; i < pass->params.num_descriptors; ++i) free((void *) pass->params.descriptors[i].name);
    for (int i = 0; i < pass->params.num_vertex_attribs; ++i) free((void *) pass->params.vertex_attribs[i].name);
    free(pass->params.descriptors); free(pass->params.vertex_attribs);
    free(pass->params.constants);
    free((void *) pass->params.glsl_shader); free((void *) pass->params.vertex_shader);
    free((void *) pass);
    --b->stats.passes;
}

static bool specialize(pl_gpu gpu, pl_pass pass, const void *data)
{
    struct backend *b = PL_PRIV(gpu);
    struct program *p = PL_PRIV(pass);
    OgpuError error;
    OgpuKernel *kernel = NULL;
    OgpuRaster *raster = NULL;
    OgpuSpecializationConstant values[CONSTANTS] = {0};
    const unsigned count = pass->params.num_constants;
    if (!data && (p->kernel || p->raster)) return true;
    REQUIRE(!count || data, "missing specialization values");
    for (unsigned i = 0; i < count; ++i) {
        values[i].id = pass->params.constants[i].id;
        memcpy(&values[i].bits, (const char *) data + pass->params.constants[i].offset, 4);
    }
    if ((p->kernel || p->raster) && !memcmp(values, p->values, count * sizeof(*values))) return true;
    const OgpuShaderDesc shader = {p->primary.words, p->primary.count, values, count, 0};
    if (pass->params.type == PL_PASS_COMPUTE) {
        TRY(ogpu_kernel_create(b->device, &shader, pass->params.push_constants_size, &kernel, &error));
    } else {
        const OgpuShaderDesc vertex = {p->vertex.words, p->vertex.count, values, count, 0};
        TRY(ogpu_raster_create(b->device, &vertex, &shader, 8, OGPU_TOPOLOGY_TRIANGLE_STRIP, &raster, &error));
    }
    ogpu_kernel_destroy(p->kernel); ogpu_raster_destroy(p->raster);
    p->kernel = kernel; p->raster = raster;
    memcpy(p->values, values, sizeof(values));
    return true;
fail:
    ogpu_kernel_destroy(kernel); ogpu_raster_destroy(raster);
    return false;
}

static pl_pass pass_create(pl_gpu gpu, const struct pl_pass_params *in)
{
    struct backend *b = PL_PRIV(gpu);
    OgpuError error;
    struct pl_pass_t *pass = NULL;
    REQUIRE(!b->failed && !in->num_variables && in->num_constants <= CONSTANTS &&
            in->num_descriptors > 0 && in->num_descriptors <= SLOTS,
            "unsupported pass variables/constants/descriptors");
    const bool compute = in->type == PL_PASS_COMPUTE;
    REQUIRE(compute || in->type == PL_PASS_RASTER, "unsupported pass type");
    if (!compute) {
        REQUIRE(in->vertex_stride == 16 && in->num_vertex_attribs == 2 &&
                in->vertex_type == PL_PRIM_TRIANGLE_STRIP && !in->push_constants_size &&
                !in->blend_params && !strcmp(in->target_format->name, "rgba8"), "unsupported raster layout");
        for (int i = 0; i < 2; ++i)
            REQUIRE(in->vertex_attribs[i].location == i && in->vertex_attribs[i].offset == (size_t) i * 8 &&
                    !strcmp(in->vertex_attribs[i].fmt->name, "rg32f"), "unsupported vertex metadata");
    }
    for (int i = 0; i < in->num_constants; ++i)
        REQUIRE(in->constants[i].type == PL_VAR_FLOAT || in->constants[i].type == PL_VAR_SINT ||
                in->constants[i].type == PL_VAR_UINT, "unsupported specialization type");
    for (int i = 0; i < in->num_descriptors; ++i) {
        const struct pl_desc *d = &in->descriptors[i];
        REQUIRE(d->binding >= 0 && d->binding < SLOTS &&
                (d->type == PL_DESC_SAMPLED_TEX || (compute && d->type == PL_DESC_STORAGE_IMG &&
                 d->access == PL_DESC_ACCESS_WRITEONLY)), "unsupported resource binding");
    }
    pass = calloc(1, PL_ALIGN_MEM(sizeof(*pass)) + sizeof(struct program));
    REQUIRE(pass, "pass allocation failed");
    ++b->stats.passes;
    // Own only the accepted fields; never retain upstream's temporary pointers.
    pass->params.type = in->type;
    pass->params.push_constants_size = in->push_constants_size;
    pass->params.vertex_type = in->vertex_type;
    pass->params.vertex_stride = in->vertex_stride;
    pass->params.target_format = in->target_format;
    pass->params.load_target = in->load_target;
    pass->params.glsl_shader = strdup(in->glsl_shader);
    if (in->vertex_shader) pass->params.vertex_shader = strdup(in->vertex_shader);
    REQUIRE(pass->params.glsl_shader && (!in->vertex_shader || pass->params.vertex_shader), "shader allocation failed");
    pass->params.descriptors = calloc(in->num_descriptors, sizeof(*in->descriptors));
    REQUIRE(pass->params.descriptors, "descriptor allocation failed");
    for (int i = 0; i < in->num_descriptors; ++i) {
        pass->params.descriptors[i] = in->descriptors[i];
        pass->params.descriptors[i].name = strdup(in->descriptors[i].name);
        ++pass->params.num_descriptors;
        REQUIRE(pass->params.descriptors[i].name, "descriptor name allocation failed");
    }
    if (in->num_vertex_attribs) {
        pass->params.vertex_attribs = calloc(in->num_vertex_attribs, sizeof(*in->vertex_attribs));
        REQUIRE(pass->params.vertex_attribs, "vertex metadata allocation failed");
        for (int i = 0; i < in->num_vertex_attribs; ++i) {
            pass->params.vertex_attribs[i] = in->vertex_attribs[i];
            pass->params.vertex_attribs[i].name = strdup(in->vertex_attribs[i].name);
            ++pass->params.num_vertex_attribs;
            REQUIRE(pass->params.vertex_attribs[i].name, "vertex name allocation failed");
        }
    }
    if (in->num_constants) {
        pass->params.constants = malloc(in->num_constants * sizeof(*in->constants));
        REQUIRE(pass->params.constants, "constant allocation failed");
        memcpy(pass->params.constants, in->constants, in->num_constants * sizeof(*in->constants));
        pass->params.num_constants = in->num_constants;
    }
    struct program *p = PL_PRIV(pass);
    REQUIRE(compile_native(in->glsl_shader, compute ? 0 : 1, NULL, NULL, &p->primary), "shader compilation failed");
    if (!compute) {
        REQUIRE(compile_native(in->vertex_shader, 2, in->vertex_attribs[0].name,
                in->vertex_attribs[1].name, &p->vertex), "vertex compilation failed");
        TRY(ogpu_buffer_create(b->device, 64, OGPU_MEMORY_HOST, &p->vertices, &error));
        TRY(ogpu_buffer_device_address(p->vertices, &p->vertex_address, &error));
        TRY(ogpu_buffer_create(b->device, 16, OGPU_MEMORY_HOST, &p->indirect, &error));
        const OgpuDrawArguments args = {4, 1, 0, 0};
        TRY(ogpu_buffer_write(p->indirect, 0, &args, sizeof(args), &error));
    }
    TRY(ogpu_image_heap_create(b->device, SLOTS, &p->images, &error));
    TRY(ogpu_sampler_heap_create(b->device, SLOTS, &p->samplers, &error));
    if (!specialize(gpu, pass, in->constant_data)) goto fail;
    ++b->stats.creates;
    return pass;
fail:
    if (pass) pass_destroy(gpu, pass);
    return NULL;
}

static bool full_rect(pl_rect2d r, pl_tex t)
{ return !r.x0 && !r.y0 && r.x1 == t->params.w && r.y1 == t->params.h; }

static void pass_run(pl_gpu gpu, const struct pl_pass_run_params *in)
{
    struct backend *b = PL_PRIV(gpu);
    struct program *p = PL_PRIV(in->pass);
    const struct pl_pass_params *params = &in->pass->params;
    const bool compute = params->type == PL_PASS_COMPUTE;
    OgpuError error;
    OgpuBatch *batch = NULL;
    REQUIRE(!b->failed && !in->num_var_updates && !in->timer, "unsupported run variables/timer or failed device");
    if (!specialize(gpu, in->pass, in->constant_data)) goto fail;
    if (!compute) {
        REQUIRE(in->vertex_data && !in->vertex_buf && !in->index_data && !in->index_buf &&
                in->vertex_count == 4 && full_rect(in->viewport, in->target) &&
                full_rect(in->scissors, in->target), "unsupported draw region/vertices");
        TRY(ogpu_buffer_write(p->vertices, 0, in->vertex_data, 64, &error));
    }
    TRY(ogpu_batch_create(b->device, &batch, &error));
    // One queue; order previous writes before this pass's reads/writes. Host writes
    // are made visible by OGPU submission. No guessed resource-pointer tracing.
    TRY(ogpu_batch_barrier(batch,
        OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_COLOR_WRITE | OGPU_ACCESS_TRANSFER_WRITE,
        OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_FRAGMENT_READ |
        OGPU_ACCESS_VERTEX_READ | OGPU_ACCESS_INDIRECT_READ | OGPU_ACCESS_COLOR_READ |
        OGPU_ACCESS_COLOR_WRITE, &error));
    for (int i = 0; i < params->num_descriptors; ++i) {
        const struct pl_desc *d = &params->descriptors[i];
        const struct pl_desc_binding *binding = &in->desc_bindings[i];
        pl_tex tex = binding->object;
        struct texture *t = PL_PRIV(tex);
        const bool sampled = d->type == PL_DESC_SAMPLED_TEX;
        if (!t->initialized) {
            REQUIRE(!sampled, "sample of unwritten image");
            TRY(ogpu_batch_discard_image(batch, t->image, &error));
        }
        const OgpuImageEntry entry = {t->image, sampled ? OGPU_IMAGE_SAMPLED : OGPU_IMAGE_STORAGE, 0};
        TRY(ogpu_image_heap_write(p->images, d->binding, &entry, 1, &error));
        if (sampled) {
            REQUIRE(binding->sample_mode == PL_TEX_SAMPLE_NEAREST || binding->sample_mode == PL_TEX_SAMPLE_LINEAR,
                    "unsupported sampler filter");
            REQUIRE(binding->address_mode == PL_TEX_ADDRESS_CLAMP || binding->address_mode == PL_TEX_ADDRESS_REPEAT,
                    "unsupported sampler address mode");
            const uint32_t filter = binding->sample_mode == PL_TEX_SAMPLE_LINEAR ? OGPU_FILTER_LINEAR : OGPU_FILTER_NEAREST;
            const uint32_t address = binding->address_mode == PL_TEX_ADDRESS_REPEAT ? OGPU_ADDRESS_REPEAT : OGPU_ADDRESS_CLAMP;
            const OgpuSamplerDesc sampler = {filter, filter, address, address};
            TRY(ogpu_sampler_heap_write(p->samplers, d->binding, &sampler, 1, &error));
        }
    }
    TRY(ogpu_batch_bind_image_heap(batch, p->images, &error));
    TRY(ogpu_batch_bind_sampler_heap(batch, p->samplers, &error));
    if (compute) {
        TRY(ogpu_batch_dispatch(batch, p->kernel, in->compute_groups[0], in->compute_groups[1],
            in->compute_groups[2], in->push_constants, params->push_constants_size, &error));
    } else {
        struct texture *target = PL_PRIV(in->target);
        TRY(ogpu_batch_retain_buffer(batch, p->vertices, &error));
        TRY(ogpu_batch_draw_indirect(batch, p->raster, target->image, p->indirect, 0,
            &p->vertex_address, 8, params->load_target ? OGPU_ATTACHMENT_LOAD : OGPU_ATTACHMENT_CLEAR, &error));
    }
    if (!finish(gpu, batch)) goto fail;
    for (int i = 0; i < params->num_descriptors; ++i) {
        struct texture *t = PL_PRIV((pl_tex) in->desc_bindings[i].object);
        t->initialized = true;
    }
    if (compute) ++b->stats.compute;
    else { ((struct texture *) PL_PRIV(in->target))->initialized = true; ++b->stats.raster; }
fail:
    ogpu_batch_destroy(batch);
    // Waiting alone is insufficient for heap edits: finish destroyed completion.
    if (ogpu_image_heap_clear(p->images, 0, SLOTS, &error) != OGPU_SUCCESS)
        reject(gpu, error.message);
}

static int desc_namespace(pl_gpu gpu, enum pl_desc_type type) { return 0; }
static void gpu_finish(pl_gpu gpu) { /* Each submitted operation was already drained. */ }
static bool gpu_failed(pl_gpu gpu) { return ((struct backend *) PL_PRIV(gpu))->failed; }
static pl_buf buf_create(pl_gpu gpu, const struct pl_buf_params *p)
{ reject(gpu, "general libplacebo buffers are outside this adapter"); return NULL; }

static const struct pl_fmt_t formats[] = {
    {.name="rgba8", .signature=1, .type=PL_FMT_UNORM,
     .caps=PL_FMT_CAP_SAMPLEABLE|PL_FMT_CAP_LINEAR|PL_FMT_CAP_STORABLE|PL_FMT_CAP_RENDERABLE|PL_FMT_CAP_HOST_READABLE,
     .num_components=4, .component_depth={8,8,8,8}, .internal_size=4,
     .texel_size=4, .texel_align=1, .host_bits={8,8,8,8}, .sample_order={0,1,2,3}, .glsl_format="rgba8"},
    {.name="r32f", .signature=2, .type=PL_FMT_FLOAT,
     .caps=PL_FMT_CAP_SAMPLEABLE|PL_FMT_CAP_LINEAR|PL_FMT_CAP_HOST_READABLE,
     .num_components=1, .component_depth={32}, .internal_size=4,
     .texel_size=4, .texel_align=4, .host_bits={32}, .glsl_type="float"},
    {.name="rg32f", .signature=3, .type=PL_FMT_FLOAT, .caps=PL_FMT_CAP_VERTEX,
     .num_components=2, .component_depth={32,32}, .internal_size=8,
     .texel_size=8, .texel_align=4, .host_bits={32,32}, .sample_order={0,1}, .glsl_type="vec2"},
};

pl_gpu ogpu_pl_create(pl_log log, unsigned index)
{
    struct pl_gpu_t *gpu = calloc(1, PL_ALIGN_MEM(sizeof(*gpu)) + sizeof(struct backend));
    if (!gpu) return NULL;
    gpu->log = log;
    struct backend *b = PL_PRIV(gpu);
    OgpuProbe *probe = NULL;
    OgpuError error;
    OgpuDeviceLimits limits;
    OgpuCapabilities enabled;
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    TRY(ogpu_device_create_graphics(probe, index, &b->device, &error));
    TRY(ogpu_device_limits(b->device, &limits, &error));
    TRY(ogpu_device_capabilities(b->device, &enabled, &error));
    REQUIRE(enabled.compute_queue && enabled.graphics_queue && enabled.descriptor_heap,
            "required compute/raster/heap capabilities are not enabled");
    // Validate the bounded profile before advertising it. These small descriptions
    // check combinations, not all extents; actual texture creation checks its exact
    // description. rg32f below is a host vertex layout, not an OGPU image format.
    const OgpuImageDesc required_images[] = {
        {.dimension=2, .width=1, .height=1, .format=OGPU_FORMAT_RGBA8_UNORM,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_STORAGE|OGPU_IMAGE_USAGE_COLOR|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
        {.dimension=1, .width=1, .height=1, .format=OGPU_FORMAT_RGBA8_UNORM,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_STORAGE|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
        {.dimension=1, .width=1, .height=1, .format=OGPU_FORMAT_R32_FLOAT,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
        {.dimension=2, .width=1, .height=1, .format=OGPU_FORMAT_R32_FLOAT,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
    };
    for (size_t i = 0; i < sizeof(required_images) / sizeof(required_images[0]); ++i)
        TRY(ogpu_image_check_support(b->device, &required_images[i], &error));
    gpu->glsl = (struct pl_glsl_version) {.version=450, .vulkan=true, .compute=true,
        .max_shmem_size=limits.max_shared_memory_bytes, .max_group_threads=limits.max_group_invocations};
    memcpy(gpu->glsl.max_group_size, limits.max_group_size, sizeof(limits.max_group_size));
    gpu->limits = (struct pl_gpu_limits) {
        .max_tex_1d_dim=limits.max_image_1d, .max_tex_2d_dim=limits.max_image_2d,
        .max_constants=CONSTANTS, .array_size_constants=true,
        .max_pushc_size=PL_MIN(limits.max_push_data_bytes, 128),
        .align_vertex_stride=1, .align_tex_xfer_pitch=4, .align_tex_xfer_offset=4,
        .fragment_queues=1, .compute_queues=1,
    };
    memcpy(gpu->limits.max_dispatch, limits.max_dispatch, sizeof(limits.max_dispatch));
    gpu->formats = calloc(3, sizeof(pl_fmt));
    REQUIRE(gpu->formats, "format allocation failed");
    for (int i = 0; i < 3; ++i) gpu->formats[i] = &formats[i];
    gpu->num_formats = 3;
    b->fns = (struct pl_gpu_fns) {
        .tex_create=tex_create, .tex_destroy=tex_destroy, .tex_upload=tex_upload, .tex_download=tex_download,
        .pass_create=pass_create, .pass_destroy=pass_destroy, .pass_run=pass_run,
        .desc_namespace=desc_namespace, .gpu_finish=gpu_finish, .gpu_is_failed=gpu_failed, .buf_create=buf_create,
    };
    atomic_init(&b->fns.cache, NULL);
    // No private allocator/finalizer symbols are exported by upstream. This narrow
    // profile uses public generation/dispatch; emulation dispatch is never enabled.
    ogpu_probe_destroy(probe);
    return gpu;
fail:
    ogpu_probe_destroy(probe);
    ogpu_device_destroy(b->device);
    free(gpu->formats); free(gpu);
    return NULL;
}

struct ogpu_stats ogpu_pl_stats(pl_gpu gpu) { return ((struct backend *) PL_PRIV(gpu))->stats; }
void ogpu_pl_destroy(pl_gpu *gpu)
{
    if (!*gpu) return;
    struct backend *b = PL_PRIV(*gpu);
    if (b->stats.textures || b->stats.passes) {
        fprintf(stderr, "OGPU libplacebo: live children at device destruction\n");
        abort();
    }
    ogpu_device_destroy(b->device);
    free((*gpu)->formats); free((void *) *gpu); *gpu = NULL;
}
