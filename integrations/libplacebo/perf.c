// Controlled whole-workload comparison, not an isolated Vulkan-call benchmark.
#include "backend.h"
#include "ogpu.h"
#include <libplacebo/vulkan.h>
#include <inttypes.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#ifdef OGPU_DIAGNOSTICS
#include "diagnostics.h"
#endif
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "check failed: %s:%d: %s\n", __FILE__, __LINE__, #x); exit(1); } } while (0)
#define OGPU_STREAM_WORKLOAD
#include "workload.h"

enum engine { NATIVE, OPERATIONS, BATCHED };
static const char *names[] = {"vulkan", "operations", "batched"};
static unsigned log_errors;
static uint32_t reference_vendor, reference_device;
static void log_message(void *priv, enum pl_log_level level, const char *message)
{
    if (level <= PL_LOG_WARN) fprintf(stderr, "libplacebo[%d]: %s\n", level, message);
    if (level <= PL_LOG_ERR) ++log_errors;
}
static double clock_ms(clockid_t clock)
{
    struct timespec t; CHECK(clock_gettime(clock, &t) == 0);
    return t.tv_sec * 1000.0 + t.tv_nsec / 1e6;
}
static double now(void) { return clock_ms(CLOCK_MONOTONIC); }
static double cpu(void) { return clock_ms(CLOCK_THREAD_CPUTIME_ID); }
static double process_cpu(void) { return clock_ms(CLOCK_PROCESS_CPUTIME_ID); }
static void completed(void *ptr) { atomic_fetch_add((atomic_uint *)ptr, 1); }
struct slot {
    pl_tex src, mid, dst;
    pl_buf upload;
    uint8_t *middle, *output;
    atomic_uint callbacks;
    unsigned frame, pattern;
    bool pending, readback;
    double start;
};
struct reference { uint8_t *middle[3], *output[3]; bool seen[3]; size_t differences; unsigned maximum; };
struct measurement { double submit_wall, submit_cpu, collect_wall, latency[4096]; };
static void upload(pl_gpu gpu, struct slot *s, uint8_t *data, size_t size)
{
    if (s->upload) {
        // Explicit reusable host staging avoids the native pointer-upload
        // heuristic's device-local vkCmdUpdateBuffer path. Slot collection
        // precedes reuse; this copy remains inside transfer-mode timing.
        pl_buf_write(gpu, s->upload, 0, data, size);
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex=s->src, .buf=s->upload)));
    } else {
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex=s->src, .ptr=data, .no_import=true)));
    }
}
static void check_bytes(const uint8_t *a, const uint8_t *b, size_t size, struct reference *ref)
{
    for (size_t i = 0; i < size; ++i) {
        unsigned delta = a[i] > b[i] ? a[i]-b[i] : b[i]-a[i];
        CHECK(delta <= 2 && (i%4 != 3 || (a[i] == 255 && b[i] == 255)));
        ref->differences += delta != 0;
        if (delta > ref->maximum) ref->maximum = delta;
    }
}
static void collect(pl_gpu gpu, enum engine engine, struct slot *s, unsigned index,
                    size_t size, struct measurement *m, struct reference *ref, bool validate)
{
    if (!s->pending) return;
    double start = now();
    if (engine != NATIVE) {
        CHECK(ogpu_pl_frame_collect(gpu, index, true));
    } else {
        // Native callbacks may already have fired. Polling drives pending callback
        // delivery as well as device completion; no sleep or global idle wait.
        while (pl_tex_poll(gpu, s->dst, UINT64_MAX)) {}
        if (s->readback) while (atomic_load(&s->callbacks) != 2) {
            (void)pl_tex_poll(gpu, s->mid, UINT64_MAX);
            (void)pl_tex_poll(gpu, s->dst, UINT64_MAX);
            CHECK(!pl_gpu_is_failed(gpu));
        }
    }
#ifdef OGPU_DIAGNOSTICS
    diagnostic_collect(gpu);
#endif
    double end = now();
    CHECK(!pl_gpu_is_failed(gpu));
    CHECK(atomic_load(&s->callbacks) == (s->readback ? 2u : 0u));
    if (m) { m->collect_wall += end-start; m->latency[s->frame] = end-s->start; }
    if (validate) {
        CHECK(s->readback && memcmp(s->middle, s->output, size) == 0);
        for (size_t i = 3; i < size; i += 4) CHECK(s->output[i] == 255);
        if (ref) {
            unsigned p = s->pattern;
            if (engine == NATIVE && !ref->seen[p]) {
                memcpy(ref->middle[p], s->middle, size); memcpy(ref->output[p], s->output, size);
                ref->seen[p] = true;
            } else {
                CHECK(ref->seen[p]);
                check_bytes(ref->middle[p], s->middle, size, ref);
                check_bytes(ref->output[p], s->output, size, ref);
            }
        }
    }
    s->pending = false;
}
static int compare_double(const void *a, const void *b)
{ double x = *(const double *)a, y = *(const double *)b; return (x>y)-(x<y); }

static void run(enum engine engine, bool transfers, int w, int h, unsigned count, struct reference *ref)
{
    double setup_start = now();
    pl_log log = pl_log_create(PL_API_VER, pl_log_params(.log_cb=log_message, .log_level=PL_LOG_WARN)); CHECK(log);
    pl_vulkan vk = NULL;
    pl_gpu gpu = NULL;
    if (engine == NATIVE) {
        vk = pl_vulkan_create(log, pl_vulkan_params(.allow_software=true, .async_compute=false, .async_transfer=false));
        CHECK(vk); gpu = vk->gpu;
        VkPhysicalDeviceProperties p;
        PFN_vkGetPhysicalDeviceProperties get = (PFN_vkGetPhysicalDeviceProperties)vk->get_proc_addr(vk->instance, "vkGetPhysicalDeviceProperties");
        CHECK(get); get(vk->phys_device, &p);
        reference_vendor = p.vendorID; reference_device = p.deviceID;
        printf("device engine=vulkan name=%s vendor=%u device=%u driver=%u api=%u\n", p.deviceName, p.vendorID, p.deviceID, p.driverVersion, p.apiVersion);
        VkPhysicalDeviceDriverProperties driver = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DRIVER_PROPERTIES};
        VkPhysicalDeviceProperties2 properties = {.sType=VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2, .pNext=&driver};
        PFN_vkGetPhysicalDeviceProperties2 get2 = (PFN_vkGetPhysicalDeviceProperties2)vk->get_proc_addr(vk->instance, "vkGetPhysicalDeviceProperties2");
        CHECK(get2); get2(vk->phys_device, &properties);
        printf("driver name=%s info=%s\n", driver.driverName, driver.driverInfo);
        CHECK(vk->queue_graphics.index == vk->queue_compute.index && vk->queue_graphics.index == vk->queue_transfer.index);
    } else {
        OgpuProbe *probe = NULL; OgpuError error; OgpuDeviceInfo info;
        CHECK(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error) == OGPU_SUCCESS);
        uint32_t devices = 0;
        CHECK(ogpu_probe_device_count(probe, &devices) == OGPU_SUCCESS && devices == 1);
        CHECK(ogpu_probe_device_info(probe, 0, &info) == OGPU_SUCCESS);
        if (ref) CHECK(info.vendor_id == reference_vendor && info.device_id == reference_device);
        printf("device engine=%s name=%s vendor=%u device=%u api=%u.%u.%u\n", names[engine], info.name,
               info.vendor_id, info.device_id, info.vulkan_api_major, info.vulkan_api_minor, info.vulkan_api_patch);
        ogpu_probe_destroy(probe);
        gpu = ogpu_pl_create(log, 0); CHECK(gpu);
    }
#ifdef OGPU_DIAGNOSTICS
    diagnostic_attach(gpu, engine == NATIVE);
#endif
    int ow = w*2+1, oh = h*2+1;
    size_t in_size = (size_t)w*h*4, out_size = (size_t)ow*oh*4;
    uint8_t *input[3];
    for (unsigned i = 0; i < 3; ++i) { input[i] = malloc(in_size); CHECK(input[i]); pattern(input[i], w, h, i == 1); }
    struct slot slots[2] = {0};
    pl_fmt rgba = pl_find_named_fmt(gpu, "rgba8"); CHECK(rgba && gpu->limits.callbacks);
    for (unsigned i = 0; i < 2; ++i) {
        struct slot *s = &slots[i]; atomic_init(&s->callbacks, 0);
        s->src = pl_tex_create(gpu, pl_tex_params(.w=w, .h=h, .format=rgba, .sampleable=true, .host_writable=true));
        s->mid = pl_tex_create(gpu, pl_tex_params(.w=ow, .h=oh, .format=rgba, .sampleable=true, .storable=true,
            .renderable=true, .host_readable=true));
        s->dst = pl_tex_create(gpu, pl_tex_params(.w=ow, .h=oh, .format=rgba, .renderable=true, .host_readable=true));
        s->middle = malloc(out_size); s->output = malloc(out_size);
        CHECK(s->src && s->mid && s->dst && s->middle && s->output);
        if (engine == NATIVE) {
            CHECK(gpu->limits.buf_transfer);
            s->upload = pl_buf_create(gpu, pl_buf_params(.size=in_size,
                .host_writable=true, .memory_type=PL_BUF_MEM_HOST));
            CHECK(s->upload);
        }
        upload(gpu, s, input[i], in_size);
    }
    pl_gpu_finish(gpu);
    pl_dispatch dp = pl_dispatch_create(log, gpu); CHECK(dp);
    pl_shader_obj lut = NULL;
    double setup_ms = now()-setup_start, warm_ms = 0, elapsed = 0, process_ms = 0;
    struct measurement m = {0};
    struct ogpu_stats before = {0}, after = {0};
    for (unsigned phase = 0; phase < (ref ? 1u : 2u); ++phase) {
        unsigned frames = phase ? count : ref ? 6 : 8;
        if (phase && engine != NATIVE) before = ogpu_pl_stats(gpu);
#ifdef OGPU_DIAGNOSTICS
        diagnostic_phase(gpu, phase != 0);
#endif
        double start = now(), cpu_start = process_cpu();
        for (unsigned frame = 0; frame < frames; ++frame) {
            unsigned index = frame%2;
            struct slot *s = &slots[index];
            collect(gpu, engine, s, index, out_size, phase ? &m : NULL, ref, !phase);
            s->frame = frame; s->pattern = transfers ? frame%3 : index;
            s->readback = transfers || !phase; atomic_store(&s->callbacks, 0);
            s->start = now(); double begin_cpu = cpu();
            if (engine == OPERATIONS) CHECK(ogpu_pl_frame_begin(gpu, index));
            if (engine == BATCHED) CHECK(ogpu_pl_frame_begin_batched(gpu, index));
            if (transfers) upload(gpu, s, input[s->pattern], in_size);
            process_frame(gpu, dp, &lut, s->src, s->mid, s->dst, ow, oh);
            if (s->readback) {
                CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=s->mid, .ptr=s->middle, .no_import=true,
                    .callback=completed, .priv=&s->callbacks)));
                CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex=s->dst, .ptr=s->output, .no_import=true,
                    .callback=completed, .priv=&s->callbacks)));
            }
            if (engine != NATIVE) CHECK(ogpu_pl_frame_end(gpu));
            pl_gpu_flush(gpu);
            double end_cpu = cpu(), end = now();
            if (phase) { m.submit_wall += end-s->start; m.submit_cpu += end_cpu-begin_cpu; }
            s->pending = true;
        }
        collect(gpu, engine, &slots[0], 0, out_size, phase ? &m : NULL, ref, !phase);
        collect(gpu, engine, &slots[1], 1, out_size, phase ? &m : NULL, ref, !phase);
        double duration = now()-start;
        if (phase) { elapsed = duration; process_ms = process_cpu()-cpu_start; }
        else warm_ms = duration;
    }
    if (engine != NATIVE) {
        after = ogpu_pl_stats(gpu);
        if (!ref) {
            CHECK(after.creates == before.creates && after.texture_creates == before.texture_creates &&
                  after.staging_allocs == before.staging_allocs && after.banks == before.banks);
            CHECK(after.compute-before.compute == count && after.raster-before.raster == count);
            CHECK(after.submissions-before.submissions == count * (engine == BATCHED ? 1u : transfers ? 5u : 2u));
            CHECK(after.waits-before.waits <= count);
        }
    }
    // getrusage is a process high-water mark, not device memory measurement.
    struct rusage usage; CHECK(getrusage(RUSAGE_SELF, &usage) == 0);
    if (!ref) {
        double sum = 0; for (unsigned i = 0; i < count; ++i) sum += m.latency[i];
        qsort(m.latency, count, sizeof(double), compare_double);
        printf("PERF engine=%s mode=%s input=%dx%d output=%dx%d frames=%u setup_ms=%.6f warm_ms=%.6f "
               "elapsed_ms=%.6f fps=%.6f latency_mean_ms=%.6f latency_p50_ms=%.6f latency_p95_ms=%.6f "
               "submit_wall_ms=%.6f submit_cpu_ms=%.6f collect_wall_ms=%.6f process_cpu_ms=%.6f "
               "submissions=%d waits=%d polls=%d texture_payload=%zu ogpu_staging_payload=%zu peak_rss_kib=%ld PASS\n",
               names[engine], transfers ? "transfers" : "resident", w,h,ow,oh,count,setup_ms,warm_ms,
               elapsed,count*1000/elapsed,sum/count,m.latency[(count-1)/2],m.latency[(95*count+99)/100-1],
               m.submit_wall/count,m.submit_cpu/count,m.collect_wall/count,process_ms/count,
               engine == NATIVE ? -1 : (int)(after.submissions-before.submissions),
               engine == NATIVE ? -1 : (int)(after.waits-before.waits),
               engine == NATIVE ? -1 : (int)(after.polls-before.polls),
               2*(in_size+2*out_size),after.staging_bytes,usage.ru_maxrss);
    } else {
        printf("VERIFY engine=%s mode=%s input=%dx%d output=%dx%d differences=%zu max_delta=%u checksum=%016" PRIx64 " PASS\n",
               names[engine],transfers ? "transfers" : "resident",w,h,ow,oh,ref->differences,ref->maximum,
               checksum(slots[0].output, out_size));
    }
    pl_dispatch_destroy(&dp); pl_shader_obj_destroy(&lut);
    for (unsigned i = 0; i < 2; ++i) {
        pl_tex_destroy(gpu,&slots[i].src); pl_tex_destroy(gpu,&slots[i].mid); pl_tex_destroy(gpu,&slots[i].dst);
        pl_buf_destroy(gpu,&slots[i].upload);
        free(slots[i].middle); free(slots[i].output);
    }
    for (unsigned i = 0; i < 3; ++i) free(input[i]);
    pl_gpu_finish(gpu);
#ifdef OGPU_DIAGNOSTICS
    diagnostic_detach(gpu);
#endif
    if (engine == NATIVE) pl_vulkan_destroy(&vk);
    else {
        struct ogpu_stats s = ogpu_pl_stats(gpu);
        CHECK(!s.textures && !s.passes && !s.outstanding && !s.inflight && !s.staging_bytes && !s.banks);
        ogpu_pl_destroy(&gpu);
    }
    pl_log_destroy(&log); CHECK(!log_errors);
}
static unsigned number(const char *s, unsigned max)
{ char *end; unsigned long n = strtoul(s,&end,10); CHECK(*s && !*end && n && n <= max); return n; }
int main(int argc, char **argv)
{
    CHECK(argc == 6);
    unsigned w = number(argv[3],1920), h = number(argv[4],1080), frames = number(argv[5],4096);
    CHECK(frames >= 2 && frames%2 == 0);
    CHECK(!strcmp(argv[2],"resident") || !strcmp(argv[2],"transfers"));
    bool transfers = !strcmp(argv[2],"transfers");
    if (!strcmp(argv[1],"verify")) {
        struct reference ref = {0};
        size_t bytes = (size_t)(w*2+1)*(h*2+1)*4;
        for (unsigned i = 0; i < 3; ++i) { ref.middle[i]=malloc(bytes); ref.output[i]=malloc(bytes); CHECK(ref.middle[i] && ref.output[i]); }
        for (enum engine e = NATIVE; e <= BATCHED; ++e) run(e, transfers, w,h,frames,&ref);
        for (unsigned i = 0; i < 3; ++i) { free(ref.middle[i]); free(ref.output[i]); }
    } else {
        enum engine e;
        for (e = NATIVE; e <= BATCHED && strcmp(argv[1],names[e]); ++e) {}
        CHECK(e <= BATCHED); run(e,transfers,w,h,frames,NULL);
    }
}
