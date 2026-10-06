//! Public counted ranges: holes, padding, empty capacity and retained replay data.
use super::*;
use crate::execution_api::rendering_tests::{buffer, image, words};
use crate::{vulkan::Instance, OgpuColorAttachment, OgpuIndirectRange, UNSUPPORTED};
use std::sync::Arc;

#[test]
#[ignore = "requires graphics Vulkan; public counted ranges, validation, count ownership and replay"]
fn gpu_indirect_ranges() {
    let instance = Arc::new(Instance::new().unwrap());
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let shader = |code: &[u32]| OgpuShaderDesc {
        code: code.as_ptr().cast(),
        code_size: std::mem::size_of_val(code) as u64,
        entry_point: ptr::null(),
        constants: ptr::null(),
        constant_count: 0,
        format: 0,
        local_size: [0; 3],
        reserved: 0,
    };
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut device = OgpuDevice { inner: d.clone() };
        let caps = d.enabled_capabilities();
        assert_eq!(
            (
                caps.multi_draw_indirect,
                caps.draw_indirect_count,
                caps.shader_draw_parameters
            ),
            (1, 1, 1)
        );
        assert!(d.execution_limits().max_indirect_draw_count >= 3);
        let foreign = Device::new_graphics(instance.clone(), physical).unwrap();
        let mut foreign_device = OgpuDevice { inner: foreign };
        for format in [None, Some(0), Some(1)] {
            for shared in [false, true] {
                for maximum in [0, 3] {
                    for current in [false, true] {
                        unsafe {
                            let mut color = image(&mut device, 0, 4 | 8);
                            let vertices = buffer(&mut device, 48, 0);
                            let mut indices = buffer(&mut device, 32, 1);
                            let mut records = buffer(&mut device, 128, 0);
                            let mut counter = buffer(&mut device, 16, 0);
                            let mut output = buffer(&mut device, 4224, 0);
                            let mut other = buffer(&mut foreign_device, 16, 0);
                            let data: Vec<u8> =
                                [-1f32, -1., 0., 1., 1., -1., 0., 1., 0., 1., 0., 1.]
                                    .into_iter()
                                    .flat_map(f32::to_ne_bytes)
                                    .collect();
                            vertices.inner.write(0, &data).unwrap();
                            let indices_data: Vec<u8> = if format == Some(0) {
                                [0u16, 1, 2]
                                    .into_iter()
                                    .flat_map(u16::to_ne_bytes)
                                    .collect()
                            } else {
                                [0u32, 1, 2]
                                    .into_iter()
                                    .flat_map(u32::to_ne_bytes)
                                    .collect()
                            };
                            indices.inner.write(8, &indices_data).unwrap();
                            let mut record_data = [0xa5u8; 128];
                            for record in 0..3 {
                                let words = if format.is_some() {
                                    vec![if record == 1 { 3u32 } else { 0 }, 1, 0, 0, 0]
                                } else {
                                    vec![if record == 1 { 3u32 } else { 0 }, 1, 0, 0]
                                };
                                for (i, word) in words.iter().enumerate() {
                                    record_data[16 + 32 * record + 4 * i..20 + 32 * record + 4 * i]
                                        .copy_from_slice(&word.to_ne_bytes());
                                }
                            }
                            records.inner.write(0, &record_data).unwrap();
                            output.inner.write(0, &[0xa5; 4224]).unwrap();
                            let root = vertices.inner.address().unwrap().to_ne_bytes();
                            let mut raw = ptr::null_mut();
                            let raster_desc = OgpuRasterDesc {
                                push_size_bytes: 8,
                                topology: 0,
                                color_format: 0,
                                depth_format: u32::MAX,
                                depth_test: 0,
                                depth_write: 0,
                                depth_compare: 7,
                                reserved: 0,
                            };
                            assert_eq!(
                                ogpu_raster_create(
                                    &mut device,
                                    &shader(&vertex),
                                    &shader(&fragment),
                                    &raster_desc,
                                    &mut raw,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            let mut raster = Box::from_raw(raw);
                            let index = OgpuIndexRange {
                                buffer: &mut *indices,
                                offset: 8,
                                size_bytes: if format == Some(0) { 6 } else { 12 },
                                format: format.unwrap_or(1),
                                reserved: 0,
                            };
                            let draws = OgpuIndirectRange {
                                buffer: &mut *records,
                                offset: if maximum == 0 { 128 } else { 16 },
                                stride_bytes: 32,
                                max_draw_count: maximum,
                                count_buffer: if shared { &mut *records } else { &mut *counter },
                                count_offset: if shared { 120 } else { 4 },
                            };
                            let count_weak = Rc::downgrade(if shared {
                                &records.inner
                            } else {
                                &counter.inner
                            });
                            let record_weak = Rc::downgrade(&records.inner);
                            let raster_weak = Rc::downgrade(&raster.inner);
                            let mut batch = OgpuBatch {
                                inner: Batch::new(d.clone()).unwrap(),
                            };
                            assert_eq!(
                                ogpu_batch_discard_image(&mut batch, &*color, ptr::null_mut()),
                                SUCCESS
                            );
                            if current {
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
                            }
                            assert_eq!(
                                ogpu_batch_begin_rendering(
                                    &mut batch,
                                    &OgpuRenderingDesc {
                                        color: OgpuColorAttachment {
                                            image: &mut *color,
                                            clear: [0., 0., 0., 1.],
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    },
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            let call = |b: &mut OgpuBatch, draws: &OgpuIndirectRange| {
                                if current {
                                    return if format.is_some() {
                                        ogpu_batch_draw_indexed_indirect_current(
                                            b,
                                            &mut *raster,
                                            &index,
                                            draws,
                                            ptr::null_mut(),
                                        )
                                    } else {
                                        ogpu_batch_draw_indirect_current(
                                            b,
                                            &mut *raster,
                                            draws,
                                            ptr::null_mut(),
                                        )
                                    };
                                }
                                if format.is_some() {
                                    ogpu_batch_draw_indexed_indirect(
                                        b,
                                        &mut *raster,
                                        &index,
                                        draws,
                                        root.as_ptr().cast(),
                                        8,
                                        ptr::null_mut(),
                                    )
                                } else {
                                    ogpu_batch_draw_indirect(
                                        b,
                                        &mut *raster,
                                        draws,
                                        root.as_ptr().cast(),
                                        8,
                                        ptr::null_mut(),
                                    )
                                }
                            };
                            let mut call = call;
                            for (bad, status) in [
                                (OgpuIndirectRange { offset: 1, ..draws }, INVALID_ARGUMENT),
                                (
                                    OgpuIndirectRange {
                                        stride_bytes: 0,
                                        ..draws
                                    },
                                    INVALID_ARGUMENT,
                                ),
                                (
                                    OgpuIndirectRange {
                                        offset: 128,
                                        max_draw_count: 1,
                                        ..draws
                                    },
                                    OUT_OF_RANGE,
                                ),
                                (
                                    OgpuIndirectRange {
                                        count_offset: 1,
                                        ..draws
                                    },
                                    INVALID_ARGUMENT,
                                ),
                                (
                                    OgpuIndirectRange {
                                        count_offset: 128,
                                        ..draws
                                    },
                                    OUT_OF_RANGE,
                                ),
                                (
                                    OgpuIndirectRange {
                                        count_buffer: ptr::null_mut(),
                                        count_offset: 4,
                                        ..draws
                                    },
                                    INVALID_ARGUMENT,
                                ),
                                (
                                    OgpuIndirectRange {
                                        count_buffer: &mut *other,
                                        ..draws
                                    },
                                    INVALID_ARGUMENT,
                                ),
                            ] {
                                assert_eq!(call(&mut batch, &bad), status);
                            }
                            assert_eq!(call(&mut batch, &draws), SUCCESS);
                            assert_eq!(
                                ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_copy_image_to_buffer(
                                    &mut batch,
                                    &mut *color,
                                    &mut *output,
                                    64,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_retain_buffer(&mut batch, &*vertices, ptr::null_mut()),
                                SUCCESS
                            );
                            let mut list = ptr::null_mut();
                            assert_eq!(
                                ogpu_batch_compile(&mut batch, 0, &mut list, ptr::null_mut()),
                                SUCCESS
                            );
                            drop(batch);
                            drop(records);
                            drop(counter);
                            drop(raster);
                            drop(indices);
                            drop(vertices);
                            drop(color);
                            assert!(
                                record_weak.upgrade().is_some()
                                    && count_weak.upgrade().is_some()
                                    && raster_weak.upgrade().is_some()
                            );
                            let mut accepted = None;
                            for active in [0u32, 1, 2, 7, 0, 2] {
                                count_weak
                                    .upgrade()
                                    .unwrap()
                                    .write(draws.count_offset as usize, &active.to_ne_bytes())
                                    .unwrap();
                                let mut receipt = ptr::null_mut();
                                assert_eq!(
                                    ogpu_command_list_submit(list, &mut receipt, ptr::null_mut()),
                                    SUCCESS
                                );
                                assert_eq!(ogpu_completion_wait(receipt, ptr::null_mut()), SUCCESS);
                                ogpu_completion_destroy(receipt);
                                let mut bytes = [0u8; 4224];
                                output
                                    .inner
                                    .read(0, bytes.as_mut_ptr(), bytes.len())
                                    .unwrap();
                                assert!(bytes[..64]
                                    .iter()
                                    .chain(&bytes[4160..])
                                    .all(|&v| v == 0xa5));
                                let pixels = &bytes[64..4160];
                                if maximum == 0 || active < 2 {
                                    assert!(pixels.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
                                } else {
                                    assert_eq!(
                                        &pixels[(16 * 32 + 16) * 4..(16 * 32 + 16) * 4 + 4],
                                        &[255, 0, 0, 255]
                                    );
                                    if let Some(ref first) = accepted {
                                        assert_eq!(pixels, first);
                                    } else {
                                        accepted = Some(pixels.to_vec());
                                    }
                                }
                                let mut actual = [0u8; 128];
                                record_weak
                                    .upgrade()
                                    .unwrap()
                                    .read(0, actual.as_mut_ptr(), actual.len())
                                    .unwrap();
                                if shared {
                                    record_data[120..124].copy_from_slice(&active.to_ne_bytes());
                                }
                                assert_eq!(actual, record_data);
                            }
                            assert!(count_weak.upgrade().is_some());
                            ogpu_command_list_destroy(list);
                            assert!(
                                count_weak.upgrade().is_none()
                                    && record_weak.upgrade().is_none()
                                    && raster_weak.upgrade().is_none()
                            );
                        }
                        tested += 1;
                    }
                }
            }
        }
    }
    assert!(tested >= 24);
}
