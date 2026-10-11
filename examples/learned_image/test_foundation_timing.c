#include "foundation_timing.h"
#include <assert.h>
int main(void) {
    double value = -1;
    assert(timestamp_wrap_ms(0,1) == 0 && timestamp_wrap_ms(65,1) == 0);
    assert(timestamp_wrap_ms(64,NAN) == 0 && timestamp_wrap_ms(64,INFINITY) == 0);
    assert(timestamp_wrap_ms(64,0) == 0 && timestamp_wrap_ms(64,-1) == 0);
    assert(timestamp_delta_ms((UINT64_C(1)<<36)-3,4,36,2,1,&value) && value == 14e-6);
    assert(timestamp_delta_ms(UINT64_MAX-2,4,64,2,1,&value) && value == 14e-6);
    assert(timestamp_delta_ms((UINT64_C(1)<<50)+3,10,36,2,1,&value) && value == 14e-6);
    assert(!timestamp_delta_ms(1,1,64,1,1,&value));
    assert(!timestamp_delta_ms(1,2,32,1,1,&value));
    assert(!timestamp_delta_ms(1,2,36,1,timestamp_wrap_ms(36,1),&value));
    assert(!timestamp_delta_ms(1,2,64,1,NAN,&value));
    assert(!timestamp_delta_ms(1,2,64,1,-1,&value));
    assert(!timestamp_delta_ms(1,2,0,1,1,&value));
    assert(!timestamp_delta_ms(1,2,65,1,1,&value));
    return 0;
}
