struct Result { uint hit; float distance; uint instance; uint primitive; };
struct Control { uint2 scene; Result* results; float plane; uint procedural; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct LocalRecord { float bias; };
[[vk::shader_record]] ConstantBuffer<LocalRecord> localRecord;
RaytracingAccelerationStructure sceneFromBits(uint2 bits) {
    return spirv_asm {
        OpExtension "SPV_KHR_ray_tracing";
        result: $$RaytracingAccelerationStructure = OpConvertUToAccelerationStructureKHR $bits;
    };
}
[shader("callable")]
void initialize(inout Result r) { r.hit=0; r.distance=-1.0; r.instance=0xffffffff; r.primitive=0xffffffff; }
[shader("raygeneration")]
void rayMain() {
    uint i=DispatchRaysIndex().x;
    Result result;
    CallShader(0,result);
    RayDesc ray;
    ray.Origin=float3(i==0 ? 0.25 : i==1 ? 4.0 : i==3 ? 2.25 : 0.1,0.25,-1.0);
    ray.Direction=float3(0,0,1); ray.TMin=0; ray.TMax=i==2 ? 0.5 : 10.0;
    TraceRay(sceneFromBits(control.scene),control.procedural != 0 ? RAY_FLAG_NONE : RAY_FLAG_FORCE_OPAQUE,255,0,1,0,ray,result);
    control.results[i]=result;
}
[shader("miss")]
void missMain(inout Result result) { result.hit=0; }
[shader("closesthit")]
void hitMain(inout Result result,BuiltInTriangleIntersectionAttributes attributes) {
    bool anyHitSeen=result.hit==17;
    result.hit=1;result.distance=RayTCurrent()+localRecord.bias;
    result.instance=InstanceID();result.primitive=PrimitiveIndex() | (GeometryIndex()<<16) | (anyHitSeen ? 0x80000000 : 0);
}

[shader("intersection")]
void intersectMain() {
    float t = (control.plane - ObjectRayOrigin().z) / ObjectRayDirection().z;
    BuiltInTriangleIntersectionAttributes attributes; attributes.barycentrics=float2(0,0);
    ReportHit(t,127,attributes);
}
[shader("anyhit")]
void anyMain(inout Result result,BuiltInTriangleIntersectionAttributes attributes) {
    result.hit=17;
}
