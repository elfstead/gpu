// Same geometry/index generation as the accepted UINT32 scene. No draw-ID use.
// control = phase, requested active count, capacity, useCount. Count is produced on GPU.
struct Root { float* vertices; uint* indices; uint* draws; uint* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
[shader("compute")]
[numthreads(8, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint id=thread.x, phase=root.control[0], active=root.control[1], capacity=root.control[2];
    uint corner=id%4;
    float x=(corner==0 || corner==3) ? -1.0 : 1.0, y=corner<2 ? -1.0 : 1.0;
    bool near=id>=4;
    root.vertices[4*id]=near ? x*0.25+(phase==0 ? -0.125 : 0.125) : x*0.75;
    root.vertices[4*id+1]=y*(near ? 0.375 : 0.75);
    root.vertices[4*id+2]=near ? 0.25 : 0.75;
    root.vertices[4*id+3]=1.0;
    root.indices[id]=id==2 || id==5 ? 1 : id==3 ? 2 : id==4 || id==6 ? 3 : id==7 ? 4 : 0xffffffffu;
    for(uint record=id;record<capacity;record+=8) {
        // Counted control keeps ALL records nonempty: zero/changing output must
        // come from the GPU count, not from masking individual draw records.
        root.draws[5*record]=(root.control[3]!=0 || record<active) ? 6 : 0;
        root.draws[5*record+1]=1;
        root.draws[5*record+2]=2;
        // Put the near surface last: full vs partial counts differ visibly even
        // at capacity 512. Capacity one intentionally contains only the far quad.
        root.draws[5*record+3]=(capacity>1 && record==capacity-1) ? 3 : 0xffffffffu;
        root.draws[5*record+4]=0;
    }
    if(id==0) root.draws[5*capacity]=active;
}
