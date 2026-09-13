use super::*;
use crate::{vulkan::Instance, UNSUPPORTED};
use std::sync::Arc;

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}
fn desc(words: &[u32], constants: &[OgpuSpecializationConstant]) -> OgpuShaderDesc {
    OgpuShaderDesc {
        words: words.as_ptr(),
        word_count: words.len() as u64,
        constants: constants.as_ptr(),
        constant_count: constants.len() as u32,
        reserved: 0,
    }
}
fn constant(id: u32, bits: u32) -> OgpuSpecializationConstant {
    OgpuSpecializationConstant { id, bits }
}

#[test]
fn shader_descriptions_reject_invalid_pointer_count_and_reserved_fields() {
    let words = [0x07230203, 0, 0, 0, 0];
    let mut shader_desc = desc(&words, &[]);
    unsafe {
        let mut limits = OgpuDeviceLimits {
            max_push_data_bytes: 123,
            ..Default::default()
        };
        assert_eq!(
            ogpu_device_limits(ptr::null(), &mut limits, ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(limits.max_push_data_bytes, 123);
        assert!(shader(ptr::null()).is_err());
        shader_desc.constants = ptr::null();
        assert!(shader(&shader_desc).is_ok());
        shader_desc.constant_count = 1;
        assert!(shader(&shader_desc).is_err());
        shader_desc.constant_count = 0;
        shader_desc.reserved = 1;
        assert!(shader(&shader_desc).is_err());
        shader_desc.reserved = 0;
        shader_desc.word_count = 4;
        assert!(shader(&shader_desc).is_err());
    }
}

#[test]
#[ignore = "requires Vulkan; creation-time int/uint/float/bool specialization and shared arrays"]
fn gpu_compute_specialization() {
    let instance = Arc::new(Instance::new().unwrap());
    let words = words(include_bytes!(
        "../../../examples/shaders/specialize.comp.spv"
    ));
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut handle = OgpuDevice {
            inner: device.clone(),
        };
        let mut limits = OgpuDeviceLimits::default();
        unsafe {
            assert_eq!(
                ogpu_device_limits(&handle, ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_device_limits(&handle, &mut limits, ptr::null_mut()),
                SUCCESS
            );
        }
        assert_eq!(limits, device.execution_limits());
        assert!(limits.max_group_size.iter().all(|&v| v > 0));
        assert!(limits.max_dispatch.iter().all(|&v| v > 0));
        assert!(limits.max_group_invocations >= 8 && limits.max_shared_memory_bytes >= 64);
        assert!(limits.max_push_data_bytes >= 8);
        let buffer = Buffer::new(device.clone(), 3 * 16 + 8).unwrap();
        buffer.write(0, &[0xaa; 56]).unwrap();
        let root = (buffer.address().unwrap() + 4).to_ne_bytes();
        let mut values = [
            constant(5, 0),
            constant(3, (-7i32) as u32),
            constant(2, (-0.5f32).to_bits()),
            constant(1, 16),
            constant(4, 9),
            constant(99, u32::MAX),
        ];
        let mut kernels = Vec::new();
        for constants in [&[][..], values.as_slice()] {
            let shader_desc = desc(&words, constants);
            let mut kernel = ptr::null_mut();
            unsafe {
                assert_eq!(
                    ogpu_kernel_create(&mut handle, &shader_desc, 8, &mut kernel, ptr::null_mut()),
                    SUCCESS
                );
                kernels.push(Box::from_raw(kernel));
            }
        }
        // Rejected duplicate IDs never replace/consume the successful executables.
        values[0].id = values[1].id;
        let mut rejected = ptr::dangling_mut();
        unsafe {
            assert_eq!(
                ogpu_kernel_create(
                    &mut handle,
                    &desc(&words, &values),
                    8,
                    &mut rejected,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
        }
        assert!(rejected.is_null());
        values.fill(constant(0, 0));
        for variant in [1, 0, 1] {
            unsafe {
                assert_eq!(
                    ogpu_dispatch_wait(
                        &mut *kernels[variant],
                        3,
                        1,
                        1,
                        root.as_ptr().cast(),
                        8,
                        ptr::null_mut()
                    ),
                    SUCCESS
                );
            }
            let mut bytes = [0u8; 56];
            unsafe {
                buffer.read(0, bytes.as_mut_ptr(), bytes.len()).unwrap();
            }
            assert_eq!(&bytes[..4], &[0xaa; 4]);
            assert_eq!(&bytes[52..], &[0xaa; 4]);
            let expected = if variant == 0 {
                [8, 1f32.to_bits(), 36f32.to_bits(), 1]
            } else {
                [16, (-0.5f32).to_bits(), (-28f32).to_bits(), 0]
            };
            for group in bytes[4..52].chunks_exact(16) {
                assert_eq!(words_from_group(group), expected);
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}

fn words_from_group(bytes: &[u8]) -> [u32; 4] {
    std::array::from_fn(|i| u32::from_ne_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
}

#[test]
#[ignore = "requires graphics Vulkan; per-stage specialization, triangle strip and lowered vertex input"]
fn gpu_raster_specialization_and_vertex_pulling() {
    let instance = Arc::new(Instance::new().unwrap());
    let vertex = words(include_bytes!(
        "../../../examples/shaders/specialize.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/specialize.frag.spv"
    ));
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut handle = OgpuDevice {
            inner: device.clone(),
        };
        let mut vertex_values = [constant(0, 0.5f32.to_bits())];
        let mut fragment_values = [constant(0, 0.25f32.to_bits())];
        let vd = desc(&vertex, &vertex_values);
        let fd = desc(&fragment, &fragment_values);
        let mut raster = ptr::null_mut();
        unsafe {
            assert_eq!(
                ogpu_raster_create(&mut handle, &vd, &fd, 8, 99, &mut raster, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(raster.is_null());
            assert_eq!(
                ogpu_raster_create(&mut handle, &vd, &fd, 8, 1, &mut raster, ptr::null_mut()),
                SUCCESS
            );
        }
        let mut raster = unsafe { Box::from_raw(raster) };
        vertex_values.fill(constant(0, 0));
        fragment_values.fill(constant(0, 0));
        let vertices = Rc::new(Buffer::new(device.clone(), 64).unwrap());
        // uv at byte 0, position at byte 8, stride 16; four-vertex strip.
        let data: [f32; 16] = [
            0., 0., -1., -1., 1., 0., 1., -1., 0., 1., -1., 1., 1., 1., 1., 1.,
        ];
        vertices
            .write(
                0,
                &data
                    .into_iter()
                    .flat_map(f32::to_ne_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let mut indirect = OgpuBuffer {
            inner: Rc::new(Buffer::new(device.clone(), 16).unwrap()),
        };
        indirect
            .inner
            .write(
                0,
                &[4u32, 1, 0, 0]
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let mut image = OgpuImage {
            inner: Rc::new(
                Image::new(device.clone(), crate::compute::ImageDesc::rgba8(32, 32)).unwrap(),
            ),
        };
        let mut output = OgpuBuffer {
            inner: Rc::new(Buffer::new(device.clone(), 4096).unwrap()),
        };
        let root = vertices.address().unwrap().to_ne_bytes();
        let mut batch = OgpuBatch {
            inner: Batch::new(device).unwrap(),
        };
        batch.inner.retain_buffer(vertices).unwrap();
        unsafe {
            assert_eq!(
                ogpu_batch_draw_indirect(
                    &mut batch,
                    &mut *raster,
                    &mut image,
                    &mut indirect,
                    0,
                    root.as_ptr().cast(),
                    8,
                    0,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    &mut batch,
                    &mut image,
                    &mut output,
                    0,
                    ptr::null_mut()
                ),
                SUCCESS
            );
            let mut completion = batch.inner.submit().unwrap();
            completion.wait().unwrap();
        }
        let mut pixels = [0u8; 4096];
        unsafe {
            output
                .inner
                .read(0, pixels.as_mut_ptr(), pixels.len())
                .unwrap();
        }
        for y in 0..32 {
            for x in 0..32 {
                let expected = if (8..24).contains(&x) && (8..24).contains(&y) {
                    [
                        64,
                        (((x - 8) as f32 + 0.5) / 16.0 * 255.0).round() as u8,
                        (((y - 8) as f32 + 0.5) / 16.0 * 255.0).round() as u8,
                        255,
                    ]
                } else {
                    [0, 0, 0, 255]
                };
                assert_eq!(&pixels[(y * 32 + x) * 4..][..4], &expected, "pixel={x},{y}");
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}
