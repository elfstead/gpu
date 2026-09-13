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

static void pattern(uint8_t *data, int w, int h, unsigned frame)
{
    for (int y = 0; y < h; ++y) {
        for (int x = 0; x < w; ++x) {
            size_t i = ((size_t) y * w + x) * 4;
            data[i] = (x * 7 + y * 3 + frame * 19) % 256;
            data[i + 1] = (x * 5 + y * 11 + frame * 23) % 256;
            data[i + 2] = ((x ^ y) * 13 + frame * 29) % 256;
            data[i + 3] = 255;
        }
    }
}

static uint64_t checksum(const uint8_t *data, size_t size)
{
    uint64_t hash = UINT64_C(14695981039346656037);
    for (size_t i = 0; i < size; ++i)
        hash = (hash ^ data[i]) * UINT64_C(1099511628211);
    return hash;
}

static void run_case(pl_gpu gpu, int w, int h)
{
    // Upscale by an odd, non-integer ratio, exercising workgroup tails.
    const int ow = w * 2 + 1, oh = h * 2 + 1;
    const size_t in_size = (size_t) w * h * 4, out_size = (size_t) ow * oh * 4;
    uint8_t *input = malloc(in_size), *middle = malloc(out_size), *output = malloc(out_size);
    uint8_t *first = malloc(out_size);
    CHECK(input && middle && output && first);
    pl_fmt rgba = pl_find_named_fmt(gpu, "rgba8");
    CHECK(rgba && (rgba->caps & PL_FMT_CAP_STORABLE));
    pl_tex src = pl_tex_create(gpu, pl_tex_params(
        .w = w, .h = h, .format = rgba, .sampleable = true, .host_writable = true));
    pl_tex mid = pl_tex_create(gpu, pl_tex_params(
        .w = ow, .h = oh, .format = rgba, .sampleable = true,
        .storable = true, .renderable = true, .host_readable = true));
    // The last pass has a genuine raster target, not a compute copy of its output.
    pl_tex dst = pl_tex_create(gpu, pl_tex_params(
        .w = ow, .h = oh, .format = rgba, .renderable = true, .host_readable = true));
    CHECK(src && mid && dst);
    pl_dispatch dp = pl_dispatch_create(gpu->log, gpu);
    CHECK(dp);
    pl_shader_obj lut = NULL;
    const unsigned initial_compute = compute_runs, initial_raster = raster_runs;
    unsigned warmed_creates = 0;
    for (unsigned frame = 0; frame < 3; ++frame) {
        // A/B/A checks both live updates and return to a deterministic earlier result.
        pattern(input, w, h, frame == 1);
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex = src, .ptr = input)));
        pl_dispatch_reset_frame(dp);
        pl_shader sh = pl_dispatch_begin(dp);
        CHECK(pl_shader_sample_polar(sh, pl_sample_src(.tex = src, .new_w = ow, .new_h = oh),
            pl_sample_filter_params(.filter = pl_filter_ewa_lanczos, .lut = &lut)));
        CHECK(pl_shader_is_compute(sh));
        CHECK(pl_dispatch_finish(dp, pl_dispatch_params(.shader = &sh, .target = mid)));
        sh = pl_dispatch_begin(dp);
        CHECK(pl_shader_sample_nearest(sh, pl_sample_src(.tex = mid)));
        CHECK(!pl_shader_is_compute(sh));
        CHECK(pl_dispatch_finish(dp, pl_dispatch_params(.shader = &sh, .target = dst)));
        CHECK(compute_runs - initial_compute == frame + 1);
        CHECK(raster_runs - initial_raster == frame + 1);
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex = mid, .ptr = middle)));
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex = dst, .ptr = output)));
        CHECK(memcmp(middle, output, out_size) == 0);
        for (size_t i = 3; i < out_size; i += 4)
            CHECK(output[i] == 255);
        if (frame == 0) {
            memcpy(first, output, out_size);
            warmed_creates = creates;
        } else {
            CHECK(creates == warmed_creates); // upstream executable reuse
            CHECK((memcmp(first, output, out_size) == 0) == (frame == 2));
        }
        printf("reference=%dx%d output=%dx%d frame=%u checksum=%016" PRIx64 " compute=1 raster=1 PASS\n",
               w, h, ow, oh, frame, checksum(output, out_size));
        // Raw outputs are local artifacts; generated upstream code is not vendored.
        char suffix[128];
        snprintf(suffix, sizeof(suffix), "%dx%d-frame%u.rgba", w, h, frame);
        save(creates, suffix, output, out_size);
    }
    pl_dispatch_destroy(&dp);
    pl_shader_obj_destroy(&lut);
    pl_tex_destroy(gpu, &dst);
    pl_tex_destroy(gpu, &mid);
    pl_tex_destroy(gpu, &src);
    free(first); free(output); free(middle); free(input);
}

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
    run_case(vk->gpu, 16, 16);
    run_case(vk->gpu, 31, 17);
    run_case(vk->gpu, 64, 33);
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
