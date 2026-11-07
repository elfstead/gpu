use super::*;
use crate::foundation::{
    ogpu_next_bind_heap, ogpu_next_copy_descriptors, ogpu_next_heap_create, ogpu_next_heap_info,
    ogpu_next_resource_descriptor_shape, rust as api,
};
use std::cell::Cell;

fn host(data: &mut [u8]) -> HostSpan {
    HostSpan {
        data: data.as_mut_ptr().cast(),
        size: data.len() as u64,
    }
}
fn empty() -> HostSpan {
    HostSpan {
        data: ptr::null_mut(),
        size: 0,
    }
}
fn span(memory: &Memory, offset: u64, size: u64) -> Span {
    Span {
        memory: ptr::from_ref(memory).cast_mut(),
        offset,
        size,
    }
}
fn request(kind: u32, storage: u32, usage: u32, capacity: u64) -> HeapRequest {
    HeapRequest {
        kind,
        storage,
        usage,
        flags: 0,
        capacity,
    }
}
fn host_desc(data: &mut [u8], kind: u32, usage: u32) -> HeapDesc {
    HeapDesc {
        request: request(kind, HEAP_HOST, usage, data.len() as u64),
        memory: Span {
            memory: ptr::null_mut(),
            offset: 0,
            size: 0,
        },
        host: host(data),
        reserved_offset: 0,
        reserved_size: 0,
    }
}
fn location(heap: &DescriptorHeap, offset: u64) -> HeapLocation {
    HeapLocation {
        heap: ptr::from_ref(heap).cast_mut(),
        offset,
    }
}
fn scratch_vec(req: HostRequirements) -> Vec<u64> {
    assert!(req.alignment <= align_of::<u64>() as u64);
    vec![0; req.size.div_ceil(8) as usize]
}
fn scratch_span(words: &mut [u64]) -> HostSpan {
    HostSpan {
        data: words.as_mut_ptr().cast(),
        size: size_of_val(words) as u64,
    }
}
fn sampler_desc() -> SamplerDesc {
    SamplerDesc {
        header: Record::new::<SamplerDesc>(SAMPLER_DESC),
        min_filter: 0,
        mag_filter: 0,
        mip_filter: 0,
        address_mode: [2; 3],
        compare_enable: 0,
        compare_op: 0,
        border_color: 0,
        unnormalized_coordinates: 0,
        min_lod: 0.0,
        max_lod: 0.0,
        lod_bias: 0.0,
        max_anisotropy: 1.0,
    }
}

#[test]
fn storage_requirements_locations_and_native_geometry() {
    let p = NativeDescriptorProperties {
        buffer_size: 16,
        buffer_alignment: 16,
        image_size: 32,
        image_alignment: 32,
        sampler_size: 16,
        sampler_alignment: 16,
        resource_heap_alignment: 64,
        resource_heap_max_size: 4096,
        resource_reserved_size: 64,
        resource_reserved_alignment: 32,
        sampler_heap_alignment: 32,
        sampler_heap_max_size: 1024,
        sampler_reserved_size: 32,
        sampler_reserved_alignment: 16,
        ..Default::default()
    };
    assert_eq!(
        resource_shape(&p, 3).unwrap(),
        DescriptorShape {
            size: 16,
            alignment: 16,
            array_stride: 16
        }
    );
    assert_eq!(resource_shape(&p, 1).unwrap().array_stride, 32);
    for kind in [1, 2] {
        let mut r = request(kind, HEAP_HOST, HEAP_CPU_WRITE, 8192);
        let req = requirements(&p, &r).unwrap();
        assert_eq!(
            (
                req.unit_bytes,
                req.backing_size,
                req.memory_usage,
                req.reserved_size
            ),
            (1, 8192, 0, 0)
        );
        r.usage |= HEAP_SHADER_VISIBLE;
        assert_eq!(requirements(&p, &r), Err(INVALID));
        r.storage = HEAP_MEMORY;
        assert_eq!(requirements(&p, &r), Err(UNSUPPORTED));
        r.capacity = 512;
        let req = requirements(&p, &r).unwrap();
        assert_eq!(req.memory_usage, 128);
        assert!(req.reserved_size > 0);
        r.storage = HEAP_NATIVE;
        assert_eq!(requirements(&p, &r), Err(UNSUPPORTED));
        r.storage = HEAP_HOST;
        r.usage = 16;
        assert_eq!(requirements(&p, &r), Err(UNSUPPORTED));
        r.usage = 0;
        r.capacity = 0;
        assert_eq!(requirements(&p, &r), Err(INVALID));
    }
    let mut bytes = [0; 128];
    let mut d = host_desc(&mut bytes, 1, 14);
    d.reserved_offset = 32;
    d.reserved_size = 32;
    assert_eq!(range(&d, 0, 32), Ok(()));
    assert_eq!(range(&d, 64, 64), Ok(()));
    assert_eq!(range(&d, 31, 2), Err(INVALID));
    assert_eq!(range(&d, 32, 0), Ok(()));
    assert_eq!(range(&d, 128, 0), Ok(()));
    assert_eq!(range(&d, 128, 1), Err(CAPACITY));
    assert_eq!(range(&d, u64::MAX, 2), Err(INVALID));
    assert_eq!(copy_layout(0).unwrap().0.size(), 0);
    assert!(copy_layout(u32::MAX).is_err());
}

fn memory(device: &Device, size: u64, alignment: u64, usage: u64) -> Box<Memory> {
    let mut desc = MemoryDesc {
        header: Record::new::<MemoryDesc>(MEMORY_DESC),
        size,
        alignment,
        usage,
        memory_type: u32::MAX,
        kind: 1,
        concurrent_domains: ptr::null(),
        concurrent_domain_count: 0,
        flags: 0,
    };
    let mut req = Requirements::default();
    unsafe { device.memory_requirements(&desc, &mut req) }.unwrap();
    let mut compatible = vec![0; req.compatible_type_count as usize];
    req.compatible_types = compatible.as_mut_ptr();
    req.compatible_type_capacity = compatible.len() as u32;
    unsafe { device.memory_requirements(&desc, &mut req) }.unwrap();
    desc.memory_type = *compatible
        .iter()
        .find(|id| {
            device
                .snapshot
                .memory_types
                .iter()
                .any(|t| t.id == **id && t.properties & 2 != 0)
        })
        .unwrap();
    unsafe { Memory::create(device, &desc) }.unwrap()
}
thread_local! {
    static WRITE: Cell<vk::PFN_vkWriteResourceDescriptorsEXT> = const { Cell::new(None) };
    static BINDS: Cell<u32> = const { Cell::new(0) };
    static WRITES: Cell<u32> = const { Cell::new(0) };
    static BIND: Cell<vk::PFN_vkCmdBindResourceHeapEXT> = const { Cell::new(None) };
}
unsafe extern "C" fn write_trace(
    d: vk::VkDevice,
    n: u32,
    inputs: *const vk::VkResourceDescriptorInfoEXT,
    outputs: *const vk::VkHostAddressRangeEXT,
) -> vk::VkResult {
    WRITES.set(WRITES.get() + 1);
    unsafe { WRITE.get().unwrap()(d, n, inputs, outputs) }
}
unsafe extern "C" fn bind_trace(c: vk::VkCommandBuffer, info: *const vk::VkBindHeapInfoEXT) {
    BINDS.set(BINDS.get() + 1);
    unsafe { BIND.get().unwrap()(c, info) }
}

#[test]
#[ignore = "requires modern Vulkan; explicit descriptor storage, atomic preflight, alias rejection and changed-entry replay"]
fn gpu_descriptor_storage_and_replay() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let snapshot = snapshot(&instance, physical).unwrap();
        if snapshot.features.baseline_supported == 0 {
            continue;
        }
        let domain = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
        let adapter = Adapter {
            instance: instance.clone(),
            physical,
            snapshot,
        };
        let mut device = Device::create(
            &adapter,
            &[QueueRequest {
                domain,
                count: 1,
                priority: 0.5,
            }],
            FeatureBits::ZERO,
        )
        .unwrap();
        WRITE.set(device.f.vkWriteResourceDescriptorsEXT);
        WRITES.set(0);
        device.f.vkWriteResourceDescriptorsEXT = Some(write_trace);
        BIND.set(device.f.vkCmdBindResourceHeapEXT);
        BINDS.set(0);
        device.f.vkCmdBindResourceHeapEXT = Some(bind_trace);
        let p = &device.snapshot.descriptor_properties;
        let stride = p.storage_address_alignment.max(256);
        let data = memory(&device, stride * 3, stride, 64);
        let map = data.map(0, stride * 3).unwrap();
        let words = unsafe {
            std::slice::from_raw_parts_mut(map.data.cast::<u32>(), (stride * 3 / 4) as usize)
        };
        words.fill(0xa5a5a5a5);
        data.cache(0, stride * 3, true).unwrap();
        let mut resource = ResourceDescriptor {
            kind: 3,
            image_state: 0,
            view: ptr::null_mut(),
            buffer: span(&data, 0, 4),
        };
        let device_pointer = ptr::from_ref(&*device).cast_mut();
        let device_handle = unsafe { api::DeviceHandle::from_raw(device_pointer) }.unwrap();
        let mut shape = DescriptorShape {
            size: 0,
            alignment: 0,
            array_stride: 0,
        };
        assert_eq!(
            unsafe { ogpu_next_resource_descriptor_shape(device_pointer, &resource, &mut shape) },
            OK
        );
        assert_eq!(
            unsafe { api::resource_descriptor_shape(device_handle, &resource) }.unwrap(),
            shape
        );
        let saved_kind = resource.kind;
        resource.kind = u32::MAX;
        let saved_shape = shape;
        assert_eq!(
            unsafe { ogpu_next_resource_descriptor_shape(device_pointer, &resource, &mut shape) },
            UNSUPPORTED
        );
        assert_eq!(shape, saved_shape);
        resource.kind = saved_kind;
        assert_eq!(shape.array_stride, p.buffer_size);
        let reserved_offset = (shape.size * 4).div_ceil(p.resource_reserved_alignment)
            * p.resource_reserved_alignment;
        let capacity = reserved_offset + p.resource_reserved_size;
        let request = request(1, HEAP_MEMORY, 15, capacity);
        let req = device.heap_requirements(&request).unwrap();
        // The extra allocation bytes must not enlarge the bound heap.
        let backing = memory(
            &device,
            req.backing_size + 256,
            req.backing_alignment,
            req.memory_usage,
        );
        let mapping = backing.map(0, req.backing_size + 256).unwrap();
        let desc = HeapDesc {
            request,
            memory: span(&backing, 0, req.backing_size + 256),
            host: HostSpan {
                data: mapping.data,
                size: mapping.size,
            },
            reserved_offset,
            reserved_size: req.reserved_size,
        };
        let mut invalid = desc;
        invalid.host.data = map.data;
        assert!(matches!(
            unsafe { DescriptorHeap::create(&device, &invalid) },
            Err(INVALID)
        ));
        invalid = desc;
        invalid.host.size += 1;
        assert!(matches!(
            unsafe { DescriptorHeap::create(&device, &invalid) },
            Err(INVALID)
        ));
        invalid = desc;
        invalid.reserved_offset += 1;
        assert!(unsafe { DescriptorHeap::create(&device, &invalid) }.is_err());
        invalid = desc;
        invalid.host = empty();
        assert!(unsafe { DescriptorHeap::create(&device, &invalid) }.is_err());
        let mut heap_pointer = ptr::null_mut();
        assert_eq!(
            unsafe { ogpu_next_heap_create(device_pointer, &desc, &mut heap_pointer) },
            OK
        );
        let heap = unsafe { Box::from_raw(heap_pointer) };
        let handle = unsafe { api::DescriptorHeapHandle::from_raw(heap_pointer) }.unwrap();
        let mut info = std::mem::MaybeUninit::uninit();
        let mut requirements = std::mem::MaybeUninit::uninit();
        unsafe {
            ogpu_next_heap_info(heap_pointer, info.as_mut_ptr(), requirements.as_mut_ptr());
        }
        assert_eq!(unsafe { requirements.assume_init() }, req);
        assert_eq!(unsafe { info.assume_init() }.request.capacity, capacity);
        assert_eq!(unsafe { api::heap_info(handle) }.1, req);
        assert_eq!(
            unsafe { heap.bind_info(&device, domain) }
                .unwrap()
                .heapRange
                .size,
            capacity
        );
        let mut staging = vec![0x5a; shape.size as usize * 4];
        let staging_desc = host_desc(&mut staging, 1, 14);
        let source = unsafe { DescriptorHeap::create(&device, &staging_desc) }.unwrap();
        assert!(unsafe { source.bind_info(&device, domain) }.is_err());
        let mut temporary = scratch_vec(
            device
                .heap_descriptor_scratch(DESCRIPTOR_WRITE_RESOURCE, 2)
                .unwrap(),
        );
        // Late destination rejection cannot encode even the first descriptor.
        let inputs = [
            ResourceDescriptor {
                buffer: resource.buffer,
                ..resource
            },
            ResourceDescriptor {
                buffer: resource.buffer,
                ..resource
            },
        ];
        let outputs = [
            location(&source, 0),
            location(&source, staging_desc.request.capacity),
        ];
        assert_eq!(
            unsafe {
                device.write_resource_locations(
                    2,
                    inputs.as_ptr(),
                    outputs.as_ptr(),
                    scratch_span(&mut temporary),
                )
            },
            Err(CAPACITY)
        );
        assert_eq!(WRITES.get(), 0);
        assert!(staging.iter().all(|v| *v == 0x5a));
        let mut copy_scratch =
            scratch_vec(device.heap_descriptor_scratch(DESCRIPTOR_COPY, 2).unwrap());
        let copy = DescriptorCopy {
            source: location(&source, 0),
            destination: location(&heap, shape.array_stride),
            size: shape.size,
        };
        // Alias checks cross wrapper identity, and preflight the whole copy batch.
        let alias = unsafe { DescriptorHeap::create(&device, &staging_desc) }.unwrap();
        let bad = DescriptorCopy {
            source: location(&source, 0),
            destination: location(&alias, 0),
            size: shape.size,
        };
        assert_eq!(
            unsafe { device.copy_descriptors(1, &bad, scratch_span(&mut copy_scratch)) },
            Err(INVALID)
        );
        let late_bad = [
            DescriptorCopy {
                destination: location(&source, shape.size),
                ..bad
            },
            bad,
        ];
        assert_eq!(
            unsafe {
                device.copy_descriptors(2, late_bad.as_ptr(), scratch_span(&mut copy_scratch))
            },
            Err(INVALID)
        );
        assert!(staging.iter().all(|v| *v == 0x5a));
        let forbidden = DescriptorCopy {
            destination: location(&heap, reserved_offset),
            ..copy
        };
        if req.reserved_size != 0 {
            assert_eq!(
                unsafe { device.copy_descriptors(1, &forbidden, scratch_span(&mut copy_scratch)) },
                Err(INVALID)
            );
        }
        let mut restricted = staging_desc;
        restricted.request.usage = HEAP_CPU_WRITE;
        let write_only = unsafe { DescriptorHeap::create(&device, &restricted) }.unwrap();
        let forbidden = DescriptorCopy {
            source: location(&write_only, 0),
            ..copy
        };
        assert_eq!(
            unsafe { device.copy_descriptors(1, &forbidden, scratch_span(&mut copy_scratch)) },
            Err(INVALID)
        );
        assert_eq!(
            unsafe { device.copy_descriptors(0, ptr::null(), empty()) },
            Ok(())
        );
        // Exercise the independent sampler location encoder against actual native bytes.
        let sampler = sampler_desc();
        let sampler_shape = device.sampler_descriptor_shape(&sampler).unwrap();
        let mut sampler_bytes = vec![0; sampler_shape.size as usize];
        let sampler_heap = unsafe {
            DescriptorHeap::create(&device, &host_desc(&mut sampler_bytes, 2, HEAP_CPU_WRITE))
        }
        .unwrap();
        let mut sampler_scratch = scratch_vec(
            device
                .heap_descriptor_scratch(DESCRIPTOR_WRITE_SAMPLER, 1)
                .unwrap(),
        );
        unsafe {
            device.write_sampler_locations(
                1,
                &sampler,
                &location(&sampler_heap, 0),
                scratch_span(&mut sampler_scratch),
            )
        }
        .unwrap();

        let field = ArgumentField {
            offset: 0,
            size: 8,
            kind: ARGUMENT_INLINE,
            address_alignment: 0,
            stages: 4,
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
        };
        let arguments = unsafe {
            ArgumentLayout::create(
                &device,
                &ArgumentLayoutDesc {
                    execution_class: ARGUMENT_COMPUTE,
                    byte_size: 8,
                    field_count: 1,
                    flags: 0,
                    fields: &field,
                },
            )
        }
        .unwrap();
        let code: Vec<u32> =
            include_bytes!("../../../../examples/shaders/foundation-heap-storage.comp.spv")
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
        let shader = Shader {
            stage: 4,
            format: 0,
            code: Bytes {
                data: code.as_ptr().cast(),
                size: code.len() * 4,
            },
            entry: c"writeHeap".as_ptr(),
            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        let requirements = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: Features::default(),
            local_size: [1; 3],
            shared_memory: 0,
        };
        let executable = unsafe {
            Executable::create(
                &device,
                &ExecutableDesc {
                    header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                    kind: 1,
                    shader_count: 1,
                    shaders: &shader,
                    static_state: ptr::null(),
                    dynamic_state: 0,
                    requirements: &requirements.header,
                    cache: ptr::null_mut(),
                    compile_flags: 0,
                    reserved: 0,
                },
                &arguments,
            )
        }
        .unwrap();
        let mut arena = Arena::create(
            &device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )
        .unwrap();
        let list = arena
            .begin(&RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 2,
                level: 0,
                inheritance: ptr::null(),
            })
            .unwrap();
        let encoder = unsafe { &*list };
        unsafe {
            ogpu_next_bind_heap(list, heap_pointer);
        }
        encoder.bind_executable(&executable).unwrap();
        let values = [1u32, 0x12345678];
        unsafe { encoder.set_arguments(&arguments, 0, 8, values.as_ptr().cast()) }.unwrap();
        encoder
            .dispatch(&Launch {
                groups: Extent { x: 1, y: 1, z: 1 },
                dynamic_shared_bytes: 0,
                extensions: ptr::null(),
            })
            .unwrap();
        unsafe {
            encoder.barrier(
                &Dependency {
                    global_count: 1,
                    global: &BarrierScope {
                        stages_before: 4,
                        stages_after: 256,
                        access_before: 32,
                        access_after: 64,
                    },
                    ..Default::default()
                },
                HostSpan {
                    data: [0u64; 64].as_mut_ptr().cast(),
                    size: 512,
                },
            )
        }
        .unwrap();
        encoder.end().unwrap();
        assert_eq!(BINDS.get(), 1);
        assert_eq!(WRITES.get(), 0);
        let timeline = Timeline::create(&device, 0).unwrap();
        let mut submission = scratch_vec(
            submit_scratch(
                &device,
                &[SubmitDesc {
                    list_count: 1,
                    signal_count: 1,
                    ..Default::default()
                }],
            )
            .unwrap(),
        );
        for replay in 1..=2 {
            // Encode and copy after recording; replay observes changed contents.
            resource.buffer.offset = stride * replay;
            unsafe {
                api::write_resource_descriptors(
                    device_handle,
                    std::slice::from_ref(&resource),
                    &[location(&source, 0)],
                    scratch_span(&mut temporary),
                )
            }
            .unwrap();
            if replay == 1 {
                assert_eq!(
                    unsafe {
                        ogpu_next_copy_descriptors(
                            device_pointer,
                            1,
                            &copy,
                            scratch_span(&mut copy_scratch),
                        )
                    },
                    OK
                );
            } else {
                unsafe {
                    api::copy_descriptors(device_handle, &[copy], scratch_span(&mut copy_scratch))
                }
                .unwrap();
            }
            let copied = unsafe {
                std::slice::from_raw_parts(
                    mapping.data.cast::<u8>().add(shape.array_stride as usize),
                    shape.size as usize,
                )
            };
            assert_eq!(copied, &staging[..shape.size as usize]);
            backing.cache(shape.array_stride, shape.size, true).unwrap();
            let point = SyncPoint {
                point: Point {
                    timeline: ptr::from_ref(&*timeline).cast_mut(),
                    value: replay,
                },
                stages: 1,
            };
            let scratch = scratch_span(&mut submission);
            unsafe {
                device.queue(domain, 0).unwrap().submit_descriptions(
                    &[SubmitDesc {
                        list_count: 1,
                        wait_count: 0,
                        signal_count: 1,
                        lists: &list,
                        waits: ptr::null(),
                        signals: &point,
                        ..Default::default()
                    }],
                    &mut [SubmitOutcome::rejected(INVALID)],
                    scratch,
                )
            }
            .unwrap();
            timeline.wait(replay, u64::MAX).unwrap();
            data.cache(0, stride * 3, false).unwrap();
            for (i, word) in words.iter().enumerate() {
                let written = i == stride as usize / 4 || replay == 2 && i == stride as usize / 2;
                assert_eq!(
                    *word,
                    if written { 0x12345678 } else { 0xa5a5a5a5 },
                    "replay={replay} word={i}"
                );
            }
        }
        assert_eq!(BINDS.get(), 1);
        assert_eq!(WRITES.get(), 2);
        // Recordings die before pipelines/layouts/heaps and their borrowed memory.
        drop(arena);
        drop(executable);
        drop(arguments);
        drop(heap);
        tested += 1;
    }
    assert!(tested > 0, "No suitable device; not a passing skip");
}
