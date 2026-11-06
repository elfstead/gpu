/* Bounded selection policy for the 61-feature validation fixture only.
 * These masks are not OGPU ABI values. The fixture uses them to combine test
 * families and print diagnostics; the runtime receives ordinary feature ID lists.
 * New IDs above 64 need another fixture representation, not an API revision. */
#define FIXTURE_BIT(id) (UINT64_C(1) << ((id)-1))
static uint64_t fixture_mask(ogpu_next_features features) {
    uint64_t mask = 0;
    for (uint32_t i=0; i<features.count; ++i)
        if (features.ids[i] && features.ids[i]<=64) mask |= FIXTURE_BIT(features.ids[i]);
    return mask;
}
static ogpu_next_features fixture_features(uint64_t mask, ogpu_next_feature ids[64]) {
    uint32_t count = 0;
    for (uint32_t i=0; i<64; ++i) if (mask & (UINT64_C(1)<<i)) ids[count++]=i+1;
    return (ogpu_next_features){ids,count,0};
}
/* C compound-literal backing storage lives until the enclosing block ends. */
#define FIXTURE_FEATURES(mask) fixture_features((mask), (ogpu_next_feature[64]){0})
