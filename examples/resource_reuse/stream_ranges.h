/* Included by stream.c after its bounded buffer adapters. No runtime policy.
 * Dedicated and arena controls have identical logical payload and HOST budgets.
 * 4 KiB-rounded per-range budgets are explicit experimental capacity, NOT an
 * assumption about native cache atoms. Actual queried granularity must fit.
 */
static unsigned stream_input_index(unsigned frame, unsigned slots) {
    return (frame/slots + frame%slots)%2; /* slots is validated nonzero by main. */
}
static int stream_backing(Stream *s, SBuffer *buffer, uint64_t bytes, int host) {
    CHECK(bytes && bytes <= SIZE_MAX);
    CHECK(sb_create(&s->base, buffer, (size_t)bytes, host, NULL));
    CHECK(reuse_add(s->requested_bytes, bytes, &s->requested_bytes)); ++s->allocations;
    if (host) {
        memset(buffer->view.data, 0xa5, (size_t)bytes);
        if (!buffer->view.coherent) {
            Context *c = &s->base;
            API(ogpu_buffer_host_flush(buffer->buffer, 0, bytes, &c->error));
        }
    }
    return 1;
}
static int stream_slice(Context *c, SBuffer *backing, ReuseArena *arena, uint64_t bytes,
                        SBuffer *slice, uint64_t *address, unsigned slot, unsigned resource, unsigned backing_id) {
    ReuseRange range;
    uint64_t atom = backing->view.data ? backing->view.access_granularity : 1;
    CHECK(reuse_reserve(arena, bytes, 4, atom, 0, &range));
    *slice = (SBuffer){.buffer=backing->buffer, .offset=range.payload, .size=bytes, .view=backing->view};
    if (address) {
        uint64_t base = 0;
        API(ogpu_buffer_device_address(backing->buffer, &base, &c->error));
        CHECK(base%4 == 0 && reuse_add(base, range.payload+SG, address));
    }
    printf("REUSE_RANGE {\"slot\":%u,\"resource\":%u,\"backing\":%u,\"offset\":%" PRIu64 ",\"size\":%" PRIu64 ",\"end\":%" PRIu64
        ",\"atom\":%" PRIu64 ",\"host\":%s}\n", slot, resource, backing_id, range.payload, bytes, range.end, atom, backing->view.data ? "true" : "false");
    return 1;
}
static int stream_allocate_ranges(Stream *s) {
    Context *c = &s->base;
    uint64_t upload = s->sizes[SI] > s->sizes[SW] ? s->sizes[SI] : s->sizes[SW];
    uint64_t budgets[3] = {0}, host_budget[2];
    CHECK(reuse_align(upload, 4096, &host_budget[0]) && reuse_align(s->final_size, 4096, &host_budget[1]));
    for (unsigned i = 0; i < c->count; ++i) {
        for (unsigned j = 0; j < SB_COUNT; ++j) CHECK(reuse_add(budgets[0], s->sizes[j], &budgets[0]));
        for (unsigned j = 0; j < 2; ++j) CHECK(reuse_add(budgets[j+1], host_budget[j], &budgets[j+1]));
    }
    ReuseArena cursors[3] = {{0}};
    if (s->arena) for (unsigned j = 0; j < 3; ++j) {
        CHECK(stream_backing(s, &s->arenas[j], budgets[j], j != 0));
        cursors[j].capacity = budgets[j];
    }
    for (unsigned i = 0; i < c->count; ++i) {
        ImageSlot *im = &s->slots[i];
        for (unsigned j = 0; j < SB_COUNT+2; ++j) {
            unsigned kind = j < SB_COUNT ? 0 : j-SB_COUNT+1;
            uint64_t bytes = j < SB_COUNT ? s->sizes[j] : kind == 1 ? upload : s->final_size;
            SBuffer *destination = j < SB_COUNT ? &im->data[j] : kind == 1 ? &im->upload : &im->readback;
            uint64_t *address = j < SB_COUNT ? &im->address[j] : NULL;
            if (s->arena) CHECK(stream_slice(c, &s->arenas[kind], &cursors[kind], bytes, destination, address, i, j, kind));
            else {
                CHECK(stream_backing(s, destination, kind ? host_budget[kind-1] : bytes, kind != 0));
                /* Owner and subrange are one value here; keep ownership on failure. */
                SBuffer owner = *destination, slice;
                ReuseArena cursor = {.capacity=owner.size};
                CHECK(stream_slice(c, &owner, &cursor, bytes, &slice, address, i, j, i*7+j));
                slice.owned = 1; *destination = slice;
            }
        }
    }
    printf("REUSE_MEMORY {\"allocation\":\"%s\",\"buffer_allocations\":%u,\"buffer_requested_bytes\":%" PRIu64
        ",\"host_upload_budget\":%" PRIu64 ",\"host_readback_budget\":%" PRIu64 "}\n",
        s->arena ? "arena" : "dedicated", s->allocations+1, s->requested_bytes+16, budgets[1], budgets[2]);
    return 1;
}
/* Only after every slot and diagnostic transfer has completed. Invalidate the
 * whole parent here, never while another range has conflicting GPU access. */
static int stream_padding(Context *c, SBuffer *parent, SBuffer **ranges, unsigned count) {
    CHECK(parent->view.data && parent->view.size_bytes <= SIZE_MAX);
    if (!parent->view.coherent) API(ogpu_buffer_host_invalidate(parent->buffer, 0, parent->view.size_bytes, &c->error));
    const unsigned char *bytes = parent->view.data;
    uint64_t cursor = 0;
    for (unsigned i = 0; i < count; ++i) {
        SBuffer *range = ranges[i];
        CHECK(range->buffer == parent->buffer && range->offset >= cursor);
        CHECK(range->offset <= parent->view.size_bytes && range->size <= parent->view.size_bytes-range->offset);
        for (; cursor < range->offset; ++cursor) CHECK(bytes[cursor] == 0xa5);
        cursor = range->offset+range->size;
    }
    for (; cursor < parent->view.size_bytes; ++cursor) CHECK(bytes[cursor] == 0xa5);
    return 1;
}
static int stream_check_range_padding(Stream *s) {
    Context *c = &s->base;
    for (unsigned i = 0; i < c->count; ++i) CHECK(reuse_execution_drained(&s->slots[i].reuse));
    for (unsigned kind = 1; kind <= 2; ++kind) {
        SBuffer *ranges[MAX_SLOTS];
        for (unsigned i = 0; i < c->count; ++i) ranges[i] = kind == 1 ? &s->slots[i].upload : &s->slots[i].readback;
        if (s->arena) CHECK(stream_padding(c, &s->arenas[kind], ranges, c->count));
        else for (unsigned i = 0; i < c->count; ++i) CHECK(stream_padding(c, ranges[i], &ranges[i], 1));
    }
    puts("Reuse HOST padding and all slot generations retired PASS");
    return 1;
}
