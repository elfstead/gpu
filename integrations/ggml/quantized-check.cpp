// Q8_0 is a consumer storage choice, decoded using baseline storage16 + FP32.
#include "backend.h"
#include "ggml-backend.h"
#include "ggml.h"
#include "timing.h"
#include <cmath>
#include <cstdio>
#include <cstring>
#include <limits>
#include <memory>
#include <string>
#include <vector>
using Context=std::unique_ptr<ggml_context,decltype(&ggml_free)>;
using Backend=std::unique_ptr<ggml_backend,decltype(&ggml_backend_free)>;
using Buffer=std::unique_ptr<ggml_backend_buffer,decltype(&ggml_backend_buffer_free)>;
static void require(bool ok,const char *message) { if (!ok) throw std::runtime_error(message); }
struct Block { ggml_fp16_t scale; int8_t values[32]; };
static_assert(sizeof(Block)==34 && offsetof(Block,values)==2);
static void check(OgpuGgmlSession &session,int k,int m,int n,ggml_type type,bool timing) {
    const char *precision=type==GGML_TYPE_Q8_0?"q8":type==GGML_TYPE_F16?"f16":"f32";
    Context ctx(ggml_init({1<<20,nullptr,true}),ggml_free);
    Backend backend(ggml_backend_init_by_name("OGPU",nullptr),ggml_backend_free);
    require(bool(ctx)&&bool(backend),"linear setup failed");
    auto *a=ggml_new_tensor_2d(ctx.get(),type,k,m);
    auto *b=ggml_new_tensor_2d(ctx.get(),GGML_TYPE_F32,k,n);
    auto *c=ggml_mul_mat(ctx.get(),a,b);
    auto *graph=ggml_new_graph(ctx.get());ggml_build_forward_expand(graph,c);
    // Narrow weights deliberately exercise two-byte-but-not-four-byte alignment.
    const size_t offset=type==GGML_TYPE_F32?4:2;
    Buffer ab(ggml_backend_alloc_buffer(backend.get(),ggml_nbytes(a)+offset+64),ggml_backend_buffer_free);
    Buffer bb(ggml_backend_alloc_buffer(backend.get(),ggml_nbytes(b)+128),ggml_backend_buffer_free);
    Buffer cb(ggml_backend_alloc_buffer(backend.get(),ggml_nbytes(c)+128),ggml_backend_buffer_free);
    require(bool(ab)&&bool(bb)&&bool(cb),"linear allocation failed");
    auto allocate=[](ggml_backend_buffer_t buffer,ggml_tensor *tensor,size_t off) {
        require(ggml_backend_tensor_alloc(buffer,tensor,static_cast<char *>(ggml_backend_buffer_get_base(buffer))+off)==GGML_STATUS_SUCCESS,"tensor allocation failed");
        ggml_backend_buffer_clear(buffer,0xa5);
    };
    allocate(ab.get(),a,offset);allocate(bb.get(),b,64);allocate(cb.get(),c,64);
    // Independent raw byte views permit allocation guard checks through public GGML callbacks.
    auto raw=[&](ggml_backend_buffer_t buffer) {
        auto *tensor=ggml_new_tensor_1d(ctx.get(),GGML_TYPE_I8,ggml_backend_buffer_get_size(buffer));
        require(ggml_backend_tensor_alloc(buffer,tensor,ggml_backend_buffer_get_base(buffer))==GGML_STATUS_SUCCESS,"raw guard view allocation failed");
        return tensor;
    };
    auto *ra=raw(ab.get()),*rb=raw(bb.get()),*rc=raw(cb.get());
    std::vector<float> original(k*m),wide(k*m);
    std::vector<ggml_fp16_t> half(k*m);std::vector<Block> blocks(k*m/32);
    for (size_t i=0;i<original.size();++i)
        original[i]=((i/32)%5==1)?0.0f:float(int((i*37+11)%251)-125)*0.0123f;
    // One exactly representable sentinel block tests -128, 127 and both byte positions.
    for (int i=0;i<32;++i) original[i]=float(i==0?-128:i==1?127:i-16)/64.0f;
    for (size_t block=0;block<blocks.size();++block) {
        float maximum=0;for (int j=0;j<32;++j) maximum=std::max(maximum,std::abs(original[block*32+j]));
        blocks[block].scale=ggml_fp32_to_fp16(block==0?1.0f/64:maximum/127);
        float scale=ggml_fp16_to_fp32(blocks[block].scale);
        for (int j=0;j<32;++j) {
            int quant=scale==0?0:int(std::round(original[block*32+j]/scale));
            blocks[block].values[j]=int8_t(block==0?quant:std::clamp(quant,-127,127));
        }
    }
    for (size_t i=0;i<original.size();++i) {
        half[i]=ggml_fp32_to_fp16(original[i]);
        wide[i]=type==GGML_TYPE_Q8_0?ggml_fp16_to_fp32(blocks[i/32].scale)*float(blocks[i/32].values[i%32]):
                type==GGML_TYPE_F16?ggml_fp16_to_fp32(half[i]):original[i];
    }
    const void *weights=type==GGML_TYPE_Q8_0?static_cast<void *>(blocks.data()):
                        type==GGML_TYPE_F16?static_cast<void *>(half.data()):static_cast<void *>(original.data());
    ggml_backend_tensor_set(a,weights,0,ggml_nbytes(a));
    std::vector<unsigned char> expected_a(ggml_backend_buffer_get_size(ab.get()),0xa5);
    std::memcpy(expected_a.data()+offset,weights,ggml_nbytes(a));
    std::vector<float> input(k*n),actual(m*n),first;
    double maximum_error=0,maximum_drift=0,maximum_ratio=0;
    uint64_t stable_staging=0;
    auto verify_guards=[&] {
        for (auto *tensor:{ra,rb,rc}) {
            std::vector<unsigned char> bytes(ggml_nbytes(tensor));ggml_backend_tensor_get(tensor,bytes.data(),0,bytes.size());
            if (tensor==ra) require(bytes==expected_a,"weight bytes/guards changed");
            else {
                const size_t length=tensor==rb?ggml_nbytes(b):ggml_nbytes(c);
                require(std::all_of(bytes.begin(),bytes.begin()+64,[](auto x){return x==0xa5;}) &&
                        std::all_of(bytes.begin()+64+length,bytes.end(),[](auto x){return x==0xa5;}),"input/output guard changed");
                if (tensor==rb) require(std::memcmp(bytes.data()+64,input.data(),length)==0,"input changed");
            }
        }
    };
    for (int phase=0;phase<3;++phase) {
        for (size_t i=0;i<input.size();++i) input[i]=float(int((i*19+(phase==1?5:17))%97)-48)*0.017123f;
        ggml_backend_tensor_set(b,input.data(),0,ggml_nbytes(b));
        std::fill(actual.begin(),actual.end(),std::numeric_limits<float>::quiet_NaN());
        ggml_backend_tensor_set(c,actual.data(),0,ggml_nbytes(c));
        auto before=session.stats();auto count=session.dispatch_count();
        require(ggml_backend_graph_compute(backend.get(),graph)==GGML_STATUS_SUCCESS,"linear execution failed");
        auto after=session.stats();
        require(session.dispatch_count()==count+1 && after.uploads==before.uploads && after.downloads==before.downloads &&
                after.staging_allocations==before.staging_allocations,"linear graph transfer/allocation/dispatch mismatch");
        ggml_backend_tensor_get(c,actual.data(),0,ggml_nbytes(c));
        for (int row=0;row<n;++row) for (int col=0;col<m;++col) {
            double reference=0,unquantized=0,magnitude=0,representation=0;
            for (int j=0;j<k;++j) {
                const double x=input[row*k+j],w=wide[col*k+j],original_w=original[col*k+j];
                reference+=w*x;unquantized+=original_w*x;magnitude+=std::abs(w*x);representation+=std::abs((w-original_w)*x);
            }
            double error=std::abs(double(actual[row*m+col])-reference);
            double drift=std::abs(double(actual[row*m+col])-unquantized);
            double bound=1e-5+8*k*std::numeric_limits<float>::epsilon()*magnitude;
            require(std::isfinite(actual[row*m+col]) && error<=bound && drift<=representation+bound,"linear numerical/representation bound failed");
            maximum_error=std::max(maximum_error,error);maximum_drift=std::max(maximum_drift,drift);maximum_ratio=std::max(maximum_ratio,error/bound);
        }
        if (phase==0) first=actual;
        if (phase==2) require(std::memcmp(first.data(),actual.data(),actual.size()*4)==0,"linear A/B/A mismatch");
        verify_guards();
        if (phase==0) stable_staging=session.stats().staging_allocations;
        else require(session.stats().staging_allocations==stable_staging,"linear staging grew");
    }
    const auto saved_a=*a,saved_b=*b,saved_c=*c;
    auto reject=[&](bool metadata=true) {
        if (metadata) require(!ggml_backend_supports_op(backend.get(),c),"invalid linear metadata advertised");
        const auto count=session.dispatch_count();
        require(ggml_backend_graph_compute(backend.get(),graph)==GGML_STATUS_FAILED && session.dispatch_count()==count,"rejected linear dispatched");
        *a=saved_a;*b=saved_b;*c=saved_c;
        std::vector<float> unchanged(actual.size());ggml_backend_tensor_get(c,unchanged.data(),0,ggml_nbytes(c));
        require(unchanged==actual,"rejected linear modified output");verify_guards();
    };
    a->type=GGML_TYPE_Q4_0;reject();
    // A singleton quantization block has no second block stride to invalidate.
    if (type==GGML_TYPE_Q8_0 && k==32) a->ne[0]=64;
    a->nb[0]+=2;reject();
    a->view_src=b;reject();
    b->type=GGML_TYPE_F16;reject();
    c->type=GGML_TYPE_F16;reject();
    c->src[2]=b;reject();
    c->op=GGML_OP_ADD;reject();
    c->data=b->data;c->buffer=b->buffer;reject(false);
    a->data=static_cast<char *>(a->data)-1;reject(false);
    a->ne[0]=k-1;reject();
    auto count=session.dispatch_count();
    require(ggml_backend_graph_compute(backend.get(),graph)==GGML_STATUS_SUCCESS && session.dispatch_count()==count+1,"linear retry failed");
    std::vector<float> retry(actual.size());ggml_backend_tensor_get(c,retry.data(),0,ggml_nbytes(c));require(retry==actual,"linear retry output changed");
    size_t allocation=ggml_backend_buffer_get_size(ab.get())+ggml_backend_buffer_get_size(bb.get())+ggml_backend_buffer_get_size(cb.get());
    const std::string identity="kind=linear k="+std::to_string(k)+" m="+std::to_string(m)+" n="+std::to_string(n)+" precision="+precision;
    if (timing) {
        time_graph(session,backend.get(),graph,identity,1,n,allocation,ggml_nbytes(a));
        ggml_backend_tensor_get(c,retry.data(),0,ggml_nbytes(c));require(retry==actual,"timed linear output changed");verify_guards();
    }
    std::printf("quantized_linear k=%d m=%d n=%d precision=%s phases=3 dispatches=3 rejected=10 retry=1 max_error=%.9g max_drift=%.9g error_bound_ratio=%.9g allocation_bytes=%zu weight_bytes=%zu PASS\n",
        k,m,n,precision,maximum_error,maximum_drift,maximum_ratio,allocation,ggml_nbytes(a));
}
int main(int argc,char **argv) {
    if (argc!=4 && argc!=5) return 2;
    try {
        const bool timing=timing_requested(argc,argv);
        const std::string placement=argv[3];require(placement=="host"||placement=="device","bad placement");
        size_t end=0;auto index=std::stoul(argv[2],&end);require(end==std::strlen(argv[2])&&index<=UINT32_MAX,"bad device index");
        const uint16_t endian=1;require(*reinterpret_cast<const unsigned char *>(&endian)==1,"fixture requires little endian");
        OgpuGgmlSession session(uint32_t(index),argv[1],placement=="host"?OgpuGgmlMemory::Host:OgpuGgmlMemory::Device,OgpuGgmlProfile::QuantizedLinear);
        std::printf("ML_DEVICE %s\n",session.device_description());
        const int shapes[][3]={{32,1,1},{32,11,3},{64,13,17},{96,17,5},{256,128,64}};
        for (const auto &shape:shapes) for (auto type:{GGML_TYPE_F32,GGML_TYPE_F16,GGML_TYPE_Q8_0})
            check(session,shape[0],shape[1],shape[2],type,timing);
        std::printf("Quantized linear profile placement=%s cases=15 PASS; no matrix-acceleration claim\n",placement.c_str());return 0;
    } catch (const std::exception &e) { std::fprintf(stderr,"quantized check failed: %s\n",e.what());return 1; }
}
