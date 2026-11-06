#ifndef OGPU_EXAMPLE_FEATURE_SELECTION_H
#define OGPU_EXAMPLE_FEATURE_SELECTION_H
#include "ogpu_next.h"
/* Caller-side cold setup helpers. No ownership, allocation or selection policy;
 * the caller supplies list storage/capacity and keeps it alive through use. */
static inline int example_feature_contains(ogpu_next_features list, ogpu_next_feature id) {
    for (uint32_t i=0; i<list.count; ++i) if (list.ids[i]==id) return 1;
    return 0;
}
static inline int example_features_include(ogpu_next_features have, ogpu_next_features need) {
    for (uint32_t i=0; i<need.count; ++i) if (!example_feature_contains(have,need.ids[i])) return 0;
    return 1;
}
static inline int example_features_merge(ogpu_next_features *out, ogpu_next_feature *storage,
    uint32_t capacity, ogpu_next_features add) {
    for (uint32_t i=0; i<add.count; ++i) if (!example_feature_contains(*out,add.ids[i])) {
        if (out->count==capacity) return 0;
        storage[out->count++]=add.ids[i];
    }
    return 1;
}
#endif
