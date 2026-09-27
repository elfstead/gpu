/* CPU-only native range-cache argument tests; not noncoherent hardware evidence. */
#define FRONTIER_NATIVE
#define FRONTIER_REUSE
#define FRONTIER_STREAM_NO_MAIN
#include "../performance_frontier/stream.c"

static VkMappedMemoryRange observed;
static unsigned cache_calls;
static VkResult VKAPI_CALL cache(VkDevice d, uint32_t count, const VkMappedMemoryRange *ranges) {
    (void)d;
    if (count != 1) abort();
    observed = ranges[0]; ++cache_calls; return VK_SUCCESS;
}
static int test(void) {
    (void)create; (void)window; (void)selftest;
    (void)stream_create; (void)stream_destroy; (void)stream_window; (void)stream_check_buffers;
    (void)stream_check_range_padding;
    Context c = {0}; unsigned char data[320] = {0};
    c.native.vkFlushMappedMemoryRanges = cache; c.native.vkInvalidateMappedMemoryRanges = cache;
    SBuffer b = {.native={.memory=(VkDeviceMemory)(uintptr_t)42,.allocated=288},
        .view={.data=data,.size_bytes=270,.access_granularity=64}};
    CHECK(sb_cache(&c,&b,67,1,1) && cache_calls == 1 && observed.offset == 64 && observed.size == 64);
    CHECK(observed.memory == b.native.memory);
    CHECK(sb_cache(&c,&b,64,64,0) && cache_calls == 2 && observed.offset == 64 && observed.size == 64);
    CHECK(sb_cache(&c,&b,260,10,0) && cache_calls == 3 && observed.offset == 256 && observed.size == VK_WHOLE_SIZE);
    CHECK(sb_cache(&c,&b,270,0,1) && cache_calls == 3);
    CHECK(!sb_cache(&c,&b,270,1,1) && !sb_cache(&c,&b,271,0,0) && cache_calls == 3);
    b.view.size_bytes = UINT64_MAX;
    CHECK(!sb_cache(&c,&b,UINT64_MAX-1,1,0) && cache_calls == 3);
    b.view.coherent = 1;
    CHECK(sb_cache(&c,&b,0,64,1) && cache_calls == 3);
    puts("Native range-cache alignment, allocation-tail, bounds and coherence tests PASS (CPU fakes)");
    return 1;
}
int main(void) { return !test(); }
