// Slang 2026.14.1's uint64 constructor unnecessarily declares SPV_KHR_ray_tracing.
// The query extension itself provides this conversion. Keep this source-level
// intrinsic in the consumer; OGPU does not rewrite artifacts or enable pipelines.
RaytracingAccelerationStructure queryScene(uint2 bits) {
    return spirv_asm {
        OpExtension "SPV_KHR_ray_query";
        result: $$RaytracingAccelerationStructure = OpConvertUToAccelerationStructureKHR $bits;
    };
}
// Native AS address is passed through ordinary argument bytes, no descriptor slot.
struct Result { uint hit; float distance; uint instance; uint primitive; };
struct Control { uint2 scene; Result* results; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("compute")]
[numthreads(4,1,1)]
void queryMain(uint3 id : SV_DispatchThreadID) {
    RayDesc ray;
    ray.Origin = float3(id.x == 0 ? 0.25 : id.x == 1 ? 4.0 : id.x == 3 ? 2.25 : 0.1, 0.25, -1.0);
    ray.Direction = float3(0, 0, 1);
    ray.TMin = 0.0;
    ray.TMax = id.x == 2 ? 0.5 : 10.0;
    RayQuery<RAY_FLAG_FORCE_OPAQUE> query;
    query.TraceRayInline(queryScene(control.scene), 0, 255, ray);
    while(query.Proceed()) {}
    Result r = {0, -1.0, 0xffffffff, 0xffffffff};
    if (query.CommittedStatus() == COMMITTED_TRIANGLE_HIT) {
        r.hit = 1;
        r.distance = query.CommittedRayT();
        r.instance = query.CommittedInstanceID();
        r.primitive = query.CommittedPrimitiveIndex() | (query.CommittedGeometryIndex()<<16);
    }
    control.results[id.x] = r;
}
