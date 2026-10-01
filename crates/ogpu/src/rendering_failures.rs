//! Indexed/depth resources through the real-driver storage failure injector.
use super::*;

#[test]
#[ignore = "requires graphics Vulkan; indexed preparation, submission, replay and drained synthetic loss"]
fn gpu_indexed_rendering_failures() {
    let instance = Arc::new(Instance::new().unwrap());
    let words = |bytes: &[u8]| {
        bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>()
    };
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        match Device::new_graphics(instance.clone(), physical) {
            Ok(_) => {}
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        }
        for explicit in [false, true] {
            for compiled in [false, true] {
                for failure in if compiled { 3..=6 } else { 1..=6 } {
                    let d = Device::create_configured(instance.clone(), physical, true, configure)
                        .unwrap();
                    let owner = Rc::new(RecordingStorage::new(d.clone()).unwrap());
                    let new_batch = || {
                        if explicit {
                            Batch::new_in(owner.clone()).unwrap()
                        } else {
                            Batch::new(d.clone()).unwrap()
                        }
                    };
                    unsafe { new_batch().submit().unwrap() }.wait().unwrap();
                    assert_eq!(COUNTS.get(), [1, 0, 1]);
                    let color = Rc::new(
                        Image::new(
                            d.clone(),
                            ImageDesc {
                                dimension: 2,
                                width: 32,
                                height: 32,
                                format: 0,
                                usage: 4 | 8,
                                reserved: 0,
                            },
                        )
                        .unwrap(),
                    );
                    let depth = Rc::new(
                        Image::new(
                            d.clone(),
                            ImageDesc {
                                dimension: 2,
                                width: 32,
                                height: 32,
                                format: 4,
                                usage: 32 | 8,
                                reserved: 0,
                            },
                        )
                        .unwrap(),
                    );
                    let vertices = Rc::new(Buffer::new(d.clone(), 48).unwrap());
                    let indices = Rc::new(
                        Buffer::with_usage(
                            d.clone(),
                            12,
                            vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_INDEX_BUFFER_BIT,
                        )
                        .unwrap(),
                    );
                    let indirect = Rc::new(Buffer::new(d.clone(), 20).unwrap());
                    let output = Rc::new(Buffer::new(d.clone(), 8192).unwrap());
                    let positions: Vec<u8> = [
                        -1.0f32, -1.0, 0.5, 1.0, 3.0, -1.0, 0.5, 1.0, -1.0, 3.0, 0.5, 1.0,
                    ]
                    .into_iter()
                    .flat_map(f32::to_ne_bytes)
                    .collect();
                    vertices.write(0, &positions).unwrap();
                    indices
                        .write(
                            0,
                            &[0u32, 1, 2]
                                .into_iter()
                                .flat_map(u32::to_ne_bytes)
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                    indirect
                        .write(
                            0,
                            &[3u32, 1, 0, 0, 0]
                                .into_iter()
                                .flat_map(u32::to_ne_bytes)
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                    let root = vertices.address().unwrap().to_ne_bytes();
                    let count = Rc::new(Buffer::new(d.clone(), 4).unwrap());
                    count.write(0, &1u32.to_ne_bytes()).unwrap();
                    let raster = Rc::new(unsafe {
                        Raster::new(
                            d.clone(),
                            &vertex,
                            &fragment,
                            crate::OgpuRasterDesc {
                                push_size_bytes: 8,
                                topology: 0,
                                color_format: 0,
                                depth_format: 4,
                                depth_test: 1,
                                depth_write: 1,
                                depth_compare: 1,
                                reserved: 0,
                            },
                            [&[], &[]],
                        )
                        .unwrap()
                    });
                    let images = [Rc::downgrade(&color), Rc::downgrade(&depth)];
                    let buffers = [
                        Rc::downgrade(&vertices),
                        Rc::downgrade(&indices),
                        Rc::downgrade(&indirect),
                        Rc::downgrade(&count),
                    ];
                    let pipeline = Rc::downgrade(&raster);
                    let mut batch = new_batch();
                    batch.discard_image(color.clone()).unwrap();
                    batch.discard_image(depth.clone()).unwrap();
                    batch.retain_buffer(vertices).unwrap();
                    // Internal encoder uses retained images, never these ABI pointers.
                    batch
                        .begin_rendering(
                            color.clone(),
                            Some(depth.clone()),
                            crate::OgpuRenderingDesc {
                                color: crate::OgpuColorAttachment {
                                    image: ptr::dangling_mut(),
                                    clear: [0.0, 0.0, 1.0, 1.0],
                                    ..Default::default()
                                },
                                depth: crate::OgpuDepthAttachment {
                                    image: ptr::dangling_mut(),
                                    clear: 1.0,
                                    ..Default::default()
                                },
                            },
                        )
                        .unwrap();
                    batch
                        .draw(
                            raster,
                            crate::compute::IndirectBinding {
                                buffer: indirect,
                                offset: 0,
                                stride: 20,
                                maximum: 1,
                                count: Some(count),
                                count_offset: 0,
                            },
                            &root,
                            Some(IndexBinding {
                                buffer: indices,
                                offset: 0,
                                size: 12,
                                format: 1,
                            }),
                        )
                        .unwrap();
                    batch.end_rendering().unwrap();
                    batch
                        .copy_image_to_buffer(color, output.clone(), 0)
                        .unwrap();
                    batch
                        .copy_image_to_buffer(depth, output.clone(), 4096)
                        .unwrap();
                    let check_pixels = || {
                        let mut pixels = [0u8; 8192];
                        unsafe { output.read(0, pixels.as_mut_ptr(), pixels.len()).unwrap() };
                        assert_eq!(&pixels[..4096], [255, 0, 0, 255].repeat(1024));
                        assert_eq!(&pixels[4096..], 0.5f32.to_ne_bytes().repeat(1024));
                    };
                    if compiled && failure >= 5 {
                        let list = Rc::new(unsafe { batch.compile_with_flags(0).unwrap() });
                        FAILURE.set(failure);
                        let expected = if failure == 5 {
                            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
                        } else {
                            vk::VkResult_VK_ERROR_UNKNOWN
                        };
                        assert!(matches!(unsafe { list.submit() }, Err(e) if e.vk == expected));
                        assert!(images.iter().all(|w| w.upgrade().is_some()));
                        assert!(buffers.iter().all(|w| w.upgrade().is_some()));
                        assert!(pipeline.upgrade().is_some());
                        FAILURE.set(0);
                        if failure == 5 {
                            unsafe { list.submit().unwrap() }.wait().unwrap();
                            check_pixels();
                        } else {
                            let submitted = SUBMITTED.get();
                            assert!(
                                matches!(unsafe { list.submit() }, Err(e) if e.status == INVALID_ARGUMENT)
                            );
                            assert_eq!(SUBMITTED.get(), submitted);
                        }
                        drop(list);
                    } else {
                        FAILURE.set(failure);
                        if compiled {
                            assert!(matches!(unsafe { batch.compile_with_flags(0) }, Err(e)
                                if e.vk == vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY));
                        } else if failure <= 2 {
                            let mut receipt = unsafe { batch.submit().unwrap() };
                            let result = receipt.wait();
                            if failure == 1 {
                                result.unwrap();
                                check_pixels();
                            } else {
                                // Injector fails reset only AFTER a real successful wait.
                                assert_eq!(
                                    result.unwrap_err().vk,
                                    vk::VkResult_VK_ERROR_DEVICE_LOST
                                );
                                assert!(d.lost.get());
                                assert_eq!(
                                    receipt.poll().unwrap_err().vk,
                                    vk::VkResult_VK_ERROR_DEVICE_LOST
                                );
                            }
                        } else {
                            let expected = if failure == 6 {
                                vk::VkResult_VK_ERROR_UNKNOWN
                            } else {
                                vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
                            };
                            assert!(
                                matches!(unsafe { batch.submit() }, Err(e) if e.vk == expected)
                            );
                        }
                        FAILURE.set(0);
                    }
                    // Preparation consumes the recording, even when unsuccessful.
                    assert!(batch.steps.is_none());
                    assert!(images.iter().all(|w| w.upgrade().is_none()));
                    assert!(buffers.iter().all(|w| w.upgrade().is_none()));
                    assert!(pipeline.upgrade().is_none());
                    assert!(!owner.busy.get());
                    assert_eq!(COUNTS.get()[0], 1, "warm storage reused");
                    if !(compiled && failure == 5) {
                        assert_eq!(COUNTS.get()[1], 1, "failed storage destroyed");
                        assert!(owner.idle.borrow().is_none());
                        assert!(d.command_storage.borrow().is_empty());
                    }
                    if failure != 2 {
                        unsafe { new_batch().submit().unwrap() }.wait().unwrap();
                    } else {
                        assert!(
                            matches!(Batch::new(d.clone()), Err(e) if e.vk == vk::VkResult_VK_ERROR_DEVICE_LOST)
                        );
                    }
                    drop(batch);
                    owner.trim().unwrap();
                    drop((owner, output, d));
                    assert_eq!(COUNTS.get()[0], COUNTS.get()[1]);
                }
            }
        }
        tested += 1;
    }
    assert!(tested > 0);
}
