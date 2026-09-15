// Fail-closed/cleanup checks against the pinned callback boundary, not Vulkan fallback.
#include "gpu.h"
#include "backend.h"
#include "ogpu.h"
#include <libplacebo/dispatch.h>
#include <libplacebo/shaders/sampling.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "check failed: %s\n", #x); exit(1); } } while (0)
static void completed(void *ptr) { ++*(unsigned *) ptr; }
// Link-time shims only in this test executable. Real receipts still own real
// GPU work; never fabricate success/retirement. Exercise rare adapter branches.
static unsigned pending_polls, poll_errors, reject_submit;
static bool batched;
static bool begin(pl_gpu gpu, unsigned slot)
{ return batched ? ogpu_pl_frame_begin_batched(gpu, slot) : ogpu_pl_frame_begin(gpu, slot); }
OgpuResult __real_ogpu_completion_poll(OgpuCompletion *, uint32_t *, OgpuError *);
OgpuResult __wrap_ogpu_completion_poll(OgpuCompletion *c, uint32_t *ready, OgpuError *error)
{
    if (pending_polls) {
        --pending_polls; *ready = 0;
        return OGPU_SUCCESS;
    }
    if (poll_errors) {
        --poll_errors;
        if (error) { memset(error, 0, sizeof(*error)); strcpy(error->message, "injected transient poll failure"); }
        return OGPU_ERROR_VULKAN;
    }
    return __real_ogpu_completion_poll(c, ready, error);
}
OgpuResult __real_ogpu_batch_submit(OgpuBatch *, OgpuCompletion **, OgpuError *);
OgpuResult __wrap_ogpu_batch_submit(OgpuBatch *b, OgpuCompletion **out, OgpuError *error)
{
    if (reject_submit) {
        --reject_submit; *out = NULL;
        if (error) { memset(error, 0, sizeof(*error)); strcpy(error->message, "injected submission rejection"); }
        return OGPU_ERROR_INTERNAL;
    }
    return __real_ogpu_batch_submit(b, out, error);
}

static void frame_transfers(pl_gpu gpu, const char *mode)
{
    pl_tex textures[9] = {0};
    for (unsigned i = 0; i < 9; ++i) {
        textures[i] = pl_tex_create(gpu, pl_tex_params(.w=4, .h=4,
            .format=pl_find_named_fmt(gpu, "rgba8"), .sampleable=true, .host_writable=true, .host_readable=true));
        CHECK(textures[i]);
    }
    unsigned callbacks = 0;
    uint8_t data[64]; memset(data, 0x5a, sizeof(data));
    CHECK(begin(gpu, 0));
    unsigned uploads = !strcmp(mode, "frame-capacity") ? 8 : 1;
    for (unsigned i = 0; i < uploads; ++i)
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex=textures[i], .ptr=data,
            .callback=completed, .priv=&callbacks)));
    CHECK(!callbacks && ogpu_pl_stats(gpu).waits == 0);
    CHECK(pl_tex_poll(gpu, textures[0], 0)); // Active slot cannot be collected yet.
    bool failed = strcmp(mode, "frame-destroy") != 0 && strcmp(mode, "frame-pending") != 0;
    if (!strcmp(mode, "frame-capacity")) {
        CHECK(!pl_tex_upload(gpu, pl_tex_transfer_params(.tex=textures[8], .ptr=data)));
    } else if (!strcmp(mode, "frame-partial")) {
        CHECK(!pl_tex_upload(gpu, pl_tex_transfer_params(.tex=textures[1], .ptr=data, .rc={0,0,0,2,2,1})));
    } else if (!strcmp(mode, "frame-staging")) {
        CHECK(!pl_tex_upload(gpu, pl_tex_transfer_params(.tex=textures[0], .ptr=data)));
    } else if (!strcmp(mode, "frame-submit-error")) {
        reject_submit = 1;
        if (batched) CHECK(!ogpu_pl_frame_end(gpu));
        else CHECK(!pl_tex_upload(gpu, pl_tex_transfer_params(.tex=textures[1], .ptr=data)));
        CHECK(!reject_submit);
    } else if (!strcmp(mode, "frame-poll-error")) {
        CHECK(ogpu_pl_frame_end(gpu));
        poll_errors = 1;
        CHECK(!ogpu_pl_frame_collect(gpu, 0, true));
        CHECK(!poll_errors && callbacks == uploads);
    } else if (!strcmp(mode, "frame-pending")) {
        CHECK(ogpu_pl_frame_end(gpu));
        pending_polls = 2;
        CHECK(!ogpu_pl_frame_collect(gpu, 0, false));
        CHECK(!callbacks && ogpu_pl_stats(gpu).outstanding == 1 && ogpu_pl_stats(gpu).waits == 0);
        CHECK(ogpu_pl_frame_collect(gpu, 0, true));
        CHECK(!pending_polls && ogpu_pl_stats(gpu).waits == 1);
    } else if (!strcmp(mode, "frame-reuse")) {
        CHECK(ogpu_pl_frame_end(gpu));
        CHECK(begin(gpu, 1));
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex=textures[1], .ptr=data,
            .callback=completed, .priv=&callbacks)));
        ++uploads;
        CHECK(ogpu_pl_frame_end(gpu));
        CHECK(!begin(gpu, 0));
    } else {
        CHECK(!strcmp(mode, "frame-destroy"));
        CHECK(ogpu_pl_frame_end(gpu));
    }
    CHECK(pl_gpu_is_failed(gpu) == failed);
    if (!batched) CHECK(ogpu_pl_stats(gpu).submissions == uploads);
    else CHECK(ogpu_pl_stats(gpu).submissions <= 2);
    // Child destruction must drain even a failed, partially open frame.
    pl_tex_destroy(gpu, &textures[8]);
    CHECK(callbacks == uploads && !ogpu_pl_stats(gpu).outstanding && !ogpu_pl_stats(gpu).inflight);
    if (!failed) {
        uint8_t result[64];
        CHECK(!pl_tex_poll(gpu, textures[0], 0));
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=textures[0], .ptr=result)));
        CHECK(!memcmp(data, result, sizeof(data)));
    }
    for (unsigned i = 0; i < 9; ++i) pl_tex_destroy(gpu, &textures[i]);
    CHECK(!ogpu_pl_stats(gpu).textures && !ogpu_pl_stats(gpu).staging_bytes);
}

static void specialization_updates(pl_gpu gpu, bool async, bool premature)
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
    uint8_t pixels[3][48];
    unsigned callbacks[3] = {0};
    for (unsigned frame = 0; frame < 3; ++frame) {
        if (async) {
            if (frame == 2) CHECK(ogpu_pl_frame_collect(gpu, 0, true));
            CHECK(begin(gpu, frame % 2));
        }
        value = frame == 1 ? 1.0f : 0.0f;
        pl_pass_run(gpu, pl_pass_run_params(.pass=pass, .constant_data=&value,
            .desc_bindings=&binding, .compute_groups={4,3,1}));
        CHECK(!pl_gpu_is_failed(gpu));
        memset(pixels[frame], 0x55, sizeof(pixels[frame]));
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=tex, .ptr=pixels[frame],
            .callback=completed, .priv=&callbacks[frame])));
        if (premature) {
            pl_pass_run(gpu, pl_pass_run_params(.pass=pass, .constant_data=&value,
                .desc_bindings=&binding, .compute_groups={4,3,1}));
            CHECK(pl_gpu_is_failed(gpu) && ogpu_pl_stats(gpu).compute == 1);
            break;
        }
        if (async) CHECK(ogpu_pl_frame_end(gpu));
    }
    // Destroy the shared executable with slots still queued (including old
    // specialization variants) or with a partially failed active frame.
    pl_pass_destroy(gpu, &pass);
    for (unsigned frame = 0; frame < (premature ? 1u : 3u); ++frame) {
        CHECK(callbacks[frame] == 1);
        if (premature) for (unsigned i = 0; i < sizeof(pixels[frame]); ++i)
            CHECK(pixels[frame][i] == 0x55); // Failure releases lifetime, not valid output.
        if (!premature) for (unsigned i = 0; i < sizeof(pixels[frame]); i += 4)
            CHECK(pixels[frame][i] == (frame == 1 ? 255 : 0) && !pixels[frame][i+1] &&
                  !pixels[frame][i+2] && pixels[frame][i+3] == 255);
    }
    pl_tex_destroy(gpu, &tex);
    const struct ogpu_stats s = ogpu_pl_stats(gpu);
    CHECK(s.creates == 1 && s.compute == (premature ? 1u : 3u) && s.downloads == s.compute &&
          !s.raster && !s.textures && !s.passes && !s.banks && !s.outstanding && !s.inflight);
    CHECK(s.receipt_reuses == (premature || (async && batched) ? 0u : async ? 1u : 2u));
}

static void unorm16_sampling(pl_gpu gpu)
{
    pl_fmt fmt = pl_find_named_fmt(gpu, "rgba16");
    CHECK(fmt && fmt->texel_size == 8 && (fmt->caps & PL_FMT_CAP_LINEAR));
    CHECK(!(fmt->caps & (PL_FMT_CAP_STORABLE | PL_FMT_CAP_RENDERABLE)));
    const uint16_t pixels[8] = {0,0,0,65535,65535,65535,65535,65535};
    uint16_t readback[8] = {0};
    pl_tex src = pl_tex_create(gpu, pl_tex_params(.w=2,.h=1,.format=fmt,
        .sampleable=true,.host_writable=true,.host_readable=true));
    pl_tex dst = pl_tex_create(gpu, pl_tex_params(.w=3,.h=1,
        .format=pl_find_named_fmt(gpu,"rgba8"),.renderable=true,.host_readable=true));
    CHECK(src && dst);
    CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex=src,.ptr=(void *)pixels)));
    CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=src,.ptr=readback)));
    CHECK(!memcmp(pixels,readback,sizeof(pixels)));
    pl_dispatch dp = pl_dispatch_create(gpu->log,gpu); CHECK(dp);
    pl_shader sh = pl_dispatch_begin(dp);
    CHECK(pl_shader_sample_bilinear(sh,pl_sample_src(.tex=src,.new_w=3,.new_h=1)));
    CHECK(pl_dispatch_finish(dp,pl_dispatch_params(.shader=&sh,.target=dst)));
    uint8_t result[12];
    CHECK(pl_tex_download(gpu,pl_tex_transfer_params(.tex=dst,.ptr=result)));
    for (unsigned c=0;c<3;++c) CHECK(result[c]==0 && result[4+c]>=127 && result[4+c]<=128 && result[8+c]==255);
    CHECK(result[3]==255 && result[7]==255 && result[11]==255);
    pl_dispatch_destroy(&dp); pl_tex_destroy(gpu,&src); pl_tex_destroy(gpu,&dst);
    const struct ogpu_stats s = ogpu_pl_stats(gpu);
    CHECK(!pl_gpu_is_failed(gpu) && !s.textures && !s.passes && !s.banks && !s.texture_bytes && !s.staging_bytes);
}

int main(int argc, char **argv)
{
    CHECK(argc == 2 || (argc == 3 && !strcmp(argv[2], "batched")));
    batched = argc == 3;
    pl_log log = pl_log_create(PL_API_VER, NULL);
    pl_gpu gpu = ogpu_pl_create(log, 0);
    CHECK(gpu);
    if (!strcmp(argv[1], "unorm16-sampling")) {
        unorm16_sampling(gpu);
        ogpu_pl_destroy(&gpu); pl_log_destroy(&log);
        puts("adapter unorm16 sampling/transfer live=0 PASS");
        return 0;
    }
    if (!strncmp(argv[1], "frame-", 6)) {
        frame_transfers(gpu, argv[1]);
        ogpu_pl_destroy(&gpu); pl_log_destroy(&log);
        printf("adapter frame=%s live=0 PASS\n", argv[1]);
        return 0;
    }
    if (!strcmp(argv[1], "specialization-update") || !strcmp(argv[1], "queued-specialization") ||
        !strcmp(argv[1], "queued-bank-reuse")) {
        specialization_updates(gpu, strcmp(argv[1], "specialization-update") != 0,
                               !strcmp(argv[1], "queued-bank-reuse"));
        ogpu_pl_destroy(&gpu);
        pl_log_destroy(&log);
        printf("adapter specialization mode=%s live=0 PASS\n", argv[1]);
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
