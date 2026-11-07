// Explicit 16x16x16 half-input/float-accumulator linear layer and separate
// ReLU/half conversion. No scalar replacement for either matrix operation.
using namespace linalg;
struct Root { half* a; half* b; float* bias; float* result; half* hidden; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[[vk::constant_id(0)]] const uint groupSize = 32;
typealias A = CoopMat<half, MemoryScope.Subgroup, 16, 16, CoopMatMatrixUse.MatrixA>;
typealias B = CoopMat<half, MemoryScope.Subgroup, 16, 16, CoopMatMatrixUse.MatrixB>;
typealias C = CoopMat<float, MemoryScope.Subgroup, 16, 16, CoopMatMatrixUse.MatrixAccumulator>;
[shader("compute")]
[numthreads(groupSize, 1, 1)]
void linearMain() {
    let a = A.Load<CoopMatMatrixLayout.RowMajor>(control.root->a, 0, 16);
    let b = B.Load<CoopMatMatrixLayout.RowMajor>(control.root->b, 0, 16);
    let bias = C.Load<CoopMatMatrixLayout.RowMajor>(control.root->bias, 0, 16);
    let result = coopMatMulAdd<float, false>(a, b, bias);
    result.Store<CoopMatMatrixLayout.RowMajor>(control.root->result, 0, 16);
}
[shader("compute")]
[numthreads(64, 1, 1)]
void activationMain(uint3 id : SV_DispatchThreadID) {
    control.root->hidden[id.x] = half(max(control.root->result[id.x], 0.0f));
}
