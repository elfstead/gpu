// Integration glue only: upstream owns generation/math/dispatch; OGPU owns execution.
// Serialized, with an explicit bounded two-frame mode. Unsupported requests fail closed.
#include "gpu.h"
#include "backend.h"
#include "compiler.h"
#include "ogpu_next.h"
#include <stdio.h>

#define SLOTS 8
#define CONSTANTS 16
#define FRAMES 2
#define OPERATIONS 8
#define PARAMETERS 240u
#define RECORD(type, kind) {kind, OGPU_NEXT_VERSION, sizeof(type), 0, NULL}
struct memory { ogpu_next_memory *handle; ogpu_next_mapping map; uint64_t size; };
struct heap { struct memory *storage; ogpu_next_heap_binding binding; uint64_t stride, payload; };
struct variant { ogpu_next_executable *executable; unsigned references; };
struct bank {
    bool reusable;
    struct heap images, samplers;
    struct memory *vertices;
    struct variant *variant;
    uint64_t vertex_address;
    bool allocated, busy;
};
struct operation {
    struct bank *bank;
    struct memory *readback;
    void *ptr;
    size_t size;
    void (*callback)(void *);
    void *priv;
};
struct frame {
    ogpu_next_arena *arena;
    ogpu_next_encoder *batch;
    uint64_t done;
    struct operation ops[OPERATIONS];
    unsigned count;
    uint64_t generation;
    bool pending, aggregated;
};
struct backend {
    struct pl_gpu_fns fns; // pinned private ABI: must remain first
    ogpu_next_device *device;
    ogpu_next_queue *queue;
    ogpu_next_timeline *timeline;
    ogpu_next_arena *serial;
    ogpu_next_encoder *recording;
    uint64_t submitted;
    ogpu_next_memory_type_info types[32];
    uint32_t type_count;
    ogpu_next_memory_limits memory_limits;
    ogpu_next_descriptor_limits descriptors;
    ogpu_next_host_span scratch;
    struct ogpu_stats stats;
    bool failed;
    int active; // -1 outside explicit frames
    uint64_t generation;
    struct frame frames[FRAMES];
};
struct texture {
    ogpu_next_image *image;
    ogpu_next_memory *backing;
    ogpu_next_view *view;
    struct memory *staging[FRAMES];
    uint64_t used[FRAMES], staged[FRAMES];
    size_t size;
    bool initialized, accounted;
};
struct program {
    struct bank banks[FRAMES];
    struct shader_binary primary, vertex;
    struct variant *current;
    struct memory *indirect;
    struct shader_constant values[CONSTANTS];
    uint32_t root_size, vertex_offset;
};

static bool reject(pl_gpu gpu, const char *why)
{
    struct backend *b = PL_PRIV(gpu);
    fprintf(stderr, "OGPU libplacebo: %s\n", why);
    b->failed = true;
    return false;
}

// All policies below belong to this bounded consumer, not the foundation runtime.
#define TRY(call) do { ogpu_next_status status_ = (call); if (status_ != OGPU_NEXT_OK) { \
    fprintf(stderr, "OGPU foundation: %s status=%d\n", #call, status_); reject(gpu, "foundation call failed"); goto fail; } } while (0)
#define REQUIRE(test, why) do { if (!(test)) { reject(gpu, why); goto fail; } } while (0)
static void gpu_finish(pl_gpu gpu);

static ogpu_next_query query(uint32_t kind, void *data, uint32_t count, uint32_t size)
{ return (ogpu_next_query){RECORD(ogpu_next_query, kind), data, count, 0, size, 0}; }
static uint64_t aligned(uint64_t n, uint64_t a) { return (n + a - 1) & ~(a - 1); }
static ogpu_next_span span(struct memory *m) { return (ogpu_next_span){m->handle, 0, m->size}; }
static void memory_destroy(struct memory *m)
{ if (m) { ogpu_next_memory_destroy(m->handle); free(m); } }
static bool memory_create(pl_gpu gpu, uint64_t size, uint64_t alignment, uint64_t usage, struct memory **out)
{
    struct backend *b = PL_PRIV(gpu);
    struct memory *m = calloc(1, sizeof(*m));
    REQUIRE(m, "host allocation failed");
    uint32_t compatible[32];
    ogpu_next_requirements req = {.compatible_types=compatible, .compatible_type_capacity=32};
    ogpu_next_memory_desc desc = {RECORD(ogpu_next_memory_desc, OGPU_NEXT_MEMORY_DESC),
        size, alignment, usage, UINT32_MAX, OGPU_NEXT_MEMORY_LINEAR, NULL, 0, 0};
    TRY(ogpu_next_memory_requirements(b->device, &desc, &req));
    for (uint32_t i = 0; i < req.compatible_type_count && desc.memory_type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < b->type_count; ++j)
            if (compatible[i] == b->types[j].id && (b->types[j].properties & OGPU_NEXT_MEMORY_HOST_VISIBLE)) {
                desc.memory_type = compatible[i]; break;
            }
    REQUIRE(desc.memory_type != UINT32_MAX, "host-visible memory type unavailable");
    m->size = size;
    TRY(ogpu_next_memory_create(b->device, &desc, &m->handle));
    TRY(ogpu_next_memory_map(span(m), &m->map));
    *out = m; return true;
fail:
    memory_destroy(m); return false;
}
static ogpu_next_status memory_write(struct memory *m, const void *data, size_t size)
{
    if (size > m->size) return OGPU_NEXT_INVALID;
    memcpy(m->map.data, data, size);
    return ogpu_next_memory_flush((ogpu_next_span){m->handle, 0, size});
}
static ogpu_next_status memory_read(struct memory *m, void *data, size_t size)
{
    if (size > m->size) return OGPU_NEXT_INVALID;
    ogpu_next_status r = ogpu_next_memory_invalidate((ogpu_next_span){m->handle, 0, size});
    if (r == OGPU_NEXT_OK) memcpy(data, m->map.data, size);
    return r;
}
static bool heap_create(pl_gpu gpu, uint32_t kind, struct heap *h)
{
    struct backend *b = PL_PRIV(gpu);
    const ogpu_next_descriptor_limits *d = &b->descriptors;
    bool sampler = kind == OGPU_NEXT_HEAP_SAMPLER;
    h->stride = aligned(sampler ? d->sampler_size : d->image_size,
                        sampler ? d->sampler_alignment : d->image_alignment);
    h->payload = h->stride * SLOTS;
    uint64_t reserved = aligned(h->payload, PL_MAX(b->memory_limits.cache_atom_size,
        sampler ? d->sampler_reserved_alignment : d->resource_reserved_alignment));
    uint64_t reserved_size = sampler ? d->sampler_reserved_size : d->resource_reserved_size;
    uint64_t size = reserved + reserved_size;
    uint64_t alignment = PL_MAX(b->memory_limits.cache_atom_size,
        sampler ? d->sampler_heap_alignment : d->resource_heap_alignment);
    if (!memory_create(gpu, size, alignment, OGPU_NEXT_USAGE_DESCRIPTOR_HEAP, &h->storage)) return false;
    // Whole allocations isolate banks. Cache atoms never overlap reserved bytes.
    REQUIRE(h->storage->map.cache_offset == 0, "heap mapping cache offset");
    h->binding = (ogpu_next_heap_binding){RECORD(ogpu_next_heap_binding, OGPU_NEXT_HEAP_BINDING),
        kind, 0, span(h->storage), reserved, reserved_size};
    return true;
fail:
    return false;
}
static void variant_release(struct variant *v)
{
    if (v && !--v->references) { ogpu_next_executable_destroy(v->executable); free(v); }
}
static void drain(struct backend *b, uint64_t value)
{
    if (value && ogpu_next_timeline_wait((ogpu_next_point){b->timeline, value}, UINT64_MAX) != OGPU_NEXT_OK) {
        // Unknown completion must not unwind borrowed native operands.
        fprintf(stderr, "OGPU libplacebo: completion unavailable; refusing unsafe teardown\n");
        abort();
    }
}
static void reset_arena(ogpu_next_arena *arena)
{
    if (ogpu_next_arena_reset(arena) != OGPU_NEXT_OK) {
        fprintf(stderr, "OGPU libplacebo: arena reset failed\n"); abort();
    }
}
static ogpu_next_status begin_batch(pl_gpu gpu, ogpu_next_encoder **batch)
{
    struct backend *b = PL_PRIV(gpu);
    struct frame *f = b->active < 0 ? NULL : &b->frames[b->active];
    if (f && f->batch) { *batch = f->batch; return OGPU_NEXT_OK; }
    const ogpu_next_recording_desc desc = {RECORD(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC),
        OGPU_NEXT_ONE_SHOT, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_status r = ogpu_next_commands_begin(f ? f->arena : b->serial, &desc, batch);
    if (r == OGPU_NEXT_OK) {
        b->recording = *batch;
        if (f && f->aggregated) f->batch = *batch;
    }
    return r;
}
static void end_batch(pl_gpu gpu, ogpu_next_encoder *batch)
{
    struct backend *b = PL_PRIV(gpu);
    if (!batch) return;
    if (b->active >= 0 && b->frames[b->active].batch == batch) return;
    if (b->recording == batch) { ogpu_next_commands_cancel(batch); b->recording = NULL; }
    if (b->active < 0) reset_arena(b->serial);
}
static bool submit(pl_gpu gpu, ogpu_next_encoder *batch, uint64_t *done)
{
    struct backend *b = PL_PRIV(gpu);
    ogpu_next_list *list = NULL;
    b->recording = NULL;
    TRY(ogpu_next_commands_end(batch, &list));
    REQUIRE(b->submitted < UINT64_MAX, "timeline exhausted");
    const ogpu_next_sync_point signal = {{b->timeline, b->submitted + 1}, OGPU_NEXT_STAGE_ALL};
    const ogpu_next_submit_desc desc = {RECORD(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC),
        1, 0, 1, &list, NULL, &signal, b->scratch.data, b->scratch.size};
    ogpu_next_status r = ogpu_next_queue_submit(b->queue, &desc);
    if (r == OGPU_NEXT_DEVICE_LOST || r == OGPU_NEXT_BACKEND_ERROR) {
        fprintf(stderr, "OGPU libplacebo: terminal submission failure; refusing unsafe teardown\n");
        abort();
    }
    TRY(r); // Rejected validation/OOM submissions have no pending execution.
    *done = ++b->submitted; ++b->stats.submissions;
    return true;
fail:
    return false;
}
static bool finish(pl_gpu gpu, ogpu_next_encoder *batch, struct bank *bank, struct variant *variant)
{
    struct backend *b = PL_PRIV(gpu);
    struct frame *f = b->active < 0 ? NULL : &b->frames[b->active];
    uint64_t done = 0;
    REQUIRE(!f || f->count < OPERATIONS, "frame operation capacity exceeded");
    if (!f || !f->aggregated) {
        if (!submit(gpu, batch, &done)) return false;
        if (f) f->done = done;
    }
    if (f) {
        f->ops[f->count++] = (struct operation){.bank=bank};
        if (bank) { bank->busy = true; bank->variant = variant; ++variant->references; }
        ++b->stats.outstanding;
        b->stats.peak_outstanding = PL_MAX(b->stats.peak_outstanding, b->stats.outstanding);
    } else {
        ++b->stats.waits; drain(b, done);
        if (bank) bank->reusable = true;
    }
    return true;
fail:
    return false;
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
        if (!b->failed) submit(gpu, f->batch, &f->done);
        else { ogpu_next_commands_cancel(f->batch); b->recording = NULL; }
        f->batch = NULL;
    }
    f->pending = true; b->active = -1;
    ++b->stats.frames; ++b->stats.inflight;
    b->stats.peak_inflight = PL_MAX(b->stats.peak_inflight, b->stats.inflight);
    return !b->failed;
}
bool ogpu_pl_frame_collect(pl_gpu gpu, unsigned slot, bool wait)
{
    struct backend *b = PL_PRIV(gpu);
    if (slot >= FRAMES || b->active >= 0) return reject(gpu, "invalid collection or active frame");
    struct frame *f = &b->frames[slot];
    if (!f->pending) return !b->failed;
    if (f->done) {
        uint64_t completed = 0; ++b->stats.polls;
        ogpu_next_status r = ogpu_next_timeline_poll(b->timeline, &completed);
        if (r != OGPU_NEXT_OK) reject(gpu, "timeline poll failed");
        if (r != OGPU_NEXT_OK || completed < f->done) {
            if (r == OGPU_NEXT_OK && !wait) return false;
            ++b->stats.waits; drain(b, f->done);
        }
    }
    // ALL-scoped completion of the last submission covers every earlier list
    // on this queue. Discard all recorded references BEFORE operands can die.
    reset_arena(f->arena);
    for (unsigned i = 0; i < f->count; ++i) {
        struct operation *op = &f->ops[i];
        if (op->bank) {
            variant_release(op->bank->variant); op->bank->variant = NULL;
            op->bank->busy = false; op->bank->reusable = !b->failed;
        }
        --b->stats.outstanding;
    }
    for (unsigned i = 0; i < f->count; ++i) {
        struct operation *op = &f->ops[i];
        if (op->readback && !b->failed && memory_read(op->readback, op->ptr, op->size) != OGPU_NEXT_OK)
            reject(gpu, "readback invalidate failed");
        if (op->callback) { ++b->stats.callbacks; op->callback(op->priv); }
    }
    memset(f->ops, 0, sizeof(f->ops));
    f->count = 0; f->pending = false; f->done = 0;
    --b->stats.inflight; ++b->stats.collected;
    return !b->failed;
}

// Conservative single-queue consumer hazards, not runtime-inserted barriers.
static void order(ogpu_next_encoder *batch)
{
    const ogpu_next_dependency dep = {RECORD(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY),
        OGPU_NEXT_STAGE_ALL, OGPU_NEXT_STAGE_ALL, OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE,
        OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE, 0, 0, 0, NULL, NULL, NULL, 0};
    ogpu_next_barrier(batch, &dep);
}
static void discard_image(struct backend *b, ogpu_next_encoder *batch, struct texture *t)
{
    const ogpu_next_image_barrier image = {t->image, {OGPU_NEXT_ASPECT_COLOR, 0, 1, 0, 1},
        OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE, OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE,
        OGPU_NEXT_STATE_UNDEFINED, OGPU_NEXT_STATE_GENERAL, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 1};
    const ogpu_next_dependency dep = {RECORD(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY),
        OGPU_NEXT_STAGE_ALL, OGPU_NEXT_STAGE_ALL, 0, 0, 0, 1, 0, NULL, &image, b->scratch.data, b->scratch.size};
    ogpu_next_barrier(batch, &dep);
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
    if (t->accounted) b->stats.texture_bytes -= t->size;
    ogpu_next_view_destroy(t->view);
    ogpu_next_image_destroy(t->image);
    ogpu_next_memory_destroy(t->backing);
    for (unsigned i = 0; i < FRAMES; ++i) {
        if (t->staging[i]) b->stats.staging_bytes -= t->size;
        memory_destroy(t->staging[i]);
    }
    free((void *) tex);
    --b->stats.textures;
}

static bool transfer(pl_gpu gpu, pl_tex tex, void *ptr, bool upload,
                     void (*callback)(void *), void *priv)
{
    struct backend *b = PL_PRIV(gpu);
    struct texture *t = PL_PRIV(tex);
    ogpu_next_encoder *batch = NULL;
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
        if (!memory_create(gpu, t->size, 8, OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST, &t->staging[slot])) goto fail;
        ++b->stats.staging_allocs; b->stats.staging_bytes += t->size;
        b->stats.peak_staging_bytes = PL_MAX(b->stats.peak_staging_bytes, b->stats.staging_bytes);
    }
    struct memory *staging = t->staging[slot];
    TRY(begin_batch(gpu, &batch));
    order(batch);
    const ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR, 0, 0, 1, {0, 0, 0},
        {tex->params.w, tex->params.h ? tex->params.h : 1, 1}}, 0, 0, OGPU_NEXT_STATE_GENERAL, 0};
    if (upload) {
        TRY(memory_write(staging, ptr, t->size));
        if (!t->initialized)
            discard_image(b, batch, t);
        ogpu_next_copy_to_image(batch, t->image, span(staging), &copy);
    } else {
        REQUIRE(t->initialized, "readback of unwritten image");
        ogpu_next_copy_from_image(batch, span(staging), t->image, &copy);
        const ogpu_next_dependency host = {RECORD(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY),
            OGPU_NEXT_STAGE_COPY, OGPU_NEXT_STAGE_HOST, OGPU_NEXT_ACCESS_COPY_WRITE, OGPU_NEXT_ACCESS_HOST_READ,
            0, 0, 0, NULL, NULL, NULL, 0};
        ogpu_next_barrier(batch, &host);
    }
    if (!finish(gpu, batch, NULL, NULL)) goto fail;
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
        if (!f) TRY(memory_read(staging, ptr, t->size));
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
    const ogpu_next_image_desc desc = {
        .header = RECORD(ogpu_next_image_desc, OGPU_NEXT_IMAGE_DESC),
        .dimension = p->h ? OGPU_NEXT_IMAGE_2D : OGPU_NEXT_IMAGE_1D,
        .extent = {p->w, p->h ? p->h : 1, 1}, .mip_count=1, .layer_count=1, .sample_count=1,
        .format = rgba ? OGPU_NEXT_RGBA8_UNORM : half ? OGPU_NEXT_RGBA16_FLOAT :
                  unorm16 ? OGPU_NEXT_RGBA16_UNORM : OGPU_NEXT_R32_FLOAT,
        .usage = (p->sampleable ? OGPU_NEXT_IMAGE_SAMPLED : 0) |
                 (p->storable ? OGPU_NEXT_IMAGE_STORAGE : 0) |
                 (p->renderable ? OGPU_NEXT_IMAGE_COLOR_ATTACHMENT : 0) |
                 (p->host_readable ? OGPU_NEXT_IMAGE_COPY_SRC : 0) |
                 (p->host_writable || p->initial_data ? OGPU_NEXT_IMAGE_COPY_DST : 0),
    };
    uint32_t compatible[32], type = UINT32_MAX;
    ogpu_next_requirements req = {.compatible_types=compatible, .compatible_type_capacity=32};
    TRY(ogpu_next_image_requirements(b->device, &desc, &req));
    for (uint32_t i = 0; i < req.compatible_type_count && type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < b->type_count; ++j)
            if (compatible[i] == b->types[j].id && (b->types[j].properties & OGPU_NEXT_MEMORY_LOCAL)) { type = compatible[i]; break; }
    REQUIRE(type != UINT32_MAX, "local image memory unavailable");
    TRY(ogpu_next_image_create_unbound(b->device, &desc, &t->image));
    TRY(ogpu_next_memory_create_dedicated_image(t->image, type, &t->backing));
    TRY(ogpu_next_image_bind(t->image, (ogpu_next_span){t->backing, 0, req.size}));
    uint32_t view_usage = desc.usage & (OGPU_NEXT_IMAGE_SAMPLED | OGPU_NEXT_IMAGE_STORAGE | OGPU_NEXT_IMAGE_COLOR_ATTACHMENT);
    if (view_usage) {
        const ogpu_next_view_desc view = {RECORD(ogpu_next_view_desc, OGPU_NEXT_VIEW_DESC),
            desc.format, p->h ? OGPU_NEXT_VIEW_2D : OGPU_NEXT_VIEW_1D, view_usage, {0, 0, 0, 0},
            {OGPU_NEXT_ASPECT_COLOR, 0, 1, 0, 1}};
        TRY(ogpu_next_view_create(t->image, &view, &t->view));
    }
    t->accounted = true;
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
        variant_release(bank->variant);
        memory_destroy(bank->images.storage);
        memory_destroy(bank->samplers.storage);
        memory_destroy(bank->vertices);
        if (bank->allocated) --b->stats.banks;
    }
    variant_release(p->current);
    memory_destroy(p->indirect);
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
    struct variant *variant = NULL;
    struct shader_constant values[CONSTANTS] = {0};
    const unsigned count = pass->params.num_constants;
    if (!data && p->current) return true;
    REQUIRE(!count || data, "missing specialization values");
    ogpu_next_specialization_entry entries[CONSTANTS];
    uint32_t bits[CONSTANTS];
    for (unsigned i = 0; i < count; ++i) {
        values[i].id = pass->params.constants[i].id;
        memcpy(&values[i].bits, (const char *) data + pass->params.constants[i].offset, 4);
        entries[i] = (ogpu_next_specialization_entry){values[i].id, i * 4, 4};
        bits[i] = values[i].bits;
    }
    if (p->current && !memcmp(values, p->values, count * sizeof(*values))) return true;
    const bool compute = pass->params.type == PL_PASS_COMPUTE;
    struct shader_metadata metadata = {0};
    if (compute) REQUIRE(reflect_compute_native(&p->primary, values, count, &metadata), "shader metadata failed");
    const ogpu_next_shader_requirements requirements = {RECORD(ogpu_next_shader_requirements, OGPU_NEXT_SHADER_REQUIREMENTS),
        0, {metadata.local[0], metadata.local[1], metadata.local[2]}, metadata.shared};
    const ogpu_next_argument_interface interface = {RECORD(ogpu_next_argument_interface, OGPU_NEXT_ARGUMENT_INTERFACE),
        p->root_size, 0, NULL};
    const ogpu_next_specialization specialization = {RECORD(ogpu_next_specialization, OGPU_NEXT_SPECIALIZATION),
        count, 0, entries, {bits, count * 4}};
    ogpu_next_shader shaders[2] = {
        {compute ? OGPU_NEXT_STAGE_COMPUTE : OGPU_NEXT_STAGE_FRAGMENT, OGPU_NEXT_SHADER_SPIRV,
            {p->primary.words, p->primary.count * 4}, "main", &interface.header, count ? &specialization.header : NULL, NULL},
        {OGPU_NEXT_STAGE_VERTEX, OGPU_NEXT_SHADER_SPIRV,
            {p->vertex.words, p->vertex.count * 4}, "main", &interface.header, count ? &specialization.header : NULL, NULL},
    };
    ogpu_next_color_state color = {.write_mask=15};
    if (!compute) {
        ogpu_next_shader tmp = shaders[0]; shaders[0] = shaders[1]; shaders[1] = tmp;
        color.format = !strcmp(pass->params.target_format->name, "rgba8") ? OGPU_NEXT_RGBA8_UNORM : OGPU_NEXT_RGBA16_FLOAT;
    }
    const ogpu_next_graphics_state graphics = {
        .header=RECORD(ogpu_next_graphics_state, OGPU_NEXT_GRAPHICS_STATE),
        .topology=OGPU_NEXT_TRIANGLE_STRIP, .samples=1, .color_count=1, .colors=&color,
    };
    const ogpu_next_executable_desc desc = {RECORD(ogpu_next_executable_desc, OGPU_NEXT_EXECUTABLE_DESC),
        compute ? OGPU_NEXT_EXECUTABLE_COMPUTE : OGPU_NEXT_EXECUTABLE_GRAPHICS, compute ? 1 : 2, shaders,
        compute ? NULL : &graphics.header, compute ? 0 : OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,
        &requirements.header, NULL, 0, 0};
    variant = calloc(1, sizeof(*variant)); REQUIRE(variant, "variant allocation failed");
    variant->references = 1;
    TRY(ogpu_next_executable_create(b->device, &desc, &variant->executable));
    variant_release(p->current); p->current = variant;
    memcpy(p->values, values, sizeof(values));
    return true;
fail:
    variant_release(variant); return false;
}

static pl_pass pass_create(pl_gpu gpu, const struct pl_pass_params *in)
{
    struct backend *b = PL_PRIV(gpu);
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
        if (!memory_create(gpu, 16, 4, OGPU_NEXT_USAGE_INDIRECT, &p->indirect)) goto fail;
        const ogpu_next_draw_arguments args = {4, 1, 0, 0};
        TRY(memory_write(p->indirect, &args, sizeof(args)));
    }
    for (unsigned i = 0; i < FRAMES; ++i) {
        struct bank *bank = &p->banks[i];
        bank->allocated = true;
        ++b->stats.banks;
        b->stats.peak_banks = PL_MAX(b->stats.peak_banks, b->stats.banks);
        if (!heap_create(gpu, OGPU_NEXT_HEAP_RESOURCE, &bank->images) ||
            !heap_create(gpu, OGPU_NEXT_HEAP_SAMPLER, &bank->samplers)) goto fail;
        if (!compute) {
            if (!memory_create(gpu, 64, 16, OGPU_NEXT_USAGE_STORAGE, &bank->vertices)) goto fail;
            TRY(ogpu_next_memory_address(span(bank->vertices), &bank->vertex_address));
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
    const bool reusing = bank->reusable;
    ogpu_next_encoder *batch = NULL;
    REQUIRE(!b->failed && !in->num_var_updates && !in->timer, "unsupported run variables/timer or failed device");
    REQUIRE(!params->push_constants_size || in->push_constants, "missing pass parameters");
    REQUIRE(!bank->busy && (b->active < 0 || b->frames[b->active].count < OPERATIONS),
            "pass bank already in use or frame capacity exceeded");
    if (!specialize(gpu, in->pass, in->constant_data)) goto fail;
    if (!compute) {
        REQUIRE(in->vertex_data && !in->vertex_buf && !in->index_data && !in->index_buf &&
                in->vertex_count == 4 && full_rect(in->viewport, in->target) &&
                full_rect(in->scissors, in->target), "unsupported draw region/vertices");
        TRY(memory_write(bank->vertices, in->vertex_data, 64));
    }
    TRY(begin_batch(gpu, &batch));
    order(batch);
    for (int i = 0; i < params->num_descriptors; ++i) {
        const struct pl_desc *d = &params->descriptors[i];
        const struct pl_desc_binding *binding = &in->desc_bindings[i];
        struct texture *t = PL_PRIV((pl_tex)binding->object);
        mark_use(gpu, t);
        const bool sampled = d->type == PL_DESC_SAMPLED_TEX;
        if (!t->initialized) {
            REQUIRE(!sampled, "sample of unwritten image");
            discard_image(b, batch, t);
        }
        const ogpu_next_resource_descriptor entry = {
            sampled ? OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE : OGPU_NEXT_DESCRIPTOR_STORAGE_IMAGE,
            OGPU_NEXT_STATE_GENERAL, t->view, {NULL, 0, 0}};
        const ogpu_next_host_span output = {(char *)bank->images.storage->map.data + d->binding * bank->images.stride,
            b->descriptors.image_size};
        TRY(ogpu_next_write_resource_descriptors(b->device, 1, &entry, &output, b->scratch));
        if (sampled) {
            REQUIRE(binding->sample_mode == PL_TEX_SAMPLE_NEAREST || binding->sample_mode == PL_TEX_SAMPLE_LINEAR,
                    "unsupported sampler filter");
            REQUIRE(binding->address_mode == PL_TEX_ADDRESS_CLAMP || binding->address_mode == PL_TEX_ADDRESS_REPEAT,
                    "unsupported sampler address mode");
            const uint32_t filter = binding->sample_mode == PL_TEX_SAMPLE_LINEAR ? OGPU_NEXT_FILTER_LINEAR : OGPU_NEXT_FILTER_NEAREST;
            const uint32_t address = binding->address_mode == PL_TEX_ADDRESS_REPEAT ? OGPU_NEXT_ADDRESS_REPEAT : OGPU_NEXT_ADDRESS_CLAMP_EDGE;
            const ogpu_next_sampler_desc sampler = {RECORD(ogpu_next_sampler_desc, OGPU_NEXT_SAMPLER_DESC),
                filter, filter, 0, {address, address, address}, 0, 0, 0, 0, 0, 0, 0, 1};
            const ogpu_next_host_span sampler_out = {(char *)bank->samplers.storage->map.data + d->binding * bank->samplers.stride,
                b->descriptors.sampler_size};
            TRY(ogpu_next_write_sampler_descriptors(b->device, 1, &sampler, &sampler_out, b->scratch));
        }
    }
    TRY(ogpu_next_memory_flush((ogpu_next_span){bank->images.storage->handle, 0, bank->images.payload}));
    TRY(ogpu_next_memory_flush((ogpu_next_span){bank->samplers.storage->handle, 0, bank->samplers.payload}));
    ogpu_next_bind_heap(batch, &bank->images.binding);
    ogpu_next_bind_heap(batch, &bank->samplers.binding);
    ogpu_next_bind_executable(batch, p->current->executable);
    if (compute) {
        ogpu_next_set_inline(batch, OGPU_NEXT_STAGE_COMPUTE, 0, params->push_constants_size, in->push_constants);
        const ogpu_next_launch launch = {{in->compute_groups[0], in->compute_groups[1], in->compute_groups[2]}, 0, NULL};
        ogpu_next_dispatch(batch, &launch);
    } else {
        struct texture *target = PL_PRIV(in->target);
        uint8_t root[PARAMETERS + 8] = {0};
        REQUIRE(!params->push_constants_size || in->push_constants, "missing raster parameters");
        REQUIRE(!params->load_target || target->initialized, "load of unwritten target");
        if (params->push_constants_size) memcpy(root, in->push_constants, params->push_constants_size);
        memcpy(root + p->vertex_offset, &bank->vertex_address, 8);
        mark_use(gpu, target);
        if (!params->load_target) discard_image(b, batch, target);
        const ogpu_next_attachment color = {.view=target->view, .state=OGPU_NEXT_STATE_GENERAL,
            .load_op=params->load_target ? OGPU_NEXT_LOAD : OGPU_NEXT_CLEAR, .store_op=OGPU_NEXT_STORE,
            .clear={.f32={0, 0, 0, 1}}};
        const ogpu_next_render_desc rendering = {.header=RECORD(ogpu_next_render_desc, OGPU_NEXT_RENDER_DESC),
            .width=in->target->params.w, .height=in->target->params.h, .layers=1, .samples=1,
            .color_count=1, .colors=&color, .scratch=b->scratch};
        const ogpu_next_viewport_state viewport = {.header=RECORD(ogpu_next_viewport_state, OGPU_NEXT_VIEWPORT_STATE),
            .width=in->target->params.w, .height=in->target->params.h, .max_depth=1,
            .scissor_width=in->target->params.w, .scissor_height=in->target->params.h};
        ogpu_next_render_begin(batch, &rendering);
        ogpu_next_set_graphics_state(batch, &viewport.header);
        ogpu_next_set_inline(batch, OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT, 0, p->root_size, root);
        const ogpu_next_indirect draws = {span(p->indirect), 16, 1, {NULL, 0, 0}};
        ogpu_next_draw_indirect(batch, &draws);
        ogpu_next_render_end(batch);
    }
    if (!finish(gpu, batch, bank, p->current)) goto fail;
    if (reusing) ++b->stats.receipt_reuses; // Historical name: now completed bank reuse.
    for (int i = 0; i < params->num_descriptors; ++i)
        ((struct texture *)PL_PRIV((pl_tex)in->desc_bindings[i].object))->initialized = true;
    if (compute) ++b->stats.compute;
    else { ((struct texture *) PL_PRIV(in->target))->initialized = true; ++b->stats.raster; }
fail:
    end_batch(gpu, batch);
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
    ogpu_next_format_info *supported = NULL;
    struct pl_gpu_t *gpu = calloc(1, PL_ALIGN_MEM(sizeof(*gpu)) + sizeof(struct backend));
    if (!gpu) return NULL;
    gpu->log = log;
    struct backend *b = PL_PRIV(gpu);
    b->active = -1;
    ogpu_next_discovery *probe = NULL;
    ogpu_next_execution_limits limits;
    TRY(ogpu_next_discovery_create(OGPU_NEXT_VERSION, &probe));
    REQUIRE(index < ogpu_next_discovery_count(probe), "invalid device index");
    const ogpu_next_adapter *adapter = ogpu_next_discovery_adapter(probe, index);
    ogpu_next_queue_info queues[32];
    ogpu_next_query q = query(OGPU_NEXT_QUERY_QUEUES, queues, 32, sizeof(queues[0]));
    TRY(ogpu_next_adapter_query(adapter, &q));
    uint32_t domain = UINT32_MAX;
    for (uint32_t i = 0; i < q.count; ++i)
        if (queues[i].count && (queues[i].flags & (OGPU_NEXT_QUEUE_COMPUTE | OGPU_NEXT_QUEUE_GRAPHICS)) ==
            (OGPU_NEXT_QUEUE_COMPUTE | OGPU_NEXT_QUEUE_GRAPHICS)) { domain = queues[i].domain; break; }
    REQUIRE(domain != UINT32_MAX, "combined compute/graphics queue required");
    const ogpu_next_queue_request request = {domain, 1, .5f};
    const ogpu_next_device_desc dd = {RECORD(ogpu_next_device_desc, OGPU_NEXT_DEVICE_DESC),
        &request, 1, 0, OGPU_NEXT_FEATURE_RASTER};
    TRY(ogpu_next_device_create(adapter, &dd, &b->device));
    b->queue = ogpu_next_device_queue(b->device, domain, 0);
    q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS, &limits, 1, sizeof(limits));
    TRY(ogpu_next_device_query(b->device, &q));
    q = query(OGPU_NEXT_QUERY_MEMORY_TYPES, b->types, 32, sizeof(b->types[0]));
    TRY(ogpu_next_device_query(b->device, &q)); b->type_count = q.count;
    q = query(OGPU_NEXT_QUERY_MEMORY_LIMITS, &b->memory_limits, 1, sizeof(b->memory_limits));
    TRY(ogpu_next_device_query(b->device, &q));
    q = query(OGPU_NEXT_QUERY_DESCRIPTOR_LIMITS, &b->descriptors, 1, sizeof(b->descriptors));
    TRY(ogpu_next_device_query(b->device, &q));
    ogpu_next_host_requirements scratch[5];
    TRY(ogpu_next_descriptor_scratch_requirements(OGPU_NEXT_HEAP_RESOURCE, 1, &scratch[0]));
    TRY(ogpu_next_descriptor_scratch_requirements(OGPU_NEXT_HEAP_SAMPLER, 1, &scratch[1]));
    TRY(ogpu_next_barrier_scratch_requirements(0, 1, &scratch[2]));
    TRY(ogpu_next_render_scratch_requirements(1, &scratch[3]));
    TRY(ogpu_next_submit_scratch_requirements(1, 0, 1, &scratch[4]));
    for (unsigned i = 0; i < 5; ++i) {
        REQUIRE(scratch[i].alignment <= _Alignof(max_align_t), "unsupported scratch alignment");
        b->scratch.size = PL_MAX(b->scratch.size, scratch[i].size);
    }
    b->scratch.data = malloc(b->scratch.size); REQUIRE(b->scratch.data, "scratch allocation failed");
    TRY(ogpu_next_timeline_create(b->device, 0, &b->timeline));
    ogpu_next_arena_desc ad = {RECORD(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1, OGPU_NEXT_PRIMARY, 0};
    TRY(ogpu_next_arena_create(b->device, &ad, &b->serial));
    ad.list_capacity = OPERATIONS + 1; // One extra failed recording, then fail closed.
    for (unsigned i = 0; i < FRAMES; ++i) TRY(ogpu_next_arena_create(b->device, &ad, &b->frames[i].arena));

    // Check every advertised image profile, and intersect their extent limits.
    const uint32_t image_formats[] = {OGPU_NEXT_RGBA16_UNORM, OGPU_NEXT_RGBA16_FLOAT, OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_R32_FLOAT};
    uint32_t dimensions[2] = {UINT32_MAX, UINT32_MAX};
    q = query(OGPU_NEXT_QUERY_FORMATS, NULL, 0, sizeof(*supported));
    TRY(ogpu_next_device_query(b->device, &q));
    supported = calloc(q.count, sizeof(*supported));
    REQUIRE(supported, "format query allocation failed");
    q.data = supported; q.capacity = q.count;
    TRY(ogpu_next_device_query(b->device, &q));
    for (unsigned i = 0; i < 4; ++i) {
        bool linear = false;
        for (uint32_t j = 0; j < q.count; ++j)
            if (supported[j].format == image_formats[i])
                linear = (supported[j].optimal_features & OGPU_NEXT_FORMAT_FILTER_LINEAR) != 0;
        REQUIRE(linear, "linear filtering required by advertised format");
        for (unsigned dim = 1; dim <= 2; ++dim) {
            uint64_t usage = OGPU_NEXT_IMAGE_SAMPLED | OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST;
            if (i == 1 || i == 2) usage |= OGPU_NEXT_IMAGE_STORAGE | (dim == 2 ? OGPU_NEXT_IMAGE_COLOR_ATTACHMENT : 0);
            const ogpu_next_image_profile profile = {RECORD(ogpu_next_image_profile, OGPU_NEXT_IMAGE_PROFILE),
                image_formats[i], dim, usage, 0, 0};
            ogpu_next_image_limits il;
            TRY(ogpu_next_image_limits_query(b->device, &profile, &il));
            dimensions[dim-1] = PL_MIN(dimensions[dim-1], il.max_extent[0]);
            if (dim == 2) dimensions[dim-1] = PL_MIN(dimensions[dim-1], il.max_extent[1]);
        }
    }
    free(supported); supported = NULL;
    gpu->glsl = (struct pl_glsl_version) {.version=450, .vulkan=true, .compute=true,
        .max_shmem_size=limits.max_shared_memory, .max_group_threads=limits.max_local_invocations};
    memcpy(gpu->glsl.max_group_size, limits.max_local_size, sizeof(limits.max_local_size));
    gpu->limits = (struct pl_gpu_limits) {
        .callbacks=true,
        .max_tex_1d_dim=dimensions[0], .max_tex_2d_dim=dimensions[1],
        .max_constants=CONSTANTS, .array_size_constants=true,
        .max_pushc_size=PL_MIN(limits.max_inline_size >= 8 ? (limits.max_inline_size - 8) & ~7ull : 0, PARAMETERS),
        .align_vertex_stride=1, .align_tex_xfer_pitch=4, .align_tex_xfer_offset=4,
        .fragment_queues=1, .compute_queues=1,
    };
    memcpy(gpu->limits.max_dispatch, limits.max_groups, sizeof(limits.max_groups));
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
    ogpu_next_discovery_destroy(probe);
    return gpu;
fail:
    ogpu_next_discovery_destroy(probe);
    for (unsigned i = 0; i < FRAMES; ++i) ogpu_next_arena_destroy(b->frames[i].arena);
    ogpu_next_arena_destroy(b->serial);
    ogpu_next_timeline_destroy(b->timeline);
    ogpu_next_device_destroy(b->device);
    free(b->scratch.data);
    free(supported); free(gpu->formats); free(gpu);
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
    for (unsigned i = 0; i < FRAMES; ++i) ogpu_next_arena_destroy(b->frames[i].arena);
    ogpu_next_arena_destroy(b->serial);
    ogpu_next_timeline_destroy(b->timeline);
    ogpu_next_device_destroy(b->device);
    free(b->scratch.data);
    free((*gpu)->formats); free((void *) *gpu); *gpu = NULL;
}
