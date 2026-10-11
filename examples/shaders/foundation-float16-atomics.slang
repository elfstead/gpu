#define ATOMIC_SCALAR half
// All partial sums remain exact half-representable quarter units, regardless of
// atomic order. This is a finite-data fixture, not a reproducible-reduction promise.
#define ATOMIC_ADD_TERM(i) (Scalar((i) % 4 + 1) * 0.25)
#include "foundation-float-atomics.slang"
