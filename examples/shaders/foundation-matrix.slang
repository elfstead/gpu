using namespace linalg;
struct Root { half* a; half* b; float* c; float* result; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[[vk::constant_id(0)]] const uint groupSize = 32;
typealias A = CoopMat<half, MemoryScope.Subgroup, 8, 8, CoopMatMatrixUse.MatrixA>;
typealias B = CoopMat<half, MemoryScope.Subgroup, 8, 8, CoopMatMatrixUse.MatrixB>;
typealias C = CoopMat<float, MemoryScope.Subgroup, 8, 8, CoopMatMatrixUse.MatrixAccumulator>;

[shader("compute")]
[numthreads(groupSize, 1, 1)]
void matrixMain(uint3 group : SV_GroupID) {
    let a = A.Load<CoopMatMatrixLayout.RowMajor>(control.root->a, 0, 8);
    let b = B.Load<CoopMatMatrixLayout.ColumnMajor>(control.root->b, 0, 8);
    let c = C.Load<CoopMatMatrixLayout.RowMajor>(control.root->c, 0, 8);
    let result = coopMatMulAdd<float, false>(a, b, c);
    result.Store<CoopMatMatrixLayout.RowMajor>(control.root->result, group.x*64, 8);
}
