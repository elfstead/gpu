//! D1 D3D12 backend tests: host-record logic runs everywhere; GPU tests need a
//! live adapter and are ignored unless run with `--ignored` on the test machine.
use super::*;

#[test]
fn scratch_layouts_are_exact_and_never_zero_alignment() {
    let submit = submit_scratch(3, 0, 0).unwrap();
    assert_eq!(submit.size, 3 * size_of::<Option<d3d::ID3D12CommandList>>() as u64);
    assert!(submit.alignment >= 8);
    let empty = submit_scratch(0, 0, 0).unwrap();
    assert_eq!(empty.size, 0);
    assert_eq!(empty.alignment, 1);
    let barrier = barrier_scratch(2, 3).unwrap();
    assert!(barrier.size > 0);
    assert!(barrier.size % barrier.alignment == 0);
    let none = barrier_scratch(0, 0).unwrap();
    assert!(none.size >= size_of::<d3d::D3D12_BARRIER_GROUP>() as u64);
}

#[test]
fn format_catalog_lists_every_revision_format() {
    assert_eq!(formats::FORMATS.len(), 172);
    // Every catalog entry maps to a DXGI format or exactly UNKNOWN.
    for format in formats::FORMATS {
        let native = formats::dxgi_format(format.id);
        assert!(
            native != windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN
                || formats::meta(format.id).is_some()
        );
    }
    assert_eq!(
        formats::dxgi_format(3),
        windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM
    );
    // Vocabulary entries without a DXGI equivalent must not substitute.
    assert_eq!(
        formats::dxgi_format(33), // R8_SRGB has no single-channel DXGI format.
        windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN
    );
}

#[test]
fn boundary_helpers_reject_malformed_storage() {
    let requirements = HostRequirements {
        size: 8,
        alignment: 8,
    };
    assert_eq!(
        scratch(ptr::null_mut(), 0, requirements),
        Err(CAPACITY)
    );
    let mut buffer = [0u64; 2];
    assert!(scratch(buffer.as_mut_ptr().cast(), 16, requirements).is_ok());
    // Misaligned nonempty storage is invalid.
    let bytes = buffer.as_mut_ptr().cast::<u8>();
    assert_eq!(
        scratch(unsafe { bytes.add(1) }.cast(), 16, requirements),
        Err(INVALID)
    );
}

// ---- D1 GPU tests: exercised on the native adapter with `cargo test -- --ignored` ----
use crate::foundation::rust as api;

/// Caller-provided scratch with exact alignment and no hidden allocation policy.
fn aligned_scratch(requirements: HostRequirements) -> (Vec<u8>, usize) {
    let mut backing = vec![0u8; (requirements.size + requirements.alignment) as usize];
    let base = backing.as_mut_ptr() as usize;
    let offset = (requirements.alignment as usize - base % requirements.alignment as usize)
        % requirements.alignment as usize;
    (backing, offset)
}

fn gpu_setup() -> (Box<Discovery>, api::DeviceHandle, api::QueueHandle, u32) {
    let discovery = api::discover().unwrap();
    let index = api::adapters(&discovery)
        .iter()
        .position(|a| api::adapter_features(a).baseline_supported != 0)
        .expect("baseline-capable D3D12 adapter");
    let adapter = &api::adapters(&discovery)[index];
    let domain = api::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE | TRANSFER) != 0)
        .unwrap()
        .domain;
    let device = api::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        0,
    )
    .unwrap();
    let queue = unsafe { api::device_queue(device, domain, 0) }.unwrap();
    (discovery, device, queue, domain)
}

fn linear_memory(
    device: api::DeviceHandle,
    size: u64,
    usage: u64,
    memory_type: u32,
) -> api::MemoryHandle {
    unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size,
                alignment: 256,
                usage,
                memory_type,
                kind: 1,
                concurrent_domains: ptr::null(),
                concurrent_domain_count: 0,
                flags: 0,
            },
        )
    }
    .unwrap()
}

fn single_slot_arena(device: api::DeviceHandle, domain: u32) -> api::ArenaHandle {
    unsafe {
        api::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )
    }
    .unwrap()
}

fn one_shot() -> RecordingDesc {
    RecordingDesc {
        header: Record::new::<RecordingDesc>(RECORDING_DESC),
        replay_mode: 1,
        level: 0,
        inheritance: ptr::null(),
    }
}

fn fill_upload(memory: api::MemoryHandle, bytes: &[u8]) {
    let mapping = unsafe { api::memory_map(memory.span(0, bytes.len() as u64)) }.unwrap();
    // SAFETY: the mapping covers the range; the copy writes exactly len bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), mapping.data.cast::<u8>(), bytes.len());
    }
    unsafe { api::memory_unmap(memory) };
}

fn read_back(memory: api::MemoryHandle, size: u64) -> Vec<u8> {
    let mapping = unsafe { api::memory_map(memory.span(0, size)) }.unwrap();
    let mut result = vec![0u8; size as usize];
    // SAFETY: the mapping covers the range; the copy reads exactly size bytes.
    unsafe {
        ptr::copy_nonoverlapping(mapping.data.cast::<u8>(), result.as_mut_ptr(), size as usize);
    }
    unsafe { api::memory_unmap(memory) };
    result
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_copy_barrier_readback_roundtrip() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let size = 4096u64;
    let pattern: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let upload = linear_memory(device, size, USAGE_COPY_SRC, 1);
    let staging = linear_memory(device, size, USAGE_COPY_SRC | USAGE_COPY_DST, 0);
    let readback = linear_memory(device, size, USAGE_COPY_DST, 2);
    fill_upload(upload, &pattern);

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::copy_memory(list, staging.span(0, size), upload.span(0, size)) };
    // One explicit memory dependency between the copies: write -> read scopes.
    let requirements = api::barrier_scratch_requirements(1, 0).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let memory_barrier = MemoryBarrier {
        range: staging.span(0, size),
        before: ACCESS_COPY_WRITE,
        after: ACCESS_COPY_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &memory_barrier,
        images: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    unsafe { api::copy_memory(list, readback.span(0, size), staging.span(0, size)) };
    let list = unsafe { api::commands_end(list) }.unwrap();

    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let signal = SyncPoint {
        point: Point {
            timeline: timeline.as_raw(),
            value: 1,
        },
        stages: STAGE_TRANSFER,
    };
    unsafe {
        api::queue_submit(
            queue,
            &[list],
            &[],
            &[signal],
            HostSpan {
                data: scratch.as_mut_ptr().add(offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(
        unsafe { api::timeline_wait(timeline, signal.point.value, u64::MAX) },
        Ok(())
    );
    assert_eq!(read_back(readback, size), pattern);

    unsafe {
        api::arena_destroy(arena);
        api::memory_destroy(readback);
        api::memory_destroy(staging);
        api::memory_destroy(upload);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_timeline_host_signal_poll_and_timeout() {
    let (_discovery, device, _queue, _domain) = gpu_setup();
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    assert_eq!(unsafe { api::timeline_poll(timeline) }, Ok(0));
    assert_eq!(unsafe { api::timeline_signal(timeline, 5) }, Ok(()));
    assert_eq!(unsafe { api::timeline_poll(timeline) }, Ok(5));
    assert_eq!(unsafe { api::timeline_wait(timeline, 5, u64::MAX) }, Ok(()));
    // A short timeout on an unsignaled future point reports TIMEOUT.
    assert_eq!(
        unsafe { api::timeline_wait(timeline, 6, 1_000_000) },
        Err(TIMEOUT)
    );
    unsafe {
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_one_shot_consumption_capacity_and_unsupported_poison() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let size = 256u64;
    let upload = linear_memory(device, size, USAGE_COPY_SRC, 1);
    let target = linear_memory(device, size, USAGE_COPY_DST, 0);
    let arena = single_slot_arena(device, domain);
    // Capacity counts slots exactly: a second begin of a one-slot arena fails.
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    assert!(matches!(
        unsafe { api::commands_begin(arena, &one_shot()) },
        Err(CAPACITY)
    ));
    unsafe { api::copy_memory(list, target.span(0, size), upload.span(0, size)) };
    let list = unsafe { api::commands_end(list) }.unwrap();

    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let signal = SyncPoint {
        point: Point {
            timeline: timeline.as_raw(),
            value: 1,
        },
        stages: STAGE_TRANSFER,
    };
    let submit = |storage: *mut std::ffi::c_void| {
        let span = HostSpan {
            data: storage,
            size: requirements.size,
        };
        unsafe { api::queue_submit(queue, &[list], &[], &[signal], span) }
    };
    let storage = unsafe { scratch.as_mut_ptr().add(offset) }.cast();
    assert_eq!(submit(storage), Ok(()));
    // One-shot lists are consumed by successful submission.
    assert_eq!(submit(storage), Err(INVALID));
    // Retirement before destruction: the contract forbids destroying resources
    // while submitted work is still pending.
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));

    // Unsupported operations poison the recording visibly instead of emulating.
    unsafe { api::arena_reset(arena) }.unwrap();
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let heap = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: Span {
            memory: ptr::null_mut(),
            offset: 0,
            size: 0,
        },
        reserved_offset: 0,
        reserved_size: 0,
    };
    unsafe { api::bind_heap(list, &heap) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));

    unsafe {
        api::arena_destroy(arena);
        api::memory_destroy(target);
        api::memory_destroy(upload);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_image_copy_roundtrip_with_explicit_states() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 64;
    let texels = (extent * extent * 4) as u64;
    let pattern: Vec<u8> = (0..texels).map(|i| ((i * 7) % 253) as u8).collect();
    let upload = linear_memory(device, texels, USAGE_COPY_SRC, 1);
    let readback = linear_memory(device, texels, USAGE_COPY_DST, 2);
    fill_upload(upload, &pattern);

    let image_desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent {
            x: extent,
            y: extent,
            z: 1,
        },
        usage: IMAGE_COPY_SRC | IMAGE_COPY_DST,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
    assert!(requirements.size >= texels);
    let backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: requirements.size,
                alignment: 1,
                usage: 0,
                memory_type: 0,
                kind: 2,
                concurrent_domains: ptr::null(),
                concurrent_domain_count: 0,
                flags: 0,
            },
        )
    }
    .unwrap();
    let image = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(image, backing.span(0, requirements.size)) }.unwrap();

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let region = ImageRegion {
        aspect: ASPECT_COLOR,
        mip: 0,
        first_layer: 0,
        layer_count: 1,
        offset: Offset { x: 0, y: 0, z: 0 },
        extent: Extent {
            x: extent,
            y: extent,
            z: 1,
        },
    };
    // Explicit states: UNDEFINED -> COPY_DST, then COPY_DST -> COPY_SRC.
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let range = Subresources {
        aspects: ASPECT_COLOR,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
    };
    let to_dst = ImageBarrier {
        image: image.as_raw(),
        range,
        before: 0,
        after: ACCESS_COPY_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COPY_DST,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_dst,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    let copy = ImageCopy {
        region,
        row_pitch: 0,
        slice_pitch: 0,
        state: STATE_COPY_DST,
        reserved: 0,
    };
    unsafe { api::copy_to_image(list, image, upload.span(0, texels), &copy) };
    let to_src = ImageBarrier {
        image: image.as_raw(),
        range,
        before: ACCESS_COPY_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COPY_DST,
        new_state: STATE_COPY_SRC,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 0,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_src,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    let copy = ImageCopy {
        region,
        row_pitch: 0,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    unsafe { api::copy_from_image(list, readback.span(0, texels), image, &copy) };
    let list = unsafe { api::commands_end(list) }.unwrap();

    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let signal = SyncPoint {
        point: Point {
            timeline: timeline.as_raw(),
            value: 1,
        },
        stages: STAGE_TRANSFER,
    };
    unsafe {
        api::queue_submit(
            queue,
            &[list],
            &[],
            &[signal],
            HostSpan {
                data: scratch.as_mut_ptr().add(offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(
        unsafe { api::timeline_wait(timeline, signal.point.value, u64::MAX) },
        Ok(())
    );
    assert_eq!(read_back(readback, texels), pattern);

    unsafe {
        api::arena_destroy(arena);
        api::image_destroy(image);
        api::memory_destroy(backing);
        api::memory_destroy(readback);
        api::memory_destroy(upload);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

// ---- D2.1 GPU tests: root-argument compute over DXIL artifacts ----

#[repr(align(4))]
struct AlignedArtifact<const N: usize>([u8; N]);
/// Fixture artifact of tests/dx12-fixtures/argument-transform.slang.
static ARGUMENT_TRANSFORM: AlignedArtifact<3816> =
    AlignedArtifact(*include_bytes!(
        "../../../../../tests/dx12-fixtures/argument-transform.dxil"
    ));

/// Prepare the fixture executable: namespace 24 bytes, two root slots.
fn argument_transform_executable(
    device: api::DeviceHandle,
) -> Result<api::ExecutableHandle, Status> {
    let roots = [
        RootSlot {
            stages: STAGE_COMPUTE,
            offset: 0,
            alignment: 4,
        },
        RootSlot {
            stages: STAGE_COMPUTE,
            offset: 8,
            alignment: 4,
        },
    ];
    let abi = ArgumentInterface {
        header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
        byte_size: 24,
        root_count: 2,
        roots: roots.as_ptr(),
    };
    let requirements = ShaderRequirements {
        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
        features: 0,
        local_size: [64, 1, 1],
        shared_memory: 0,
    };
    let artifact = &ARGUMENT_TRANSFORM.0;
    let shader = Shader {
        stage: STAGE_COMPUTE as u32,
        format: SHADER_DXIL,
        code: Bytes {
            data: artifact.as_ptr().cast(),
            size: artifact.len(),
        },
        entry: c"main".as_ptr(),
        interface_metadata: &abi.header,
        specialization: ptr::null(),
        subgroup: ptr::null(),
    };
    let desc = ExecutableDesc {
        header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
        kind: EXECUTABLE_COMPUTE,
        shader_count: 1,
        shaders: &shader,
        static_state: ptr::null(),
        dynamic_state: 0,
        requirements: &requirements.header,
        cache: ptr::null_mut(),
        compile_flags: 0,
        reserved: 0,
    };
    unsafe { api::executable_create(device, &desc) }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_root_argument_dispatch_roundtrip() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let elements = 128u32;
    let bytes = u64::from(elements) * 4;
    let staging = linear_memory(device, bytes, USAGE_COPY_SRC, 1);
    let input = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_DST, 0);
    let output = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let second = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let readback = linear_memory(device, bytes, USAGE_COPY_DST, 2);
    let pattern: Vec<u8> = (0..bytes).map(|i| (i % 251) as u8).collect();
    fill_upload(staging, &pattern);

    let executable = argument_transform_executable(device).unwrap();
    let input_address = unsafe { api::memory_address(input.span(0, bytes)) }.unwrap();
    let output_address = unsafe { api::memory_address(output.span(0, bytes)) }.unwrap();
    let second_address = unsafe { api::memory_address(second.span(0, bytes)) }.unwrap();

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    // Storage buffers are DEFAULT-heap only; the pattern is copied in explicitly.
    unsafe { api::copy_memory(list, input.span(0, bytes), staging.span(0, bytes)) };
    let input_barrier = MemoryBarrier {
        range: input.span(0, bytes),
        before: ACCESS_COPY_WRITE,
        after: ACCESS_SHADER_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let copy_scratch = api::barrier_scratch_requirements(1, 0).unwrap();
    let (mut copy_scratch_bytes, copy_offset) = aligned_scratch(copy_scratch);
    let copy_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_COMPUTE,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &input_barrier,
        images: ptr::null(),
        scratch: unsafe { copy_scratch_bytes.as_mut_ptr().add(copy_offset) }.cast(),
        scratch_size: copy_scratch.size,
    };
    unsafe { api::barrier(list, &copy_dependency) };
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 0, input_address) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 1, output_address) };
    let constants = [elements, 3u32];
    unsafe {
        api::set_inline(
            list,
            STAGE_COMPUTE,
            16,
            bytemuck_bytes(&constants),
        )
    };
    unsafe {
        api::dispatch(
            list,
            &Launch {
                groups: Extent {
                    x: elements / 64,
                    y: 1,
                    z: 1,
                },
                dynamic_shared_bytes: 0,
                extensions: ptr::null(),
            },
        )
    };
    // Rebinding the same executable must preserve the argument bytes; only the
    // output root changes before the second, indirect dispatch.
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 1, second_address) };
    let indirect = [elements / 64, 1u32, 1u32];
    let indirect_bytes = bytemuck_bytes(&indirect);
    let args = linear_memory(device, 256, USAGE_INDIRECT, 1);
    fill_upload(args, indirect_bytes);
    // Arguments stay in caller memory; the GPU reads them at execution.
    unsafe { api::dispatch_indirect(list, args.span(0, 12), 0) };

    // One memory dependency, then read both outputs back.
    let requirements = api::barrier_scratch_requirements(1, 0).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let barrier = MemoryBarrier {
        range: output.span(0, bytes),
        before: ACCESS_STORAGE_WRITE,
        after: ACCESS_COPY_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COMPUTE,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &barrier,
        images: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    let barrier = MemoryBarrier {
        range: second.span(0, bytes),
        ..barrier
    };
    let dependency = Dependency {
        memory: &barrier,
        ..dependency
    };
    unsafe { api::barrier(list, &dependency) };
    unsafe { api::copy_memory(list, readback.span(0, bytes), output.span(0, bytes)) };
    let readback2 = linear_memory(device, bytes, USAGE_COPY_DST, 2);
    unsafe { api::copy_memory(list, readback2.span(0, bytes), second.span(0, bytes)) };
    let list = unsafe { api::commands_end(list) }.unwrap();

    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let signal = SyncPoint {
        point: Point {
            timeline: timeline.as_raw(),
            value: 1,
        },
        stages: STAGE_ALL,
    };
    unsafe {
        api::queue_submit(
            queue,
            &[list],
            &[],
            &[signal],
            HostSpan {
                data: scratch.as_mut_ptr().add(offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));

    let multiplier = 3u32;
    let count = elements;
    let mut expected = Vec::with_capacity(bytes as usize);
    for i in 0..elements {
        let base = (i as usize) * 4;
        let mut value = u32::from_le_bytes(pattern[base..base + 4].try_into().unwrap());
        value = value.wrapping_mul(multiplier).wrapping_add(count);
        expected.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(read_back(readback, bytes), expected);
    assert_eq!(read_back(readback2, bytes), expected);

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::memory_destroy(readback2);
        api::memory_destroy(readback);
        api::memory_destroy(args);
        api::memory_destroy(second);
        api::memory_destroy(output);
        api::memory_destroy(input);
        api::memory_destroy(staging);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Interpret plain values as command bytes without external crates.
fn bytemuck_bytes<T>(value: &T) -> &[u8] {
    // SAFETY: T is plain data (u32 arrays in these tests); the bytes are the
    // object's exact initialized representation.
    unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_argument_contract_rejections() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let bytes = 256u64;
    let memory = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let address = unsafe { api::memory_address(memory.span(0, bytes)) }.unwrap();
    let executable = argument_transform_executable(device).unwrap();
    let one = one_shot();

    // Inline ranges overlapping root-slot bytes cannot follow byte writes.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_inline(list, STAGE_COMPUTE, 0, &[0u8; 8]) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));
    unsafe { api::arena_destroy(arena) };

    // Unknown root slot and misaligned addresses are invalid.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 2, address) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 0, address + 2) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    // Dispatch requires a bound executable; extensions remain unsupported.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one) }.unwrap();
    let launch = Launch {
        groups: Extent { x: 1, y: 1, z: 1 },
        dynamic_shared_bytes: 0,
        extensions: ptr::null(),
    };
    unsafe { api::dispatch(list, &launch) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    let launch = Launch {
        groups: Extent { x: 1, y: 1, z: 1 },
        dynamic_shared_bytes: 4,
        extensions: ptr::null(),
    };
    unsafe { api::dispatch(list, &launch) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));
    unsafe { api::arena_destroy(arena) };

    // Preparation rejects SPIR-V pointer-ABI artifacts and specialization.
    let roots = [RootSlot {
        stages: STAGE_COMPUTE,
        offset: 0,
        alignment: 4,
    }];
    let abi = ArgumentInterface {
        header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
        byte_size: 24,
        root_count: 1,
        roots: roots.as_ptr(),
    };
    let requirements = ShaderRequirements {
        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
        features: 0,
        local_size: [64, 1, 1],
        shared_memory: 0,
    };
    let artifact = &ARGUMENT_TRANSFORM.0;
    let mut shader = Shader {
        stage: STAGE_COMPUTE as u32,
        format: SHADER_DXIL,
        code: Bytes {
            data: artifact.as_ptr().cast(),
            size: artifact.len(),
        },
        entry: c"main".as_ptr(),
        interface_metadata: &abi.header,
        specialization: ptr::null(),
        subgroup: ptr::null(),
    };
    let desc = ExecutableDesc {
        header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
        kind: EXECUTABLE_COMPUTE,
        shader_count: 1,
        shaders: &shader,
        static_state: ptr::null(),
        dynamic_state: 0,
        requirements: &requirements.header,
        cache: ptr::null_mut(),
        compile_flags: 0,
        reserved: 0,
    };
    shader.format = SHADER_SPIRV;
    assert_eq!(unsafe { api::executable_create(device, &desc) }, Err(UNSUPPORTED));
    shader.format = SHADER_DXIL;
    let specialization = Specialization {
        header: Record::new::<Specialization>(SPECIALIZATION),
        count: 0,
        reserved: 0,
        entries: ptr::null(),
        data: Bytes {
            data: ptr::null(),
            size: 0,
        },
    };
    shader.specialization = &specialization.header;
    assert_eq!(unsafe { api::executable_create(device, &desc) }, Err(UNSUPPORTED));

    unsafe {
        api::executable_destroy(executable);
        api::memory_destroy(memory);
        api::device_destroy(device);
    }
}
