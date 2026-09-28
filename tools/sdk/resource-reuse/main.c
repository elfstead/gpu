/* Same measured consumer source, packaged with all public-only dependencies.
 * run.py selects the bounded handoff policy; the C CLI remains experimental. */
#define FRONTIER_REUSE
#include "support/performance_frontier/stream.c"
