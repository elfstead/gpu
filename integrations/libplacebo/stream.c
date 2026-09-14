// Same processing bodies as the reference; only frame scheduling differs.
#include "backend.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "check failed: %s:%d: %s\n", __FILE__, __LINE__, #x); exit(1); } } while (0)
#define OGPU_STREAM_WORKLOAD
#include "workload.h"

static unsigned log_errors;
static void log_message(void *priv, enum pl_log_level level, const char *message)
{
    fprintf(stderr, "libplacebo[%d]: %s\n", level, message);
    if (level <= PL_LOG_ERR) ++log_errors;
}
static void completed(void *ptr) { ++*(unsigned *)ptr; }
struct slot {
    pl_tex src, mid, dst;
    uint8_t *middle, *output;
    unsigned callbacks, frame;
    bool pending;
};
static void save(const char *dir, unsigned pass, int w, int h, unsigned frame,
                 const char *suffix, const void *data, size_t size)
{
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/pass-%u.%dx%d-frame%u%s.rgba", dir, pass, w, h, frame, suffix);
    CHECK(n > 0 && (size_t)n < sizeof(path));
    FILE *f = fopen(path, "wb"); CHECK(f);
    CHECK(fwrite(data, 1, size, f) == size && fclose(f) == 0);
}
static void collect(pl_gpu gpu, struct slot *s, unsigned index, bool async,
                    const char *dir, unsigned pass, int w, int h, uint8_t *known[2])
{
    if (!s->pending) return;
    if (async && !ogpu_pl_frame_collect(gpu, index, false)) {
        CHECK(!pl_gpu_is_failed(gpu));
        CHECK(ogpu_pl_frame_collect(gpu, index, true));
    }
    CHECK(!pl_gpu_is_failed(gpu) && s->callbacks == 2);
    CHECK(!pl_tex_poll(gpu, s->dst, 0));
    size_t size = (size_t)(w*2+1)*(h*2+1)*4;
    CHECK(memcmp(s->middle, s->output, size) == 0);
    for (size_t i = 3; i < size; i += 4) CHECK(s->output[i] == 255);
    unsigned pattern_index = s->frame % 3 == 1;
    if (s->frame < 2) memcpy(known[pattern_index], s->output, size);
    else CHECK(memcmp(known[pattern_index], s->output, size) == 0);
    if (s->frame == 1) CHECK(memcmp(known[0], known[1], size) != 0);
    save(dir, pass, w, h, s->frame, "", s->output, size);
    save(dir, pass, w, h, s->frame, "-middle", s->middle, size);
    printf("stream=%s extent=%dx%d frame=%u checksum=%016" PRIx64 " PASS\n",
           async ? "two" : "sync", w, h, s->frame, checksum(s->output, size));
    s->pending = false;
}
static void run(pl_gpu gpu, int w, int h, bool async, const char *dir)
{
    int ow = w*2+1, oh = h*2+1;
    size_t in_size = (size_t)w*h*4, out_size = (size_t)ow*oh*4;
    uint8_t *input = malloc(in_size), *known[2] = {malloc(out_size), malloc(out_size)};
    CHECK(input && known[0] && known[1]);
    struct slot slots[2] = {0};
    pl_fmt rgba = pl_find_named_fmt(gpu, "rgba8");
    for (unsigned i = 0; i < 2; ++i) {
        struct slot *s = &slots[i];
        s->src = pl_tex_create(gpu, pl_tex_params(.w=w, .h=h, .format=rgba, .sampleable=true, .host_writable=true));
        s->mid = pl_tex_create(gpu, pl_tex_params(.w=ow, .h=oh, .format=rgba, .sampleable=true,
            .storable=true, .renderable=true, .host_readable=true));
        s->dst = pl_tex_create(gpu, pl_tex_params(.w=ow, .h=oh, .format=rgba, .renderable=true, .host_readable=true));
        s->middle = malloc(out_size); s->output = malloc(out_size);
        CHECK(s->src && s->mid && s->dst && s->middle && s->output);
    }
    pl_dispatch dp = pl_dispatch_create(gpu->log, gpu);
    CHECK(dp);
    pl_shader_obj lut = NULL;
    const struct ogpu_stats initial = ogpu_pl_stats(gpu);
    struct ogpu_stats warm = {0};
    unsigned pass = 0;
    for (unsigned frame = 0; frame < 12; ++frame) {
        unsigned index = frame % 2;
        struct slot *s = &slots[index];
        collect(gpu, s, index, async, dir, pass, w, h, known);
        s->frame = frame; s->callbacks = 0;
        if (async) CHECK(ogpu_pl_frame_begin(gpu, index));
        pattern(input, w, h, frame % 3 == 1);
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex=s->src, .ptr=input)));
        memset(input, 0xcc, in_size); // Upload must have copied the caller's bytes.
        process_frame(gpu, dp, &lut, s->src, s->mid, s->dst, ow, oh);
        memset(s->middle, 0x55, out_size); memset(s->output, 0xaa, out_size);
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=s->mid, .ptr=s->middle,
            .callback=completed, .priv=&s->callbacks)));
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=s->dst, .ptr=s->output,
            .callback=completed, .priv=&s->callbacks)));
        if (async) {
            CHECK(s->callbacks == 0 && s->middle[0] == 0x55 && s->output[0] == 0xaa);
            CHECK(ogpu_pl_frame_end(gpu));
        }
        s->pending = true;
        const struct ogpu_stats current = ogpu_pl_stats(gpu);
        CHECK(current.compute-initial.compute == frame+1 && current.raster-initial.raster == frame+1);
        if (async) CHECK(current.waits-initial.waits <= frame);
        if (frame == 0) pass = current.creates;
        CHECK(current.creates == pass && current.creates-initial.creates == 2);
        if (frame == 1) {
            warm = current;
            if (async) CHECK(current.inflight == 2 && slots[0].callbacks == 0 && slots[1].callbacks == 0);
        } else if (frame > 1) {
            CHECK(current.texture_creates == warm.texture_creates && current.staging_allocs == warm.staging_allocs &&
                  current.staging_bytes == warm.staging_bytes && current.banks == warm.banks &&
                  current.texture_bytes == warm.texture_bytes);
        }
        if (!async) collect(gpu, s, index, false, dir, pass, w, h, known);
    }
    collect(gpu, &slots[0], 0, async, dir, pass, w, h, known);
    collect(gpu, &slots[1], 1, async, dir, pass, w, h, known);
    pl_dispatch_destroy(&dp); pl_shader_obj_destroy(&lut);
    for (unsigned i = 0; i < 2; ++i) {
        pl_tex_destroy(gpu, &slots[i].src); pl_tex_destroy(gpu, &slots[i].mid); pl_tex_destroy(gpu, &slots[i].dst);
        free(slots[i].middle); free(slots[i].output);
    }
    free(input); free(known[0]); free(known[1]);
}
int main(int argc, char **argv)
{
    CHECK(argc == 3 && (!strcmp(argv[2], "sync") || !strcmp(argv[2], "two")));
    bool async = !strcmp(argv[2], "two");
    pl_log log = pl_log_create(PL_API_VER, pl_log_params(.log_cb=log_message, .log_level=PL_LOG_INFO));
    CHECK(log);
    pl_gpu gpu = ogpu_pl_create(log, 0); CHECK(gpu);
    run(gpu, 16, 16, async, argv[1]); run(gpu, 31, 17, async, argv[1]); run(gpu, 64, 33, async, argv[1]);
    pl_gpu_finish(gpu);
    struct ogpu_stats s = ogpu_pl_stats(gpu);
    CHECK(!pl_gpu_is_failed(gpu) && !s.textures && !s.passes && !s.banks && !s.staging_bytes && !s.texture_bytes && !s.outstanding);
    CHECK(s.creates == 6 && s.compute == 36 && s.raster == 36 && s.uploads == 39 && s.downloads == 72);
    CHECK(s.submissions == 183 && s.callbacks == 72 && s.staging_allocs == 21 && s.peak_banks == 4 &&
          s.texture_creates == 21 && s.peak_textures == 7);
    if (async) CHECK(s.frames == 36 && s.collected == 36 && s.peak_inflight == 2 && s.waits <= 36 && s.peak_outstanding <= 16);
    else CHECK(!s.frames && !s.inflight && s.waits == 183);
    ogpu_pl_destroy(&gpu); pl_log_destroy(&log); CHECK(!log_errors);
    printf("stream-summary mode=%s frames=%u collected=%u peak_inflight=%u submissions=%u waits=%u polls=%u "
           "peak_operations=%u callbacks=%u staging_allocs=%u peak_staging_bytes=%zu peak_banks=%u "
           "peak_textures=%u peak_texture_bytes=%zu live=0 PASS\n",
           argv[2], s.frames, s.collected, s.peak_inflight, s.submissions, s.waits, s.polls,
           s.peak_outstanding, s.callbacks, s.staging_allocs, s.peak_staging_bytes, s.peak_banks,
           s.peak_textures, s.peak_texture_bytes);
}
