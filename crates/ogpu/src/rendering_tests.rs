use super::*;
use crate::{
    contract, vulkan::Instance, OgpuColorAttachment, OgpuDepthAttachment, OgpuImageDesc,
    UNSUPPORTED,
};
use std::sync::Arc;

#[test]
fn rendering_descriptions_and_ranges_need_no_driver() {
    let buffer = OgpuBufferDesc {
        size_bytes: 16,
        placement: 0,
        extra_usage: 1,
    };
    assert_eq!(contract::buffer_desc(&buffer).unwrap(), 16);
    for desc in [
        OgpuBufferDesc {
            size_bytes: 0,
            ..buffer
        },
        OgpuBufferDesc {
            size_bytes: u64::MAX,
            ..buffer
        },
        OgpuBufferDesc {
            placement: 2,
            ..buffer
        },
        OgpuBufferDesc {
            extra_usage: 2,
            ..buffer
        },
    ] {
        assert_eq!(
            contract::buffer_desc(&desc).unwrap_err().status,
            INVALID_ARGUMENT
        );
    }
    for format in 0..2 {
        contract::index_range(32, 16, 8, 8, format).unwrap();
    }
    for (address, offset, size, format) in [
        (16, 0, 0, 0),
        (16, 1, 4, 0),
        (16, 2, 4, 1),
        (16, 0, 3, 0),
        (16, 0, 8, 2),
        (17, 0, 8, 0),
        (u64::MAX - 3, 4, 4, 1),
    ] {
        assert_eq!(
            contract::index_range(32, address, offset, size, format)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
    }
    assert_eq!(
        contract::index_range(32, 0, 32, 4, 1).unwrap_err().status,
        OUT_OF_RANGE
    );
    assert_eq!(
        contract::index_range(32, 0, 0, u64::MAX - 3, 1)
            .unwrap_err()
            .status,
        OUT_OF_RANGE
    );
    let valid = OgpuRenderingDesc {
        color: OgpuColorAttachment {
            image: ptr::dangling_mut(),
            clear: [0.0; 4],
            ..Default::default()
        },
        depth: OgpuDepthAttachment {
            image: ptr::dangling_mut(),
            clear: 1.0,
            ..Default::default()
        },
    };
    contract::attachments(&valid).unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
        let mut d = valid;
        d.depth.clear = value;
        assert_eq!(
            contract::attachments(&d).unwrap_err().status,
            INVALID_ARGUMENT
        );
        d.depth.load = 1;
        contract::attachments(&d).unwrap();
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut d = valid;
        d.color.clear[3] = value;
        assert!(contract::attachments(&d).is_err());
        d.color.load = 2;
        contract::attachments(&d).unwrap();
    }
    for field in 0..7 {
        let mut d = valid;
        match field {
            0 => d.color.image = ptr::null_mut(),
            1 => d.color.load = 3,
            2 => d.color.store = 2,
            3 => d.depth.load = 3,
            4 => d.depth.store = 2,
            5 => d.depth.reserved = 1,
            _ => d.depth.image = ptr::null_mut(),
        }
        assert!(contract::attachments(&d).is_err());
    }
    unsafe {
        assert_eq!(
            ogpu_batch_begin_rendering(ptr::null_mut(), ptr::null(), ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_batch_end_rendering(ptr::null_mut(), ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_batch_draw_indexed_indirect(
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                0,
                ptr::null(),
                0,
                ptr::null_mut()
            ),
            INVALID_ARGUMENT
        );
    }
}

#[test]
#[ignore = "requires graphics Vulkan; every depth compare/test/write combination and read-only LOAD draws"]
fn gpu_depth_compare_matrix() {
    let instance = Arc::new(Instance::new().unwrap());
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let shader = |words: &[u32]| OgpuShaderDesc {
        code: words.as_ptr().cast(),
        code_size: (words.len() * 4) as u64,
        entry_point: ptr::null(),
        constants: ptr::null(),
        constant_count: 0,
        format: 0,
        local_size: [0; 3],
        reserved: 0,
    };
    // Incoming z=.5 against [.25, .5, .75, 1]: NEVER, LESS, EQUAL,
    // LESS_EQUAL, GREATER, NOT_EQUAL, GREATER_EQUAL, ALWAYS.
    // This explicit truth table is independent of the runtime's native mapping.
    let pass_masks = [0u8, 12, 2, 14, 1, 13, 3, 15];
    let depths = [0.25f32, 0.5, 0.75, 1.0];
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
            let mut depth = image(&mut device, 4, 32 | 8 | 16);
            let vertices = buffer(&mut device, 48, 0);
            let mut indices = buffer(&mut device, 12, 1);
            let mut indirect = buffer(&mut device, 20, 0);
            let mut source = buffer(&mut device, 4096, 0);
            let mut output = buffer(&mut device, 8200, 0);
            // Oversized triangle covers every pixel center: no edge exclusions.
            let data: Vec<u8> = [
                -1.0f32, -1.0, 0.5, 1.0, 3.0, -1.0, 0.5, 1.0, -1.0, 3.0, 0.5, 1.0,
            ]
            .into_iter()
            .flat_map(f32::to_ne_bytes)
            .collect();
            vertices.inner.write(0, &data).unwrap();
            let record: Vec<u8> = [3u32, 1, 0, 0, 0]
                .into_iter()
                .flat_map(u32::to_ne_bytes)
                .collect();
            indirect.inner.write(0, &record).unwrap();
            let uploaded: Vec<u8> = (0..1024)
                .flat_map(|i| depths[i % 4].to_ne_bytes())
                .collect();
            source.inner.write(0, &uploaded).unwrap();
            output.inner.write(0, &[0xa5; 8200]).unwrap();
            let root = vertices.inner.address().unwrap().to_ne_bytes();
            for format in 0..2 {
                let bytes: Vec<u8> = if format == 0 {
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
                indices.inner.write(0, &bytes).unwrap();
                let range = OgpuIndexRange {
                    buffer: &mut *indices,
                    offset: 0,
                    size_bytes: bytes.len() as u64,
                    format,
                    reserved: 0,
                };
                for (compare, mask) in pass_masks.into_iter().enumerate() {
                    for test in 0..2 {
                        for write in 0..2 {
                            let desc = OgpuRasterDesc {
                                push_size_bytes: 8,
                                topology: 0,
                                color_format: 0,
                                depth_format: 4,
                                depth_test: test,
                                depth_write: write,
                                depth_compare: compare as u32,
                                reserved: 0,
                            };
                            let mut raw = ptr::null_mut();
                            assert_eq!(
                                ogpu_raster_create(
                                    &mut device,
                                    &shader(&vertex),
                                    &shader(&fragment),
                                    &desc,
                                    &mut raw,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            let mut raster = Box::from_raw(raw);
                            let mut batch = OgpuBatch {
                                inner: Batch::new(d.clone()).unwrap(),
                            };
                            assert_eq!(
                                ogpu_batch_discard_image(&mut batch, &*color, ptr::null_mut()),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_discard_image(&mut batch, &*depth, ptr::null_mut()),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_copy_buffer_to_image(
                                    &mut batch,
                                    &mut *source,
                                    0,
                                    &mut *depth,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_barrier(&mut batch, 64, 1024 | 2048, ptr::null_mut()),
                                SUCCESS
                            );
                            let mut rendering = OgpuRenderingDesc {
                                color: OgpuColorAttachment {
                                    image: &mut *color,
                                    clear: [0.0, 0.0, 1.0, 1.0],
                                    ..Default::default()
                                },
                                depth: OgpuDepthAttachment {
                                    image: &mut *depth,
                                    load: 1,
                                    ..Default::default()
                                },
                            };
                            for pass in 0..2 {
                                if pass == 1 {
                                    assert_eq!(
                                        ogpu_batch_barrier(
                                            &mut batch,
                                            16 | 1024 | 2048,
                                            256 | 16 | 1024 | 2048,
                                            ptr::null_mut()
                                        ),
                                        SUCCESS
                                    );
                                    rendering.color.load = 1;
                                }
                                assert_eq!(
                                    ogpu_batch_begin_rendering(
                                        &mut batch,
                                        &rendering,
                                        ptr::null_mut()
                                    ),
                                    SUCCESS
                                );
                                assert_eq!(
                                    ogpu_batch_draw_indexed_indirect(
                                        &mut batch,
                                        &mut *raster,
                                        &range,
                                        &mut *indirect,
                                        0,
                                        root.as_ptr().cast(),
                                        8,
                                        ptr::null_mut()
                                    ),
                                    SUCCESS
                                );
                                assert_eq!(
                                    ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                                    SUCCESS
                                );
                            }
                            assert_eq!(
                                ogpu_batch_copy_image_to_buffer(
                                    &mut batch,
                                    &mut *color,
                                    &mut *output,
                                    4,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_copy_image_to_buffer(
                                    &mut batch,
                                    &mut *depth,
                                    &mut *output,
                                    4100,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            batch.inner.submit().unwrap().wait().unwrap();
                            let mut actual = [0u8; 8200];
                            output
                                .inner
                                .read(0, actual.as_mut_ptr(), actual.len())
                                .unwrap();
                            assert_eq!(&actual[..4], &[0xa5; 4]);
                            assert_eq!(&actual[8196..], &[0xa5; 4]);
                            for pixel in 0..1024 {
                                let pass = test == 0 || mask & (1 << (pixel % 4)) != 0;
                                let expected = if pass {
                                    [255, 0, 0, 255]
                                } else {
                                    [0, 0, 255, 255]
                                };
                                let z = if pass && test != 0 && write != 0 {
                                    0.5
                                } else {
                                    depths[pixel % 4]
                                };
                                assert_eq!(&actual[4+pixel*4..8+pixel*4], &expected,
                                    "color: index={format} compare={compare} test={test} write={write} pixel={pixel}");
                                assert_eq!(&actual[4100+pixel*4..4104+pixel*4], &z.to_ne_bytes(),
                                    "depth: index={format} compare={compare} test={test} write={write} pixel={pixel}");
                            }
                        }
                    }
                }
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

unsafe fn buffer(device: &mut OgpuDevice, size: u64, usage: u32) -> Box<OgpuBuffer> {
    unsafe {
        let mut out = ptr::null_mut();
        assert_eq!(
            ogpu_buffer_create(
                device,
                &OgpuBufferDesc {
                    size_bytes: size,
                    placement: 0,
                    extra_usage: usage
                },
                &mut out,
                ptr::null_mut()
            ),
            SUCCESS
        );
        Box::from_raw(out)
    }
}

unsafe fn image(device: &mut OgpuDevice, format: u32, usage: u32) -> Box<OgpuImage> {
    unsafe {
        let desc = OgpuImageDesc {
            dimension: 2,
            width: 32,
            height: 32,
            format,
            usage,
            reserved: 0,
        };
        assert_eq!(
            ogpu_image_check_support(device, &desc, ptr::null_mut()),
            SUCCESS
        );
        let mut out = ptr::null_mut();
        assert_eq!(
            ogpu_image_create(device, &desc, &mut out, ptr::null_mut()),
            SUCCESS
        );
        Box::from_raw(out)
    }
}

#[test]
#[ignore = "requires graphics Vulkan; public indexed/depth scopes, rejection, replay and retained ownership"]
fn gpu_indexed_depth_scopes() {
    let instance = Arc::new(Instance::new().unwrap());
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let shader = |words: &[u32]| OgpuShaderDesc {
        code: words.as_ptr().cast(),
        code_size: (words.len() * 4) as u64,
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
        for format in 0..2 {
            unsafe {
                let mut color = image(&mut device, 0, 4 | 8);
                let mut depth = image(&mut device, 4, 32 | 8 | 16);
                let mut vertices = buffer(&mut device, 96, 0);
                let mut indices = buffer(&mut device, 64, 1);
                let mut indirect = buffer(&mut device, 40, 0);
                let mut output = buffer(&mut device, 8200, 0);
                output.inner.write(0, &[0xaa; 8200]).unwrap();
                let mut data = Vec::new();
                for z in [0.75f32, 0.25] {
                    for p in [
                        [-1.0, -1.0, z, 1.0],
                        [1.0, -1.0, z, 1.0],
                        [0.0, 1.0, z, 1.0],
                    ] {
                        data.extend(p.into_iter().flat_map(f32::to_ne_bytes));
                    }
                }
                vertices.inner.write(0, &data).unwrap();
                let index_words = [u32::MAX, u32::MAX, 1, 2, 3];
                let bytes: Vec<u8> = if format == 0 {
                    index_words
                        .into_iter()
                        .flat_map(|v| (v as u16).to_ne_bytes())
                        .collect()
                } else {
                    index_words.into_iter().flat_map(u32::to_ne_bytes).collect()
                };
                indices.inner.write(8, &bytes).unwrap();
                let record: Vec<u8> = [3, 1, 2, u32::MAX, 0, 3, 1, 2, 2, 0]
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect();
                indirect.inner.write(0, &record).unwrap();
                let root = vertices.inner.address().unwrap().to_ne_bytes();
                let raster_desc = OgpuRasterDesc {
                    push_size_bytes: 8,
                    topology: 0,
                    color_format: 0,
                    depth_format: 4,
                    depth_test: 1,
                    depth_write: 1,
                    depth_compare: 1,
                    reserved: 0,
                };
                let mut raw = ptr::null_mut();
                for field in 0..8 {
                    let mut bad = raster_desc;
                    match field {
                        0 => bad.depth_format = 1,
                        1 => bad.color_format = 4,
                        2 => bad.depth_test = 2,
                        3 => bad.depth_write = 2,
                        4 => bad.depth_compare = 8,
                        5 => bad.reserved = 1,
                        6 => bad.topology = 2,
                        _ => bad.depth_format = u32::MAX,
                    }
                    raw = ptr::dangling_mut();
                    assert_eq!(
                        ogpu_raster_create(
                            &mut device,
                            &shader(&vertex),
                            &shader(&fragment),
                            &bad,
                            &mut raw,
                            ptr::null_mut()
                        ),
                        INVALID_ARGUMENT
                    );
                    assert!(raw.is_null());
                }
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
                let range = OgpuIndexRange {
                    buffer: &mut *indices,
                    offset: 8,
                    size_bytes: bytes.len() as u64,
                    format,
                    reserved: 0,
                };
                let desc = OgpuRenderingDesc {
                    color: OgpuColorAttachment {
                        image: &mut *color,
                        clear: [0.0, 0.0, 1.0, 1.0],
                        ..Default::default()
                    },
                    depth: OgpuDepthAttachment {
                        image: &mut *depth,
                        clear: 1.0,
                        ..Default::default()
                    },
                };
                // Distinct logical devices on one physical GPU are enough to test
                // identity rejection; this is not a second-physical-GPU claim.
                let mut foreign = OgpuDevice {
                    inner: Device::new_graphics(instance.clone(), physical).unwrap(),
                };
                let mut foreign_color = image(&mut foreign, 0, 4 | 8);
                let mut foreign_depth = image(&mut foreign, 4, 32 | 8);
                let mut foreign_indices = buffer(&mut foreign, 64, 1);
                let mut foreign_indirect = buffer(&mut foreign, 40, 0);
                let mut foreign_raw = ptr::null_mut();
                assert_eq!(
                    ogpu_raster_create(
                        &mut foreign,
                        &shader(&vertex),
                        &shader(&fragment),
                        &raster_desc,
                        &mut foreign_raw,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                let mut foreign_raster = Box::from_raw(foreign_raw);
                let mut wrong_color = image(&mut device, 2, 4 | 8);
                let mut wrong_usage = image(&mut device, 0, 1 | 8);
                let mut small = ptr::null_mut();
                assert_eq!(
                    ogpu_image_create(
                        &mut device,
                        &OgpuImageDesc {
                            dimension: 2,
                            width: 16,
                            height: 32,
                            format: 4,
                            usage: 32 | 8,
                            reserved: 0,
                        },
                        &mut small,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                let mut small = Box::from_raw(small);
                // Rejected begin must not open a scope or retain bad attachments.
                let mut rejected = OgpuBatch {
                    inner: Batch::new(d.clone()).unwrap(),
                };
                for bad in 0..5 {
                    let mut r = desc;
                    match bad {
                        0 => r.color.image = &mut *foreign_color,
                        1 => r.depth.image = &mut *foreign_depth,
                        2 => r.depth.image = &mut *small,
                        3 => r.color.image = &mut *wrong_usage,
                        _ => r.depth.image = &mut *color,
                    }
                    assert_eq!(
                        ogpu_batch_begin_rendering(&mut rejected, &r, ptr::null_mut()),
                        INVALID_ARGUMENT
                    );
                    assert_eq!(
                        ogpu_batch_end_rendering(&mut rejected, ptr::null_mut()),
                        INVALID_ARGUMENT
                    );
                }
                // Valid scopes with incompatible raster formats reject draws,
                // then can end and be discarded without native execution.
                for no_depth in [false, true] {
                    let mut r = desc;
                    if no_depth {
                        r.depth = OgpuDepthAttachment::default();
                    } else {
                        r.color.image = &mut *wrong_color;
                    }
                    assert_eq!(
                        ogpu_batch_begin_rendering(&mut rejected, &r, ptr::null_mut()),
                        SUCCESS
                    );
                    assert_eq!(
                        ogpu_batch_draw_indexed_indirect(
                            &mut rejected,
                            &mut *raster,
                            &range,
                            &mut *indirect,
                            0,
                            root.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        INVALID_ARGUMENT
                    );
                    assert_eq!(
                        ogpu_batch_end_rendering(&mut rejected, ptr::null_mut()),
                        SUCCESS
                    );
                }
                drop(rejected);
                // Initialize layouts once, separately from the replayed CLEAR scopes.
                let mut init = OgpuBatch {
                    inner: Batch::new(d.clone()).unwrap(),
                };
                assert_eq!(
                    ogpu_batch_discard_image(&mut init, &*color, ptr::null_mut()),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_discard_image(&mut init, &*depth, ptr::null_mut()),
                    SUCCESS
                );
                init.inner.submit().unwrap().wait().unwrap();
                let mut batch = OgpuBatch {
                    inner: Batch::new(d.clone()).unwrap(),
                };
                assert_eq!(
                    ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_draw_indexed_indirect(
                        &mut batch,
                        &mut *raster,
                        &range,
                        &mut *indirect,
                        0,
                        root.as_ptr().cast(),
                        8,
                        ptr::null_mut()
                    ),
                    INVALID_ARGUMENT
                );
                // Previous replay copies finish before subsequent attachment writes.
                assert_eq!(
                    ogpu_batch_barrier(
                        &mut batch,
                        32 | 16 | 2048,
                        16 | 1024 | 2048,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_begin_rendering(&mut batch, &desc, ptr::null_mut()),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_begin_rendering(&mut batch, &desc, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_barrier(&mut batch, 1, 2, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_discard_image(&mut batch, &*depth, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_copy_image_to_buffer(
                        &mut batch,
                        &mut *depth,
                        &mut *output,
                        0,
                        ptr::null_mut()
                    ),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_copy_buffer(
                        &mut batch,
                        &*vertices,
                        0,
                        &*output,
                        0,
                        4,
                        ptr::null_mut()
                    ),
                    INVALID_ARGUMENT
                );
                let mut token = 0;
                assert_eq!(
                    ogpu_batch_dependency_begin(&mut batch, 2, 1, &mut token, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    ogpu_batch_dependency_end(&mut batch, token, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                let mut receipt = ptr::dangling_mut();
                assert_eq!(
                    ogpu_batch_submit(&mut batch, &mut receipt, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert!(receipt.is_null());
                let mut list = ptr::dangling_mut();
                assert_eq!(
                    ogpu_batch_compile(&mut batch, 0, &mut list, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
                assert!(list.is_null());
                for offset in [1, 24, u64::MAX] {
                    assert_ne!(
                        ogpu_batch_draw_indexed_indirect(
                            &mut batch,
                            &mut *raster,
                            &range,
                            &mut *indirect,
                            offset,
                            root.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                }
                for bad in 0..5 {
                    let mut r = range;
                    match bad {
                        0 => r.buffer = &mut *vertices,
                        1 => r.size_bytes = 0,
                        2 => r.offset = 1,
                        3 => r.format = 2,
                        _ => r.reserved = 1,
                    }
                    assert_eq!(
                        ogpu_batch_draw_indexed_indirect(
                            &mut batch,
                            &mut *raster,
                            &r,
                            &mut *indirect,
                            0,
                            root.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        INVALID_ARGUMENT
                    );
                }
                // Reject each foreign draw object independently. A valid draw
                // after these failures must still compile, execute and replay.
                for bad in 0..4 {
                    let mut r = range;
                    if bad == 0 {
                        r.buffer = &mut *foreign_indices;
                    }
                    let pipeline = if bad == 1 {
                        &mut *foreign_raster
                    } else {
                        &mut *raster
                    };
                    let records = if bad == 2 {
                        &mut *foreign_indirect
                    } else {
                        &mut *indirect
                    };
                    assert_eq!(
                        ogpu_batch_draw_indexed_indirect(
                            &mut batch,
                            pipeline,
                            &r,
                            records,
                            0,
                            root.as_ptr().cast(),
                            if bad == 3 { 4 } else { 8 },
                            ptr::null_mut()
                        ),
                        INVALID_ARGUMENT
                    );
                }
                let weak_foreign_image = Rc::downgrade(&foreign_depth.inner);
                let weak_foreign_index = Rc::downgrade(&foreign_indices.inner);
                let weak_foreign_raster = Rc::downgrade(&foreign_raster.inner);
                drop((
                    foreign_color,
                    foreign_depth,
                    foreign_indices,
                    foreign_indirect,
                    foreign_raster,
                ));
                assert!(weak_foreign_image.upgrade().is_none());
                assert!(weak_foreign_index.upgrade().is_none());
                assert!(weak_foreign_raster.upgrade().is_none());
                assert_eq!(
                    ogpu_batch_retain_buffer(&mut batch, &*vertices, ptr::null_mut()),
                    SUCCESS
                );
                for offset in [0, 20] {
                    assert_eq!(
                        ogpu_batch_draw_indexed_indirect(
                            &mut batch,
                            &mut *raster,
                            &range,
                            &mut *indirect,
                            offset,
                            root.as_ptr().cast(),
                            8,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                }
                assert_eq!(
                    ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_copy_image_to_buffer(
                        &mut batch,
                        &mut *color,
                        &mut *output,
                        4,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_copy_image_to_buffer(
                        &mut batch,
                        &mut *depth,
                        &mut *output,
                        4100,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_batch_compile(&mut batch, 0, &mut list, ptr::null_mut()),
                    SUCCESS
                );
                let mut list = Box::from_raw(list);
                for phase in 0..3 {
                    let bytes = if phase == 1 {
                        [0, 1, 2, u32::MAX, 0, 0, 1, 2, 2, 0]
                            .into_iter()
                            .flat_map(u32::to_ne_bytes)
                            .collect::<Vec<_>>()
                    } else {
                        record.clone()
                    };
                    indirect.inner.write(0, &bytes).unwrap();
                    assert_eq!(
                        ogpu_command_list_submit(&mut *list, &mut receipt, ptr::null_mut()),
                        SUCCESS
                    );
                    let mut receipt = Box::from_raw(receipt);
                    assert_eq!(
                        ogpu_completion_wait(&mut *receipt, ptr::null_mut()),
                        SUCCESS
                    );
                    let mut pixels = [0u8; 8200];
                    output
                        .inner
                        .read(0, pixels.as_mut_ptr(), pixels.len())
                        .unwrap();
                    let center = (16 * 32 + 16) * 4;
                    assert_eq!(
                        &pixels[4 + center..8 + center],
                        if phase == 1 {
                            &[0, 0, 255, 255]
                        } else {
                            &[255, 0, 0, 255]
                        }
                    );
                    let z = f32::from_ne_bytes(
                        pixels[4100 + center..4104 + center].try_into().unwrap(),
                    );
                    assert_eq!(z, if phase == 1 { 1.0 } else { 0.25 });
                    assert_eq!(&pixels[..4], &[0xaa; 4]);
                    assert_eq!(&pixels[8196..], &[0xaa; 4]);
                }
                let weak_image = Rc::downgrade(&depth.inner);
                let weak_index = Rc::downgrade(&indices.inner);
                let weak_vertex = Rc::downgrade(&vertices.inner);
                drop((color, depth, indices, vertices, indirect, raster));
                assert!(
                    weak_image.upgrade().is_some()
                        && weak_index.upgrade().is_some()
                        && weak_vertex.upgrade().is_some()
                );
                assert_eq!(
                    ogpu_command_list_submit(&mut *list, &mut receipt, ptr::null_mut()),
                    SUCCESS
                );
                let mut receipt = Box::from_raw(receipt);
                assert_eq!(
                    ogpu_completion_wait(&mut *receipt, ptr::null_mut()),
                    SUCCESS
                );
                drop(receipt);
                drop(list);
                assert!(
                    weak_image.upgrade().is_none()
                        && weak_index.upgrade().is_none()
                        && weak_vertex.upgrade().is_none()
                );
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}

#[test]
#[ignore = "requires graphics Vulkan; empty scopes, D32 upload, independent LOAD/CLEAR and abandoned ownership"]
fn gpu_depth_attachment_operations() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut device = OgpuDevice { inner: d.clone() };
        unsafe {
            for (format, usage, dimension) in [
                (4, 32 | 1, 2),
                (4, 32 | 2, 2),
                (4, 32 | 4, 2),
                (4, 8, 2),
                (1, 32, 2),
                (4, 32, 1),
            ] {
                let desc = OgpuImageDesc {
                    dimension,
                    width: 32,
                    height: 1,
                    format,
                    usage,
                    reserved: 0,
                };
                assert_eq!(
                    ogpu_image_check_support(&device, &desc, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
            }
            let mut color = image(&mut device, 0, 4 | 8);
            let mut depth = image(&mut device, 4, 32 | 8 | 16);
            let mut source = buffer(&mut device, 4096, 0);
            let mut output = buffer(&mut device, 8192, 0);
            let expected_depth: Vec<u8> = (0..1024)
                .flat_map(|i| ((i % 4) as f32 / 4.0).to_ne_bytes())
                .collect();
            source.inner.write(0, &expected_depth).unwrap();
            let mut batch = OgpuBatch {
                inner: Batch::new(d.clone()).unwrap(),
            };
            assert_eq!(
                ogpu_batch_discard_image(&mut batch, &*color, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_discard_image(&mut batch, &*depth, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_buffer_to_image(
                    &mut batch,
                    &mut *source,
                    0,
                    &mut *depth,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_barrier(&mut batch, 64, 1024 | 2048, ptr::null_mut()),
                SUCCESS
            );
            let mut desc = OgpuRenderingDesc {
                color: OgpuColorAttachment {
                    image: &mut *color,
                    clear: [0.0, 1.0, 0.0, 1.0],
                    ..Default::default()
                },
                depth: OgpuDepthAttachment {
                    image: &mut *depth,
                    load: 1,
                    ..Default::default()
                },
            };
            let mut invalid = desc;
            invalid.color.clear[0] = f32::NAN;
            assert_eq!(
                ogpu_batch_begin_rendering(&mut batch, &invalid, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_begin_rendering(&mut batch, &desc, ptr::null_mut()),
                SUCCESS
            );
            // No draws: color clear still executes, uploaded depth LOAD still survives.
            assert_eq!(
                ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    &mut batch,
                    &mut *color,
                    &mut *output,
                    0,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    &mut batch,
                    &mut *depth,
                    &mut *output,
                    4096,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            batch.inner.submit().unwrap().wait().unwrap();
            let mut actual = [0u8; 8192];
            output
                .inner
                .read(0, actual.as_mut_ptr(), actual.len())
                .unwrap();
            assert_eq!(&actual[..4096], [0, 255, 0, 255].repeat(1024));
            assert_eq!(&actual[4096..], expected_depth);
            // Independent operations: preserve color, replace depth without reinitializing layout.
            let mut batch = OgpuBatch {
                inner: Batch::new(d.clone()).unwrap(),
            };
            assert_eq!(
                ogpu_batch_barrier(&mut batch, 16 | 2048 | 32, 256 | 16 | 2048, ptr::null_mut()),
                SUCCESS
            );
            desc.color.load = 1;
            desc.depth.load = 0;
            desc.depth.clear = 0.5;
            assert_eq!(
                ogpu_batch_begin_rendering(&mut batch, &desc, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    &mut batch,
                    &mut *color,
                    &mut *output,
                    0,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    &mut batch,
                    &mut *depth,
                    &mut *output,
                    4096,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            batch.inner.submit().unwrap().wait().unwrap();
            output
                .inner
                .read(0, actual.as_mut_ptr(), actual.len())
                .unwrap();
            assert_eq!(&actual[..4096], [0, 255, 0, 255].repeat(1024));
            assert_eq!(&actual[4096..], 0.5f32.to_ne_bytes().repeat(1024));
            // Disposable results are never inspected; a subsequent explicit clear defines them.
            let mut batch = OgpuBatch {
                inner: Batch::new(d.clone()).unwrap(),
            };
            assert_eq!(
                ogpu_batch_barrier(&mut batch, 16 | 2048 | 32, 16 | 2048, ptr::null_mut()),
                SUCCESS
            );
            desc.color.load = 2;
            desc.depth.load = 2;
            desc.color.store = 1;
            desc.depth.store = 1;
            assert_eq!(
                ogpu_batch_begin_rendering(&mut batch, &desc, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_end_rendering(&mut batch, ptr::null_mut()),
                SUCCESS
            );
            batch.inner.submit().unwrap().wait().unwrap();
            let mut abandoned = OgpuBatch {
                inner: Batch::new(d.clone()).unwrap(),
            };
            desc.color.load = 0;
            desc.depth.load = 0;
            desc.color.store = 0;
            desc.depth.store = 0;
            let mut reset = OgpuBatch {
                inner: Batch::new(d.clone()).unwrap(),
            };
            assert_eq!(
                ogpu_batch_barrier(&mut reset, 16 | 2048, 16 | 2048, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_begin_rendering(&mut reset, &desc, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_end_rendering(&mut reset, ptr::null_mut()),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    &mut reset,
                    &mut *depth,
                    &mut *output,
                    4096,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            reset.inner.submit().unwrap().wait().unwrap();
            output
                .inner
                .read(0, actual.as_mut_ptr(), actual.len())
                .unwrap();
            assert_eq!(&actual[4096..], 0.5f32.to_ne_bytes().repeat(1024));
            let weak = Rc::downgrade(&depth.inner);
            assert_eq!(
                ogpu_batch_begin_rendering(&mut abandoned, &desc, ptr::null_mut()),
                SUCCESS
            );
            drop(depth);
            assert!(weak.upgrade().is_some());
            drop(abandoned);
            assert!(weak.upgrade().is_none());
        }
        tested += 1;
    }
    assert!(tested > 0);
}
