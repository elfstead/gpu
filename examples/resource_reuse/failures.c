/* Failure integration of the SAME mapped learned-image stream. Test injection
 * is confined to an explicitly loaded Vulkan shim, not a runtime API. */
#define FRONTIER_REUSE
#define FRONTIER_STREAM_NO_MAIN
#include "../performance_frontier/stream.c"
#include "diagnostics.h"
#include <dlfcn.h>

enum { REJECT = 1, TIMEOUT, POLL_ERROR, WAIT_ERROR, LOSS, UNKNOWN_SUBMIT };
typedef int (*Arm)(unsigned);

static void diagnostic(Stream *s, unsigned index, const char *operation, OgpuResult result, const OgpuError *error) {
    ImageSlot *im = &s->slots[index]; Slot *slot = &s->base.slots[index];
    printf("REUSE_EVENT {\"slot\":%u,\"frame\":%u,\"generation\":%" PRIu64 ",\"state\":\"%s\","
        "\"pending_receipt\":%s,\"retains_list\":%s,\"operation\":", index, slot->sample_index,
        im->reuse.generation, reuse_state_name(im->reuse.state), slot->pending ? "true" : "false", slot->list ? "true" : "false");
    reuse_json_string(stdout, operation);
    printf(",\"result\":%d,\"native_result\":%d,\"message\":", result, error ? error->vulkan_result : 0);
    reuse_json_string(stdout, error ? error->message : "");
    puts("}");
}
static int reject_reuse(ImageSlot *im) {
    uint64_t ticket = UINT64_MAX, generation = im->reuse.generation;
    CHECK(!reuse_acquire(&im->reuse, &ticket) && ticket == UINT64_MAX && im->reuse.generation == generation);
    return 1;
}
static int prepare_frame(Stream *s, unsigned index, unsigned frame) {
    ImageSlot *im = &s->slots[index]; Slot *slot = &s->base.slots[index];
    CHECK(!slot->pending && reuse_acquire(&im->reuse, &im->ticket));
    slot->sample_index = frame; im->input_index = stream_input_index(frame, s->base.count);
    CHECK(sb_write(&s->base, &im->upload, s->inputs[im->input_index], s->sizes[SI]));
    CHECK(stream_record(s, index) && reuse_recorded(&im->reuse, im->ticket));
    return 1;
}
static OgpuResult attempt(Stream *s, unsigned index) {
    Context *c = &s->base; Slot *slot = &c->slots[index];
    OgpuResult result = slot->list ? ogpu_command_list_submit(slot->list, &slot->completion, &c->error)
        : ogpu_batch_submit(slot->batch, &slot->completion, &c->error);
    /* A one-shot batch is consumed even by rejected submission. Never retry it. */
    ogpu_batch_destroy(slot->batch); slot->batch = NULL;
    if (result == OGPU_SUCCESS) { slot->pending = 1; ++slot->submitted; }
    diagnostic(s, index, "submit", result, &c->error);
    return result;
}
static int accept_frame(Stream *s, unsigned index) {
    CHECK(attempt(s, index) == OGPU_SUCCESS);
    CHECK(reuse_submitted(&s->slots[index].reuse, s->slots[index].ticket));
    return reject_reuse(&s->slots[index]);
}
static int quarantine_drained(Stream *s, unsigned index) {
    ImageSlot *im = &s->slots[index];
    CHECK(!s->base.slots[index].pending);
    CHECK(reuse_quarantine(&im->reuse, im->ticket) && reject_reuse(im));
    diagnostic(s, index, "quarantine", OGPU_SUCCESS, NULL);
    CHECK(reuse_failed_drained(&im->reuse, im->ticket) && reject_reuse(im));
    diagnostic(s, index, "drained-no-recycle", OGPU_SUCCESS, NULL);
    return 1;
}
static int scenario(Stream *s, unsigned mode, Arm arm) {
    Context *c = &s->base; StreamFrame frames[64] = {{0}}; double wall;
    CHECK(stream_window(s, 12, frames, 1, &wall)); /* Warm each slot through wraparound. */
    for (unsigned i = 1; i < c->count; ++i) CHECK(prepare_frame(s, i, 12+i) && accept_frame(s, i));
    CHECK(prepare_frame(s, 0, 12));
    ImageSlot *im = &s->slots[0]; Slot *slot = &c->slots[0];
    if (mode == REJECT || mode == UNKNOWN_SUBMIT) {
        CHECK(arm(mode));
        CHECK(attempt(s, 0) == OGPU_ERROR_VULKAN);
        CHECK(!slot->completion && !slot->pending);
        CHECK(c->error.vulkan_result == (mode == REJECT ? -1 : -13));
        /* A failed submit does not retire any other receipt, even if its actual
         * GPU work happened to finish. Check ownership, not physical overlap. */
        for (unsigned i = 1; i < c->count; ++i)
            CHECK(c->slots[i].pending && reject_reuse(&s->slots[i]));
        if (mode == REJECT) {
            uint64_t previous = im->ticket;
            CHECK(reuse_abort_unsubmitted(&im->reuse, im->ticket));
            diagnostic(s, 0, "known-unsubmitted-abort", OGPU_SUCCESS, NULL);
            CHECK(prepare_frame(s, 0, 12) && im->ticket == previous+1 && accept_frame(s, 0));
        } else CHECK(quarantine_drained(s, 0)); /* Public submit drains indeterminate errors. */
    } else {
        CHECK(accept_frame(s, 0) && arm(mode));
        if (mode == TIMEOUT || mode == POLL_ERROR) {
            uint32_t complete = 123;
            OgpuResult result = ogpu_completion_poll(slot->completion, &complete, &c->error);
            diagnostic(s, 0, "poll", result, &c->error);
            CHECK(result == (mode == TIMEOUT ? OGPU_SUCCESS : OGPU_ERROR_VULKAN) && complete == 0);
            CHECK(mode == TIMEOUT || c->error.vulkan_result == -1);
            CHECK(slot->pending && slot->completion && im->reuse.state == REUSE_PENDING && reject_reuse(im));
        } else {
            OgpuResult result = ogpu_completion_wait(slot->completion, &c->error);
            diagnostic(s, 0, "wait", result, &c->error);
            CHECK(result == OGPU_ERROR_VULKAN && c->error.vulkan_result == (mode == LOSS ? -4 : -1));
            /* Wait has a terminal drained/lost contract even on error. Its error
             * is sticky, but must never become an output-validity assertion. */
            CHECK(ogpu_completion_wait(slot->completion, &c->error) == result);
            uint32_t complete = 123;
            CHECK(ogpu_completion_poll(slot->completion, &complete, &c->error) == result && !complete);
            ogpu_completion_destroy(slot->completion); slot->completion = NULL; slot->pending = 0;
            CHECK(quarantine_drained(s, 0));
        }
    }
    if (mode == LOSS || mode == WAIT_ERROR || mode == UNKNOWN_SUBMIT) {
        for (unsigned i = 1; i < c->count; ++i) {
            int okay = wait_slot(c, &c->slots[i]);
            CHECK(okay == (mode != LOSS));
            CHECK(quarantine_drained(s, i));
        }
        puts("REUSE_FAILURE_GATE {\"recovered\":false,\"outputs_after_error_accepted\":false}");
    } else {
        for (unsigned i = 0; i < c->count; ++i) CHECK(stream_retire(s, i, frames, 1));
        CHECK(stream_window(s, 64, frames, 1, &wall));
        CHECK(stream_check_buffers(s) && stream_check_range_padding(s));
        puts("REUSE_FAILURE_GATE {\"recovered\":true,\"verified_recovery_frames\":64}");
    }
    for (unsigned i = 0; i < c->count; ++i) {
        CHECK(!c->slots[i].pending && !c->slots[i].completion && reuse_execution_drained(&s->slots[i].reuse));
        diagnostic(s, i, "before-teardown", OGPU_SUCCESS, NULL);
    }
    return 1;
}
int main(int argc, char **argv) {
    (void)create; (void)window; (void)selftest;
    if (argc != 15) return 1; /* Same stream arguments, plus injection mode 1..6. */
    Stream s = {0}; Context *c = &s.base;
    c->policy = argv[1]; c->count = number(argv[2]); unsigned mode = number(argv[14]);
    const char *allocation = getenv("OGPU_STREAM_ALLOCATION");
    if ((strcmp(c->policy, "ogpu") && strcmp(c->policy, "compiled")) || c->count < 2 || c->count > 3
        || !mode || mode > UNKNOWN_SUBMIT || !allocation
        || (strcmp(allocation, "arena") && strcmp(allocation, "dedicated"))) return 1;
    s.arena = !strcmp(allocation, "arena");
    s.w = number(argv[3]); s.h = number(argv[4]); s.ow = number(argv[5]); s.oh = number(argv[6]);
    if (s.w != 65 || s.h != 47 || s.ow != 131 || s.oh != 95 || strcmp(argv[8], "validate")) return 1;
    s.serial_paths[0] = getenv("OGPU_STREAM_SERIAL_A"); s.serial_paths[1] = getenv("OGPU_STREAM_SERIAL_B");
    if (!s.serial_paths[0] || !s.serial_paths[1]) return 1;
    const char *library = getenv("OGPU_VULKAN_LIBRARY"); if (!library) return 1;
    void *shim = dlopen(library, RTLD_NOW | RTLD_LOCAL); if (!shim) return 1;
    Arm arm = (Arm)dlsym(shim, "reuse_fault_arm"); if (!arm) { dlclose(shim); return 1; }
    int okay = 0;
    if (!stream_create(&s, argv)) goto cleanup;
    OgpuCapabilities caps; OgpuError error;
    if (ogpu_device_capabilities(c->device, &caps, &error) != OGPU_SUCCESS) goto cleanup;
    printf("REUSE_CONTEXT {\"abi\":%u,\"backend\":\"vulkan\",\"workload\":\"learned-image\",\"mode\":%u,"
        "\"allocation\":\"%s\",\"policy\":\"%s\",\"slots\":%u,\"graphics\":%u,\"compute\":%u,"
        "\"buffer_address\":%u,\"timeline\":%u,\"synchronization2\":%u,\"descriptor_heap\":%u,"
        "\"address_commands\":%u,\"untyped_pointers\":%u}\n", OGPU_ABI_VERSION, mode, allocation, c->policy,
        c->count, caps.graphics_queue, caps.compute_queue, caps.buffer_device_address, caps.timeline_semaphore,
        caps.synchronization2, caps.descriptor_heap, caps.device_address_commands, caps.shader_untyped_pointers);
    okay = scenario(&s, mode, arm);
cleanup:
    (void)arm(0); /* Unexpected test failure must not leave an injection armed during cleanup. */
    stream_destroy(&s); dlclose(shim);
    if (okay) puts("Reuse failure/drain integration PASS; synthetic faults, not hardware-loss evidence");
    return !okay;
}
