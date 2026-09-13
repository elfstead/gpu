//! Dispatch contract tests through both C entry points. Resource setup uses internals
//! so the checks focus on forwarding all axes, not a second discovery/ownership test.
use super::*;
use crate::{vulkan::Instance, UNSUPPORTED};
use std::sync::Arc;

fn encode(v: [u32; 3]) -> u32 {
    1 + v[0] + 100 * v[1] + 10000 * v[2]
}

#[test]
#[ignore = "requires Vulkan; multidimensional dispatch through both C entry points"]
fn gpu_dispatch_grids() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/dispatch-grid.comp.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut kernel = OgpuKernel {
            inner: Rc::new(unsafe { Kernel::new(device.clone(), &words, 24, &[]).unwrap() }),
        };
        // Pure X/Y/Z, 2D and asymmetric 3D; every case has partial workgroups.
        for extent in [[9u32, 1, 1], [1, 5, 1], [1, 1, 7], [9, 5, 1], [17, 5, 3]] {
            let groups = [
                extent[0].div_ceil(4),
                extent[1].div_ceil(2),
                extent[2].div_ceil(2),
            ];
            let len = 4 * extent.into_iter().product::<u32>() as usize + 8;
            let buffer = Buffer::new(device.clone(), len * 4).unwrap();
            let sentinel = 42u32;
            let input: Vec<u8> = (0..len).flat_map(|_| sentinel.to_ne_bytes()).collect();
            buffer.write(0, &input).unwrap();
            let mut root = [0u8; 24];
            root[..8].copy_from_slice(&(buffer.address().unwrap() + 16).to_ne_bytes());
            for (axis, value) in extent.into_iter().enumerate() {
                root[8 + 4 * axis..12 + 4 * axis].copy_from_slice(&value.to_ne_bytes());
            }
            let mut batch = OgpuBatch {
                inner: Batch::new(device.clone()).unwrap(),
            };
            let mut error = OgpuError {
                vulkan_result: 0,
                message: [0; 256],
            };
            unsafe {
                // Both exported entry points must reject zero on each axis.
                // Actual per-axis upper bounds are covered by gpu_batches.
                for axis in 0..3 {
                    let mut bad = groups;
                    bad[axis] = 0;
                    assert_eq!(
                        ogpu_dispatch_wait(
                            &mut kernel,
                            bad[0],
                            bad[1],
                            bad[2],
                            root.as_ptr().cast(),
                            24,
                            &mut error
                        ),
                        INVALID_ARGUMENT
                    );
                    assert_eq!(
                        ogpu_batch_dispatch(
                            &mut batch,
                            &mut kernel,
                            bad[0],
                            bad[1],
                            bad[2],
                            root.as_ptr().cast(),
                            24,
                            &mut error
                        ),
                        INVALID_ARGUMENT
                    );
                }
                assert_eq!(
                    ogpu_dispatch_wait(
                        &mut kernel,
                        groups[0],
                        groups[1],
                        groups[2],
                        root.as_ptr().cast(),
                        24,
                        &mut error
                    ),
                    SUCCESS
                );
                // Depend explicitly on the convenience call's read/modify/write.
                assert_eq!(ogpu_batch_barrier(&mut batch, 2, 3, &mut error), SUCCESS);
                assert_eq!(
                    ogpu_batch_dispatch(
                        &mut batch,
                        &mut kernel,
                        groups[0],
                        groups[1],
                        groups[2],
                        root.as_ptr().cast(),
                        24,
                        &mut error
                    ),
                    SUCCESS
                );
                root.fill(0); // Recording owns its argument copy, including all extents.
                let mut completion = batch.inner.submit().unwrap();
                completion.wait().unwrap();
            }
            let mut bytes = vec![0u8; len * 4];
            unsafe {
                buffer.read(0, bytes.as_mut_ptr(), bytes.len()).unwrap();
            }
            let output: Vec<u32> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(&output[..4], &[sentinel; 4]);
            assert_eq!(&output[len - 4..], &[sentinel; 4]);
            for z in 0..extent[2] {
                for y in 0..extent[1] {
                    for x in 0..extent[0] {
                        let i = 4 + 4 * ((z * extent[1] + y) * extent[0] + x) as usize;
                        let expected = [
                            [x, y, z],
                            [x / 4, y / 2, z / 2],
                            [x % 4, y % 2, z % 2],
                            groups,
                        ]
                        .map(|v| sentinel + 2 * encode(v));
                        assert_eq!(
                            output[i..i + 4],
                            expected,
                            "extent={extent:?}, position={x},{y},{z}"
                        );
                    }
                }
            }
        }
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
}
