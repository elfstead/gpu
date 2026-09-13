// OGPU consumer audit: upstream Vulkan reference ONLY, not an OGPU backend.
// The pinned private backend table is intercepted to capture exactly what upstream
// generates and executes; every operation still delegates to upstream Vulkan.
#include "gpu.h"
#include <libplacebo/shaders/sampling.h>
#include <libplacebo/vulkan.h>
#include <stdio.h>
#include <string.h>

#define CHECK(x) do { if (!(x)) { fprintf(stderr, "check failed: %s:%d: %s\n", \
    __FILE__, __LINE__, #x); exit(1); } } while (0)

static struct pl_gpu_fns original;
static const char *output_dir;
static FILE *manifest;
static unsigned creates, compute_runs, raster_runs, log_errors;
static unsigned run_depth;

static void log_message(void *priv, enum pl_log_level level, const char *message)
{
    (void) priv;
    fprintf(stderr, "libplacebo[%d]: %s\n", level, message);
    if (level <= PL_LOG_ERR)
        ++log_errors;
}

static void save(unsigned pass, const char *suffix, const void *data, size_t size)
{
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/pass-%u.%s", output_dir, pass, suffix);
    CHECK(n > 0 && (size_t) n < sizeof(path));
    FILE *file = fopen(path, "wb");
    CHECK(file);
    CHECK(fwrite(data, 1, size, file) == size);
    CHECK(fclose(file) == 0);
}

static pl_pass capture_create(pl_gpu gpu, const struct pl_pass_params *p)
{
    unsigned id = creates++;
    fprintf(manifest, "create=%u type=%s descriptors=%d constants=%d push_bytes=%zu "
            "vertex_stride=%zu vertex_attributes=%d topology=%d target=%s blend=%d\n",
            id, p->type == PL_PASS_COMPUTE ? "compute" : "raster", p->num_descriptors,
            p->num_constants, p->push_constants_size, p->vertex_stride,
            p->num_vertex_attribs, p->vertex_type,
            p->target_format ? p->target_format->name : "none", p->blend_params != NULL);
    for (int i = 0; i < p->num_descriptors; ++i) {
        const struct pl_desc *d = &p->descriptors[i];
        fprintf(manifest, "  descriptor name=%s type=%d binding=%d access=%d\n",
                d->name, d->type, d->binding, d->access);
    }
    for (int i = 0; i < p->num_constants; ++i) {
        const struct pl_constant *c = &p->constants[i];
        uint32_t bits = 0;
        CHECK(p->constant_data);
        memcpy(&bits, (const char *) p->constant_data + c->offset, sizeof(bits));
        fprintf(manifest, "  constant id=%u type=%d bits=%08x\n", c->id, c->type, bits);
    }
    for (int i = 0; i < p->num_vertex_attribs; ++i) {
        const struct pl_vertex_attrib *a = &p->vertex_attribs[i];
        fprintf(manifest, "  vertex location=%d offset=%zu format=%s name=%s\n",
                a->location, a->offset, a->fmt->name, a->name);
    }
    save(id, p->type == PL_PASS_COMPUTE ? "comp" : "frag", p->glsl_shader,
         strlen(p->glsl_shader));
    if (p->vertex_shader)
        save(id, "vert", p->vertex_shader, strlen(p->vertex_shader));
    return original.pass_create(gpu, p);
}

static void capture_run(pl_gpu gpu, const struct pl_pass_run_params *p)
{
    // Vulkan's host-vertex helper re-enters this callback with an uploaded VBO.
    // Count the upstream operation once, not both forwarding levels.
    if (run_depth++) {
        original.pass_run(gpu, p);
        --run_depth;
        return;
    }
    const struct pl_pass_params *params = &p->pass->params;
    if (params->type == PL_PASS_COMPUTE)
        ++compute_runs;
    else
        ++raster_runs;
    fprintf(manifest, "run type=%s grid=%d,%d,%d vertices=%d\n",
            params->type == PL_PASS_COMPUTE ? "compute" : "raster",
            p->compute_groups[0], p->compute_groups[1], p->compute_groups[2], p->vertex_count);
    for (int i = 0; i < params->num_descriptors; ++i) {
        const struct pl_desc *d = &params->descriptors[i];
        if (d->type == PL_DESC_SAMPLED_TEX || d->type == PL_DESC_STORAGE_IMG) {
            pl_tex t = p->desc_bindings[i].object;
            fprintf(manifest, "  image name=%s extent=%d,%d,%d format=%s sample=%d address=%d\n",
                    d->name, t->params.w, t->params.h, t->params.d, t->params.format->name,
                    p->desc_bindings[i].sample_mode, p->desc_bindings[i].address_mode);
        }
    }
    original.pass_run(gpu, p);
    --run_depth;
}

#include "workload.h"
static struct workload_counts workload_counts(pl_gpu gpu)
{ return (struct workload_counts) {creates, compute_runs, raster_runs}; }

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: %s existing-output-directory\n", argv[0]);
        return 2;
    }
    output_dir = argv[1];
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/manifest.txt", output_dir);
    CHECK(n > 0 && (size_t) n < sizeof(path));
    manifest = fopen(path, "w");
    CHECK(manifest);
    pl_log log = pl_log_create(PL_API_VER, pl_log_params(
        .log_cb = log_message, .log_level = PL_LOG_INFO));
    CHECK(log);
    pl_vulkan vk = pl_vulkan_create(log, pl_vulkan_params(
        .allow_software = true, .async_compute = false, .async_transfer = false));
    CHECK(vk);
    struct pl_gpu_fns *f = PL_PRIV(vk->gpu);
    original = *f;
    f->pass_create = capture_create;
    f->pass_run = capture_run;
    run_case(vk->gpu, 16, 16, "reference");
    run_case(vk->gpu, 31, 17, "reference");
    run_case(vk->gpu, 64, 33, "reference");
    pl_gpu_finish(vk->gpu);
    *f = original;
    pl_vulkan_destroy(&vk);
    pl_log_destroy(&log);
    CHECK(log_errors == 0);
    CHECK(fclose(manifest) == 0);
    printf("upstream-reference creates=%u compute=%u raster=%u PASS (not OGPU execution)\n",
           creates, compute_runs, raster_runs);
    return 0;
}
