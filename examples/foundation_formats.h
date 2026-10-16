/* Physical discovery is distinct from per-description placement requirements. */
static int format_capabilities(const ogpu_next_adapter *adapter, ogpu_next_device *device) {
    int result = EXIT_FAILURE;
    ogpu_next_format_info *physical = NULL, *enabled = NULL, *saved = NULL;
    ogpu_next_float_controls float_physical = {0}, float_device = {0};
    ogpu_next_query fq = query(OGPU_NEXT_QUERY_FLOAT_CONTROLS,NULL,0,sizeof(float_physical));
    TRY(ogpu_next_adapter_query(adapter,&fq)); REQUIRE(fq.count == 1);
    fq.data = &float_physical; fq.capacity = 1; TRY(ogpu_next_adapter_query(adapter,&fq));
    fq.data = &float_device; TRY(ogpu_next_device_query(device,&fq));
    REQUIRE(memcmp(&float_physical,&float_device,sizeof(float_physical)) == 0);
    REQUIRE(((float_device.binary16 | float_device.binary32 | float_device.binary64) & ~31u) == 0);
    REQUIRE(float_device.denorm_independence <= 2 && float_device.rounding_independence <= 2 && float_device.reserved == 0);
    memset(&float_device,0xa5,sizeof(float_device)); ogpu_next_float_controls float_saved = float_device;
    fq.element_size--; fq.count = 79;
    REQUIRE(ogpu_next_device_query(device,&fq) == OGPU_NEXT_INVALID && fq.count == 79);
    REQUIRE(memcmp(&float_device,&float_saved,sizeof(float_device)) == 0);
    printf("Float controls: binary16=%u binary32=%u binary64=%u; denorm independence=%u rounding independence=%u (capabilities, not selected policy).\n",
        float_physical.binary16,float_physical.binary32,float_physical.binary64,float_physical.denorm_independence,float_physical.rounding_independence);
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FORMATS, NULL, 0, sizeof(physical[0]));
    TRY(ogpu_next_adapter_query(adapter, &q));
    uint32_t count = q.count;
    REQUIRE(count == 118);
    size_t bytes = count * sizeof(*physical);
    physical = malloc(bytes); enabled = malloc(bytes); saved = malloc(bytes);
    REQUIRE(physical && enabled && saved);
    memset(physical, 0xa5, bytes); memcpy(saved, physical, bytes);
    q.data = physical; q.capacity = count - 1;
    REQUIRE(ogpu_next_adapter_query(adapter, &q) == OGPU_NEXT_CAPACITY && q.count == count);
    REQUIRE(memcmp(physical, saved, bytes) == 0);
    q.capacity = count; TRY(ogpu_next_adapter_query(adapter, &q));
    q.data = enabled; TRY(ogpu_next_device_query(device, &q));
    REQUIRE(q.count == count && memcmp(physical, enabled, count*sizeof(physical[0])) == 0);
    for (uint32_t i = 0; i < count; ++i) {
        REQUIRE(physical[i].format != 0 && physical[i].aspects != 0);
        REQUIRE((physical[i].optimal_features & ~UINT64_C(8191)) == 0);
        REQUIRE((physical[i].linear_features & ~UINT64_C(8191)) == 0);
        REQUIRE((physical[i].buffer_features & ~UINT64_C(15)) == 0);
        for (uint32_t j = 0; j < i; ++j) REQUIRE(physical[i].format != physical[j].format);
    }
    ogpu_next_image_profile profile = {HEADER(ogpu_next_image_profile, OGPU_NEXT_IMAGE_PROFILE),
        OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_IMAGE_2D, OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST, 0, 0};
    ogpu_next_image_limits limits = {0};
    TRY(ogpu_next_image_limits_query(device, &profile, &limits));
    REQUIRE(limits.max_extent[0] >= 16 && limits.max_extent[1] >= 8 && limits.max_extent[2] == 1);
    REQUIRE(limits.max_mips >= 2 && limits.max_layers >= 2 && (limits.sample_counts & 1) && limits.max_resource_size > 0);
    profile.usage |= OGPU_NEXT_IMAGE_STORAGE;
    TRY(ogpu_next_image_limits_query(device, &profile, &limits));
    REQUIRE(limits.sample_counts == 1); /* implemented profile, not physical multisampled-storage enabling */
    profile.dimension = OGPU_NEXT_IMAGE_3D;
    TRY(ogpu_next_image_limits_query(device, &profile, &limits));
    REQUIRE(limits.max_extent[2] > 1 && limits.max_layers == 1 && limits.sample_counts == 1);
    for (uint32_t bad = 0; bad < 7; ++bad) {
        ogpu_next_image_profile invalid = profile;
        if (bad == 0) invalid.format = UINT32_MAX;
        if (bad == 1) invalid.dimension = 0;
        if (bad == 2) invalid.usage = 0;
        if (bad == 3) invalid.image_flags = 2; /* cube flag with 3D */
        if (bad == 4) invalid.reserved = 1;
        if (bad == 5) invalid.header.version += 1;
        if (bad == 6) invalid.header.byte_size -= 1;
        memset(&limits, 0xa5, sizeof(limits));
        ogpu_next_image_limits sentinel = limits;
        ogpu_next_status expected = bad == 0 || bad == 4 || bad == 5 ? OGPU_NEXT_UNSUPPORTED : OGPU_NEXT_INVALID;
        REQUIRE(ogpu_next_image_limits_query(device, &invalid, &limits) == expected);
        REQUIRE(memcmp(&limits, &sentinel, sizeof(limits)) == 0);
    }
    printf("Format discovery passes: immutable adapter/device capabilities, capacity atomicity, exact image-profile limits and unchanged failure outputs.\n");
    result = EXIT_SUCCESS;
cleanup:
    free(physical); free(enabled); free(saved);
    return result;
}
