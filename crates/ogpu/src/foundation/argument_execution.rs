//! Native execution acceptance for the installed argument mapping.
use super::*;
use crate::foundation::{
    ogpu_next_argument_layout_create, ogpu_next_argument_layout_info, ogpu_next_executable_create,
    ogpu_next_set_arguments, rust as api,
};

#[repr(C)]
struct Values {
    count: u32,
    multiplier: u32,
    uniform: u64,
    source: u64,
    destination: u64,
    extra: u64,
}

#[test]
#[ignore = "requires modern Vulkan; mapped inline/uniform/read/write arguments, shared-layout binds, partial updates and replay"]
fn gpu_mapped_argument_execution() {
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
        let device = Device::create(
            &adapter,
            &[QueueRequest {
                domain,
                count: 1,
                priority: 0.5,
            }],
            FeatureBits::ZERO,
        )
        .unwrap();
        let uniform_alignment = u32::try_from(
            device
                .snapshot
                .descriptor_properties
                .uniform_address_alignment,
        )
        .unwrap();
        let fields = [
            ArgumentField {
                offset: 0,
                size: 8,
                kind: ARGUMENT_INLINE,
                address_alignment: 0,
                stages: 4,
                space: 2,
                binding: 3,
            },
            ArgumentField {
                offset: 8,
                size: 8,
                kind: ARGUMENT_UNIFORM_ADDRESS,
                address_alignment: uniform_alignment,
                stages: 4,
                space: 5,
                binding: 0,
            },
            ArgumentField {
                offset: 16,
                size: 8,
                kind: ARGUMENT_READ_ADDRESS,
                address_alignment: 4,
                stages: 4,
                space: 3,
                binding: 4,
            },
            ArgumentField {
                offset: 24,
                size: 8,
                kind: ARGUMENT_READ_WRITE_ADDRESS,
                address_alignment: 4,
                stages: 4,
                space: 7,
                binding: 1,
            },
            ArgumentField {
                offset: 32,
                size: 8,
                kind: ARGUMENT_INLINE,
                address_alignment: 0,
                stages: 4,
                space: ARGUMENT_DIRECT,
                binding: ARGUMENT_DIRECT,
            },
        ];
        assert_eq!(size_of::<Values>(), 40);
        let description = ArgumentLayoutDesc {
            execution_class: ARGUMENT_COMPUTE,
            byte_size: 40,
            field_count: fields.len() as u32,
            flags: 0,
            fields: fields.as_ptr(),
        };
        let device_pointer = ptr::from_ref(&*device).cast_mut();
        let mut layout_pointer = ptr::null_mut();
        assert_eq!(
            unsafe {
                ogpu_next_argument_layout_create(device_pointer, &description, &mut layout_pointer)
            },
            OK
        );
        let _layout = unsafe { Box::from_raw(layout_pointer) };
        let layout_handle = unsafe { api::ArgumentLayoutHandle::from_raw(layout_pointer) }.unwrap();
        let mut info = std::mem::MaybeUninit::uninit();
        unsafe {
            ogpu_next_argument_layout_info(layout_pointer, info.as_mut_ptr());
        }
        assert_eq!(unsafe { info.assume_init() }, unsafe {
            api::argument_layout_info(layout_handle)
        });
        let code: Vec<u32> =
            include_bytes!("../../../../examples/shaders/foundation-arguments.comp.spv")
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
            entry: c"mappedArguments".as_ptr(),

            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        let requirements = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: Features::default(),
            local_size: [32, 1, 1],
            shared_memory: 0,
        };
        let mut executable_desc = ExecutableDesc {
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
        };
        let mut executable_pointer = ptr::null_mut();
        assert_eq!(
            unsafe {
                ogpu_next_executable_create(
                    device_pointer,
                    &executable_desc,
                    layout_pointer,
                    &mut executable_pointer,
                )
            },
            OK
        );
        let first = unsafe { Box::from_raw(executable_pointer) };
        let bias = 11u32;
        let entry = SpecializationEntry {
            id: 7,
            offset: 0,
            size: 4,
        };
        let specialization = Specialization {
            header: Record::new::<Specialization>(SPECIALIZATION),
            count: 1,
            reserved: 0,
            entries: &entry,
            data: Bytes {
                data: ptr::from_ref(&bias).cast(),
                size: 4,
            },
        };
        let shader2 = Shader {
            specialization: &specialization.header,
            ..shader
        };
        executable_desc.shaders = &shader2;
        let second_handle = unsafe {
            api::executable_create(
                api::DeviceHandle::from_raw(device_pointer).unwrap(),
                &executable_desc,
                layout_handle,
            )
        }
        .unwrap();
        let second = unsafe { Box::from_raw(second_handle.as_raw()) };

        // One explicit allocation supplies all four pointees. Storage addresses
        // intentionally have only four-byte alignment, unlike descriptor offsets.
        let stride = u64::from(uniform_alignment).max(256);
        let mut memory_desc = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: stride * 5,
            alignment: stride,
            usage: 32 | 64,
            memory_type: u32::MAX,
            kind: 1,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut req = Requirements::default();
        unsafe { device.memory_requirements(&memory_desc, &mut req) }.unwrap();
        let mut compatible = vec![0; req.compatible_type_count as usize];
        req.compatible_types = compatible.as_mut_ptr();
        req.compatible_type_capacity = compatible.len() as u32;
        unsafe { device.memory_requirements(&memory_desc, &mut req) }.unwrap();
        memory_desc.memory_type = *compatible
            .iter()
            .find(|id| {
                device
                    .snapshot
                    .memory_types
                    .iter()
                    .any(|t| t.id == **id && t.properties & 2 != 0)
            })
            .unwrap();
        let memory = unsafe { Memory::create(&device, &memory_desc) }.unwrap();
        let mapping = memory.map(0, memory_desc.size).unwrap();
        let base = memory.address(0, memory_desc.size).unwrap();
        let mut values = Values {
            count: 37,
            multiplier: 3,
            uniform: base,
            source: base + stride * 2 + 4,
            destination: base + stride * 3 + 4,
            extra: base + stride,
        };
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
        encoder.bind_executable(&first).unwrap();
        unsafe {
            ogpu_next_set_arguments(list, layout_pointer, 0, 40, ptr::from_ref(&values).cast());
        }
        let launch = Launch {
            groups: Extent { x: 2, y: 1, z: 1 },
            dynamic_shared_bytes: 0,
            extensions: ptr::null(),
        };
        encoder.dispatch(&launch).unwrap();
        // Binding a second pipeline with the same layout preserves every field.
        encoder.bind_executable(&second).unwrap();
        values.multiplier = 5;
        values.destination = base + stride * 4 + 4;
        unsafe {
            let handle = api::ListHandle::from_raw(list).unwrap();
            api::set_arguments(handle, layout_handle, 4, &values.multiplier.to_ne_bytes());
            api::set_arguments(handle, layout_handle, 24, &values.destination.to_ne_bytes());
        }
        encoder.dispatch(&launch).unwrap();
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
        let timeline = Timeline::create(&device, 0).unwrap();
        let scratch_req = submit_scratch(
            &device,
            &[SubmitDesc {
                list_count: 1,
                wait_count: 0,
                signal_count: 1,
                ..Default::default()
            }],
        )
        .unwrap();
        let mut scratch = vec![0u64; scratch_req.size.div_ceil(8) as usize];
        assert!(scratch_req.alignment <= align_of::<u64>() as u64);
        // Poison the former host input after recording: commands capture values,
        // whereas the memory behind all addresses must remain live on replay.
        values = Values {
            count: 0,
            multiplier: 0,
            uniform: 0,
            source: 0,
            destination: 0,
            extra: 0,
        };
        std::hint::black_box(&values);
        for replay in 1..=2u32 {
            let words = unsafe {
                std::slice::from_raw_parts_mut(
                    mapping.data.cast::<u32>(),
                    memory_desc.size as usize / 4,
                )
            };
            let region = stride as usize / 4;
            const GUARD: u32 = 0x9a37f251;
            words.fill(GUARD);
            words[0] = replay * 17;
            words[region] = replay * 31;
            for i in 0..37 {
                words[region * 2 + 1 + i] = i as u32 + replay * 7;
            }
            memory.cache(0, memory_desc.size, true).unwrap();
            let point = SyncPoint {
                point: Point {
                    timeline: ptr::from_ref(&*timeline).cast_mut(),
                    value: u64::from(replay),
                },
                stages: 1,
            };
            let submit = SubmitDesc {
                list_count: 1,
                wait_count: 0,
                signal_count: 1,
                lists: &list,
                waits: ptr::null(),
                signals: &point,
                ..Default::default()
            };
            unsafe {
                device.queue(domain, 0).unwrap().submit_descriptions(
                    &[submit],
                    &mut [SubmitOutcome::rejected(INVALID)],
                    HostSpan {
                        data: scratch.as_mut_ptr().cast(),
                        size: scratch_req.size,
                    },
                )
            }
            .unwrap();
            timeline.wait(u64::from(replay), u64::MAX).unwrap();
            memory.cache(0, memory_desc.size, false).unwrap();
            for (region_index, multiplier, bias) in [(3, 3, 1), (4, 5, 11)] {
                for i in 0..37 {
                    assert_eq!(
                        words[region * region_index + 1 + i],
                        (i as u32 + replay * 7) * multiplier + replay * (17 + 31) + bias,
                        "replay={replay} output={region_index} index={i}"
                    );
                }
            }
            for (i, &word) in words.iter().enumerate() {
                let written = i == 0
                    || i == region
                    || [2, 3, 4]
                        .into_iter()
                        .any(|r| (region * r + 1..region * r + 38).contains(&i));
                if !written {
                    assert_eq!(word, GUARD, "guard at word {i}");
                }
            }
        }
        // Discard recorded references before borrowed layouts, executables/memory.
        arena.reset(false).unwrap();
        tested += 1;
    }
    assert!(tested > 0, "No suitable device, not a passing skip");
}
