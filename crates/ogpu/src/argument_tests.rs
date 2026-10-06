//! Public byte-range contract, without executing any uninitialized shader read.
use super::*;
use crate::{vulkan::Instance, UNSUPPORTED};
use std::sync::Arc;

#[test]
fn argument_entry_points_reject_null_handles() {
    unsafe {
        assert_eq!(
            ogpu_batch_set_arguments(ptr::null_mut(), 0, ptr::null(), 0, ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_batch_dispatch_current(ptr::null_mut(), ptr::null_mut(), 1, 1, 1, ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_batch_draw_indirect_current(
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut()
            ),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_batch_draw_indexed_indirect_current(
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null_mut()
            ),
            INVALID_ARGUMENT
        );
    }
}

#[test]
#[ignore = "requires Vulkan; sparse/overlapping argument updates, atomic failure, compatible prefixes and owned replay"]
fn gpu_recording_arguments() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
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
        // Same shader reads only bytes 0..12, independently of declared padding.
        let mut small = OgpuKernel {
            inner: Rc::new(unsafe { Kernel::new(device.clone(), &words, 12, &[]).unwrap() }),
        };
        let mut padded = OgpuKernel {
            inner: Rc::new(unsafe { Kernel::new(device.clone(), &words, 16, &[]).unwrap() }),
        };
        let buffer = Rc::new(Buffer::new(device.clone(), 16).unwrap());
        let storage = Rc::new(RecordingStorage::new(device.clone()).unwrap());
        for mode in 0..3 {
            let input: Vec<_> = [1u32; 4].into_iter().flat_map(u32::to_ne_bytes).collect();
            buffer.write(0, &input).unwrap();
            let mut batch = OgpuBatch {
                inner: Batch::new_in(storage.clone()).unwrap(),
            };
            batch.inner.retain_buffer(buffer.clone()).unwrap();
            // Orders replayed read/modify/write operations on the same queue.
            batch.inner.barrier(3, 3).unwrap();
            let mut root = [0u8; 16];
            root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
            root[8..12].copy_from_slice(&1u32.to_ne_bytes());
            unsafe {
                // Sparse initialization: byte 12..16 stays undefined and unread.
                assert_eq!(
                    ogpu_batch_set_arguments(
                        &mut batch,
                        0,
                        root.as_ptr().cast(),
                        8,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_set_arguments(
                        &mut batch,
                        8,
                        root[8..].as_ptr().cast(),
                        4,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                // Unused bank bytes need not match either executable's extent.
                assert_eq!(
                    ogpu_batch_set_arguments(
                        &mut batch,
                        20,
                        root.as_ptr().cast(),
                        4,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                let limit = device.execution_limits().max_push_data_bytes as u32;
                for (offset, bytes, status) in [
                    (1, 4, INVALID_ARGUMENT),
                    (0, 3, INVALID_ARGUMENT),
                    (3, 0, INVALID_ARGUMENT),
                    (limit, 4, OUT_OF_RANGE),
                    (u32::MAX - 3, 8, OUT_OF_RANGE),
                ] {
                    assert_eq!(
                        ogpu_batch_set_arguments(
                            &mut batch,
                            offset,
                            root.as_ptr().cast(),
                            bytes,
                            ptr::null_mut()
                        ),
                        status
                    );
                }
                assert_eq!(
                    ogpu_batch_set_arguments(&mut batch, 0, ptr::null(), 4, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_set_arguments(&mut batch, limit, ptr::null(), 0, ptr::null_mut()),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_dispatch_current(&mut batch, &mut small, 1, 1, 1, ptr::null_mut()),
                    SUCCESS
                );
                batch.inner.barrier(3, 3).unwrap();
                // Fully sized but invalid operation must not poison current count.
                root[8..12].copy_from_slice(&4u32.to_ne_bytes());
                assert_eq!(
                    ogpu_batch_dispatch(
                        &mut batch,
                        &mut padded,
                        0,
                        1,
                        1,
                        root.as_ptr().cast(),
                        16,
                        ptr::null_mut()
                    ),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_dispatch(
                        &mut batch,
                        &mut padded,
                        1,
                        1,
                        1,
                        root.as_ptr().cast(),
                        12,
                        ptr::null_mut()
                    ),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_dispatch_current(&mut batch, &mut small, 1, 1, 1, ptr::null_mut()),
                    SUCCESS
                );
                batch.inner.barrier(3, 3).unwrap();
                // Partial overlapping write preserves pointer. Same caller address,
                // different bytes; copied before immediate overwrite.
                root[8..12].copy_from_slice(&2u32.to_ne_bytes());
                assert_eq!(
                    ogpu_batch_set_arguments(
                        &mut batch,
                        8,
                        root[8..].as_ptr().cast(),
                        4,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                root[8..12].fill(0xff);
                assert_eq!(
                    ogpu_batch_dispatch_current(&mut batch, &mut padded, 1, 1, 1, ptr::null_mut()),
                    SUCCESS
                );
                batch.inner.barrier(3, 3).unwrap();
                root[8..12].copy_from_slice(&3u32.to_ne_bytes());
                assert_eq!(
                    ogpu_batch_dispatch(
                        &mut batch,
                        &mut padded,
                        1,
                        1,
                        1,
                        root.as_ptr().cast(),
                        16,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                root.fill(0);
                batch.inner.barrier(3, 3).unwrap();
                assert_eq!(
                    ogpu_batch_dispatch_current(&mut batch, &mut small, 1, 1, 1, ptr::null_mut()),
                    SUCCESS
                );
            }
            let runs = if mode == 0 { 1 } else { 2 };
            if mode == 0 {
                let mut completion = unsafe { batch.inner.submit().unwrap() };
                drop(batch);
                completion.wait().unwrap();
            } else {
                let list = Rc::new(unsafe { batch.inner.compile_with_flags(mode - 1).unwrap() });
                assert!(Batch::new_in(storage.clone()).is_err());
                drop(batch);
                let mut first = unsafe { list.submit().unwrap() };
                if mode == 1 {
                    first.wait().unwrap();
                }
                let mut second = unsafe { list.submit().unwrap() };
                drop(list); // Receipts retain both immutable commands and their bytes.
                second.wait().unwrap();
                first.wait().unwrap();
            }
            let mut expected = [1u32; 4];
            for _ in 0..runs {
                for count in [1, 1, 2, 3, 3] {
                    for v in &mut expected[..count] {
                        *v = *v * 3 + 7;
                    }
                }
            }
            let mut actual = [0u8; 16];
            unsafe {
                buffer.read(0, actual.as_mut_ptr(), actual.len()).unwrap();
            }
            assert_eq!(
                actual.as_slice(),
                expected
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect::<Vec<_>>()
            );
            // Retired owner can be reused, but argument state is not inherited.
            let mut fresh = OgpuBatch {
                inner: Batch::new_in(storage.clone()).unwrap(),
            };
            let mut root = [0u8; 12];
            root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
            // Initialize count=0 explicitly, never execute an undefined read.
            unsafe {
                assert_eq!(
                    ogpu_batch_set_arguments(
                        &mut fresh,
                        0,
                        root.as_ptr().cast(),
                        12,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_dispatch_current(&mut fresh, &mut small, 1, 1, 1, ptr::null_mut()),
                    SUCCESS
                );
                let mut done = fresh.inner.submit().unwrap();
                assert_eq!(
                    ogpu_batch_set_arguments(&mut fresh, 0, ptr::null(), 0, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                done.wait().unwrap();
                buffer.read(0, actual.as_mut_ptr(), actual.len()).unwrap();
            }
            assert_eq!(
                actual.as_slice(),
                expected
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect::<Vec<_>>()
            );
        }
        tested += 1;
    }
    assert!(tested > 0);
}

#[test]
#[ignore = "requires graphics Vulkan; shared compute/graphics bank, scope persistence and failed/empty draw conveniences"]
fn gpu_shared_graphics_arguments() {
    use super::rendering_tests::{buffer, image, words};
    let instance = Arc::new(Instance::new().unwrap());
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let compute = words(include_bytes!("../../../examples/shaders/roundtrip.spv"));
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut device = OgpuDevice { inner: d.clone() };
        unsafe {
            let mut color = image(&mut device, 0, 4 | 8);
            let vertices = buffer(&mut device, 96, 0);
            let mut records = buffer(&mut device, 16, 0);
            let output = buffer(&mut device, 6 * 4096, 0);
            // A covers the center; B is entirely outside the viewport. Both
            // addresses remain valid even if argument-state assertions fail.
            let data: Vec<_> = [
                -1f32, -1., 0., 1., 1., -1., 0., 1., 0., 1., 0., 1., 3., 3., 0., 1., 4., 3., 0.,
                1., 3., 4., 0., 1.,
            ]
            .into_iter()
            .flat_map(f32::to_ne_bytes)
            .collect();
            vertices.inner.write(0, &data).unwrap();
            records
                .inner
                .write(
                    0,
                    &[3u32, 1, 0, 0]
                        .into_iter()
                        .flat_map(u32::to_ne_bytes)
                        .collect::<Vec<_>>(),
                )
                .unwrap();
            let a = vertices.inner.address().unwrap().to_ne_bytes();
            let b = (vertices.inner.address().unwrap() + 48).to_ne_bytes();
            let mut raster = OgpuRaster {
                inner: Rc::new(
                    Raster::new(
                        d.clone(),
                        &vertex,
                        &fragment,
                        OgpuRasterDesc {
                            push_size_bytes: 8,
                            topology: 0,
                            color_format: 0,
                            depth_format: u32::MAX,
                            depth_test: 0,
                            depth_write: 0,
                            depth_compare: 7,
                            reserved: 0,
                        },
                        [&[], &[]],
                    )
                    .unwrap(),
                ),
            };
            let mut kernel = OgpuKernel {
                inner: Rc::new(Kernel::new(d.clone(), &compute, 16, &[]).unwrap()),
            };
            let scope = OgpuRenderingDesc {
                color: crate::OgpuColorAttachment {
                    image: &mut *color,
                    clear: [0., 0., 0., 1.],
                    ..Default::default()
                },
                ..Default::default()
            };
            let draws = OgpuIndirectRange {
                buffer: &mut *records,
                offset: 0,
                stride_bytes: 16,
                max_draw_count: 1,
                count_buffer: ptr::null_mut(),
                count_offset: 0,
            };
            let mut batch = OgpuBatch {
                inner: Batch::new(d.clone()).unwrap(),
            };
            batch.inner.discard_image(color.inner.clone()).unwrap();
            assert_eq!(
                ogpu_batch_set_arguments(&mut batch, 0, a.as_ptr().cast(), 8, ptr::null_mut()),
                SUCCESS
            );
            // Explicitly initialize compute count, but not the unused padding.
            let zero = 0u32;
            assert_eq!(
                ogpu_batch_set_arguments(
                    &mut batch,
                    8,
                    (&zero as *const u32).cast(),
                    4,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            for pass in 0..6 {
                if pass == 1 {
                    let mut root = [0u8; 16];
                    root[..8].copy_from_slice(&b);
                    // Successful compute convenience overwrites graphics' pointer.
                    assert_eq!(
                        ogpu_batch_dispatch(
                            &mut batch,
                            &mut kernel,
                            1,
                            1,
                            1,
                            root.as_ptr().cast(),
                            16,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                    root.fill(0xff);
                }
                if pass == 2 {
                    // Two overlapping pointer writes outside a scope. Only the
                    // final initialized pointer is consumed; no undefined reads.
                    assert_eq!(
                        ogpu_batch_set_arguments(
                            &mut batch,
                            0,
                            b.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                    assert_eq!(
                        ogpu_batch_set_arguments(
                            &mut batch,
                            0,
                            a.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                }
                batch.inner.barrier(3, 3).unwrap();
                assert_eq!(
                    ogpu_batch_begin_rendering(&mut batch, &scope, ptr::null_mut()),
                    SUCCESS
                );
                if pass == 2 {
                    let bad = OgpuIndirectRange {
                        stride_bytes: 0,
                        ..draws
                    };
                    // Exact-size root with an invalid operation must not install B.
                    assert_eq!(
                        ogpu_batch_draw_indirect(
                            &mut batch,
                            &mut raster,
                            &bad,
                            b.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        INVALID_ARGUMENT
                    );
                }
                if pass == 3 {
                    // Empty CURRENT draw does not change A.
                    let empty = OgpuIndirectRange {
                        max_draw_count: 0,
                        ..draws
                    };
                    assert_eq!(
                        ogpu_batch_draw_indirect_current(
                            &mut batch,
                            &mut raster,
                            &empty,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                }
                if pass == 4 {
                    // Empty convenience is still successful update + empty draw.
                    let empty = OgpuIndirectRange {
                        max_draw_count: 0,
                        ..draws
                    };
                    assert_eq!(
                        ogpu_batch_draw_indirect(
                            &mut batch,
                            &mut raster,
                            &empty,
                            b.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                }
                if pass == 5 {
                    assert_eq!(
                        ogpu_batch_set_arguments(
                            &mut batch,
                            0,
                            a.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                }
                assert_eq!(
                    ogpu_batch_set_arguments(&mut batch, 0, ptr::null(), 0, ptr::null_mut()),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_draw_indirect_current(
                        &mut batch,
                        &mut raster,
                        &draws,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                    SUCCESS
                );
                batch
                    .inner
                    .copy_image_to_buffer(color.inner.clone(), output.inner.clone(), pass * 4096)
                    .unwrap();
                // Graphics only changed 0..8; count=0 survives, so dispatch
                // consumes the same bank without touching vertex pointees.
                assert_eq!(
                    ogpu_batch_dispatch_current(&mut batch, &mut kernel, 1, 1, 1, ptr::null_mut()),
                    SUCCESS
                );
            }
            let list = Rc::new(batch.inner.compile_with_flags(0).unwrap());
            drop(batch);
            for _ in 0..2 {
                let mut done = list.submit().unwrap();
                done.wait().unwrap();
                let mut actual = vec![0u8; 6 * 4096];
                output
                    .inner
                    .read(0, actual.as_mut_ptr(), actual.len())
                    .unwrap();
                for pass in 0..6 {
                    let center = pass * 4096 + 4 * (16 * 32 + 16);
                    assert_eq!(
                        &actual[center..center + 4],
                        if pass == 1 || pass == 4 {
                            &[0, 0, 0, 255]
                        } else {
                            &[255, 0, 0, 255]
                        },
                        "pass={pass}"
                    );
                }
                assert_eq!(&actual[..4096], &actual[2 * 4096..3 * 4096]);
                assert_eq!(&actual[..4096], &actual[3 * 4096..4 * 4096]);
                assert_eq!(&actual[..4096], &actual[5 * 4096..]);
                assert!(actual[4096..2 * 4096]
                    .chunks_exact(4)
                    .chain(actual[4 * 4096..5 * 4096].chunks_exact(4))
                    .all(|p| p == [0, 0, 0, 255]));
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}
