// Fixed M5 graph and independent FP64 intermediate oracle. No CPU graph fallback.
#include "backend.h"
#include "ggml-backend.h"
#include "ggml-alloc.h"
#include "ggml.h"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <limits>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

using Values = std::vector<double>;
using Context = std::unique_ptr<ggml_context,decltype(&ggml_free)>;
using Backend = std::unique_ptr<ggml_backend,decltype(&ggml_backend_free)>;
using Buffer = std::unique_ptr<ggml_backend_buffer,decltype(&ggml_backend_buffer_free)>;
static void require(bool ok,const char *message) { if (!ok) throw std::runtime_error(message); }
static Values multiply(const Values &a,const Values &b,int k,int m,int n) {
    Values c(m*n);
    for (int y=0;y<n;++y) for (int x=0;x<m;++x)
        for (int j=0;j<k;++j) c[y*m+x]+=a[x*k+j]*b[y*k+j];
    return c;
}
static Values norm(const Values &a,int d,int t) {
    Values result(a.size());
    for (int row=0;row<t;++row) {
        double sum=0;
        for (int i=0;i<d;++i) sum+=a[row*d+i]*a[row*d+i];
        const double divisor=std::sqrt(sum/d+double(1e-5f));
        for (int i=0;i<d;++i) result[row*d+i]=a[row*d+i]/divisor;
    }
    return result;
}
static Values add(const Values &a,const Values &b) {
    Values c(a.size());for (size_t i=0;i<a.size();++i) c[i]=a[i]+b[i];return c;
}
static Values transpose(const Values &a,int m,int n) {
    Values c(a.size());for (int y=0;y<n;++y) for (int x=0;x<m;++x) c[x*n+y]=a[y*m+x];return c;
}
static Values probabilities(const Values &a,int t,float scale) {
    Values p(a.size());
    for (int y=0;y<t;++y) {
        double maximum=-std::numeric_limits<double>::infinity(),sum=0;
        for (int x=0;x<t;++x) maximum=std::max(maximum,a[y*t+x]*scale);
        for (int x=0;x<t;++x) { p[y*t+x]=std::exp(a[y*t+x]*scale-maximum);sum+=p[y*t+x]; }
        for (int x=0;x<t;++x) p[y*t+x]/=sum;
    }
    return p;
}

static std::vector<float> check(OgpuGgmlSession &session,int d,int h,int t,bool half) {
    Context ctx(ggml_init({4<<20,nullptr,true}),ggml_free);
    Backend backend(ggml_backend_init_by_name("OGPU",nullptr),ggml_backend_free);
    require(bool(ctx)&&bool(backend),"transformer setup failed");
    std::map<std::string,ggml_tensor *> tensors;
    std::vector<std::string> outputs,weights;
    auto named=[&](const char *name,ggml_tensor *value) {
        ggml_set_name(value,name);tensors[name]=value;return value;
    };
    auto weight=[&](const char *name,int k,int m) {
        weights.emplace_back(name);return named(name,ggml_new_tensor_2d(ctx.get(),half?GGML_TYPE_F16:GGML_TYPE_F32,k,m));
    };
    auto result=[&](const char *name,ggml_tensor *value) {
        outputs.emplace_back(name);return named(name,value);
    };
    auto *x=named("X",ggml_new_tensor_2d(ctx.get(),GGML_TYPE_F32,d,t));
    auto *wq=weight("Wq",d,d),*wk=weight("Wk",d,d),*wv=weight("Wv",d,d),*wo=weight("Wo",d,d);
    auto *wu=weight("Wup",d,h),*wd=weight("Wdown",h,d);
    auto *n=result("N",ggml_rms_norm(ctx.get(),x,1e-5f));
    auto *q=result("Q",ggml_mul_mat(ctx.get(),wq,n));
    auto *k=result("K",ggml_mul_mat(ctx.get(),wk,n));
    auto *v=result("V",ggml_mul_mat(ctx.get(),wv,n));
    auto *scores=result("scores",ggml_mul_mat(ctx.get(),k,q));
    const float scale=1.0f/std::sqrt(float(d));
    auto *p=result("P",ggml_soft_max_ext(ctx.get(),scores,nullptr,scale,0));
    auto *view=named("V_view",ggml_transpose(ctx.get(),v));
    auto *vt=result("Vt",ggml_cont(ctx.get(),view));
    auto *a=result("A",ggml_mul_mat(ctx.get(),vt,p));
    auto *o=result("O",ggml_mul_mat(ctx.get(),wo,a));
    auto *r=result("R",ggml_add(ctx.get(),x,o));
    auto *nr=result("NR",ggml_rms_norm(ctx.get(),r,1e-5f));
    auto *up=result("up",ggml_mul_mat(ctx.get(),wu,nr));
    auto *relu=result("relu",ggml_relu(ctx.get(),up));
    auto *down=result("down",ggml_mul_mat(ctx.get(),wd,relu));
    auto *y=result("Y",ggml_add(ctx.get(),r,down));
    auto *graph=ggml_new_graph(ctx.get());ggml_build_forward_expand(graph,y);
    require(outputs.size()==15,"unexpected block definition");
    Buffer buffer(ggml_backend_alloc_ctx_tensors(ctx.get(),backend.get()),ggml_backend_buffer_free);
    require(bool(buffer),"transformer allocation failed");
    for (int i=0;i<ggml_graph_n_nodes(graph);++i)
        require(ggml_backend_supports_op(backend.get(),ggml_graph_node(graph,i)),"block operation not advertised");

    std::map<std::string,Values> reference;
    std::map<std::string,std::vector<unsigned char>> saved_weights;
    int salt=0;
    for (const auto &name:weights) {
        auto *tensor=tensors.at(name);size_t count=ggml_nelements(tensor);
        std::vector<float> f32(count);std::vector<ggml_fp16_t> f16(count);Values values(count);
        for (size_t i=0;i<count;++i) {
            f32[i]=float(int((i*17+size_t(++salt))%101)-50)*0.0031f;
            f16[i]=ggml_fp32_to_fp16(f32[i]);values[i]=half?ggml_fp16_to_fp32(f16[i]):f32[i];
        }
        const void *data=half?static_cast<void *>(f16.data()):static_cast<void *>(f32.data());
        auto &saved=saved_weights[name];saved.resize(ggml_nbytes(tensor));std::memcpy(saved.data(),data,saved.size());
        ggml_backend_tensor_set(tensor,data,0,saved.size());reference[name]=std::move(values);
    }
    std::map<std::string,std::vector<float>> first,actual;
    std::map<std::string,double> errors;
    uint64_t first_staging=0;
    for (int phase=0;phase<3;++phase) {
        std::vector<float> input(d*t);Values input64(input.size());
        for (size_t i=0;i<input.size();++i) {
            input[i]=d==1?0.0f:float(int((i*13+(phase==1?23:7))%71)-35)*0.021f;
            input64[i]=input[i];
        }
        ggml_backend_tensor_set(x,input.data(),0,ggml_nbytes(x));
        for (const auto &name:outputs) {
            auto *tensor=tensors.at(name);
            std::vector<float> poison(ggml_nelements(tensor),std::numeric_limits<float>::quiet_NaN());
            ggml_backend_tensor_set(tensor,poison.data(),0,ggml_nbytes(tensor));
        }
        reference["N"]=norm(input64,d,t);
        for (const char *name:{"Q","K","V"}) {
            std::string weight_name="W";weight_name+=char(name[0]-'A'+'a');
            reference[name]=multiply(reference.at(weight_name),reference.at("N"),d,d,t);
        }
        reference["scores"]=multiply(reference.at("K"),reference.at("Q"),d,t,t);
        reference["P"]=probabilities(reference.at("scores"),t,scale);
        reference["Vt"]=transpose(reference.at("V"),d,t);
        reference["A"]=multiply(reference.at("Vt"),reference.at("P"),t,d,t);
        reference["O"]=multiply(reference.at("Wo"),reference.at("A"),d,d,t);
        reference["R"]=add(input64,reference.at("O"));
        reference["NR"]=norm(reference.at("R"),d,t);
        reference["up"]=multiply(reference.at("Wup"),reference.at("NR"),d,h,t);
        reference["relu"]=reference.at("up");for (auto &value:reference["relu"]) value=std::max(value,0.0);
        reference["down"]=multiply(reference.at("Wdown"),reference.at("relu"),h,d,t);
        reference["Y"]=add(reference.at("R"),reference.at("down"));
        auto count=session.dispatch_count();auto before=session.stats();
        require(ggml_backend_graph_compute(backend.get(),graph)==GGML_STATUS_SUCCESS,"transformer graph failed");
        auto after=session.stats();
        require(session.dispatch_count()==count+15,"block did not execute exactly 15 GPU dispatches");
        require(after.uploads==before.uploads && after.downloads==before.downloads &&
                after.staging_allocations==before.staging_allocations,"transfer/allocation inside graph");
        for (const auto &name:outputs) {
            auto *tensor=tensors.at(name);auto &values=actual[name];values.resize(ggml_nelements(tensor));
            ggml_backend_tensor_get(tensor,values.data(),0,ggml_nbytes(tensor));
            const auto &expected=reference.at(name);
            for (size_t i=0;i<values.size();++i) {
                double error=std::abs(double(values[i])-expected[i]);
                require(std::isfinite(values[i]) && error<=2e-4+2e-4*std::abs(expected[i]),"intermediate numerical mismatch");
                errors[name]=std::max(errors[name],error);
            }
            if (phase==0) first[name]=values;
            if (phase==2) require(std::memcmp(values.data(),first.at(name).data(),values.size()*sizeof(float))==0,"A/B/A mismatch");
        }
        for (int row=0;row<t;++row) {
            double sum=0;for (int col=0;col<t;++col) { float value=actual.at("P")[row*t+col];require(value>=0,"negative probability");sum+=value; }
            require(std::abs(sum-1)<=2e-5,"probability row does not sum to one");
        }
        for (int row=0;row<t;++row) for (int col=0;col<d;++col)
            require(std::memcmp(&actual.at("V")[row*d+col],&actual.at("Vt")[col*t+row],sizeof(float))==0,"transpose not exact");
        std::vector<float> read_input(input.size());ggml_backend_tensor_get(x,read_input.data(),0,ggml_nbytes(x));
        require(read_input==input,"input was overwritten");
        for (const auto &name:weights) {
            const auto &saved=saved_weights.at(name);std::vector<unsigned char> bytes(saved.size());
            ggml_backend_tensor_get(tensors.at(name),bytes.data(),0,bytes.size());require(bytes==saved,"weight bytes changed");
        }
        if (phase==0) first_staging=session.stats().staging_allocations;
        else require(session.stats().staging_allocations==first_staging,"steady-state staging grew");
    }
    // Complete preflight must reject even a late malformed node before any write.
    auto reject=[&](ggml_tensor *tensor,const ggml_tensor &saved,bool metadata=true) {
        const auto count=session.dispatch_count();
        if (metadata) require(!ggml_backend_supports_op(backend.get(),tensor),"invalid metadata advertised");
        require(ggml_backend_graph_compute(backend.get(),graph)==GGML_STATUS_FAILED && session.dispatch_count()==count,
                "invalid graph partially executed");
        *tensor=saved;
        for (const auto &name:outputs) {
            std::vector<float> unchanged(actual.at(name).size());
            ggml_backend_tensor_get(tensors.at(name),unchanged.data(),0,ggml_nbytes(tensors.at(name)));
            require(unchanged==actual.at(name),"rejected graph changed an intermediate");
        }
    };
    const auto saved_n=*n,saved_p=*p,saved_view=*view,saved_vt=*vt,saved_y=*y;
    float bad=-1;std::memcpy(n->op_params,&bad,sizeof(bad));reject(n,saved_n);
    bad=std::numeric_limits<float>::quiet_NaN();std::memcpy(n->op_params,&bad,sizeof(bad));reject(n,saved_n);
    std::memcpy(p->op_params,&bad,sizeof(bad));reject(p,saved_p);
    p->src[1]=x;reject(p,saved_p);p->src[2]=x;reject(p,saved_p);
    bad=1;std::memcpy(reinterpret_cast<char *>(p->op_params)+4,&bad,sizeof(bad));reject(p,saved_p);
    view->nb[0]+=4;reject(view,saved_view);
    view->data=static_cast<char *>(view->data)+4;reject(view,saved_view,false);
    vt->src[0]=v;reject(vt,saved_vt);
    n->data=x->data;reject(n,saved_n,false);
    y->type=GGML_TYPE_F16;reject(y,saved_y);
    require(ggml_backend_graph_compute(backend.get(),graph)==GGML_STATUS_SUCCESS,"valid retry failed");
    std::vector<float> retry(actual.at("Y").size());ggml_backend_tensor_get(y,retry.data(),0,ggml_nbytes(y));
    require(retry==actual.at("Y"),"retry changed output");
    for (const auto &[name,error]:errors)
        std::printf("transformer_error d=%d h=%d t=%d precision=%s tensor=%s max_abs=%.9g\n",d,h,t,half?"f16":"f32",name.c_str(),error);
    std::printf("transformer d=%d h=%d t=%d precision=%s phases=3 dispatches=45 rejected=11 retry=15 allocation_bytes=%zu PASS\n",
        d,h,t,half?"f16":"f32",ggml_backend_buffer_get_size(buffer.get()));
    return actual.at("Y");
}

int main(int argc,char **argv) {
    if (argc!=4) return 2;
    try {
        const std::string placement=argv[3];require(placement=="host"||placement=="device","bad placement");
        size_t end=0;auto index=std::stoul(argv[2],&end);require(end==std::strlen(argv[2])&&index<=UINT32_MAX,"bad device index");
        OgpuGgmlSession session(uint32_t(index),argv[1],placement=="host"?OgpuGgmlMemory::Host:OgpuGgmlMemory::Device,OgpuGgmlProfile::Transformer);
        const int shapes[][3]={{1,1,1},{13,29,3},{13,29,17},{64,128,16},{65,129,17}};
        for (const auto &shape:shapes) {
            const auto f32=check(session,shape[0],shape[1],shape[2],false);
            const auto f16=check(session,shape[0],shape[1],shape[2],true);
            double drift=0;for (size_t i=0;i<f32.size();++i) drift=std::max(drift,std::abs(double(f32[i])-f16[i]));
            std::printf("transformer_weight_drift d=%d h=%d t=%d max_abs=%.9g\n",shape[0],shape[1],shape[2],drift);
        }
        std::printf("Transformer profile placement=%s cases=10 PASS; no timing or matrix-acceleration claim\n",placement.c_str());
        return 0;
    } catch (const std::exception &e) { std::fprintf(stderr,"transformer check failed: %s\n",e.what());return 1; }
}
