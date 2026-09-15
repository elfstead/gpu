//! Image contracts through the C boundary; internal setup provides deterministic
//! driver selection and weak references for checking retained ownership.
use super::*;
use crate::{compute::Placement, vulkan::Instance, UNSUPPORTED};
use std::sync::Arc;

const SAMPLED: u32 = 1;
const STORAGE: u32 = 2;
const COPY_SRC: u32 = 8;
const COPY_DST: u32 = 16;

unsafe fn make_image(device: &Rc<Device>, desc: OgpuImageDesc) -> Box<OgpuImage> {
    let mut handle = OgpuDevice {
        inner: device.clone(),
    };
    let mut image = ptr::null_mut();
    unsafe {
        assert_eq!(
            ogpu_image_check_support(&handle, &desc, ptr::null_mut()),
            SUCCESS
        );
        assert_eq!(
            ogpu_image_create(&mut handle, &desc, &mut image, ptr::null_mut()),
            SUCCESS
        );
        Box::from_raw(image)
    }
}

unsafe fn finish(batch: &mut OgpuBatch) {
    unsafe {
        let mut completion = ptr::null_mut();
        assert_eq!(
            ogpu_batch_submit(batch, &mut completion, ptr::null_mut()),
            SUCCESS
        );
        assert_eq!(ogpu_completion_wait(completion, ptr::null_mut()), SUCCESS);
        ogpu_completion_destroy(completion);
    }
}

fn read(buffer: &Buffer, size: usize) -> Vec<u8> {
    let mut result = vec![0; size];
    unsafe {
        buffer.read(0, result.as_mut_ptr(), result.len()).unwrap();
    }
    result
}

#[test]
fn image_null_arguments_need_no_driver() {
    unsafe {
        let mut caps = crate::OgpuCapabilities {
            graphics_queue: 42,
            ..Default::default()
        };
        let before = caps;
        assert_eq!(
            ogpu_device_capabilities(ptr::null(), &mut caps, ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(caps, before);
        assert_eq!(
            ogpu_device_capabilities(ptr::dangling(), ptr::null_mut(), ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_image_check_support(ptr::null(), ptr::dangling(), ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_image_check_support(ptr::dangling(), ptr::null(), ptr::null_mut()),
            INVALID_ARGUMENT
        );
        let mut image = ptr::dangling_mut();
        assert_eq!(
            ogpu_image_create(ptr::null_mut(), ptr::null(), &mut image, ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert!(image.is_null());
        assert_eq!(
            ogpu_image_create(
                ptr::dangling_mut(),
                ptr::null(),
                &mut image,
                ptr::null_mut()
            ),
            INVALID_ARGUMENT
        );
        assert!(image.is_null());
        assert_eq!(
            ogpu_batch_copy_buffer_to_image(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            INVALID_ARGUMENT
        );
    }
}

#[test]
#[ignore = "requires graphics Vulkan; image descriptions, copies, usages and retained ownership"]
fn gpu_image_transfers() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let foreign = Device::new_graphics(instance.clone(), physical).unwrap();
        for dimension in [1, 2] {
            for format in [0, 1, 2, 3] {
                for placement in [Placement::Host, Placement::Device] {
                    let (width, height) = (17, if dimension == 1 { 1 } else { 7 });
                    let size = width as usize * height as usize * if format >= 2 { 8 } else { 4 };
                    let desc = OgpuImageDesc {
                        dimension,
                        width,
                        height,
                        format,
                        usage: COPY_SRC | COPY_DST,
                        reserved: 0,
                    };
                    let mut image = unsafe { make_image(&device, desc) };
                    let image_weak = Rc::downgrade(&image.inner);
                    let mut source = OgpuBuffer {
                        inner: Rc::new(
                            Buffer::placed(device.clone(), size + 16, placement).unwrap(),
                        ),
                    };
                    let source_weak = Rc::downgrade(&source.inner);
                    let staging = Rc::new(Buffer::new(device.clone(), size + 16).unwrap());
                    let mut destination = OgpuBuffer {
                        inner: Rc::new(Buffer::new(device.clone(), size + 16).unwrap()),
                    };
                    let mut batch = OgpuBatch {
                        inner: Batch::new(device.clone()).unwrap(),
                    };
                    let mut foreign_image = unsafe { make_image(&foreign, desc) };
                    let mut no_upload = unsafe {
                        make_image(
                            &device,
                            OgpuImageDesc {
                                usage: COPY_SRC,
                                ..desc
                            },
                        )
                    };
                    let mut no_readback = unsafe {
                        make_image(
                            &device,
                            OgpuImageDesc {
                                usage: COPY_DST,
                                ..desc
                            },
                        )
                    };
                    unsafe {
                        for offset in [1, 20, u64::MAX] {
                            assert_ne!(
                                ogpu_batch_copy_buffer_to_image(
                                    &mut batch,
                                    &mut source,
                                    offset,
                                    &mut *image,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            assert_ne!(
                                ogpu_batch_copy_image_to_buffer(
                                    &mut batch,
                                    &mut *image,
                                    &mut destination,
                                    offset,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                        }
                        for invalid in [&mut *foreign_image, &mut *no_upload] {
                            assert_eq!(
                                ogpu_batch_copy_buffer_to_image(
                                    &mut batch,
                                    &mut source,
                                    8,
                                    invalid,
                                    ptr::null_mut()
                                ),
                                INVALID_ARGUMENT
                            );
                        }
                        assert_eq!(
                            ogpu_batch_copy_image_to_buffer(
                                &mut batch,
                                &mut *no_readback,
                                &mut destination,
                                8,
                                ptr::null_mut()
                            ),
                            INVALID_ARGUMENT
                        );
                    }
                    // Unsubmitted copies retain, then release, both operands and do no work.
                    batch.inner.discard_image(image.inner.clone()).unwrap();
                    batch
                        .inner
                        .copy_buffer_to_image(source.inner.clone(), 8, image.inner.clone())
                        .unwrap();
                    assert_eq!(Rc::strong_count(&source.inner), 2);
                    drop(batch);
                    assert_eq!(Rc::strong_count(&source.inner), 1);
                    for pass in 0..3u32 {
                        let mut input = vec![0x9b; size + 16];
                        for (i, texel) in input[8..8 + size].chunks_exact_mut(4).enumerate() {
                            let bits = if format == 0 {
                                (i as u32 * 7919).wrapping_add(pass % 2 * 101)
                            } else {
                                (i as f32 * 0.25 - 37.5 + (pass % 2) as f32).to_bits()
                            };
                            texel.copy_from_slice(&bits.to_ne_bytes());
                        }
                        staging.write(0, &input).unwrap();
                        destination.inner.write(0, &vec![0xcd; size + 16]).unwrap();
                        let mut batch = OgpuBatch {
                            inner: Batch::new(device.clone()).unwrap(),
                        };
                        // Exercise both HOST upload and an explicit transfer into DEVICE memory.
                        if matches!(placement, Placement::Host) {
                            source.inner.write(0, &input).unwrap();
                        } else {
                            batch
                                .inner
                                .copy_buffer(staging.clone(), 0, source.inner.clone(), 0, size + 16)
                                .unwrap();
                        }
                        unsafe {
                            if pass == 0 {
                                assert_eq!(
                                    ogpu_batch_discard_image(&mut batch, &*image, ptr::null_mut()),
                                    SUCCESS
                                );
                            }
                            assert_eq!(
                                ogpu_batch_copy_buffer_to_image(
                                    &mut batch,
                                    &mut source,
                                    8,
                                    &mut *image,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            assert_eq!(
                                ogpu_batch_copy_image_to_buffer(
                                    &mut batch,
                                    &mut *image,
                                    &mut destination,
                                    8,
                                    ptr::null_mut()
                                ),
                                SUCCESS
                            );
                            finish(&mut batch);
                        }
                        let output = read(&destination.inner, size + 16);
                        assert_eq!(&output[..8], &[0xcd; 8]);
                        assert_eq!(&output[8..8 + size], &input[8..8 + size]);
                        assert_eq!(&output[8 + size..], &[0xcd; 8]);
                    }
                    // A submitted copy keeps destroyed public handles alive until completion cleanup.
                    let mut batch = OgpuBatch {
                        inner: Batch::new(device.clone()).unwrap(),
                    };
                    batch
                        .inner
                        .copy_buffer_to_image(source.inner.clone(), 8, image.inner.clone())
                        .unwrap();
                    drop(source);
                    drop(image);
                    let mut completion = unsafe { batch.inner.submit().unwrap() };
                    drop(batch);
                    assert!(source_weak.upgrade().is_some() && image_weak.upgrade().is_some());
                    completion.wait().unwrap();
                    assert!(source_weak.upgrade().is_none() && image_weak.upgrade().is_none());
                    assert!(completion.poll().unwrap());
                    drop(completion);
                }
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}

#[test]
#[ignore = "requires graphics Vulkan; native 1D/2D R32F nearest/linear sampling and storage"]
fn gpu_float_image_sampling() {
    float_image_sampling(true);
}

#[test]
#[ignore = "requires compute Vulkan; images/heaps without enabled rasterization"]
fn gpu_compute_image_sampling() {
    float_image_sampling(false);
}

fn float_image_sampling(graphics: bool) {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/image-float.comp.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let probe = OgpuProbe {
            devices: vec![instance.device_info(physical).unwrap()],
            physical_devices: vec![physical],
            _vulkan: instance.clone(),
        };
        let mut raw = ptr::null_mut();
        let create = if graphics {
            ogpu_device_create_graphics
        } else {
            ogpu_device_create
        };
        let status = unsafe { create(&probe, 0, &mut raw, ptr::null_mut()) };
        if status == UNSUPPORTED {
            continue;
        }
        assert_eq!(status, SUCCESS);
        let mut handle = unsafe { Box::from_raw(raw) };
        let device = handle.inner.clone();
        let mut caps = crate::OgpuCapabilities::default();
        unsafe {
            assert_eq!(
                ogpu_device_capabilities(&*handle, &mut caps, ptr::null_mut()),
                SUCCESS
            );
        }
        assert_eq!(
            caps,
            crate::OgpuCapabilities {
                graphics_queue: u32::from(graphics),
                compute_queue: 1,
                buffer_device_address: 1,
                timeline_semaphore: 1,
                synchronization2: 1,
                descriptor_heap: 1,
                device_address_commands: 1,
                shader_untyped_pointers: 1,
                storage_buffer_16bit_access: 1,
                ..Default::default()
            }
        );
        // Query and creation share validation, including the execution profile.
        let color = OgpuImageDesc {
            dimension: 2,
            width: 2,
            height: 2,
            format: 0,
            usage: 4,
            reserved: 0,
        };
        for (desc, expected) in [
            (
                OgpuImageDesc {
                    reserved: 1,
                    ..color
                },
                INVALID_ARGUMENT,
            ),
            (OgpuImageDesc { width: 0, ..color }, INVALID_ARGUMENT),
            (color, if graphics { SUCCESS } else { UNSUPPORTED }),
        ] {
            unsafe {
                assert_eq!(
                    ogpu_image_check_support(&*handle, &desc, ptr::null_mut()),
                    expected
                );
                let mut image = ptr::dangling_mut();
                assert_eq!(
                    ogpu_image_create(&mut *handle, &desc, &mut image, ptr::null_mut()),
                    expected
                );
                if expected == SUCCESS {
                    ogpu_image_destroy(image);
                } else {
                    assert!(image.is_null());
                }
            }
        }
        let mut kernel = OgpuKernel {
            inner: Rc::new(unsafe { Kernel::new(device.clone(), &words, 12, &[]).unwrap() }),
        };
        for dimension in [1u32, 2] {
            let (width, height) = if dimension == 1 {
                (256u32, 1u32)
            } else {
                (17u32, 7u32)
            };
            let mut source_image = unsafe {
                make_image(
                    &device,
                    OgpuImageDesc {
                        dimension,
                        width,
                        height,
                        format: 1,
                        usage: SAMPLED | COPY_DST,
                        reserved: 0,
                    },
                )
            };
            let mut output_image = unsafe {
                make_image(
                    &device,
                    OgpuImageDesc {
                        dimension: 2,
                        width: width * 2,
                        height,
                        format: 1,
                        usage: STORAGE | COPY_SRC,
                        reserved: 0,
                    },
                )
            };
            let mut images = OgpuImageHeap {
                inner: Rc::new(ImageHeap::new(device.clone(), 2).unwrap()),
            };
            let mut samplers = OgpuSamplerHeap {
                inner: Rc::new(SamplerHeap::new(device.clone(), 2).unwrap()),
            };
            let entries = [
                OgpuImageEntry {
                    image: &*source_image,
                    kind: 0,
                    reserved: 0,
                },
                OgpuImageEntry {
                    image: &*output_image,
                    kind: 1,
                    reserved: 0,
                },
            ];
            let filters = [
                OgpuSamplerDesc::default(),
                OgpuSamplerDesc {
                    min_filter: 1,
                    mag_filter: 1,
                    ..OgpuSamplerDesc::default()
                },
            ];
            unsafe {
                assert_eq!(
                    ogpu_image_heap_write(&mut images, 0, entries.as_ptr(), 2, ptr::null_mut()),
                    SUCCESS
                );
                assert_eq!(
                    ogpu_sampler_heap_write(&mut samplers, 0, filters.as_ptr(), 2, ptr::null_mut()),
                    SUCCESS
                );
            }
            // Wrong-use descriptors reject atomically before replacing valid slots.
            let invalid = [OgpuImageEntry {
                image: &*output_image,
                kind: 0,
                reserved: 0,
            }];
            unsafe {
                assert_eq!(
                    ogpu_image_heap_write(&mut images, 0, invalid.as_ptr(), 1, ptr::null_mut()),
                    INVALID_ARGUMENT
                );
            }
            let mut input = OgpuBuffer {
                inner: Rc::new(Buffer::new(device.clone(), (width * height * 4) as usize).unwrap()),
            };
            let mut output = OgpuBuffer {
                inner: Rc::new(Buffer::new(device.clone(), (width * height * 8) as usize).unwrap()),
            };
            for pass in 0..3 {
                let value = |x: u32, y: u32| {
                    x as f32 * 4.0 + y as f32 * 16.0 - 80.0 + (pass % 2) as f32 * 8.0
                };
                let bytes: Vec<u8> = (0..height)
                    .flat_map(|y| (0..width).flat_map(move |x| value(x, y).to_ne_bytes()))
                    .collect();
                input.inner.write(0, &bytes).unwrap();
                let mut batch = OgpuBatch {
                    inner: Batch::new(device.clone()).unwrap(),
                };
                let root: Vec<u8> = [dimension, width, height]
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect();
                unsafe {
                    if pass == 0 {
                        assert_eq!(
                            ogpu_batch_discard_image(&mut batch, &*source_image, ptr::null_mut()),
                            SUCCESS
                        );
                        assert_eq!(
                            ogpu_batch_discard_image(&mut batch, &*output_image, ptr::null_mut()),
                            SUCCESS
                        );
                    }
                    assert_eq!(
                        ogpu_batch_copy_buffer_to_image(
                            &mut batch,
                            &mut input,
                            0,
                            &mut *source_image,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                    // Includes last pass's readback before this pass's storage writes.
                    assert_eq!(
                        ogpu_batch_barrier(&mut batch, 32 | 64, 1 | 2, ptr::null_mut()),
                        SUCCESS
                    );
                    assert_eq!(
                        ogpu_batch_bind_image_heap(&mut batch, &images, ptr::null_mut()),
                        SUCCESS
                    );
                    assert_eq!(
                        ogpu_batch_bind_sampler_heap(&mut batch, &samplers, ptr::null_mut()),
                        SUCCESS
                    );
                    assert_eq!(
                        ogpu_batch_dispatch(
                            &mut batch,
                            &mut kernel,
                            width.div_ceil(8),
                            height.div_ceil(4),
                            1,
                            root.as_ptr().cast(),
                            12,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                    assert_eq!(
                        ogpu_batch_copy_image_to_buffer(
                            &mut batch,
                            &mut *output_image,
                            &mut output,
                            0,
                            ptr::null_mut()
                        ),
                        SUCCESS
                    );
                    finish(&mut batch);
                }
                let result = read(&output.inner, (width * height * 8) as usize);
                for y in 0..height {
                    for x in 0..width {
                        for filter in 0..2 {
                            let i = ((y * width * 2 + x + filter * width) * 4) as usize;
                            let actual = f32::from_ne_bytes(result[i..i + 4].try_into().unwrap());
                            let expected = value(x, y)
                                + if filter == 1 {
                                    (if x + 1 < width { 1.0 } else { 0.0 })
                                        + (if y + 1 < height { 4.0 } else { 0.0 })
                                } else {
                                    0.0
                                };
                            assert!((actual - expected).abs() < 0.0001, "dimension={dimension} pass={pass} pos={x},{y} filter={filter}: {actual} != {expected}");
                        }
                    }
                }
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}
