// Consumer recording-recovery checks; no GGML graph or native headers needed.
#include "runtime-foundation.h"
#include <array>
#include <cstdio>
#include <stdexcept>

static void require(bool value,const char *message) {
    if (!value) throw std::runtime_error(message);
}
int main() {
    try {
        gpu::Runtime runtime; runtime.initialize(0);
        auto host = runtime.allocate(64,true), local = runtime.allocate(64,false);
        std::array<unsigned char,64> input{}, actual{};
        for (size_t i = 0; i < input.size(); ++i) input[i] = static_cast<unsigned char>(i*3+1);
        host.write(0,input.data(),input.size());
        runtime.begin(); runtime.barrier(false); runtime.copy(local,0,host,0,64); runtime.finish();
        require(runtime.value == 1 && !runtime.dirty,"first transfer did not retire");
        for (int failure = 0; failure < 2; ++failure) {
            const auto value = runtime.value;
            runtime.begin(); runtime.barrier(false);
            if (failure == 0) runtime.copy(local,2,local,0,8); // Forbidden overlap.
            else runtime.copy(local,63,host,0,2); // Invalid destination range.
            bool rejected = false;
            try { runtime.finish(); } catch (const std::exception &) { rejected = true; }
            require(rejected && runtime.value == value && runtime.dirty,"invalid recording submitted or lost recovery state");
            runtime.begin(); runtime.barrier(false); runtime.copy(host,0,local,0,64); runtime.finish();
            host.read(0,actual.data(),actual.size());
            require(actual == input && runtime.value == value+1 && !runtime.dirty,"recovery changed data or retirement");
        }
        // Public-copy byte spans must not acquire an adapter-imposed 4-byte rule.
        const unsigned char half[2] = {0x21,0x43}; host.write(2,half,2);
        runtime.begin(); runtime.barrier(false); runtime.copy(local,2,host,2,2); runtime.finish();
        runtime.begin(); runtime.barrier(false); runtime.copy(host,0,local,0,64); runtime.finish();
        host.read(0,actual.data(),actual.size()); input[2] = half[0]; input[3] = half[1];
        require(actual == input,"two-byte copy changed surrounding bytes");
        std::puts("GGML foundation invalid-record recovery, explicit retirement and two-byte copy guards PASS");
        return 0;
    } catch (const std::exception &error) {
        std::fprintf(stderr,"%s\n",error.what()); return 1;
    }
}
