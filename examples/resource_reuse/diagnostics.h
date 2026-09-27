#ifndef OGPU_EXAMPLE_REUSE_DIAGNOSTICS_H
#define OGPU_EXAMPLE_REUSE_DIAGNOSTICS_H
#include <stdio.h>
#include "slots.h"

static inline const char *reuse_state_name(ReuseState state) {
    switch (state) {
    case REUSE_IDLE: return "idle";
    case REUSE_RESERVED: return "reserved";
    case REUSE_RECORDED: return "recorded";
    case REUSE_PENDING: return "pending";
    case REUSE_READY: return "ready";
    case REUSE_FAILED_PENDING: return "failed-pending";
    case REUSE_FAILED_DRAINED: return "failed-drained";
    }
    return "invalid";
}
/* Escape arbitrary diagnostic bytes, including non-ASCII, as valid JSON. */
static inline void reuse_json_string(FILE *file, const char *text) {
    fputc('"', file);
    for (const unsigned char *p = (const unsigned char *)text; *p; ++p) {
        if (*p == '"' || *p == '\\') { fputc('\\', file); fputc(*p, file); }
        else if (*p < 32 || *p >= 127) fprintf(file, "\\u%04x", *p);
        else fputc(*p, file);
    }
    fputc('"', file);
}
#endif
