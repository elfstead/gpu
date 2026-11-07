#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/matrix_ml.rs"]
mod consumer;
#[test]
#[ignore = "requires native 16x16x16 FP16/FP32 cooperative matrices; unsupported remains unaccepted"]
fn gpu_matrix_ml() {
    match consumer::run() {
        Ok(()) => (),
        Err(ogpu::foundation::types::UNSUPPORTED) => {
            println!("MATRIX_ML_EXECUTION_NOT_ACCEPTED: required native profile unavailable")
        }
        Err(e) => panic!("matrix consumer failed: {e}"),
    }
}
