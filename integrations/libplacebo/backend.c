// Integration glue only: upstream owns generation/math/dispatch; OGPU owns execution.
// Serialized, with an explicit bounded two-frame mode. Unsupported requests fail closed.
#include "gpu.h"
#include "backend.h"
#include "compiler.h"
#include "ogpu.h"
#include <stdio.h>

#define SLOTS 8
#define CONSTANTS 16
#define FRAMES 2
#define OPERATIONS 8
#define PARAMETERS 240u
struct bank {
    OgpuCompletion *receipt;
    OgpuImageHeap *images;
    OgpuSamplerHeap *samplers;
    OgpuBuffer *vertices;
    uint64_t vertex_address;
    bool allocated, busy;
};
struct operation {
    OgpuCompletion *done;
    struct bank *bank;
    OgpuBuffer *readback;
    void *ptr;
    size_t size;
    void (*callback)(void *);
    void *priv;
};
struct frame {
    OgpuBatch *batch;
    OgpuCompletion *done;
    struct operation ops[OPERATIONS];
    unsigned count;
    uint64_t generation;
    bool pending, aggregated;
};
struct backend {
    struct pl_gpu_fns fns; // pinned private ABI: must remain first
    OgpuDevice *device;
    struct ogpu_stats stats;
    bool failed;
    int active; // -1 outside explicit frames
    uint64_t generation;
    struct frame frames[FRAMES];
};
struct texture {
    OgpuImage *image;
    OgpuBuffer *staging[FRAMES];
    uint64_t used[FRAMES], staged[FRAMES];
    size_t size;
    bool initialized;
};
struct program {
    struct bank banks[FRAMES];
    struct shader_binary primary, vertex;
    OgpuKernel *kernel;
    OgpuRaster *raster;
    OgpuBuffer *indirect;
    OgpuSpecializationConstant values[CONSTANTS];
    uint32_t root_size, vertex_offset;
};

static bool reject(pl_gpu gpu, const char *why)
{
    struct backend *b = PL_PRIV(gpu);
    fprintf(stderr, "OGPU libplacebo: %s\n", why);
    b->failed = true;
    return false;
}

// Failure paths discard recordings and drain/retire submitted uses before operands
// can be released. Already-retired result receipts may survive resource reuse.
#define TRY(call) do { if ((call) != OGPU_SUCCESS) { reject(gpu, error.message); goto fail; } } while (0)
#define REQUIRE(test, why) do { if (!(test)) { reject(gpu, why); goto fail; } } while (0)
static void gpu_finish(pl_gpu gpu);

static OgpuResult begin_batch(pl_gpu gpu, OgpuBatch **batch, OgpuError *error)
{
    struct backend *b = PL_PRIV(gpu);
    if (b->active >= 0 && b->frames[b->active].aggregated) {
        struct frame *f = &b->frames[b->active];
        if (!f->batch) {
            OgpuResult r = ogpu_batch_create(b->device, &f->batch, error);
            if (r != OGPU_SUCCESS) return r;
        }
        *batch = f->batch;
        return OGPU_SUCCESS;
    }
    return ogpu_batch_create(b->device, batch, error);
}
static void end_batch(pl_gpu gpu, OgpuBatch *batch)
{
    struct backend *b = PL_PRIV(gpu);
    if (b->active >= 0 && b->frames[b->active].batch == batch) return;
    ogpu_batch_destroy(batch);
}

static bool finish(pl_gpu gpu, OgpuBatch *batch, struct bank *bank)
{
    struct backend *b = PL_PRIV(gpu);
    OgpuError error;
    OgpuCompletion *done = NULL;
    bool ok = false;
    struct frame *frame = b->active < 0 ? NULL : &b->frames[b->active];
    REQUIRE(!frame || frame->count < OPERATIONS, "frame operation capacity exceeded");
    if (!frame || !frame->aggregated) {
        TRY(ogpu_batch_submit(batch, &done, &error));
        ++b->stats.submissions;
    }
    if (frame) {
        frame->ops[frame->count++] = (struct operation) {.done=done, .bank=bank};
        if (bank) bank->busy = true;
        ++b->stats.outstanding;
        b->stats.peak_outstanding = PL_MAX(b->stats.peak_outstanding, b->stats.outstanding);
        return true;
    }
    ++b->stats.waits;
    TRY(ogpu_completion_wait(done, &error));
    if (bank) {
        ogpu_completion_destroy(bank->receipt);
        bank->receipt = done;
        done = NULL;
    }
    ok = true;
fail:
    // Destruction drains submitted work even after a wait error.
    ogpu_completion_destroy(done);
    return ok;
}

static bool frame_begin(pl_gpu gpu, unsigned slot, bool aggregated)
{
    struct backend *b = PL_PRIV(gpu);
    if (b->failed || slot >= FRAMES || b->active >= 0 || b->frames[slot].pending ||
        b->generation == UINT64_MAX)
        return reject(gpu, "invalid or premature frame slot reuse");
    b->active = slot;
    b->frames[slot].generation = ++b->generation;
    b->frames[slot].aggregated = aggregated;
    return true;
}
bool ogpu_pl_frame_begin(pl_gpu gpu, unsigned slot) { return frame_begin(gpu, slot, false); }
bool ogpu_pl_frame_begin_batched(pl_gpu gpu, unsigned slot) { return frame_begin(gpu, slot, true); }
bool ogpu_pl_frame_end(pl_gpu gpu)
{
    struct backend *b = PL_PRIV(gpu);
    if (b->active < 0) return reject(gpu, "no active frame");
    struct frame *f = &b->frames[b->active];
    if (f->batch) {
        OgpuError error;
        if (!b->failed) {
            if (ogpu_batch_submit(f->batch, &f->done, &error) != OGPU_SUCCESS)
                reject(gpu, error.message);
            else ++b->stats.submissions;
        }
        ogpu_batch_destroy(f->batch);
        f->batch = NULL;
    }
    f->pending = true;
    b->active = -1;
    ++b->stats.frames;
    ++b->stats.inflight;
    b->stats.peak_inflight = PL_MAX(b->stats.peak_inflight, b->stats.inflight);
    return !b->failed;
}
bool ogpu_pl_frame_collect(pl_gpu gpu, unsigned slot, bool wait)
{
    struct backend *b = PL_PRIV(gpu);
    if (slot >= FRAMES || b->active >= 0)
        return reject(gpu, "invalid collection or active frame");
    struct frame *f = &b->frames[slot];
    if (!f->pending) return !b->failed;
    OgpuError error;
    OgpuCompletion *last = f->aggregated ? f->done : f->count ? f->ops[f->count-1].done : NULL;
    if (last) {
        uint32_t ready = 0;
        ++b->stats.polls;
        OgpuResult result = ogpu_completion_poll(last, &ready, &error);
        if (result != OGPU_SUCCESS) reject(gpu, error.message);
        if (result == OGPU_SUCCESS && !ready) {
            if (!wait) return false;
            ++b->stats.waits;
            if (ogpu_completion_wait(last, &error) != OGPU_SUCCESS)
                reject(gpu, error.message);
        }
    }
    // A shared frame receipt retires every recorded operation at once. On errors
    // destruction drains before bank/staging reuse. No receipt is multiply owned.
    ogpu_completion_destroy(f->done);
    f->done = NULL;
    // Observing a later timeline value does not retire earlier receipts. Retire
    // every operation before clearing heaps or delivering host readbacks.
    for (unsigned i = 0; i < f->count; ++i) {
        struct operation *op = &f->ops[i];
        uint32_t ready = 0;
        if (op->done) {
            ++b->stats.polls;
            if (ogpu_completion_poll(op->done, &ready, &error) != OGPU_SUCCESS || !ready)
                reject(gpu, "frame completion failed to retire");
        }
        if (op->bank) {
            // Destruction drains error paths before releasing raw adapter operands.
            ogpu_completion_destroy(op->bank->receipt);
            op->bank->receipt = op->done;
            if (b->failed) {
                ogpu_completion_destroy(op->bank->receipt);
                op->bank->receipt = NULL;
            }
            op->bank->busy = false;
            if (ogpu_image_heap_clear(op->bank->images, 0, SLOTS, &error) != OGPU_SUCCESS)
                reject(gpu, error.message);
        } else {
            ogpu_completion_destroy(op->done);
        }
        --b->stats.outstanding;
    }
    for (unsigned i = 0; i < f->count; ++i) {
        struct operation *op = &f->ops[i];
        if (op->readback && !b->failed &&
            ogpu_buffer_read(op->readback, 0, op->ptr, op->size, &error) != OGPU_SUCCESS)
            reject(gpu, error.message);
        if (op->callback) { ++b->stats.callbacks; op->callback(op->priv); }
    }
    memset(f->ops, 0, sizeof(f->ops));
    f->count = 0; f->pending = false;
    --b->stats.inflight; ++b->stats.collected;
    return !b->failed;
}

static void mark_use(pl_gpu gpu, struct texture *t)
{
    struct backend *b = PL_PRIV(gpu);
    if (b->active >= 0) t->used[b->active] = b->frames[b->active].generation;
}

static void tex_destroy(pl_gpu gpu, pl_tex tex)
{
    struct backend *b = PL_PRIV(gpu);
    struct texture *t = PL_PRIV(tex);
    gpu_finish(gpu);
    if (t->image) b->stats.texture_bytes -= t->size;
    ogpu_image_destroy(t->image);
    for (unsigned i = 0; i < FRAMES; ++i) {
        if (t->staging[i]) b->stats.staging_bytes -= t->size;
        ogpu_buffer_destroy(t->staging[i]);
    }
    free((void *) tex);
    --b->stats.textures;
}

static bool transfer(pl_gpu gpu, pl_tex tex, void *ptr, bool upload,
                     void (*callback)(void *), void *priv)
{
    struct backend *b = PL_PRIV(gpu);
    struct texture *t = PL_PRIV(tex);
    OgpuError error;
    OgpuBatch *batch = NULL;
    bool ok = false;
    REQUIRE(!b->failed, "device already failed");
    if (b->active < 0) gpu_finish(gpu);
    REQUIRE(!b->failed, "draining previous frames failed");
    const unsigned slot = b->active < 0 ? 0 : b->active;
    struct frame *f = b->active < 0 ? NULL : &b->frames[slot];
    REQUIRE(!f || (f->count < OPERATIONS && t->staged[slot] != f->generation),
            "frame capacity or repeated staging use");
    REQUIRE(!f || upload || callback, "frame readback requires a callback");
    if (!t->staging[slot]) {
        TRY(ogpu_buffer_create(b->device, t->size, OGPU_MEMORY_HOST, &t->staging[slot], &error));
        ++b->stats.staging_allocs; b->stats.staging_bytes += t->size;
        b->stats.peak_staging_bytes = PL_MAX(b->stats.peak_staging_bytes, b->stats.staging_bytes);
    }
    OgpuBuffer *staging = t->staging[slot];
    TRY(begin_batch(gpu, &batch, &error));
    if (upload) {
        TRY(ogpu_buffer_write(staging, 0, ptr, t->size, &error));
        if (!t->initialized)
            TRY(ogpu_batch_discard_image(batch, t->image, &error));
        TRY(ogpu_batch_copy_buffer_to_image(batch, staging, 0, t->image, &error));
    } else {
        REQUIRE(t->initialized, "readback of unwritten image");
        TRY(ogpu_batch_copy_image_to_buffer(batch, t->image, staging, 0, &error));
    }
    if (!finish(gpu, batch, NULL)) goto fail;
    t->initialized = true;
    mark_use(gpu, t);
    if (f) {
        t->staged[slot] = f->generation;
        struct operation *op = &f->ops[f->count-1];
        op->callback = callback; op->priv = priv;
        if (!upload) { op->readback = staging; op->ptr = ptr; op->size = t->size; }
    }
    if (upload) ++b->stats.uploads;
    else {
        if (!f) TRY(ogpu_buffer_read(staging, 0, ptr, t->size, &error));
        ++b->stats.downloads;
    }
    if (!f && callback) { ++b->stats.callbacks; callback(priv); }
    ok = true;
fail:
    end_batch(gpu, batch);
    return ok;
}

static pl_tex tex_create(pl_gpu gpu, const struct pl_tex_params *p)
{
    struct backend *b = PL_PRIV(gpu);
    OgpuError error;
    struct pl_tex_t *tex = NULL;
    const bool rgba = !strcmp(p->format->name, "rgba8");
    const bool half = !strcmp(p->format->name, "rgba16hf");
    const bool unorm16 = !strcmp(p->format->name, "rgba16");
    REQUIRE(!b->failed && (rgba || half || unorm16 || !strcmp(p->format->name, "r32f")), "unsupported image format");
    REQUIRE(!p->d && !p->blit_src && !p->blit_dst && !p->import_handle && !p->export_handle,
            "unsupported image dimension/interop/blit");
    REQUIRE(!p->renderable || ((rgba || half) && p->h), "unsupported color attachment");
    tex = calloc(1, PL_ALIGN_MEM(sizeof(*tex)) + sizeof(struct texture));
    REQUIRE(tex, "image allocation failed");
    ++b->stats.textures;
    ++b->stats.texture_creates;
    b->stats.peak_textures = PL_MAX(b->stats.peak_textures, b->stats.textures);
    tex->params = *p;
    tex->params.initial_data = NULL;
    struct texture *t = PL_PRIV(tex);
    t->size = (size_t) p->w * (p->h ? p->h : 1) * p->format->texel_size;
    const OgpuImageDesc desc = {
        .dimension = p->h ? OGPU_IMAGE_2D : OGPU_IMAGE_1D,
        .width = p->w, .height = p->h ? p->h : 1,
        .format = rgba ? OGPU_FORMAT_RGBA8_UNORM : half ? OGPU_FORMAT_RGBA16_FLOAT :
                  unorm16 ? OGPU_FORMAT_RGBA16_UNORM : OGPU_FORMAT_R32_FLOAT,
        .usage = (p->sampleable ? OGPU_IMAGE_USAGE_SAMPLED : 0) |
                 (p->storable ? OGPU_IMAGE_USAGE_STORAGE : 0) |
                 (p->renderable ? OGPU_IMAGE_USAGE_COLOR : 0) |
                 (p->host_readable ? OGPU_IMAGE_USAGE_COPY_SRC : 0) |
                 (p->host_writable || p->initial_data ? OGPU_IMAGE_USAGE_COPY_DST : 0),
    };
    TRY(ogpu_image_create(b->device, &desc, &t->image, &error));
    b->stats.texture_bytes += t->size;
    b->stats.peak_texture_bytes = PL_MAX(b->stats.peak_texture_bytes, b->stats.texture_bytes);
    if (p->initial_data && !transfer(gpu, tex, (void *) p->initial_data, true, NULL, NULL)) goto fail;
    return tex;
fail:
    if (tex) tex_destroy(gpu, tex);
    return NULL;
}

static bool tex_transfer(pl_gpu gpu, const struct pl_tex_transfer_params *p, bool upload)
{
    const struct pl_tex_params *t = &p->tex->params;
    if (!p->ptr || p->buf || p->timer || p->rc.x0 || p->rc.x1 != t->w ||
        (t->h && (p->rc.y0 || p->rc.y1 != t->h || p->row_pitch != (size_t) t->w * t->format->texel_size)))
        return reject(gpu, "only full packed host image transfers are supported");
    return transfer(gpu, p->tex, p->ptr, upload, p->callback, p->priv);
}
static bool tex_upload(pl_gpu gpu, const struct pl_tex_transfer_params *p) { return tex_transfer(gpu, p, true); }
static bool tex_download(pl_gpu gpu, const struct pl_tex_transfer_params *p) { return tex_transfer(gpu, p, false); }

static void pass_destroy(pl_gpu gpu, pl_pass pass)
{
    struct backend *b = PL_PRIV(gpu);
    struct program *p = PL_PRIV(pass);
    gpu_finish(gpu);
    for (unsigned i = 0; i < FRAMES; ++i) {
        struct bank *bank = &p->banks[i];
        ogpu_completion_destroy(bank->receipt);
        ogpu_image_heap_destroy(bank->images);
        ogpu_sampler_heap_destroy(bank->samplers);
        ogpu_buffer_destroy(bank->vertices);
        if (bank->allocated) --b->stats.banks;
    }
    ogpu_kernel_destroy(p->kernel);
    ogpu_raster_destroy(p->raster);
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
        const uint32_t format = !strcmp(pass->params.target_format->name, "rgba8") ?
            OGPU_FORMAT_RGBA8_UNORM : OGPU_FORMAT_RGBA16_FLOAT;
        TRY(ogpu_raster_create(b->device, &vertex, &shader, p->root_size,
            OGPU_TOPOLOGY_TRIANGLE_STRIP, format, &raster, &error));
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
    REQUIRE(in->push_constants_size <= gpu->limits.max_pushc_size &&
            in->push_constants_size <= PARAMETERS && in->push_constants_size % 4 == 0,
            "unsupported pass root size");
    REQUIRE(compute || in->type == PL_PASS_RASTER, "unsupported pass type");
    if (!compute) {
        REQUIRE(in->vertex_stride == 16 && in->num_vertex_attribs == 2 &&
                in->vertex_type == PL_PRIM_TRIANGLE_STRIP &&
                !in->blend_params && (!strcmp(in->target_format->name, "rgba8") ||
                !strcmp(in->target_format->name, "rgba16hf")), "unsupported raster layout");
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
    p->vertex_offset = (in->push_constants_size + 7u) & ~7u;
    p->root_size = compute ? in->push_constants_size : p->vertex_offset + 8;
    REQUIRE(compile_native(in->glsl_shader, compute ? 0 : 1, NULL, NULL, in->push_constants_size, &p->primary), "shader compilation failed");
    if (!compute) {
        REQUIRE(compile_native(in->vertex_shader, 2, in->vertex_attribs[0].name,
                in->vertex_attribs[1].name, in->push_constants_size, &p->vertex), "vertex compilation failed");
        TRY(ogpu_buffer_create(b->device, 16, OGPU_MEMORY_HOST, &p->indirect, &error));
        const OgpuDrawArguments args = {4, 1, 0, 0};
        TRY(ogpu_buffer_write(p->indirect, 0, &args, sizeof(args), &error));
    }
    for (unsigned i = 0; i < FRAMES; ++i) {
        struct bank *bank = &p->banks[i];
        bank->allocated = true;
        ++b->stats.banks;
        b->stats.peak_banks = PL_MAX(b->stats.peak_banks, b->stats.banks);
        TRY(ogpu_image_heap_create(b->device, SLOTS, &bank->images, &error));
        TRY(ogpu_sampler_heap_create(b->device, SLOTS, &bank->samplers, &error));
        if (!compute) {
            TRY(ogpu_buffer_create(b->device, 64, OGPU_MEMORY_HOST, &bank->vertices, &error));
            TRY(ogpu_buffer_device_address(bank->vertices, &bank->vertex_address, &error));
        }
    }
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
    if (b->active < 0) gpu_finish(gpu);
    struct bank *bank = &p->banks[b->active < 0 ? 0 : b->active];
    const bool reusing_receipt = bank->receipt != NULL;
    OgpuError error;
    OgpuBatch *batch = NULL;
    REQUIRE(!b->failed && !in->num_var_updates && !in->timer, "unsupported run variables/timer or failed device");
    REQUIRE(!bank->busy && (b->active < 0 || b->frames[b->active].count < OPERATIONS),
            "pass bank already in use or frame capacity exceeded");
    if (!specialize(gpu, in->pass, in->constant_data)) goto fail;
    if (!compute) {
        REQUIRE(in->vertex_data && !in->vertex_buf && !in->index_data && !in->index_buf &&
                in->vertex_count == 4 && full_rect(in->viewport, in->target) &&
                full_rect(in->scissors, in->target), "unsupported draw region/vertices");
        TRY(ogpu_buffer_write(bank->vertices, 0, in->vertex_data, 64, &error));
    }
    TRY(begin_batch(gpu, &batch, &error));
    // One queue; order previous writes before this pass's reads/writes. Host writes
    // are made visible by OGPU submission. No guessed resource-pointer tracing.
    TRY(ogpu_batch_barrier(batch,
        OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_COLOR_READ |
        OGPU_ACCESS_COLOR_WRITE | OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE |
        OGPU_ACCESS_FRAGMENT_READ | OGPU_ACCESS_VERTEX_READ | OGPU_ACCESS_INDIRECT_READ,
        OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_FRAGMENT_READ |
        OGPU_ACCESS_VERTEX_READ | OGPU_ACCESS_INDIRECT_READ | OGPU_ACCESS_COLOR_READ |
        OGPU_ACCESS_COLOR_WRITE, &error));
    for (int i = 0; i < params->num_descriptors; ++i) {
        const struct pl_desc *d = &params->descriptors[i];
        const struct pl_desc_binding *binding = &in->desc_bindings[i];
        pl_tex tex = binding->object;
        struct texture *t = PL_PRIV(tex);
        mark_use(gpu, t);
        const bool sampled = d->type == PL_DESC_SAMPLED_TEX;
        if (!t->initialized) {
            REQUIRE(!sampled, "sample of unwritten image");
            TRY(ogpu_batch_discard_image(batch, t->image, &error));
        }
        const OgpuImageEntry entry = {t->image, sampled ? OGPU_IMAGE_SAMPLED : OGPU_IMAGE_STORAGE, 0};
        TRY(ogpu_image_heap_write(bank->images, d->binding, &entry, 1, &error));
        if (sampled) {
            REQUIRE(binding->sample_mode == PL_TEX_SAMPLE_NEAREST || binding->sample_mode == PL_TEX_SAMPLE_LINEAR,
                    "unsupported sampler filter");
            REQUIRE(binding->address_mode == PL_TEX_ADDRESS_CLAMP || binding->address_mode == PL_TEX_ADDRESS_REPEAT,
                    "unsupported sampler address mode");
            const uint32_t filter = binding->sample_mode == PL_TEX_SAMPLE_LINEAR ? OGPU_FILTER_LINEAR : OGPU_FILTER_NEAREST;
            const uint32_t address = binding->address_mode == PL_TEX_ADDRESS_REPEAT ? OGPU_ADDRESS_REPEAT : OGPU_ADDRESS_CLAMP;
            const OgpuSamplerDesc sampler = {filter, filter, address, address};
            TRY(ogpu_sampler_heap_write(bank->samplers, d->binding, &sampler, 1, &error));
        }
    }
    TRY(ogpu_batch_bind_image_heap(batch, bank->images, &error));
    TRY(ogpu_batch_bind_sampler_heap(batch, bank->samplers, &error));
    if (compute) {
        TRY(ogpu_batch_dispatch(batch, p->kernel, in->compute_groups[0], in->compute_groups[1],
            in->compute_groups[2], in->push_constants, params->push_constants_size, &error));
    } else {
        struct texture *target = PL_PRIV(in->target);
        uint8_t root[PARAMETERS + 8] = {0};
        REQUIRE(!params->push_constants_size || in->push_constants, "missing raster parameters");
        if (params->push_constants_size) memcpy(root, in->push_constants, params->push_constants_size);
        memcpy(root + p->vertex_offset, &bank->vertex_address, 8);
        mark_use(gpu, target);
        TRY(ogpu_batch_retain_buffer(batch, bank->vertices, &error));
        TRY(ogpu_batch_draw_indirect(batch, p->raster, target->image, p->indirect, 0,
            root, p->root_size, params->load_target ? OGPU_ATTACHMENT_LOAD : OGPU_ATTACHMENT_CLEAR, &error));
    }
    if (!finish(gpu, batch, bank)) goto fail;
    if (reusing_receipt) ++b->stats.receipt_reuses;
    for (int i = 0; i < params->num_descriptors; ++i) {
        struct texture *t = PL_PRIV((pl_tex) in->desc_bindings[i].object);
        t->initialized = true;
    }
    if (compute) ++b->stats.compute;
    else { ((struct texture *) PL_PRIV(in->target))->initialized = true; ++b->stats.raster; }
fail:
    end_batch(gpu, batch);
    // Async banks are cleared only after their slot's receipts retire.
    if (!bank->busy && ogpu_image_heap_clear(bank->images, 0, SLOTS, &error) != OGPU_SUCCESS)
        reject(gpu, error.message);
}

static int desc_namespace(pl_gpu gpu, enum pl_desc_type type) { return 0; }
static void gpu_finish(pl_gpu gpu)
{
    struct backend *b = PL_PRIV(gpu);
    if (b->active >= 0) ogpu_pl_frame_end(gpu);
    // Collect in submission order, including after a partial-frame failure.
    unsigned first = b->frames[0].generation < b->frames[1].generation ? 0 : 1;
    ogpu_pl_frame_collect(gpu, first, true);
    ogpu_pl_frame_collect(gpu, 1-first, true);
}
static bool tex_poll(pl_gpu gpu, pl_tex tex, uint64_t timeout)
{
    struct backend *b = PL_PRIV(gpu);
    struct texture *t = PL_PRIV(tex);
    bool busy = false;
    for (unsigned i = 0; i < FRAMES; ++i) {
        if (t->used[i] != b->frames[i].generation) continue;
        if (b->active >= 0) {
            busy |= b->frames[i].pending || b->active == (int) i;
        } else if (b->frames[i].pending) {
            ogpu_pl_frame_collect(gpu, i, timeout == UINT64_MAX);
            busy |= b->frames[i].pending;
        }
    }
    return busy;
}
static void gpu_flush(pl_gpu gpu) { /* Operations submit immediately. */ }
static bool gpu_failed(pl_gpu gpu) { return ((struct backend *) PL_PRIV(gpu))->failed; }
static pl_buf buf_create(pl_gpu gpu, const struct pl_buf_params *p)
{ reject(gpu, "general libplacebo buffers are outside this adapter"); return NULL; }

static const struct pl_fmt_t formats[] = {
    {.name="rgba16", .signature=5, .type=PL_FMT_UNORM,
     .caps=PL_FMT_CAP_SAMPLEABLE|PL_FMT_CAP_LINEAR|PL_FMT_CAP_HOST_READABLE,
     .num_components=4, .component_depth={16,16,16,16}, .internal_size=8,
     .texel_size=8, .texel_align=2, .host_bits={16,16,16,16}, .sample_order={0,1,2,3}},
    {.name="rgba16hf", .signature=4, .type=PL_FMT_FLOAT,
     .caps=PL_FMT_CAP_SAMPLEABLE|PL_FMT_CAP_LINEAR|PL_FMT_CAP_STORABLE|PL_FMT_CAP_RENDERABLE|PL_FMT_CAP_HOST_READABLE,
     .num_components=4, .component_depth={16,16,16,16}, .internal_size=8,
     .texel_size=8, .texel_align=2, .host_bits={16,16,16,16}, .sample_order={0,1,2,3}, .glsl_format="rgba16f"},
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
    b->active = -1;
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
        {.dimension=2, .width=1, .height=1, .format=OGPU_FORMAT_RGBA16_UNORM,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
        {.dimension=1, .width=1, .height=1, .format=OGPU_FORMAT_RGBA16_UNORM,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
        {.dimension=2, .width=1, .height=1, .format=OGPU_FORMAT_RGBA16_FLOAT,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_STORAGE|OGPU_IMAGE_USAGE_COLOR|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
        {.dimension=1, .width=1, .height=1, .format=OGPU_FORMAT_RGBA16_FLOAT,
         .usage=OGPU_IMAGE_USAGE_SAMPLED|OGPU_IMAGE_USAGE_STORAGE|OGPU_IMAGE_USAGE_COPY_SRC|OGPU_IMAGE_USAGE_COPY_DST},
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
        .callbacks=true,
        .max_tex_1d_dim=limits.max_image_1d, .max_tex_2d_dim=limits.max_image_2d,
        .max_constants=CONSTANTS, .array_size_constants=true,
        // Reserve the aligned appended vertex address alongside native parameters.
        .max_pushc_size=PL_MIN(limits.max_push_data_bytes >= 8 ? (limits.max_push_data_bytes - 8) & ~7ull : 0, PARAMETERS),
        .align_vertex_stride=1, .align_tex_xfer_pitch=4, .align_tex_xfer_offset=4,
        .fragment_queues=1, .compute_queues=1,
    };
    memcpy(gpu->limits.max_dispatch, limits.max_dispatch, sizeof(limits.max_dispatch));
    const int format_count = sizeof(formats) / sizeof(formats[0]);
    gpu->formats = calloc(format_count, sizeof(pl_fmt));
    REQUIRE(gpu->formats, "format allocation failed");
    for (int i = 0; i < format_count; ++i) gpu->formats[i] = &formats[i];
    gpu->num_formats = format_count;
    b->fns = (struct pl_gpu_fns) {
        .tex_create=tex_create, .tex_destroy=tex_destroy, .tex_upload=tex_upload, .tex_download=tex_download,
        .pass_create=pass_create, .pass_destroy=pass_destroy, .pass_run=pass_run,
        .desc_namespace=desc_namespace, .gpu_finish=gpu_finish, .gpu_flush=gpu_flush,
        .tex_poll=tex_poll, .gpu_is_failed=gpu_failed, .buf_create=buf_create,
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
    gpu_finish(*gpu);
    ogpu_device_destroy(b->device);
    free((*gpu)->formats); free((void *) *gpu); *gpu = NULL;
}
