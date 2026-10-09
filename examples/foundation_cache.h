/* Consumer policy: first forbid compilation, then explicitly allow it if needed.
 * The runtime itself never retries. Native cache hits/misses are not deterministic. */
static int prepare_with_compile_policy(ogpu_next_device *device, const ogpu_next_executable_desc *input, ogpu_next_executable **out) {
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES, &features, 1, sizeof(features));
    ogpu_next_executable_desc desc = *input;
    int result = EXIT_FAILURE;
    TRY(ogpu_next_device_query(device, &q));
    desc.compile_flags = 2;
    REQUIRE(ogpu_next_executable_create(device, &desc, out) == OGPU_NEXT_UNSUPPORTED && *out == NULL);
    desc.compile_flags = 0; desc.reserved = 1;
    REQUIRE(ogpu_next_executable_create(device, &desc, out) == OGPU_NEXT_UNSUPPORTED && *out == NULL);
    desc.reserved = 0; desc.compile_flags = OGPU_NEXT_COMPILE_FAIL_IF_REQUIRED;
    ogpu_next_status status = ogpu_next_executable_create(device, &desc, out);
    if (!(features.enabled & OGPU_NEXT_FEATURE_CACHE_CONTROL)) {
        REQUIRE(status == OGPU_NEXT_UNSUPPORTED && *out == NULL);
    } else {
        REQUIRE(status == OGPU_NEXT_OK || status == OGPU_NEXT_COMPILE_REQUIRED);
        REQUIRE((*out == NULL) == (status == OGPU_NEXT_COMPILE_REQUIRED));
        printf("Compile policy (%s, %s cache): %s.\n", desc.kind == OGPU_NEXT_EXECUTABLE_COMPUTE ? "compute" : "graphics",
            desc.cache ? "explicit" : "no", status == OGPU_NEXT_OK ? "prepared without compilation" : "caller elects to compile");
        if (status == OGPU_NEXT_OK) return EXIT_SUCCESS;
    }
    desc.compile_flags = 0;
    TRY(ogpu_next_executable_create(device, &desc, out));
    result = EXIT_SUCCESS;
cleanup:
    return result;
}

/* Public-C cache round trip. Disk persistence, cache keys and eviction belong to consumers. */
static int cache_roundtrip(ogpu_next_device *device, ogpu_next_executable_cache **cache, uint32_t synchronization) {
    int result = EXIT_FAILURE;
    unsigned char *bytes = NULL;
    ogpu_next_executable_cache *restored = NULL;
    size_t size = 0;
    TRY(ogpu_next_executable_cache_data(*cache, &size, NULL));
    REQUIRE(size >= 32 && size <= SIZE_MAX - 16);
    bytes = malloc(size + 16); REQUIRE(bytes != NULL); memset(bytes, 0xa5, size + 16);
    size_t written = 1;
    REQUIRE(ogpu_next_executable_cache_data(*cache, &written, bytes) == OGPU_NEXT_CAPACITY);
    REQUIRE(written == 0 && bytes[0] == 0xa5);
    written = size;
    TRY(ogpu_next_executable_cache_data(*cache, &written, bytes));
    REQUIRE(written >= 32 && written <= size);
    for (size_t i = written; i < size + 16; ++i) REQUIRE(bytes[i] == 0xa5);
    ogpu_next_executable_cache_desc desc = {HEADER(ogpu_next_executable_cache_desc, OGPU_NEXT_EXECUTABLE_CACHE_DESC),
        synchronization, 0, {bytes, written}};
    bytes[8] ^= 1; /* Wrong vendor: rejected before opaque bytes reach the driver. */
    REQUIRE(ogpu_next_executable_cache_create(device, &desc, &restored) == OGPU_NEXT_UNSUPPORTED && restored == NULL);
    bytes[8] ^= 1;
    desc.initial.size = 31;
    REQUIRE(ogpu_next_executable_cache_create(device, &desc, &restored) == OGPU_NEXT_INVALID && restored == NULL);
    desc.initial.size = written;
    TRY(ogpu_next_executable_cache_create(device, &desc, &restored));
    REQUIRE(ogpu_next_executable_cache_merge(restored, 1, &restored) == OGPU_NEXT_INVALID);
    TRY(ogpu_next_executable_cache_merge(restored, 0, NULL));
    TRY(ogpu_next_executable_cache_merge(restored, 1, cache));
    ogpu_next_executable_cache_destroy(*cache); *cache = restored; restored = NULL;
    printf("Executable cache passes: export/import, merge, incompatible header rejection, capacity guards (%s synchronization).\n",
        synchronization == OGPU_NEXT_CACHE_CALLER_SYNCHRONIZATION ? "caller" : "native");
    result = EXIT_SUCCESS;
cleanup:
    ogpu_next_executable_cache_destroy(restored);
    free(bytes);
    return result;
}
