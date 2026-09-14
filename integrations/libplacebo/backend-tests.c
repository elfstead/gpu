// Fail-closed/cleanup checks against the pinned callback boundary, not Vulkan fallback.
#include "gpu.h"
#include "backend.h"
#include <stdio.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "check failed: %s\n", #x); exit(1); } } while (0)

static void specialization_updates(pl_gpu gpu)
{
    pl_tex tex = pl_tex_create(gpu, pl_tex_params(.w=4, .h=3,
        .format=pl_find_named_fmt(gpu, "rgba8"), .storable=true, .host_readable=true));
    CHECK(tex);
    float value = 0.0f;
    struct pl_constant constant = {.id=0, .type=PL_VAR_FLOAT};
    struct pl_desc descriptor = {.name="image_out", .type=PL_DESC_STORAGE_IMG, .access=PL_DESC_ACCESS_WRITEONLY};
    pl_pass pass = pl_pass_create(gpu, pl_pass_params(.type=PL_PASS_COMPUTE,
        .constants=&constant, .num_constants=1, .constant_data=&value,
        .descriptors=&descriptor, .num_descriptors=1,
        .glsl_shader="#version 450\nlayout(local_size_x=1,local_size_y=1) in;\n"
            "layout(constant_id=0) const float value = 0.0;\n"
            "layout(binding=0, rgba8) writeonly restrict uniform image2D image_out;\n"
            "void main() { imageStore(image_out, ivec2(gl_GlobalInvocationID.xy), vec4(value, 0, 0, 1)); }\n"));
    CHECK(pass);
    struct pl_desc_binding binding = {.object=tex};
    for (unsigned frame = 0; frame < 3; ++frame) {
        value = frame == 1 ? 1.0f : 0.0f;
        pl_pass_run(gpu, pl_pass_run_params(.pass=pass, .constant_data=&value,
            .desc_bindings=&binding, .compute_groups={4,3,1}));
        CHECK(!pl_gpu_is_failed(gpu));
        uint8_t pixels[48];
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=tex, .ptr=pixels)));
        for (unsigned i = 0; i < sizeof(pixels); i += 4)
            CHECK(pixels[i] == (frame == 1 ? 255 : 0) && !pixels[i+1] && !pixels[i+2] && pixels[i+3] == 255);
    }
    pl_pass_destroy(gpu, &pass);
    pl_tex_destroy(gpu, &tex);
    const struct ogpu_stats s = ogpu_pl_stats(gpu);
    CHECK(s.creates == 1 && s.compute == 3 && s.downloads == 3 && !s.raster && !s.textures && !s.passes);
    CHECK(s.receipt_reuses == 2);
}

int main(int argc, char **argv)
{
    CHECK(argc == 2);
    pl_log log = pl_log_create(PL_API_VER, NULL);
    pl_gpu gpu = ogpu_pl_create(log, 0);
    CHECK(gpu);
    if (!strcmp(argv[1], "specialization-update")) {
        specialization_updates(gpu);
        ogpu_pl_destroy(&gpu);
        pl_log_destroy(&log);
        puts("adapter specialization A/B/A live=0 PASS");
        return 0;
    }
    const struct pl_gpu_fns *f = PL_PRIV(gpu);
    pl_tex tex = NULL;
    pl_fmt rgba = pl_find_named_fmt(gpu, "rgba8");
    if (!strcmp(argv[1], "partial-upload") || !strcmp(argv[1], "live-child")) {
        tex = pl_tex_create(gpu, pl_tex_params(.w=4, .h=4, .format=rgba, .sampleable=true, .host_writable=true));
        CHECK(tex);
        if (!strcmp(argv[1], "live-child")) { ogpu_pl_destroy(&gpu); return 1; }
        unsigned char data[16] = {0};
        CHECK(!pl_tex_upload(gpu, pl_tex_transfer_params(.tex=tex, .ptr=data, .rc={0,0,0,2,2,1})));
    } else if (!strcmp(argv[1], "unsupported-image")) {
        CHECK(!f->tex_create(gpu, pl_tex_params(.w=4, .h=4, .format=rgba, .sampleable=true, .blit_src=true)));
    } else if (!strcmp(argv[1], "unsupported-shader")) {
        struct pl_desc desc = {.name="input_image", .type=PL_DESC_SAMPLED_TEX};
        struct pl_pass_params p = {.type=PL_PASS_COMPUTE, .descriptors=&desc, .num_descriptors=1,
            .glsl_shader="#version 450\nlayout(binding=0) uniform sampler3D input_image;\nvoid main() {}\n"};
        CHECK(!f->pass_create(gpu, &p));
    } else { return 2; }
    CHECK(pl_gpu_is_failed(gpu));
    pl_tex_destroy(gpu, &tex);
    const struct ogpu_stats s = ogpu_pl_stats(gpu);
    CHECK(!s.textures && !s.passes && !s.compute && !s.raster);
    ogpu_pl_destroy(&gpu);
    pl_log_destroy(&log);
    printf("adapter rejection=%s live=0 PASS\n", argv[1]);
}
