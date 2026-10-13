#pragma once
// Consumer wall time, including graph preflight, recording, submission and wait.
#include "backend.h"
#include "ggml-backend.h"
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <string>
#include <vector>
inline bool timing_requested(int argc,char **argv) {
    if (argc==4) return false;
    if (argc!=5 || std::string(argv[4])!="--timing") throw std::runtime_error("expected optional --timing");
    const char *layers=std::getenv("VK_INSTANCE_LAYERS"),*sync=std::getenv("VK_LAYER_VALIDATE_SYNC");
    const char *disabled=std::getenv("VK_LOADER_LAYERS_DISABLE");
    if ((layers && *layers) || (sync && *sync) || !disabled || std::string(disabled)!="VK_LAYER_KHRONOS_validation")
        throw std::runtime_error("timing requires unset validation variables and VK_LOADER_LAYERS_DISABLE=VK_LAYER_KHRONOS_validation");
    return true;
}
inline void time_graph(OgpuGgmlSession &session,ggml_backend_t backend,ggml_cgraph *graph,
                       const std::string &identity,uint64_t dispatches,uint64_t items,size_t allocation,size_t weights) {
    auto execute=[&] { if (ggml_backend_graph_compute(backend,graph)!=GGML_STATUS_SUCCESS) throw std::runtime_error("timed graph failed"); };
    for (int i=0;i<2;++i) execute();
    const auto before=session.stats();const auto count=session.dispatch_count();
    std::vector<double> samples;
    for (int sample=0;sample<9;++sample) {
        auto start=std::chrono::steady_clock::now();
        for (int i=0;i<10;++i) execute();
        samples.push_back(std::chrono::duration<double,std::milli>(std::chrono::steady_clock::now()-start).count()/10);
    }
    const auto after=session.stats();
    if (session.dispatch_count()!=count+90*dispatches || after.uploads!=before.uploads ||
        after.downloads!=before.downloads || after.staging_allocations!=before.staging_allocations)
        throw std::runtime_error("timed graph transfer/dispatch/allocation mismatch");
    std::printf("ml_samples %s per_graph_ms=",identity.c_str());
    for (size_t i=0;i<samples.size();++i) std::printf("%s%.9g",i?",":"",samples[i]);
    std::printf("\n");std::sort(samples.begin(),samples.end());
    std::printf("ml_timing %s warmups=2 samples=9 iterations=10 min_ms=%.9g median_ms=%.9g max_ms=%.9g items_per_s=%.9g allocation_bytes=%zu weight_bytes=%zu\n",
        identity.c_str(),samples.front(),samples[4],samples.back(),items*1000.0/samples[4],allocation,weights);
}
