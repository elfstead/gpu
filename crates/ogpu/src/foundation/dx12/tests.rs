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

/// Print D3D12 debug-layer messages of the live device (test diagnostics).
fn dump_validation(device: api::DeviceHandle) {
    // SAFETY: the handle identifies a live device.
    let device_ref: &Device = unsafe { &*device.as_raw() };
    if let Ok(info) = device_ref.device.cast::<d3d::ID3D12InfoQueue>() {
        let count = unsafe { info.GetNumStoredMessages() };
        eprintln!("DX12PROBE validation messages: {}", count);
        for i in 0..count.min(10) {
            let mut len = 0usize;
            unsafe {
                info.GetMessage(i, None, &mut len);
                let mut buffer = vec![0u8; len];
                info.GetMessage(i, Some(buffer.as_mut_ptr().cast()), &mut len);
                let message: &d3d::D3D12_MESSAGE = &*buffer.as_ptr().cast();
                let text = std::slice::from_raw_parts(
                    message.pDescription.cast::<u8>(),
                    message.DescriptionByteLength,
                );
                eprintln!(
                    "DX12PROBE msg[{}]: {}",
                    i,
                    String::from_utf8_lossy(text).trim_end_matches('\0')
                );
            }
        }
    }
}

fn gpu_setup() -> (Box<Discovery>, api::DeviceHandle, api::QueueHandle, u32) {
    gpu_setup_features(0)
}

fn gpu_setup_features(
    features: u64,
) -> (Box<Discovery>, api::DeviceHandle, api::QueueHandle, u32) {
    // Test runs execute under the D3D12 debug layer when it is installed:
    // validation evidence for the review gate (best-effort, never required).
    static ENABLE: std::sync::Once = std::sync::Once::new();
    ENABLE.call_once(|| {
        let mut debug: Option<d3d::ID3D12Debug> = None;
        if let Ok(debug) = unsafe { d3d::D3D12GetDebugInterface(&mut debug) }.map(|_| debug) {
            if let Some(debug) = debug.as_ref() {
                unsafe { debug.EnableDebugLayer() };
            }
        }
    });
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
        features,
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
        flags: 1,
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
#[allow(unused_assignments)]
fn gpu_argument_contract_rejections() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let bytes = 256u64;
    let memory = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let address = unsafe { api::memory_address(memory.span(0, bytes)) }.unwrap();
    let executable = argument_transform_executable(device).unwrap();
    let one = one_shot();

    // SHARED_BYTES per the contract: "set_root changes only its 8 bytes; inline
    // may intentionally change those same bytes". The write is accepted and
    // updates the slot's root descriptor from the resulting namespace bytes.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_inline(list, STAGE_COMPUTE, 0, &[0u8; 8]) };
    assert!(unsafe { api::commands_end(list) }.is_ok());
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

// ---- D2.2 GPU tests: descriptor-heap recipes and direct heap indexing ----

/// Fixture artifact of tests/dx12-fixtures/heap-transform.slang.
static HEAP_TRANSFORM: AlignedArtifact<4056> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/heap-transform.dxil"
));

/// Prepare the heap-indexing fixture: 16-byte namespace, no root slots.
fn heap_transform_executable(device: api::DeviceHandle) -> Result<api::ExecutableHandle, Status> {
    heap_transform_executable_with(device, None)
}

fn heap_transform_executable_with(
    device: api::DeviceHandle,
    cache: Option<api::ExecutableCacheHandle>,
) -> Result<api::ExecutableHandle, Status> {
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
    let artifact = &HEAP_TRANSFORM.0;
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
        cache: cache.map_or(ptr::null_mut(), |cache| cache.as_raw()),
        compile_flags: 0,
        reserved: 0,
    };
    unsafe { api::executable_create(device, &desc) }
}

/// Encode resource descriptors into host storage, then place them into
/// descriptor-heap memory at the given slots (byte offset = slot * 40).
fn place_resource_recipes(
    device: api::DeviceHandle,
    heap: api::MemoryHandle,
    slot_offsets: &[u64],
    inputs: &[ResourceDescriptor],
) {
    let mut host: Vec<[u8; 40]> = vec![[0; 40]; inputs.len()];
    let outputs: Vec<HostSpan> = host
        .iter_mut()
        .map(|bytes| HostSpan {
            data: bytes.as_mut_ptr().cast(),
            size: 40,
        })
        .collect();
    unsafe {
        api::write_resource_descriptors(
            device,
            inputs,
            &outputs,
            HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
        )
    }
    .unwrap();
    for (bytes, offset) in host.iter().zip(slot_offsets) {
        let mapping = unsafe { api::memory_map(heap.span(*offset, 40)) }.unwrap();
        // SAFETY: the mapping covers the recipe slot.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), mapping.data.cast::<u8>(), 40);
        }
        unsafe { api::memory_unmap(heap) };
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_descriptor_heap_roundtrip() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let elements = 128u32;
    let bytes = u64::from(elements) * 4;
    let staging = linear_memory(device, bytes, USAGE_COPY_SRC, 1);
    let input = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_DST, 0);
    let output = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let second = linear_memory(device, bytes, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let diag = linear_memory(device, 256, USAGE_STORAGE | USAGE_COPY_SRC, 0);
    let readback = linear_memory(device, bytes, USAGE_COPY_DST, 2);
    let readback2 = linear_memory(device, bytes, USAGE_COPY_DST, 2);
    let diag_readback = linear_memory(device, 256, USAGE_COPY_DST, 2);
    let pattern: Vec<u8> = (0..bytes).map(|i| (i % 251) as u8).collect();
    fill_upload(staging, &pattern);

    // Descriptor-heap memory: host-visible, recipe bytes at 40-byte slots.
    let heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    place_resource_recipes(
        device,
        heap,
        &[0, 40],
        &[
            ResourceDescriptor {
                kind: DESCRIPTOR_STORAGE_BUFFER,
                image_state: STATE_GENERAL,
                view: ptr::null_mut(),
                buffer: input.span(0, bytes),
            },
            ResourceDescriptor {
                kind: DESCRIPTOR_STORAGE_BUFFER,
                image_state: STATE_GENERAL,
                view: ptr::null_mut(),
                buffer: output.span(0, bytes),
            },
        ],
    );

    let executable = heap_transform_executable(device).unwrap();
    let diag_address = unsafe { api::memory_address(diag.span(0, 256)) }.unwrap();
    // Two independent submissions consume two slots until an arena reset.
    let arena = unsafe {
        api::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 2,
                level: 0,
                reserved: 0,
            },
        )
    }
    .unwrap();
    let binding = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: heap.span(0, 80),
        reserved_offset: 0,
        reserved_size: 0,
    };

    // Submission 1: copy the pattern in, transform through slot 1 -> output.
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::copy_memory(list, input.span(0, bytes), staging.span(0, bytes)) };
    // The copy-in must be visible to the dispatch's reads and later copies.
    let copy_in_requirements = api::barrier_scratch_requirements(1, 0).unwrap();
    let (mut copy_in_scratch, copy_in_offset) = aligned_scratch(copy_in_requirements);
    let copy_in_barrier = MemoryBarrier {
        range: input.span(0, bytes),
        before: ACCESS_COPY_WRITE,
        after: ACCESS_COPY_READ | ACCESS_SHADER_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let copy_in_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_COMPUTE | STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &copy_in_barrier,
        images: ptr::null(),
        scratch: unsafe { copy_in_scratch.as_mut_ptr().add(copy_in_offset) }.cast(),
        scratch_size: copy_in_requirements.size,
    };
    unsafe { api::barrier(list, &copy_in_dependency) };
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 0, diag_address) };
    // Namespace: [8..24) = index_in, index_out, count, reserved.
    let constants = [0u32, 1, elements, 0];
    unsafe { api::set_inline(list, STAGE_COMPUTE, 8, bytemuck_bytes(&constants)) };
    unsafe { api::bind_heap(list, &binding) };
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
    let requirements = api::barrier_scratch_requirements(3, 0).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let barriers = [
        MemoryBarrier {
            range: output.span(0, bytes),
            before: ACCESS_STORAGE_WRITE,
            after: ACCESS_COPY_READ,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
        },
        MemoryBarrier {
            range: input.span(0, bytes),
            before: ACCESS_STORAGE_WRITE,
            after: ACCESS_COPY_READ,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
        },
        MemoryBarrier {
            range: diag.span(0, 256),
            before: ACCESS_STORAGE_WRITE,
            after: ACCESS_COPY_READ,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
        },
    ];
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COMPUTE,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 3,
        image_count: 0,
        flags: 0,
        memory: barriers.as_ptr(),
        images: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    unsafe { api::copy_memory(list, readback.span(0, bytes), output.span(0, bytes)) };
    unsafe { api::copy_memory(list, diag_readback.span(0, 256), diag.span(0, 256)) };
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
    // Mutation of an independent range plus a rebind: descriptor bytes are
    // captured at bind on this backend (docs/dx12-backend-d2.md).
    place_resource_recipes(
        device,
        heap,
        &[40],
        &[ResourceDescriptor {
            kind: DESCRIPTOR_STORAGE_BUFFER,
            image_state: STATE_GENERAL,
            view: ptr::null_mut(),
            buffer: second.span(0, bytes),
        }],
    );
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 0, diag_address) };
    let constants = [0u32, 1, elements / 2, 0];
    unsafe { api::set_inline(list, STAGE_COMPUTE, 8, bytemuck_bytes(&constants)) };
    unsafe { api::bind_heap(list, &binding) };
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
    let requirements = api::barrier_scratch_requirements(1, 0).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let barrier = MemoryBarrier {
        range: second.span(0, bytes),
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
    unsafe { api::copy_memory(list, readback2.span(0, bytes), second.span(0, bytes)) };
    unsafe { api::copy_memory(list, diag_readback.span(0, 256), diag.span(0, 256)) };
    let list = unsafe { api::commands_end(list) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let signal = SyncPoint {
        point: Point {
            timeline: timeline.as_raw(),
            value: 2,
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
    assert_eq!(unsafe { api::timeline_wait(timeline, 2, u64::MAX) }, Ok(()));

    let mut expected_full = Vec::with_capacity(bytes as usize);
    let mut expected_half = Vec::with_capacity(bytes as usize);
    for i in 0..elements {
        let base = (i as usize) * 4;
        let value = u32::from_le_bytes(pattern[base..base + 4].try_into().unwrap()) + 1;
        expected_full.extend_from_slice(&value.to_le_bytes());
        if i < elements / 2 {
            expected_half.extend_from_slice(&value.to_le_bytes());
        } else {
            expected_half.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    assert_eq!(read_back(readback, bytes), expected_full);
    assert_eq!(read_back(readback2, bytes), expected_half);
    // The diagnostic root-slot buffer proves the second dispatch executed.
    let diag_bytes = read_back(diag_readback, 256);
    assert_eq!(
        u32::from_le_bytes(diag_bytes[..4].try_into().unwrap()),
        elements / 2
    );

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::memory_destroy(diag_readback);
        api::memory_destroy(readback2);
        api::memory_destroy(readback);
        api::memory_destroy(heap);
        api::memory_destroy(second);
        api::memory_destroy(output);
        api::memory_destroy(input);
        api::memory_destroy(staging);
        api::memory_destroy(diag);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_descriptor_contract_rejections() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let bytes = 256u64;
    let plain = linear_memory(device, bytes, USAGE_STORAGE, 0);
    let heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    let span = plain.span(0, bytes);
    let recipe = ResourceDescriptor {
        kind: DESCRIPTOR_STORAGE_BUFFER,
        image_state: STATE_GENERAL,
        view: ptr::null_mut(),
        buffer: span,
    };

    // Recipe encoding rejects unusable outputs and unsupported descriptor kinds.
    let mut host = [0u8; 40];
    let short = HostSpan {
        data: host.as_mut_ptr().cast(),
        size: 39,
    };
    assert_eq!(
        unsafe {
            api::write_resource_descriptors(
                device,
                &[recipe],
                &[short],
                HostSpan {
                    data: ptr::null_mut(),
                    size: 0
                },
            )
        },
        Err(INVALID)
    );
    // Image recipes are implemented: they reject on missing views or on a
    // buffer span mixed into an image descriptor.
    let output = HostSpan {
        data: host.as_mut_ptr().cast(),
        size: 40,
    };
    let image_recipe = ResourceDescriptor {
        kind: DESCRIPTOR_SAMPLED_IMAGE,
        image_state: STATE_GENERAL,
        view: ptr::null_mut(),
        buffer: Span {
            memory: ptr::null_mut(),
            offset: 0,
            size: 0,
        },
    };
    assert_eq!(
        unsafe {
            api::write_resource_descriptors(
                device,
                &[image_recipe],
                &[output],
                HostSpan {
                    data: ptr::null_mut(),
                    size: 0
                },
            )
        },
        Err(INVALID)
    );
    let mixed_recipe = ResourceDescriptor {
        kind: DESCRIPTOR_SAMPLED_IMAGE,
        image_state: STATE_GENERAL,
        view: ptr::null_mut(),
        buffer: span,
    };
    assert_eq!(
        unsafe {
            api::write_resource_descriptors(
                device,
                &[mixed_recipe],
                &[output],
                HostSpan {
                    data: ptr::null_mut(),
                    size: 0
                },
            )
        },
        Err(INVALID)
    );

    // Bind rejects non-descriptor memory, misaligned ranges and reservations.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let bad = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: plain.span(0, 40),
        reserved_offset: 0,
        reserved_size: 0,
    };
    unsafe { api::bind_heap(list, &bad) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let misaligned = HeapBinding {
        storage: heap.span(8, 40),
        ..bad
    };
    unsafe { api::bind_heap(list, &misaligned) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let reserved = HeapBinding {
        storage: heap.span(0, 40),
        reserved_offset: 0,
        reserved_size: 40,
        ..bad
    };
    unsafe { api::bind_heap(list, &reserved) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));
    unsafe { api::arena_destroy(arena) };

    // Descriptor-heap memory requires a host-visible type on this backend.
    let attempted = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: 512,
                alignment: 256,
                usage: USAGE_DESCRIPTOR_HEAP,
                memory_type: 0,
                kind: 1,
                concurrent_domains: ptr::null(),
                concurrent_domain_count: 0,
                flags: 0,
            },
        )
    };
    assert_eq!(attempted, Err(UNSUPPORTED));

    unsafe {
        api::memory_destroy(heap);
        api::memory_destroy(plain);
        api::device_destroy(device);
    }
}

// ---- D2.3 GPU tests: render scopes, attachment views and load-op clears ----

fn attachment_view(
    device: api::DeviceHandle,
    image: api::ImageHandle,
    usage: u32,
) -> Result<api::ViewHandle, Status> {
    let desc = ViewDesc {
        header: Record::new::<ViewDesc>(VIEW_DESC),
        format: RGBA8_UNORM,
        dimension: VIEW_2D,
        usage,
        component_mapping: [0, 0, 0, 0],
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
    };
    unsafe { api::view_create(image, &desc) }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_render_scope_clear_readback() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 64u32;
    let texels = u64::from(extent * extent) * 4;
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
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let view = attachment_view(device, image, IMAGE_COLOR_ATTACHMENT as u32).unwrap();

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = unsafe { api::render_scratch_requirements(1) };
    let scratch_requirements = scratch_requirements.unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let clear = ClearValue {
        f32: [0.0, 0.0, 0.0, 1.0],
    };
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear,
    };
    // The caller owns attachment state: images start UNDEFINED and render
    // scopes infer no transition.
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);
    let to_attachment = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let state_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_COLOR,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_attachment,
        scratch: unsafe { state_scratch.as_mut_ptr().add(state_offset) }.cast(),
        scratch_size: state_requirements.size,
    };
    unsafe { api::barrier(list, &state_dependency) };
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            size: scratch_requirements.size,
        },
    };
        unsafe { api::render_begin(list, &render) };
        unsafe { api::render_end(list) };
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let barrier = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: ACCESS_COLOR_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COLOR_ATTACHMENT,
        new_state: STATE_COPY_SRC,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 0,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &barrier,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    let readback = linear_memory(device, texels, USAGE_COPY_DST, 2);
    let copy = ImageCopy {
        region: ImageRegion {
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
        },
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

    // Clear values 0.0/1.0 map exactly to UNORM bytes without rounding rules.
    let pixels = read_back(readback, texels);
    let expected: Vec<u8> = [0, 0, 0, 255]
        .iter()
        .cycle()
        .take(texels as usize)
        .copied()
        .collect();
    assert_eq!(pixels, expected);

    unsafe {
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::image_destroy(image);
        api::memory_destroy(backing);
        api::memory_destroy(readback);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_render_scope_contract_rejections() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let extent = 16u32;
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
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let view = attachment_view(device, image, IMAGE_COLOR_ATTACHMENT as u32).unwrap();

    let scratch_requirements = unsafe { api::render_scratch_requirements(1) }.unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };

    // RENDER_SECONDARY is the secondary-execution profile; reject visibly.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    // The caller owns attachment state: images start UNDEFINED and render
    // scopes infer no transition.
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);
    let to_attachment = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let state_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_COLOR,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_attachment,
        scratch: unsafe { state_scratch.as_mut_ptr().add(state_offset) }.cast(),
        scratch_size: state_requirements.size,
    };
    unsafe { api::barrier(list, &state_dependency) };
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: RENDER_SECONDARY,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            size: scratch_requirements.size,
        },
    };
    unsafe { api::render_begin(list, &render) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));
    unsafe { api::arena_destroy(arena) };

    // Attachment states must be the declared attachment states.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let bad = Attachment {
        state: STATE_COPY_DST,
        ..attachment
    };
    let render = RenderDesc {
        flags: 0,
        colors: &bad,
        ..render
    };
    unsafe { api::render_begin(list, &render) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    // render_end outside a scope is invalid.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::render_end(list) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    // Views without attachment usage cannot be attachment views.
    let bad_view = attachment_view(device, image, IMAGE_SAMPLED as u32);
    assert!(bad_view.is_err());

    unsafe {
        api::view_destroy(view);
        api::image_destroy(image);
        api::memory_destroy(backing);
        api::device_destroy(device);
    }
}

// ---- D2.3b GPU tests: graphics executables and rasterization ----

/// Fixture artifacts of tests/dx12-fixtures/triangle.slang.
static TRIANGLE_VS: AlignedArtifact<3336> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/triangle.vs.dxil"
));
static TRIANGLE_PS: AlignedArtifact<2790> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/triangle.ps.dxil"
));

fn triangle_executable(device: api::DeviceHandle) -> Result<api::ExecutableHandle, Status> {
    triangle_executable_with(device, None)
}

fn triangle_executable_with(
    device: api::DeviceHandle,
    cache: Option<api::ExecutableCacheHandle>,
) -> Result<api::ExecutableHandle, Status> {
    let abi = ArgumentInterface {
        header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
        byte_size: 0,
        root_count: 0,
        roots: ptr::null(),
    };
    let requirements = ShaderRequirements {
        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
        features: 0,
        local_size: [1, 1, 1],
        shared_memory: 0,
    };
    let vs = &TRIANGLE_VS.0;
    let ps = &TRIANGLE_PS.0;
    let shaders = [
        Shader {
            stage: STAGE_VERTEX as u32,
            format: SHADER_DXIL,
            code: Bytes {
                data: vs.as_ptr().cast(),
                size: vs.len(),
            },
            entry: c"vsMain".as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        },
        Shader {
            stage: STAGE_FRAGMENT as u32,
            format: SHADER_DXIL,
            code: Bytes {
                data: ps.as_ptr().cast(),
                size: ps.len(),
            },
            entry: c"fsMain".as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        },
    ];
    let color = ColorState {
        format: RGBA8_UNORM,
        write_mask: 0xf,
        blend: 0,
        src_color: 0,
        dst_color: 0,
        color_op: 0,
        src_alpha: 0,
        dst_alpha: 0,
        alpha_op: 0,
    };
    let graphics = GraphicsState {
        header: Record::new::<GraphicsState>(GRAPHICS_STATE),
        topology: TRIANGLES,
        cull: CULL_NONE,
        front_face: FRONT_CCW,
        samples: 1,
        color_count: 1,
        colors: &color,
        depth_format: 0,
        depth_test: 0,
        depth_write: 0,
        depth_compare: 0,
        blend_constants: [0.0; 4],
        stencil_test: 0,
        stencil_front: StencilState {
            fail: 0,
            pass: 0,
            depth_fail: 0,
            compare: 0,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        },
        stencil_back: StencilState {
            fail: 0,
            pass: 0,
            depth_fail: 0,
            compare: 0,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        },
        vertex_input: ptr::null(),
        raster_options: ptr::null(),
        logic_op_enable: 0,
        logic_op: 0,
    };
    let desc = ExecutableDesc {
        header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
        kind: EXECUTABLE_GRAPHICS,
        shader_count: 2,
        shaders: shaders.as_ptr(),
        static_state: &graphics.header,
        dynamic_state: DYNAMIC_VIEWPORT_SCISSOR,
        requirements: &requirements.header,
        cache: cache.map_or(ptr::null_mut(), |cache| cache.as_raw()),
        compile_flags: 0,
        reserved: 0,
    };
    unsafe { api::executable_create(device, &desc) }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_graphics_triangle_raster() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 64u32;
    let texels = u64::from(extent * extent) * 4;
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
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let view = attachment_view(device, image, IMAGE_COLOR_ATTACHMENT as u32).unwrap();
    let executable = triangle_executable(device).unwrap();

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = unsafe { api::render_scratch_requirements(1) }.unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 1.0, 0.0, 1.0],
        },
    };
    // The caller owns attachment state: images start UNDEFINED and render
    // scopes infer no transition.
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);
    let to_attachment = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let state_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_COLOR,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_attachment,
        scratch: unsafe { state_scratch.as_mut_ptr().add(state_offset) }.cast(),
        scratch_size: state_requirements.size,
    };
    unsafe { api::barrier(list, &state_dependency) };
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            size: scratch_requirements.size,
        },
    };
    unsafe { api::render_begin(list, &render) };
    unsafe { api::bind_executable(list, executable) };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: extent as f32,
        height: extent as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: extent,
        scissor_height: extent,
    };
        unsafe { api::set_graphics_state(list, &viewport) };
    let draw = DrawDesc {
        count: 3,
        instances: 1,
        first: 0,
        first_instance: 0,
        vertex_offset: 0,
    };
    unsafe { api::draw(list, &draw) };
    unsafe { api::render_end(list) };
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let barrier = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: ACCESS_COLOR_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COLOR_ATTACHMENT,
        new_state: STATE_COPY_SRC,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 0,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &barrier,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    let readback = linear_memory(device, texels, USAGE_COPY_DST, 2);
    let copy = ImageCopy {
        region: ImageRegion {
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
        },
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

    dump_validation(device);
    let pixels = read_back(readback, u64::from(extent) * 256);
    let pixel = |x: usize, y: usize| -> [u8; 4] {
        let base = y * 256 + x * 4;
        pixels[base..base + 4].try_into().unwrap()
    };
    // Center falls inside the drawn triangle: red over the green clear.
    assert_eq!(pixel(32, 32), [255, 0, 0, 255]);
    // Corners remain the clear color.
    assert_eq!(pixel(0, 0), [0, 255, 0, 255]);
    assert_eq!(pixel(63, 0), [0, 255, 0, 255]);
    assert_eq!(pixel(0, 63), [0, 255, 0, 255]);
    assert_eq!(pixel(63, 63), [0, 255, 0, 255]);

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::image_destroy(image);
        api::memory_destroy(backing);
        api::memory_destroy(readback);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_graphics_contract_rejections() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let executable = triangle_executable(device).unwrap();

    // PSO-baked dynamic states reject visibly on this backend.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let draw = DrawDesc {
        count: 3,
        instances: 1,
        first: 0,
        first_instance: 0,
        vertex_offset: 0,
    };
    unsafe { api::draw(list, &draw) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_cull(list, 0) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));
    unsafe { api::arena_destroy(arena) };

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    unsafe { api::set_stencil_compare_mask(list, 0, 0) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));
    unsafe { api::arena_destroy(arena) };

    // A zero-area viewport is invalid.
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::bind_executable(list, executable) };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: 1,
        scissor_height: 1,
    };
    unsafe { api::set_graphics_state(list, &viewport) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(INVALID));
    unsafe { api::arena_destroy(arena) };

    unsafe {
        api::executable_destroy(executable);
        api::device_destroy(device);
    }
}

// ---- D2.4 GPU tests: textured raster through descriptor tables ----

/// Fixture artifacts of tests/dx12-fixtures/textured.slang.
static TEXTURED_VS: AlignedArtifact<3299> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/textured.vs.dxil"
));
static TEXTURED_PS: AlignedArtifact<4423> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/textured.ps.dxil"
));

fn textured_executable(device: api::DeviceHandle) -> Result<api::ExecutableHandle, Status> {
    let abi = ArgumentInterface {
        header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
        byte_size: 0,
        root_count: 0,
        roots: ptr::null(),
    };
    let requirements = ShaderRequirements {
        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
        features: 0,
        local_size: [1, 1, 1],
        shared_memory: 0,
    };
    let vs = &TEXTURED_VS.0;
    let ps = &TEXTURED_PS.0;
    let shaders = [
        Shader {
            stage: STAGE_VERTEX as u32,
            format: SHADER_DXIL,
            code: Bytes {
                data: vs.as_ptr().cast(),
                size: vs.len(),
            },
            entry: c"vsMain".as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        },
        Shader {
            stage: STAGE_FRAGMENT as u32,
            format: SHADER_DXIL,
            code: Bytes {
                data: ps.as_ptr().cast(),
                size: ps.len(),
            },
            entry: c"fsMain".as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        },
    ];
    let bindings = [VertexBinding {
        binding: 0,
        stride: 16,
        rate: 0,
    }];
    let attributes = [
        VertexAttribute {
            location: 0,
            binding: 0,
            format: image_formats::RG32_FLOAT,
            offset: 0,
        },
        VertexAttribute {
            location: 1,
            binding: 0,
            format: image_formats::RG32_FLOAT,
            offset: 8,
        },
    ];
    let vertex_input = VertexInput {
        header: Record::new::<VertexInput>(VERTEX_INPUT),
        binding_count: 1,
        attribute_count: 2,
        bindings: bindings.as_ptr(),
        attributes: attributes.as_ptr(),
    };
    let color = ColorState {
        format: RGBA8_UNORM,
        write_mask: 0xf,
        blend: 0,
        src_color: 0,
        dst_color: 0,
        color_op: 0,
        src_alpha: 0,
        dst_alpha: 0,
        alpha_op: 0,
    };
    let graphics = GraphicsState {
        header: Record::new::<GraphicsState>(GRAPHICS_STATE),
        topology: TRIANGLES,
        cull: CULL_NONE,
        front_face: FRONT_CCW,
        samples: 1,
        color_count: 1,
        colors: &color,
        depth_format: 0,
        depth_test: 0,
        depth_write: 0,
        depth_compare: 0,
        blend_constants: [0.0; 4],
        stencil_test: 0,
        stencil_front: StencilState {
            fail: 0,
            pass: 0,
            depth_fail: 0,
            compare: 0,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        },
        stencil_back: StencilState {
            fail: 0,
            pass: 0,
            depth_fail: 0,
            compare: 0,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        },
        vertex_input: &vertex_input.header,
        raster_options: ptr::null(),
        logic_op_enable: 0,
        logic_op: 0,
    };
    let desc = ExecutableDesc {
        header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
        kind: EXECUTABLE_GRAPHICS,
        shader_count: 2,
        shaders: shaders.as_ptr(),
        static_state: &graphics.header,
        dynamic_state: DYNAMIC_VIEWPORT_SCISSOR,
        requirements: &requirements.header,
        cache: ptr::null_mut(),
        compile_flags: 0,
        reserved: 0,
    };
    unsafe { api::executable_create(device, &desc) }
}

#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_textured_raster() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 64u32;
    let texels = u64::from(extent * extent) * 4;

    // 4x4 texture with an exact per-texel pattern.
    let image_desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: 4, y: 4, z: 1 },
        usage: IMAGE_SAMPLED | IMAGE_COPY_DST | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
    let texture_backing = unsafe {
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
    let texture = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(texture, texture_backing.span(0, requirements.size)) }.unwrap();
    let staging = linear_memory(device, 1024, USAGE_COPY_SRC, 1);
    let mut pattern = vec![0u8; 1024];
    for ty in 0..4u32 {
        for tx in 0..4u32 {
            let base = (ty * 256 + tx * 4) as usize;
            pattern[base] = (tx * 60) as u8;
            pattern[base + 1] = (ty * 60) as u8;
            pattern[base + 2] = 100;
            pattern[base + 3] = 255;
        }
    }
    fill_upload(staging, &pattern);

    // Vertex buffer: full-screen triangle (position, uv) pairs.
    // Clip-space fullscreen triangle with UVs in ogpu/Vulkan convention
    // (+Y down): v=0 at clip y=-1 (framebuffer top).
    let vertices: Vec<u8> = [
        [-1.0f32, -1.0, 0.0, 0.0],
        [-1.0, 3.0, 0.0, 2.0],
        [3.0, -1.0, 2.0, 0.0],
    ]
    .iter()
    .flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes()))
    .collect();
    let vertex_memory = linear_memory(device, 4096, USAGE_VERTEX, 1);
    fill_upload(vertex_memory, &vertices);

    // Descriptor recipes: texture SRV at resource slot 0, sampler at slot 0.
    let resource_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    let sampler_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    let texture_view = unsafe {
        api::view_create(
            texture,
            &ViewDesc {
                header: Record::new::<ViewDesc>(VIEW_DESC),
                format: RGBA8_UNORM,
                dimension: VIEW_2D,
                usage: IMAGE_SAMPLED as u32,
                component_mapping: [0, 0, 0, 0],
                range: Subresources {
                    aspects: ASPECT_COLOR,
                    first_mip: 0,
                    mip_count: 1,
                    first_layer: 0,
                    layer_count: 1,
                },
            },
        )
    }
    .unwrap();
    place_resource_recipes(
        device,
        resource_heap,
        &[0],
        &[ResourceDescriptor {
            kind: DESCRIPTOR_SAMPLED_IMAGE,
            image_state: STATE_SHADER_READ,
            view: texture_view.as_raw(),
            buffer: Span {
                memory: ptr::null_mut(),
                offset: 0,
                size: 0,
            },
        }],
    );
    let sampler = SamplerDesc {
        header: Record::new::<SamplerDesc>(SAMPLER_DESC),
        min_filter: 0,
        mag_filter: 0,
        mip_filter: 0,
        address_mode: [0, 0, 0],
        compare_enable: 0,
        compare_op: 0,
        border_color: 0,
        unnormalized_coordinates: 0,
        min_lod: 0.0,
        max_lod: 1000.0,
        lod_bias: 0.0,
        max_anisotropy: 1.0,
    };
    let mut host = [[0u8; 80]; 1];
    let outputs = [HostSpan {
        data: host[0].as_mut_ptr().cast(),
        size: 80,
    }];
    unsafe {
        api::write_sampler_descriptors(
            device,
            &[sampler],
            &outputs,
            HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
        )
    }
    .unwrap();
    {
        let mapping = unsafe { api::memory_map(sampler_heap.span(0, 80)) }.unwrap();
        // SAFETY: the mapping covers the recipe slot.
        unsafe {
            ptr::copy_nonoverlapping(host[0].as_ptr(), mapping.data.cast::<u8>(), 80);
        }
        unsafe { api::memory_unmap(sampler_heap) };
    }

    let executable = textured_executable(device).unwrap();
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    // Upload the texture through an explicit copy with explicit states.
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let to_dst = ImageBarrier {
        image: texture.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
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
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent { x: 4, y: 4, z: 1 },
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_DST,
        reserved: 0,
    };
    unsafe { api::copy_to_image(list, texture, staging.span(0, 1024), &copy) };
    let to_shader = ImageBarrier {
        before: ACCESS_COPY_WRITE,
        after: ACCESS_SHADER_READ,
        old_state: STATE_COPY_DST,
        new_state: STATE_SHADER_READ,
        discard: 0,
        ..to_dst
    };
    let dependency = Dependency {
        images: &to_shader,
        ..dependency
    };
    unsafe { api::barrier(list, &dependency) };

    // Direct texture content check through the independently verified copy path.
    let texture_check = linear_memory(device, 1024, USAGE_COPY_DST, 2);
    let check_copy = ImageCopy {
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent { x: 4, y: 4, z: 1 },
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_SHADER_READ,
        reserved: 0,
    };
    // Render target with a black clear, then the textured triangle.
    let out_desc = ImageDesc {
        extent: Extent {
            x: extent,
            y: extent,
            z: 1,
        },
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        ..image_desc
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &out_desc, &mut requirements) }.unwrap();
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
    let output = unsafe { api::image_create_unbound(device, &out_desc) }.unwrap();
    unsafe { api::image_bind(output, backing.span(0, requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();
    let scratch_requirements = unsafe { api::render_scratch_requirements(1) }.unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    // The caller owns attachment state: images start UNDEFINED and render
    // scopes infer no transition.
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);
    let to_attachment = ImageBarrier {
        image: output.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let state_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_COLOR,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_attachment,
        scratch: unsafe { state_scratch.as_mut_ptr().add(state_offset) }.cast(),
        scratch_size: state_requirements.size,
    };
    unsafe { api::barrier(list, &state_dependency) };
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            size: scratch_requirements.size,
        },
    };
    unsafe { api::render_begin(list, &render) };
    unsafe { api::bind_executable(list, executable) };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: extent as f32,
        height: extent as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: extent,
        scissor_height: extent,
    };
    unsafe { api::set_graphics_state(list, &viewport) };
    let scratch_requirements = unsafe { api::vertex_scratch_requirements(1) }.unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let vertex_span = vertex_memory.span(0, 48);
    unsafe {
        api::bind_vertices(
            list,
            0,
            &[vertex_span],
            HostSpan {
                data: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
                size: scratch_requirements.size,
            },
        )
    };
    let resource_binding = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: resource_heap.span(0, 40),
        reserved_offset: 0,
        reserved_size: 0,
    };
    let sampler_binding = HeapBinding {
        kind: HEAP_SAMPLER,
        storage: sampler_heap.span(0, 80),
        ..resource_binding
    };
        unsafe { api::bind_heap(list, &resource_binding) };
        unsafe { api::bind_heap(list, &sampler_binding) };
    let draw = DrawDesc {
        count: 3,
        instances: 1,
        first: 0,
        first_instance: 0,
        vertex_offset: 0,
    };
    unsafe { api::draw(list, &draw) };
    unsafe { api::render_end(list) };
    // The texture was sampled in SHADER_READ; transition it for the content check.
    let check_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut check_scratch, check_offset) = aligned_scratch(check_requirements);
    let to_check = ImageBarrier {
        image: texture.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: ACCESS_SHADER_READ,
        after: ACCESS_COPY_READ,
        old_state: STATE_SHADER_READ,
        new_state: STATE_COPY_SRC,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 0,
    };
    let check_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_FRAGMENT,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_check,
        scratch: unsafe { check_scratch.as_mut_ptr().add(check_offset) }.cast(),
        scratch_size: check_requirements.size,
    };
    unsafe { api::barrier(list, &check_dependency) };
    let check_copy = ImageCopy {
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent { x: 4, y: 4, z: 1 },
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    unsafe { api::copy_from_image(list, texture_check.span(0, 1024), texture, &check_copy) };

    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let to_copy = ImageBarrier {
        image: output.as_raw(),
        before: ACCESS_COLOR_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COLOR_ATTACHMENT,
        new_state: STATE_COPY_SRC,
        discard: 0,
        ..to_dst
    };
    let dependency = Dependency {
        images: &to_copy,
        ..dependency
    };
    unsafe { api::barrier(list, &dependency) };
    let readback = linear_memory(device, u64::from(extent) * 256, USAGE_COPY_DST, 2);
    let copy_out = ImageCopy {
        region: ImageRegion {
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
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    let readback_size = u64::from(extent) * 256;
    unsafe { api::copy_from_image(list, readback.span(0, readback_size), output, &copy_out) };
    let list = unsafe { api::commands_end(list) };
    if list.is_err() {
        dump_validation(device);
    }
    let list = list.unwrap();

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

    dump_validation(device);
    let texture_bytes = read_back(texture_check, 1024);
    eprintln!("DX12PROBE texture texel(0,0) = {:?}", &texture_bytes[..4]);
    eprintln!("DX12PROBE texture row0       = {:?}", &texture_bytes[..16]);
    let pixels = read_back(readback, u64::from(extent) * 256);
    let pixel = |x: usize, y: usize| -> [u8; 4] {
        let base = y * 256 + x * 4;
        pixels[base..base + 4].try_into().unwrap()
    };
    // DIAGNOSTIC: the shader reports texel(0,0).r and the interpolated UVs.
    // UVs interpolate as ((x+0.5)/extent, (y+0.5)/extent) in this topology.
    // CPU oracle: the fixture snaps UVs to texel centres, so the linear
    // filter returns the exact texel floor(u*4) x floor(v*4) at every pixel.
    let expected = |x: usize, y: usize| -> [u8; 4] {
        let (u, v) = (x as f32 / 64.0, y as f32 / 64.0);
        let col = (u * 4.0).floor().clamp(0.0, 3.0) as u8;
        let row = (v * 4.0).floor().clamp(0.0, 3.0) as u8;
        [col * 60, row * 60, 100, 255]
    };
    for (x, y) in [(0usize, 0usize), (32, 32), (63, 63), (16, 48), (48, 16)] {
        let got = pixel(x, y);
        let want = expected(x, y);
        // Float-to-UNORM rounding may differ by one byte.
        for (g, w) in got.iter().zip(want.iter()) {
            assert!(
                (*g as i32 - *w as i32).abs() <= 1,
                "pixel ({}, {}): got {:?} want {:?}",
                x,
                y,
                got,
                want
            );
        }
    }

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::view_destroy(texture_view);
        api::image_destroy(output);
        api::image_destroy(texture);
        api::memory_destroy(readback);
        api::memory_destroy(sampler_heap);
        api::memory_destroy(resource_heap);
        api::memory_destroy(vertex_memory);
        api::memory_destroy(staging);
        api::memory_destroy(backing);
        api::memory_destroy(texture_backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

#[ignore = "probe"]
#[test]
fn probe_debug_layer_pso() {
    // Enable the OS debug layer before any device creation, then read the
    // validation messages the PSO preparation triggers.
    let mut debug: Option<d3d::ID3D12Debug> = None;
    let result = unsafe { d3d::D3D12GetDebugInterface(&mut debug) };
    eprintln!("DX12PROBE debug interface: {:#x}", result.as_ref().err().map_or(0, |e| e.code().0 as u32));
    if let Some(debug) = debug.as_ref() {
        unsafe { debug.EnableDebugLayer() };
        eprintln!("DX12PROBE debug layer ENABLED");
    }
    let (_discovery, device, _queue, _domain) = gpu_setup();
    let handle_ptr = device.as_raw();
    // SAFETY: the handle identifies the live device created above.
    let device_ref: &Device = unsafe { &*handle_ptr };
    let outcome = textured_executable(device);
    eprintln!("DX12PROBE textured executable: {:?}", outcome.map(|_| "ok").map_err(|s| s));
    if let Ok(executable) = outcome {
        unsafe { api::executable_destroy(executable) };
    }
    if let Ok(info) = device_ref.device.cast::<d3d::ID3D12InfoQueue>() {
        let count = unsafe { info.GetNumStoredMessages() };
        eprintln!("DX12PROBE validation messages: {}", count);
        for i in 0..count.min(20) {
            let mut len = 0usize;
            unsafe {
                info.GetMessage(i, None, &mut len);
                let mut buffer = vec![0u8; len];
                info.GetMessage(i, Some(buffer.as_mut_ptr().cast()), &mut len);
                let message: &d3d::D3D12_MESSAGE = &*buffer.as_ptr().cast();
                let text = std::slice::from_raw_parts(
                    message.pDescription.cast::<u8>(),
                    message.DescriptionByteLength,
                );
                eprintln!(
                    "DX12PROBE msg[{}]: {}",
                    i,
                    String::from_utf8_lossy(text).trim_end_matches('\0')
                );
            }
        }
    }
    unsafe { api::device_destroy(device) };
}


// ---- M4 scene-shape GPU test: shared inline bank, root-descriptor vertices,
// descriptor-indexed sampling, indexed draws with depth rejection ----

/// Fixture artifact of tests/dx12-fixtures/scene.slang (vertex stage).
static SCENE_VS: AlignedArtifact<4110> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/scene.vs.dxil"
));

/// Fixture artifact of tests/dx12-fixtures/scene.slang (fragment stage).
static SCENE_FS: AlignedArtifact<5252> = AlignedArtifact(*include_bytes!(
    "../../../../../tests/dx12-fixtures/scene.fs.dxil"
));

/// Prepare the M4 scene fixture: 24-byte shared namespace, one root slot at
/// [0..8), no vertex input (positions arrive through the root descriptor).
fn scene_executable(device: api::DeviceHandle) -> Result<api::ExecutableHandle, Status> {
    let roots = [RootSlot {
        stages: STAGE_VERTEX,
        offset: 0,
        alignment: 8,
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
        local_size: [1, 1, 1],
        shared_memory: 0,
    };
    let vs = &SCENE_VS.0;
    let fs = &SCENE_FS.0;
    let shaders = [
        Shader {
            stage: STAGE_VERTEX as u32,
            format: SHADER_DXIL,
            code: Bytes {
                data: vs.as_ptr().cast(),
                size: vs.len(),
            },
            entry: c"vsMain".as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        },
        Shader {
            stage: STAGE_FRAGMENT as u32,
            format: SHADER_DXIL,
            code: Bytes {
                data: fs.as_ptr().cast(),
                size: fs.len(),
            },
            entry: c"fsMain".as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        },
    ];
    let vertex_input = VertexInput {
        header: Record::new::<VertexInput>(VERTEX_INPUT),
        binding_count: 0,
        attribute_count: 0,
        bindings: ptr::null(),
        attributes: ptr::null(),
    };
    let color = ColorState {
        format: RGBA8_UNORM,
        write_mask: 0xf,
        blend: 0,
        src_color: 0,
        dst_color: 0,
        color_op: 0,
        src_alpha: 0,
        dst_alpha: 0,
        alpha_op: 0,
    };
    let graphics = GraphicsState {
        header: Record::new::<GraphicsState>(GRAPHICS_STATE),
        topology: TRIANGLES,
        cull: CULL_NONE,
        front_face: FRONT_CCW,
        samples: 1,
        color_count: 1,
        colors: &color,
        depth_format: 14,
        depth_test: 1,
        depth_write: 1,
        depth_compare: 1,
        blend_constants: [0.0; 4],
        stencil_test: 0,
        stencil_front: StencilState {
            fail: 0,
            pass: 0,
            depth_fail: 0,
            compare: 0,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        },
        stencil_back: StencilState {
            fail: 0,
            pass: 0,
            depth_fail: 0,
            compare: 0,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        },
        vertex_input: &vertex_input.header,
        raster_options: ptr::null(),
        logic_op_enable: 0,
        logic_op: 0,
    };
    let desc = ExecutableDesc {
        header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
        kind: EXECUTABLE_GRAPHICS,
        shader_count: 2,
        shaders: shaders.as_ptr(),
        static_state: &graphics.header,
        dynamic_state: DYNAMIC_VIEWPORT_SCISSOR,
        requirements: &requirements.header,
        cache: ptr::null_mut(),
        compile_flags: 0,
        reserved: 0,
    };
    unsafe { api::executable_create(device, &desc) }
}

/// The M4 argument shape end to end: shared inline bank read by both stages,
/// a flat root-descriptor vertex buffer, descriptor-indexed image and sampler
/// selection, an indexed two-quad draw with depth rejection, and a scissored
/// overlay phase. Geometry: quad N (surface 0) covers pixels [4,28)^2 at
/// z=0.25; quad F (surface 1) covers [16,32)^2 at z=0.75, drawn second and
/// depth-rejected where they overlap.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_scene_raster() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 32u32;
    let out_bytes = u64::from(extent) * 256;

    // Two 2x2 solid textures at descriptor slots 0 and 1: sampling a constant
    // is constant at any UV (exact pixels) while descriptor-index selection
    // stays observable through their different colours.
    let image_desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: 2, y: 2, z: 1 },
        usage: IMAGE_SAMPLED | IMAGE_COPY_DST,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
    let mut textures = Vec::new();
    let mut texture_backings = Vec::new();
    let mut texture_views = Vec::new();
    for _ in 0..2 {
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
        let texture = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
        unsafe { api::image_bind(texture, backing.span(0, requirements.size)) }.unwrap();
        let view = unsafe {
            api::view_create(
                texture,
                &ViewDesc {
                    header: Record::new::<ViewDesc>(VIEW_DESC),
                    format: RGBA8_UNORM,
                    dimension: VIEW_2D,
                    usage: IMAGE_SAMPLED as u32,
                    component_mapping: [0, 0, 0, 0],
                    range: Subresources {
                        aspects: ASPECT_COLOR,
                        first_mip: 0,
                        mip_count: 1,
                        first_layer: 0,
                        layer_count: 1,
                    },
                },
            )
        }
        .unwrap();
        textures.push(texture);
        texture_backings.push(backing);
        texture_views.push(view);
    }

    // Root-descriptor vertex buffer: two quads x four corners x four floats.
    let vertices = linear_memory(device, 256, USAGE_STORAGE | USAGE_COPY_DST, 0);
    let mut vertex_data = [0f32; 32];
    for (surface, (min, max, z)) in [
        ((-0.75f32, -0.75f32), (0.75f32, 0.75f32), 0.25f32),
        ((0.0, -1.0), (1.0, 0.0), 0.75),
    ]
    .into_iter()
    .enumerate()
    {
        for (corner, [x, y]) in [[min.0, min.1], [max.0, min.1], [max.0, max.1], [min.0, max.1]]
            .into_iter()
            .enumerate()
        {
            for (k, v) in [x, y, z, 1.0f32].into_iter().enumerate() {
                vertex_data[surface * 16 + corner * 4 + k] = v;
            }
        }
    }
    let vertex_staging = linear_memory(device, 256, USAGE_COPY_SRC, 1);
    {
        let mapping = unsafe { api::memory_map(vertex_staging.span(0, 128)) }.unwrap();
        // SAFETY: the mapping covers the vertex region.
        unsafe {
            ptr::copy_nonoverlapping(
                vertex_data.as_ptr().cast::<u8>(),
                mapping.data.cast::<u8>(),
                128,
            );
        }
        unsafe { api::memory_unmap(vertex_staging) };
    }
    let vertices_address = unsafe { api::memory_address(vertices.span(0, 128)) }.unwrap();

    // Indexed draw: two triangles per quad, quad vertices 0..3 and 4..7.
    let indices = linear_memory(device, 256, USAGE_INDEX, 1);
    let index_data: [u32; 12] = [0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4];
    {
        let mapping = unsafe { api::memory_map(indices.span(0, 48)) }.unwrap();
        // SAFETY: the mapping covers the index region.
        unsafe {
            ptr::copy_nonoverlapping(
                index_data.as_ptr().cast::<u8>(),
                mapping.data.cast::<u8>(),
                48,
            );
        }
        unsafe { api::memory_unmap(indices) };
    }

    // Descriptor heaps: two image recipes (40-byte slots) and one sampler.
    let resource_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    let sampler_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    place_resource_recipes(
        device,
        resource_heap,
        &[0, 40],
        &[
            ResourceDescriptor {
                kind: DESCRIPTOR_SAMPLED_IMAGE,
                image_state: STATE_SHADER_READ,
                view: texture_views[0].as_raw(),
                buffer: Span {
                    memory: ptr::null_mut(),
                    offset: 0,
                    size: 0,
                },
            },
            ResourceDescriptor {
                kind: DESCRIPTOR_SAMPLED_IMAGE,
                image_state: STATE_SHADER_READ,
                view: texture_views[1].as_raw(),
                buffer: Span {
                    memory: ptr::null_mut(),
                    offset: 0,
                    size: 0,
                },
            },
        ],
    );
    let sampler = SamplerDesc {
        header: Record::new::<SamplerDesc>(SAMPLER_DESC),
        min_filter: 0,
        mag_filter: 0,
        mip_filter: 0,
        address_mode: [0, 0, 0],
        compare_enable: 0,
        compare_op: 0,
        border_color: 0,
        unnormalized_coordinates: 0,
        min_lod: 0.0,
        max_lod: 1000.0,
        lod_bias: 0.0,
        max_anisotropy: 1.0,
    };
    let mut host = [[0u8; 80]; 1];
    let outputs = [HostSpan {
        data: host[0].as_mut_ptr().cast(),
        size: 80,
    }];
    unsafe {
        api::write_sampler_descriptors(
            device,
            &[sampler],
            &outputs,
            HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
        )
    }
    .unwrap();
    {
        let mapping = unsafe { api::memory_map(sampler_heap.span(0, 80)) }.unwrap();
        // SAFETY: the mapping covers the recipe slot.
        unsafe {
            ptr::copy_nonoverlapping(host[0].as_ptr(), mapping.data.cast::<u8>(), 80);
        }
        unsafe { api::memory_unmap(sampler_heap) };
    }

    // Output and depth attachments.
    let out_desc = ImageDesc {
        extent: Extent {
            x: extent,
            y: extent,
            z: 1,
        },
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        ..image_desc
    };
    let mut out_requirements = Requirements::default();
    unsafe { api::image_requirements(device, &out_desc, &mut out_requirements) }.unwrap();
    let out_backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: out_requirements.size,
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
    let output = unsafe { api::image_create_unbound(device, &out_desc) }.unwrap();
    unsafe { api::image_bind(output, out_backing.span(0, out_requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();

    let depth_desc = ImageDesc {
        format: 14,
        usage: IMAGE_DEPTH_STENCIL_ATTACHMENT,
        ..image_desc
    };
    let depth_desc = ImageDesc {
        extent: Extent {
            x: extent,
            y: extent,
            z: 1,
        },
        ..depth_desc
    };
    let mut depth_requirements = Requirements::default();
    unsafe { api::image_requirements(device, &depth_desc, &mut depth_requirements) }.unwrap();
    let depth_backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: depth_requirements.size,
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
    let depth = unsafe { api::image_create_unbound(device, &depth_desc) }.unwrap();
    unsafe { api::image_bind(depth, depth_backing.span(0, depth_requirements.size)) }.unwrap();
    let depth_view = unsafe {
        api::view_create(
            depth,
            &ViewDesc {
                header: Record::new::<ViewDesc>(VIEW_DESC),
                format: 14,
                dimension: VIEW_2D,
                usage: IMAGE_DEPTH_STENCIL_ATTACHMENT as u32,
                component_mapping: [0, 0, 0, 0],
                range: Subresources {
                    aspects: ASPECT_DEPTH,
                    first_mip: 0,
                    mip_count: 1,
                    first_layer: 0,
                    layer_count: 1,
                },
            },
        )
    }
    .unwrap();

    let executable = scene_executable(device);
    if executable.is_err() {
        dump_validation(device);
    }
    let executable = executable.unwrap();
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = api::barrier_scratch_requirements(0, 3).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);

    // Upload both solid textures through explicit copies with explicit states.
    let staging = linear_memory(device, 1024, USAGE_COPY_SRC, 1);
    let mut upload = vec![0u8; 1024];
    for (slot, color) in [[64u8, 128, 255, 255], [200, 100, 50, 255]]
        .into_iter()
        .enumerate()
    {
        for row in 0..2 {
            for col in 0..2 {
                let base = slot * 512 + row * 256 + col * 4;
                upload[base..base + 4].copy_from_slice(&color);
            }
        }
    }
    fill_upload(staging, &upload);
    let mut upload_barriers = Vec::new();
    for (index, texture) in textures.iter().enumerate() {
        let to_dst = ImageBarrier {
            image: texture.as_raw(),
            range: Subresources {
                aspects: ASPECT_COLOR,
                first_mip: 0,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            before: 0,
            after: ACCESS_COPY_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COPY_DST,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
            discard: 1,
        };
        let to_shader = ImageBarrier {
            before: ACCESS_COPY_WRITE,
            after: ACCESS_SHADER_READ,
            old_state: STATE_COPY_DST,
            new_state: STATE_SHADER_READ,
            ..to_dst
        };
        upload_barriers.push((index, to_dst, to_shader));
    }
    for (_, to_dst, _) in &upload_barriers {
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
            images: to_dst,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
    }
    unsafe { api::copy_memory(list, vertices.span(0, 128), vertex_staging.span(0, 128)) };
    for (index, _, _) in &upload_barriers {
        let copy = ImageCopy {
            region: ImageRegion {
                aspect: ASPECT_COLOR,
                mip: 0,
                first_layer: 0,
                layer_count: 1,
                offset: Offset { x: 0, y: 0, z: 0 },
                extent: Extent { x: 2, y: 2, z: 1 },
            },
            row_pitch: 256,
            slice_pitch: 0,
            state: STATE_COPY_DST,
            reserved: 0,
        };
        unsafe {
            api::copy_to_image(
                list,
                textures[*index],
                staging.span(*index as u64 * 512, 512),
                &copy,
            )
        };
    }
    for (_, _, to_shader) in &upload_barriers {
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_TRANSFER,
            after: STAGE_FRAGMENT,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            images: to_shader,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
    }

    // This runtime's enhanced-barrier validation requires buffer barriers to
    // cover the whole resource (Size = width or UINT64_MAX): sub-ranges are
    // rejected at record time.
    let vertex_barrier = MemoryBarrier {
        range: vertices.span(0, 256),
        before: ACCESS_COPY_WRITE,
        after: ACCESS_SHADER_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let vertex_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_VERTEX,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &vertex_barrier,
        images: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &vertex_dependency) };

    // Depth enters its attachment state once and stays there.
    let to_depth = ImageBarrier {
        image: depth.as_raw(),
        range: Subresources {
            aspects: ASPECT_DEPTH,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_DEPTH_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_DEPTH_STENCIL_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };

    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    let depth_attachment = Attachment {
        view: depth_view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_DEPTH_STENCIL_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [1.0, 0.0, 0.0, 0.0],
        },
    };
    let render_requirements = api::render_scratch_requirements(2).unwrap();
    let (mut render_scratch, render_offset) = aligned_scratch(render_requirements);
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: &depth_attachment,
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { render_scratch.as_mut_ptr().add(render_offset) }.cast(),
            size: render_requirements.size,
        },
    };
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);

    let readback = linear_memory(device, out_bytes * 3, USAGE_COPY_DST, 2);
    let copy_out = ImageCopy {
        region: ImageRegion {
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
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    let resource_binding = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: resource_heap.span(0, 80),
        reserved_offset: 0,
        reserved_size: 0,
    };
    let sampler_binding = HeapBinding {
        kind: HEAP_SAMPLER,
        storage: sampler_heap.span(0, 80),
        ..resource_binding
    };

    let to_depth_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_depth,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &to_depth_dependency) };

    let mut check = |list: api::ListHandle,
                     phase: u64,
                     image: u32,
                     overlay: u32,
                     scissor: (i32, i32, u32, u32)| {
        // The caller owns attachment states: colour re-enters the attachment
        // state per phase (content is cleared), depth stays in it.
        let to_attachment = ImageBarrier {
            image: output.as_raw(),
            range: Subresources {
                aspects: ASPECT_COLOR,
                first_mip: 0,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
            discard: 1,
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_TRANSFER,
            after: STAGE_FRAGMENT,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            images: &to_attachment,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
        let mut bank = [0u8; 24];
        bank[0..8].copy_from_slice(&vertices_address.to_ne_bytes());
        bank[8..12].copy_from_slice(&image.to_ne_bytes());
        bank[12..16].copy_from_slice(&0u32.to_ne_bytes());
        bank[16..20].copy_from_slice(&0f32.to_ne_bytes());
        bank[20..24].copy_from_slice(&overlay.to_ne_bytes());
        let viewport = ViewportState {
            header: Record::new::<ViewportState>(VIEWPORT_STATE),
            x: 0.0,
            y: 0.0,
            width: extent as f32,
            height: extent as f32,
            min_depth: 0.0,
            max_depth: 1.0,
            scissor_x: scissor.0,
            scissor_y: scissor.1,
            scissor_width: scissor.2,
            scissor_height: scissor.3,
        };
        unsafe { api::render_begin(list, &render) };
        unsafe { api::bind_executable(list, executable) };
        unsafe { api::set_graphics_state(list, &viewport) };
        unsafe { api::set_root(list, STAGE_VERTEX, 0, vertices_address) };
        unsafe { api::set_inline(list, STAGE_VERTEX | STAGE_FRAGMENT, 0, &bank) };
        unsafe { api::bind_heap(list, &resource_binding) };
        unsafe { api::bind_heap(list, &sampler_binding) };
        unsafe { api::bind_indices(list, indices.span(0, 48), INDEX_U32) };
        let draw = DrawDesc {
            count: 12,
            instances: 1,
            first: 0,
            first_instance: 0,
            vertex_offset: 0,
        };
        unsafe { api::draw_indexed(list, &draw) };
        unsafe { api::render_end(list) };
        let to_copy = ImageBarrier {
            before: ACCESS_COLOR_WRITE,
            after: ACCESS_COPY_READ,
            old_state: STATE_COLOR_ATTACHMENT,
            new_state: STATE_COPY_SRC,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_FRAGMENT,
            after: STAGE_TRANSFER,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            images: &to_copy,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe {
            api::copy_from_image(
                list,
                readback.span(phase * out_bytes, out_bytes),
                output,
                &copy_out,
            )
        };
    };

    // Phase 1: texture 0, no overlay. Quad N shows through; quad F is
    // depth-rejected where it overlaps (drawn second, farther).
    check(list, 0, 0, 0, (0, 0, extent, extent));
    // Phase 2: texture 1 through the same descriptor index channel.
    check(list, 1, 1, 0, (0, 0, extent, extent));
    // Phase 3: overlay bank value, scissored to [8,24) x [10,20).
    check(list, 2, 0, 1, (8, 10, 16, 10));

    let list = unsafe { api::commands_end(list) };
    if list.is_err() {
        dump_validation(device);
    }
    let list = list.unwrap();
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
    dump_validation(device);

    // Verify each phase's readback region once the submission completed.
    let pixels = read_back(readback, out_bytes * 3);
    let verify = |phase: usize, samples: &[(usize, usize, [u8; 4])]| {
        let pixel = |x: usize, y: usize| -> [u8; 4] {
            let base = phase * out_bytes as usize + y * 256 + x * 4;
            pixels[base..base + 4].try_into().unwrap()
        };
        for &(x, y, want) in samples {
            let got = pixel(x, y);
            for (g, w) in got.iter().zip(want.iter()) {
                assert!(
                    (*g as i32 - *w as i32).abs() <= 1,
                    "phase {phase} pixel ({x}, {y}): got {got:?} want {want:?}"
                );
            }
        }
    };
    verify(
        0,
        &[
            (16, 20, [64, 128, 255, 255]),
            (8, 8, [64, 128, 255, 255]),
            // Quad 1 is clip (0,-1)-(1,0): Vulkan top-right, not D3D bottom-right.
            (30, 2, [32, 128, 128, 255]),
            (2, 2, [0, 0, 0, 255]),
        ],
    );
    verify(
        1,
        &[
            (16, 20, [200, 100, 50, 255]),
            (30, 2, [100, 100, 25, 255]),
        ],
    );
    verify(
        2,
        &[
            (16, 14, [64, 128, 255, 128]),
            (2, 2, [0, 0, 0, 255]),
        ],
    );

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(depth_view);
        api::view_destroy(view);
        for view in texture_views {
            api::view_destroy(view);
        }
        api::image_destroy(depth);
        api::image_destroy(output);
        for texture in textures {
            api::image_destroy(texture);
        }
        api::memory_destroy(readback);
        api::memory_destroy(staging);
        api::memory_destroy(indices);
        api::memory_destroy(vertex_staging);
        api::memory_destroy(vertices);
        api::memory_destroy(sampler_heap);
        api::memory_destroy(resource_heap);
        api::memory_destroy(depth_backing);
        api::memory_destroy(out_backing);
        for backing in texture_backings {
            api::memory_destroy(backing);
        }
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Executable cache contract: advisory native cached pipeline-state blobs,
/// export/import with identity checking, valid partial export at capacity, and
/// merge preserving the destination.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_scene_consumer_clip_sample() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let width = 640u32;
    let height = 480u32;
    let out_bytes = u64::from(width) * u64::from(height) * 4;

    // Two 2x2 solid textures at descriptor slots 0 and 1: sampling a constant
    // is constant at any UV (exact pixels) while descriptor-index selection
    // stays observable through their different colours.
    let image_desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: 2, y: 2, z: 1 },
        usage: IMAGE_SAMPLED | IMAGE_COPY_DST,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
    let mut textures = Vec::new();
    let mut texture_backings = Vec::new();
    let mut texture_views = Vec::new();
    for _ in 0..2 {
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
        let texture = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
        unsafe { api::image_bind(texture, backing.span(0, requirements.size)) }.unwrap();
        let view = unsafe {
            api::view_create(
                texture,
                &ViewDesc {
                    header: Record::new::<ViewDesc>(VIEW_DESC),
                    format: RGBA8_UNORM,
                    dimension: VIEW_2D,
                    usage: IMAGE_SAMPLED as u32,
                    component_mapping: [0, 0, 0, 0],
                    range: Subresources {
                        aspects: ASPECT_COLOR,
                        first_mip: 0,
                        mip_count: 1,
                        first_layer: 0,
                        layer_count: 1,
                    },
                },
            )
        }
        .unwrap();
        textures.push(texture);
        texture_backings.push(backing);
        texture_views.push(view);
    }

    // Root-descriptor vertex buffer: two quads x four corners x four floats.
    // Consumer writes vertices through a host-visible storage buffer.
    let vertices = linear_memory(device, 256, USAGE_STORAGE | USAGE_INDEX, 1);
    let mut vertex_data = [0f32; 32];
    // Consumer --scene-check frame 0 camera (x=-0.08, y=-0.04, zoom=0.9).
    for (surface, (min, max, z)) in [
        ((-0.80f32, -0.67f32), (0.505f32, 0.59f32), 0.7f32),
        ((-0.395, -0.40), (0.595, 0.365), 0.3),
    ]
    .into_iter()
    .enumerate()
    {
        for (corner, [x, y]) in [[min.0, min.1], [max.0, min.1], [max.0, max.1], [min.0, max.1]]
            .into_iter()
            .enumerate()
        {
            for (k, v) in [x, y, z, 1.0f32].into_iter().enumerate() {
                vertex_data[surface * 16 + corner * 4 + k] = v;
            }
        }
    }
    {
        let mapping = unsafe { api::memory_map(vertices.span(0, 128)) }.unwrap();
        unsafe {
            ptr::copy_nonoverlapping(
                vertex_data.as_ptr().cast::<u8>(),
                mapping.data.cast::<u8>(),
                128,
            );
        }
        unsafe { api::memory_unmap(vertices) };
    }
    let vertices_address = unsafe { api::memory_address(vertices.span(0, 128)) }.unwrap();

    // Indexed draw: two triangles per quad, quad vertices 0..3 and 4..7.
    let indices = linear_memory(device, 256, USAGE_INDEX, 1);
    let index_data: [u32; 12] = [4, 5, 6, 4, 6, 7, 0, 1, 2, 0, 2, 3];
    {
        let mapping = unsafe { api::memory_map(indices.span(0, 48)) }.unwrap();
        // SAFETY: the mapping covers the index region.
        unsafe {
            ptr::copy_nonoverlapping(
                index_data.as_ptr().cast::<u8>(),
                mapping.data.cast::<u8>(),
                48,
            );
        }
        unsafe { api::memory_unmap(indices) };
    }

    // Descriptor heaps: two image recipes (40-byte slots) and one sampler.
    let resource_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    let sampler_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    place_resource_recipes(
        device,
        resource_heap,
        &[0, 40],
        &[
            ResourceDescriptor {
                kind: DESCRIPTOR_SAMPLED_IMAGE,
                image_state: STATE_SHADER_READ,
                view: texture_views[0].as_raw(),
                buffer: Span {
                    memory: ptr::null_mut(),
                    offset: 0,
                    size: 0,
                },
            },
            ResourceDescriptor {
                kind: DESCRIPTOR_SAMPLED_IMAGE,
                image_state: STATE_SHADER_READ,
                view: texture_views[1].as_raw(),
                buffer: Span {
                    memory: ptr::null_mut(),
                    offset: 0,
                    size: 0,
                },
            },
        ],
    );
    let sampler = SamplerDesc {
        header: Record::new::<SamplerDesc>(SAMPLER_DESC),
        min_filter: 0,
        mag_filter: 0,
        mip_filter: 0,
        address_mode: [0, 0, 0],
        compare_enable: 0,
        compare_op: 0,
        border_color: 0,
        unnormalized_coordinates: 0,
        min_lod: 0.0,
        max_lod: 1000.0,
        lod_bias: 0.0,
        max_anisotropy: 1.0,
    };
    let mut host = [[0u8; 80]; 1];
    let outputs = [HostSpan {
        data: host[0].as_mut_ptr().cast(),
        size: 80,
    }];
    unsafe {
        api::write_sampler_descriptors(
            device,
            &[sampler],
            &outputs,
            HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
        )
    }
    .unwrap();
    {
        let mapping = unsafe { api::memory_map(sampler_heap.span(0, 80)) }.unwrap();
        // SAFETY: the mapping covers the recipe slot.
        unsafe {
            ptr::copy_nonoverlapping(host[0].as_ptr(), mapping.data.cast::<u8>(), 80);
        }
        unsafe { api::memory_unmap(sampler_heap) };
    }

    // Output and depth attachments.
    let out_desc = ImageDesc {
        extent: Extent {
            x: width,
            y: height,
            z: 1,
        },
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        ..image_desc
    };
    let mut out_requirements = Requirements::default();
    unsafe { api::image_requirements(device, &out_desc, &mut out_requirements) }.unwrap();
    let out_backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: out_requirements.size,
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
    let output = unsafe { api::image_create_unbound(device, &out_desc) }.unwrap();
    unsafe { api::image_bind(output, out_backing.span(0, out_requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();

    let depth_desc = ImageDesc {
        format: 14,
        usage: IMAGE_DEPTH_STENCIL_ATTACHMENT,
        ..image_desc
    };
    let depth_desc = ImageDesc {
        extent: Extent {
            x: width,
            y: height,
            z: 1,
        },
        ..depth_desc
    };
    let mut depth_requirements = Requirements::default();
    unsafe { api::image_requirements(device, &depth_desc, &mut depth_requirements) }.unwrap();
    let depth_backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: depth_requirements.size,
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
    let depth = unsafe { api::image_create_unbound(device, &depth_desc) }.unwrap();
    unsafe { api::image_bind(depth, depth_backing.span(0, depth_requirements.size)) }.unwrap();
    let depth_view = unsafe {
        api::view_create(
            depth,
            &ViewDesc {
                header: Record::new::<ViewDesc>(VIEW_DESC),
                format: 14,
                dimension: VIEW_2D,
                usage: IMAGE_DEPTH_STENCIL_ATTACHMENT as u32,
                component_mapping: [0, 0, 0, 0],
                range: Subresources {
                    aspects: ASPECT_DEPTH,
                    first_mip: 0,
                    mip_count: 1,
                    first_layer: 0,
                    layer_count: 1,
                },
            },
        )
    }
    .unwrap();

    let executable = scene_executable(device);
    if executable.is_err() {
        dump_validation(device);
    }
    let executable = executable.unwrap();
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = api::barrier_scratch_requirements(0, 3).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);

    // Upload both solid textures through explicit copies with explicit states.
    let staging = linear_memory(device, 1024, USAGE_COPY_SRC, 1);
    let mut upload = vec![0u8; 1024];
    for (slot, color) in [[64u8, 128, 255, 255], [200, 100, 50, 255]]
        .into_iter()
        .enumerate()
    {
        for row in 0..2 {
            for col in 0..2 {
                let base = slot * 512 + row * 256 + col * 4;
                upload[base..base + 4].copy_from_slice(&color);
            }
        }
    }
    fill_upload(staging, &upload);
    let mut upload_barriers = Vec::new();
    for (index, texture) in textures.iter().enumerate() {
        let to_dst = ImageBarrier {
            image: texture.as_raw(),
            range: Subresources {
                aspects: ASPECT_COLOR,
                first_mip: 0,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            before: 0,
            after: ACCESS_COPY_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COPY_DST,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
            discard: 1,
        };
        let to_shader = ImageBarrier {
            before: ACCESS_COPY_WRITE,
            after: ACCESS_SHADER_READ,
            old_state: STATE_COPY_DST,
            new_state: STATE_SHADER_READ,
            ..to_dst
        };
        upload_barriers.push((index, to_dst, to_shader));
    }
    for (_, to_dst, _) in &upload_barriers {
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
            images: to_dst,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
    }
    for (index, _, _) in &upload_barriers {
        let copy = ImageCopy {
            region: ImageRegion {
                aspect: ASPECT_COLOR,
                mip: 0,
                first_layer: 0,
                layer_count: 1,
                offset: Offset { x: 0, y: 0, z: 0 },
                extent: Extent { x: 2, y: 2, z: 1 },
            },
            row_pitch: 256,
            slice_pitch: 0,
            state: STATE_COPY_DST,
            reserved: 0,
        };
        unsafe {
            api::copy_to_image(
                list,
                textures[*index],
                staging.span(*index as u64 * 512, 512),
                &copy,
            )
        };
    }
    for (_, _, to_shader) in &upload_barriers {
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_TRANSFER,
            after: STAGE_FRAGMENT,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            images: to_shader,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
    }

    // This runtime's enhanced-barrier validation requires buffer barriers to
    // cover the whole resource (Size = width or UINT64_MAX): sub-ranges are
    // rejected at record time.
    let vertex_barrier = MemoryBarrier {
        range: vertices.span(0, 256),
        before: ACCESS_COPY_WRITE,
        after: ACCESS_SHADER_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let vertex_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_VERTEX,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &vertex_barrier,
        images: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &vertex_dependency) };

    // Depth enters its attachment state once and stays there.
    let to_depth = ImageBarrier {
        image: depth.as_raw(),
        range: Subresources {
            aspects: ASPECT_DEPTH,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_DEPTH_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_DEPTH_STENCIL_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };

    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    let depth_attachment = Attachment {
        view: depth_view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_DEPTH_STENCIL_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [1.0, 0.0, 0.0, 0.0],
        },
    };
    let render_requirements = api::render_scratch_requirements(2).unwrap();
    let (mut render_scratch, render_offset) = aligned_scratch(render_requirements);
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: width,
        height: height,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: &depth_attachment,
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { render_scratch.as_mut_ptr().add(render_offset) }.cast(),
            size: render_requirements.size,
        },
    };
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);

    let readback = linear_memory(device, out_bytes * 3, USAGE_COPY_DST, 2);
    let copy_out = ImageCopy {
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent {
                x: width,
                y: height,
                z: 1,
            },
        },
        row_pitch: 0,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    let resource_binding = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: resource_heap.span(0, 80),
        reserved_offset: 0,
        reserved_size: 0,
    };
    let sampler_binding = HeapBinding {
        kind: HEAP_SAMPLER,
        storage: sampler_heap.span(0, 80),
        ..resource_binding
    };

    let to_depth_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_depth,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
    };
    unsafe { api::barrier(list, &to_depth_dependency) };

    let mut check = |list: api::ListHandle,
                     phase: u64,
                     image: u32,
                     overlay: u32,
                     scissor: (i32, i32, u32, u32)| {
        // The caller owns attachment states: colour re-enters the attachment
        // state per phase (content is cleared), depth stays in it.
        let to_attachment = ImageBarrier {
            image: output.as_raw(),
            range: Subresources {
                aspects: ASPECT_COLOR,
                first_mip: 0,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
            discard: 1,
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_TRANSFER,
            after: STAGE_FRAGMENT,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            images: &to_attachment,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
        let mut bank = [0u8; 24];
        bank[0..8].copy_from_slice(&vertices_address.to_ne_bytes());
        bank[8..12].copy_from_slice(&image.to_ne_bytes());
        bank[12..16].copy_from_slice(&0u32.to_ne_bytes());
        bank[16..20].copy_from_slice(&0f32.to_ne_bytes());
        bank[20..24].copy_from_slice(&overlay.to_ne_bytes());
        let viewport = ViewportState {
            header: Record::new::<ViewportState>(VIEWPORT_STATE),
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
            scissor_x: scissor.0,
            scissor_y: scissor.1,
            scissor_width: scissor.2,
            scissor_height: scissor.3,
        };
        // Consumer order: heaps, then the executable. Tables must follow the bind.
        unsafe { api::bind_heap(list, &resource_binding) };
        unsafe { api::bind_heap(list, &sampler_binding) };
        unsafe { api::bind_indices(list, indices.span(0, 48), INDEX_U32) };
        unsafe { api::render_begin(list, &render) };
        unsafe { api::bind_executable(list, executable) };
        unsafe { api::set_graphics_state(list, &viewport) };
        unsafe { api::set_root(list, STAGE_VERTEX, 0, vertices_address) };
        unsafe { api::set_inline(list, STAGE_VERTEX | STAGE_FRAGMENT, 0, &bank) };
        let draw = DrawDesc {
            count: 12,
            instances: 1,
            first: 0,
            first_instance: 0,
            vertex_offset: 0,
        };
        unsafe { api::draw_indexed(list, &draw) };
        unsafe { api::render_end(list) };
        let to_copy = ImageBarrier {
            before: ACCESS_COLOR_WRITE,
            after: ACCESS_COPY_READ,
            old_state: STATE_COLOR_ATTACHMENT,
            new_state: STATE_COPY_SRC,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_FRAGMENT,
            after: STAGE_TRANSFER,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            images: &to_copy,
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe {
            api::copy_from_image(
                list,
                readback.span(phase * out_bytes, out_bytes),
                output,
                &copy_out,
            )
        };
    };

    // Phase 1: texture 0, no overlay. Quad N shows through; quad F is
    // depth-rejected where it overlaps (drawn second, farther).
    check(list, 0, 0, 0, (0, 0, width, height));

    let list = unsafe { api::commands_end(list) };
    if list.is_err() {
        dump_validation(device);
    }
    let list = list.unwrap();
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
    dump_validation(device);

    // Verify each phase's readback region once the submission completed.
    let pixels = read_back(readback, out_bytes * 3);
    let verify = |phase: usize, samples: &[(usize, usize, [u8; 4])]| {
        let pixel = |x: usize, y: usize| -> [u8; 4] {
            let base = phase * out_bytes as usize + y * width as usize * 4 + x * 4;
            pixels[base..base + 4].try_into().unwrap()
        };
        for &(x, y, want) in samples {
            let got = pixel(x, y);
            for (g, w) in got.iter().zip(want.iter()) {
                assert!(
                    (*g as i32 - *w as i32).abs() <= 1,
                    "phase {phase} pixel ({x}, {y}): got {got:?} want {want:?}"
                );
            }
        }
    };
    verify(
        0,
        &[
            (69, 80, [64, 128, 255, 255]),
            (0, 0, [0, 0, 0, 255]),
        ],
    );

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(depth_view);
        api::view_destroy(view);
        for view in texture_views {
            api::view_destroy(view);
        }
        api::image_destroy(depth);
        api::image_destroy(output);
        for texture in textures {
            api::image_destroy(texture);
        }
        api::memory_destroy(readback);
        api::memory_destroy(staging);
        api::memory_destroy(indices);
        api::memory_destroy(vertices);
        api::memory_destroy(sampler_heap);
        api::memory_destroy(resource_heap);
        api::memory_destroy(depth_backing);
        api::memory_destroy(out_backing);
        for backing in texture_backings {
            api::memory_destroy(backing);
        }
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Executable cache contract: advisory native cached pipeline-state blobs,
/// export/import with identity checking, valid partial export at capacity, and
/// merge preserving the destination.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_executable_cache_roundtrip() {
    let (_discovery, device, _queue, _domain) = gpu_setup();
    let empty = ExecutableCacheDesc {
        header: Record::new::<ExecutableCacheDesc>(EXECUTABLE_CACHE_DESC),
        synchronization: 0,
        reserved: 0,
        initial: Bytes {
            data: ptr::null(),
            size: 0,
        },
    };
    let cache = unsafe { api::executable_cache_create(device, &empty) }.unwrap();

    // Preparation through the cache: both a graphics and a compute executable.
    let graphics = triangle_executable_with(device, Some(cache)).unwrap();
    let compute = heap_transform_executable_with(device, Some(cache)).unwrap();

    // Export: query the maximum size, then the full container.
    let mut size = 0usize;
    unsafe { api::executable_cache_data(cache, &mut size, None) }.unwrap();
    assert!(size > 32);
    let mut data = vec![0u8; size];
    let mut written = size;
    unsafe { api::executable_cache_data(cache, &mut written, Some(&mut data)) }.unwrap();
    assert_eq!(written, size);
    assert_eq!(&data[..8], b"OGPUCAC1");

    // Import unchanged exported bytes and prepare again through it.
    let imported = unsafe {
        api::executable_cache_create(
            device,
            &ExecutableCacheDesc {
                initial: Bytes {
                    data: data.as_ptr().cast(),
                    size,
                },
                ..empty
            },
        )
    }
    .unwrap();
    let graphics2 = triangle_executable_with(device, Some(imported)).unwrap();
    let mut imported_size = 0usize;
    unsafe { api::executable_cache_data(imported, &mut imported_size, None) }.unwrap();
    assert!(imported_size >= size);

    // Identity mismatch is UNSUPPORTED, never silently retried as empty.
    let mut corrupt = data.clone();
    corrupt[12] ^= 0xff;
    assert_eq!(
        unsafe {
            api::executable_cache_create(
                device,
                &ExecutableCacheDesc {
                    initial: Bytes {
                        data: corrupt.as_ptr().cast(),
                        size,
                    },
                    ..empty
                },
            )
        },
        Err(UNSUPPORTED)
    );

    // A partial export at reduced capacity stays a valid importable cache.
    let capacity = size / 2;
    let mut partial = vec![0u8; capacity];
    let mut partial_size = capacity;
    unsafe { api::executable_cache_data(cache, &mut partial_size, Some(&mut partial)) }.unwrap();
    assert!(partial_size <= capacity);
    let merged = unsafe {
        api::executable_cache_create(
            device,
            &ExecutableCacheDesc {
                initial: Bytes {
                    data: partial.as_ptr().cast(),
                    size: partial_size,
                },
                ..empty
            },
        )
    }
    .unwrap();

    // Merge borrows sources and preserves destination contents; self-merge is
    // invalid and count=0 is a no-op.
    unsafe { api::executable_cache_merge(merged, &[imported]) }.unwrap();
    assert_eq!(unsafe { api::executable_cache_merge(cache, &[cache]) }, Err(INVALID));
    unsafe { api::executable_cache_merge(cache, &[]) }.unwrap();
    let mut merged_size = 0usize;
    unsafe { api::executable_cache_data(merged, &mut merged_size, None) }.unwrap();
    assert!(merged_size >= size);

    unsafe {
        api::executable_cache_destroy(merged);
        api::executable_cache_destroy(imported);
        api::executable_cache_destroy(cache);
        api::executable_destroy(graphics2);
        api::executable_destroy(graphics);
        api::executable_destroy(compute);
    }
}

/// Query contract on the native backend: occlusion around a render scope,
/// timestamp pairs, native 64-bit resolves, and the visible rejections
/// (32-bit result width, non-native strides, availability/partials).
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_query_occlusion_timestamp() {
    let (_discovery, device, queue, domain) =
        gpu_setup_features(RASTER | PIPELINE_STATISTICS | PRECISE_OCCLUSION);
    let extent = 32u32;
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
        usage: IMAGE_COLOR_ATTACHMENT,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let output = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(output, backing.span(0, requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();

    let occlusion_desc = QueryPoolDesc {
        header: Record::new::<QueryPoolDesc>(QUERY_POOL_DESC),
        kind: 2,
        count: 4,
        statistics: 0,
    };
    let timestamp_desc = QueryPoolDesc {
        kind: 1,
        ..occlusion_desc
    };
    let occlusion = unsafe { api::query_pool_create(device, &occlusion_desc) }.unwrap();
    let timestamps = unsafe { api::query_pool_create(device, &timestamp_desc) }.unwrap();
    // Statistics pool: the physical counter mask is the full native set.
    let statistics_desc = QueryPoolDesc {
        kind: 3,
        statistics: (1 << 11) - 1,
        ..occlusion_desc
    };
    let statistics = unsafe { api::query_pool_create(device, &statistics_desc) }.unwrap();
    let result = linear_memory(device, 256, USAGE_COPY_DST, 2);

    let executable = triangle_executable(device).unwrap();
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::queries_reset(list, occlusion, 0, 1) };
    unsafe { api::queries_reset(list, timestamps, 0, 2) };
    unsafe { api::queries_reset(list, statistics, 0, 1) };

    // A timestamp pair around the draw, one narrow scope bit each.
    unsafe { api::timestamp(list, timestamps, 0, STAGE_COPY) };
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);
    let to_attachment = ImageBarrier {
        image: output.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { state_scratch.as_mut_ptr().add(state_offset) }.cast(),
        scratch_size: state_requirements.size,
        images: &to_attachment,
    };
    unsafe { api::barrier(list, &dependency) };

    // The occlusion query encloses the complete render scope.
    unsafe { api::query_begin(list, occlusion, 0, 1) };
    unsafe { api::query_begin(list, statistics, 0, 0) };
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    let render_requirements = api::render_scratch_requirements(1).unwrap();
    let (mut render_scratch, render_offset) = aligned_scratch(render_requirements);
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { render_scratch.as_mut_ptr().add(render_offset) }.cast(),
            size: render_requirements.size,
        },
    };
    unsafe { api::render_begin(list, &render) };
    unsafe { api::bind_executable(list, executable) };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: extent as f32,
        height: extent as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: extent,
        scissor_height: extent,
    };
    unsafe { api::set_graphics_state(list, &viewport) };
    let draw = DrawDesc {
        count: 3,
        instances: 1,
        first: 0,
        first_instance: 0,
        vertex_offset: 0,
    };
    unsafe { api::draw(list, &draw) };
    unsafe { api::render_end(list) };
    unsafe { api::query_end(list, statistics, 0) };
    unsafe { api::query_end(list, occlusion, 0) };
    unsafe { api::timestamp(list, timestamps, 1, STAGE_FRAGMENT) };

    // Native resolves: 64-bit results at the native record stride.
    unsafe { api::queries_resolve(list, occlusion, 0, 1, result.span(0, 256), 8, 1) };
    unsafe { api::queries_resolve(list, timestamps, 0, 2, result.span(64, 64), 8, 1) };
    unsafe { api::queries_resolve(list, statistics, 0, 1, result.span(128, 88), 88, 1) };

    let list = unsafe { api::commands_end(list) };
    if list.is_err() {
        dump_validation(device);
    }
    let list = list.unwrap();
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
    dump_validation(device);

    // Visible rejections, each on its own list (error-at-end semantics): the
    // contract's 32-bit width and foreign strides have no native form;
    // availability and partial results do not exist natively.
    for (count, stride, flags) in [
        (1u32, 8u64, 0u32),
        (2, 16, 1),
        (1, 8, 2),
        (1, 8, 8),
    ] {
        let rejection_arena = single_slot_arena(device, domain);
        let rejected = unsafe { api::commands_begin(rejection_arena, &one_shot()) }.unwrap();
        unsafe {
            api::queries_resolve(
                rejected,
                occlusion,
                0,
                count,
                result.span(0, 256),
                stride,
                flags,
            )
        };
        assert_eq!(unsafe { api::commands_end(rejected) }, Err(UNSUPPORTED));
        unsafe { api::arena_destroy(rejection_arena) };
    }

    let bytes = read_back(result, 256);
    let value = |offset: usize| -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
    };
    // The triangle covers the image centre: some samples passed, not all.
    let samples = value(0);
    assert!(samples > 0 && samples < u64::from(extent) * u64::from(extent));
    // Timestamps advance across the draw and are nonzero GPU-clock values.
    let (t0, t1) = (value(64), value(72));
    assert!(t0 != 0 && t1 >= t0);

    unsafe {
        api::query_pool_destroy(statistics);
        api::query_pool_destroy(timestamps);
        api::query_pool_destroy(occlusion);
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::image_destroy(output);
        api::memory_destroy(result);
        api::memory_destroy(backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Indirect draws: fixed-count and count-based argument buffers, indexed
/// variants, the native wire records, and the visible stride rejection.
#[ignore = "blocked: first closure phase's draw renders nothing (docs/dx12-backend-d2.md)"]
#[test]
fn gpu_draw_indirect() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 64u32;
    let out_bytes = u64::from(extent) * 256;
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
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let output = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(output, backing.span(0, requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();
    let executable = triangle_executable(device).unwrap();

    // Argument buffer: one draw record, one indexed draw record, two counts.
    let arguments = linear_memory(device, 256, USAGE_INDIRECT, 1);
    let mut wire = [0u8; 256];
    // ogpu_next_draw_arguments { count, instances, first, first_instance }.
    wire[0..16].copy_from_slice(&[3u32, 1, 0, 0].map(u32::to_ne_bytes).concat());
    // ogpu_next_draw_indexed_arguments { count, instances, first, vertex_offset,
    // first_instance }.
    wire[16..36].copy_from_slice(&[3u32, 1, 0, 0, 0].map(u32::to_ne_bytes).concat());
    // Count buffer values: zero (no draws) at 64, one (a single draw) at 68.
    wire[64..68].copy_from_slice(&0u32.to_ne_bytes());
    wire[68..72].copy_from_slice(&1u32.to_ne_bytes());
    {
        let mapping = unsafe { api::memory_map(arguments.span(0, 256)) }.unwrap();
        // SAFETY: the mapping covers the argument region.
        unsafe {
            ptr::copy_nonoverlapping(wire.as_ptr(), mapping.data.cast::<u8>(), 256);
        }
        unsafe { api::memory_unmap(arguments) };
    }
    let indices = linear_memory(device, 256, USAGE_INDEX, 1);
    let index_data: [u32; 3] = [0, 1, 2];
    {
        let mapping = unsafe { api::memory_map(indices.span(0, 12)) }.unwrap();
        // SAFETY: the mapping covers the index region.
        unsafe {
            ptr::copy_nonoverlapping(
                index_data.as_ptr().cast::<u8>(),
                mapping.data.cast::<u8>(),
                12,
            );
        }
        unsafe { api::memory_unmap(indices) };
    }
    let readback = linear_memory(device, out_bytes * 4, USAGE_COPY_DST, 2);

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let to_attachment = ImageBarrier {
        image: output.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let copy_out = ImageCopy {
        region: ImageRegion {
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
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    let render_requirements = api::render_scratch_requirements(1).unwrap();
    let (mut render_scratch, render_offset) = aligned_scratch(render_requirements);
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { render_scratch.as_mut_ptr().add(render_offset) }.cast(),
            size: render_requirements.size,
        },
    };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: extent as f32,
        height: extent as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: extent,
        scissor_height: extent,
    };

    // Each phase records one variant and copies its output for verification.
    let mut phase = |list: api::ListHandle,
                     phase: u64,
                     variant: u32,
                     records: Span,
                     stride: u32,
                     count: Option<Span>| {
        let to_attachment = ImageBarrier {
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_COLOR,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_attachment,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe { api::render_begin(list, &render) };
        unsafe { api::bind_executable(list, executable) };
        unsafe { api::set_graphics_state(list, &viewport) };
        let direct = DrawDesc {
            count: 3,
            instances: 1,
            first: 0,
            first_instance: 0,
            vertex_offset: 0,
        };
        let indirect = Indirect {
            arguments: records,
            stride,
            maximum_count: 1,
            count: count.unwrap_or(Span {
                memory: ptr::null_mut(),
                offset: 0,
                size: 0,
            }),
        };
        if variant == 0 {
            unsafe { api::draw(list, &direct) };
        } else if variant == 1 {
            unsafe { api::bind_indices(list, indices.span(0, 12), INDEX_U32) };
            unsafe { api::draw_indexed_indirect(list, &indirect) };
        } else {
            unsafe { api::draw_indirect(list, &indirect) };
        }
        unsafe { api::render_end(list) };
        let to_copy = ImageBarrier {
            before: ACCESS_COLOR_WRITE,
            after: ACCESS_COPY_READ,
            old_state: STATE_COLOR_ATTACHMENT,
            new_state: STATE_COPY_SRC,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_TRANSFER,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_copy,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe {
            api::copy_from_image(
                list,
                readback.span(phase * out_bytes, out_bytes),
                output,
                &copy_out,
            )
        };
    };

    // Fixed-count indirect draws (draw and indexed) cover the clear with the
    // centre-covering triangle; a count-based draw whose GPU count is zero
    // must issue nothing.
    phase(list, 0, 2, arguments.span(0, 16), 16, Some(arguments.span(64, 4)));
    phase(list, 1, 2, arguments.span(0, 16), 16, None);
    phase(list, 2, 1, arguments.span(16, 20), 20, None);
    phase(list, 3, 2, arguments.span(0, 16), 16, Some(arguments.span(64, 4)));

    let list = unsafe { api::commands_end(list) };
    if list.is_err() {
        dump_validation(device);
    }
    let list = list.unwrap();
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut submit_scratch, submit_offset) = aligned_scratch(requirements);
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
                data: submit_scratch.as_mut_ptr().add(submit_offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));
    dump_validation(device);

    let bytes = read_back(readback, out_bytes * 4);
    let pixel = |phase: usize, x: usize, y: usize| -> [u8; 4] {
        let base = phase * out_bytes as usize + y * 256 + x * 4;
        bytes[base..base + 4].try_into().unwrap()
    };
    // The fixed-count variants draw the centre-covering triangle over the
    // black clear; the count-based variant with a zero count draws nothing.
    for phase in [1usize, 2] {
        assert_eq!(
            pixel(phase, 32, 32),
            [255, 0, 0, 255],
            "phase {phase} centre"
        );
        assert_eq!(pixel(phase, 1, 1), [0, 0, 0, 255], "phase {phase} corner");
    }
    assert_eq!(pixel(0, 32, 32), [0, 0, 0, 255], "zero count must not draw");
    // The late zero-count phase is the meaningful one: a drawn phase preceded
    // it on the same list, so a stale record would show red here.
    assert_eq!(pixel(3, 32, 32), [0, 0, 0, 255], "late zero count must not draw");

    // Visible rejection: wider-than-record strides have no native command
    // stride and stay unsupported (each on its own list).
    for (stride, maximum, status) in [(24u32, 2u32, UNSUPPORTED), (16, 0, INVALID)] {
        let rejection_arena = single_slot_arena(device, domain);
        let rejected = unsafe { api::commands_begin(rejection_arena, &one_shot()) }.unwrap();
        let (mut reject_scratch, reject_offset) = aligned_scratch(scratch_requirements);
        let to_render = ImageBarrier {
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_COLOR,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { reject_scratch.as_mut_ptr().add(reject_offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_render,
        };
        unsafe { api::barrier(rejected, &dependency) };
        unsafe { api::render_begin(rejected, &render) };
        unsafe { api::bind_executable(rejected, executable) };
        let indirect = Indirect {
            arguments: arguments.span(0, 16),
            stride,
            maximum_count: maximum,
            count: Span {
                memory: ptr::null_mut(),
                offset: 0,
                size: 0,
            },
        };
        unsafe { api::draw_indirect(rejected, &indirect) };
        unsafe { api::render_end(rejected) };
        assert_eq!(unsafe { api::commands_end(rejected) }, Err(status));
        unsafe { api::arena_destroy(rejection_arena) };
    }

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::image_destroy(output);
        api::memory_destroy(readback);
        api::memory_destroy(indices);
        api::memory_destroy(arguments);
        api::memory_destroy(backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}


/// Image clears through native render-target/depth-stencil clears: exact
/// values, and the visible rejections (integer formats, bad state, ranges).
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_image_clear() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 32u32;
    let out_bytes = u64::from(extent) * 256;
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
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let output = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(output, backing.span(0, requirements.size)) }.unwrap();
    let readback = linear_memory(device, out_bytes, USAGE_COPY_DST, 2);

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let to_attachment = ImageBarrier {
        image: output.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_COLOR,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
        images: &to_attachment,
    };
    unsafe { api::barrier(list, &dependency) };
    let all = Subresources {
        aspects: ASPECT_COLOR,
        first_mip: 0,
        mip_count: u32::MAX,
        first_layer: 0,
        layer_count: u32::MAX,
    };
    let value = ClearValue {
        f32: [0.25, 0.5, 0.75, 1.0],
    };
    unsafe { api::clear_image(list, output, STATE_COLOR_ATTACHMENT, all, value) };
    let to_copy = ImageBarrier {
        before: ACCESS_COLOR_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COLOR_ATTACHMENT,
        new_state: STATE_COPY_SRC,
        ..to_attachment
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: scratch_requirements.size,
        images: &to_copy,
    };
    unsafe { api::barrier(list, &dependency) };
    let copy = ImageCopy {
        region: ImageRegion {
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
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    unsafe { api::copy_from_image(list, readback.span(0, out_bytes), output, &copy) };
    let list = unsafe { api::commands_end(list) }.unwrap();
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut submit_scratch, submit_offset) = aligned_scratch(requirements);
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
                data: submit_scratch.as_mut_ptr().add(submit_offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));

    let bytes = read_back(readback, out_bytes);
    for (x, y) in [(0usize, 0usize), (17, 23), (31, 31)] {
        let base = y * 256 + x * 4;
        let got: [u8; 4] = bytes[base..base + 4].try_into().unwrap();
        assert_eq!(got, [64, 128, 191, 255], "pixel ({x}, {y}): {got:?}");
    }

    // Visible rejections: a bad state and an out-of-range mip; each on its own
    // list (error-at-end semantics).
    for (state, range, status) in [
        (
            STATE_COPY_DST,
            all,
            INVALID,
        ),
        (
            STATE_COLOR_ATTACHMENT,
            Subresources {
                aspects: ASPECT_COLOR,
                first_mip: 4,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            INVALID,
        ),
    ] {
        let rejection_arena = single_slot_arena(device, domain);
        let rejected = unsafe { api::commands_begin(rejection_arena, &one_shot()) }.unwrap();
        let (mut reject_scratch, reject_offset) = aligned_scratch(scratch_requirements);
        let to_render = ImageBarrier {
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_COLOR,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { reject_scratch.as_mut_ptr().add(reject_offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_render,
        };
        unsafe { api::barrier(rejected, &dependency) };
        unsafe { api::clear_image(rejected, output, state, range, value) };
        assert_eq!(unsafe { api::commands_end(rejected) }, Err(status));
        unsafe { api::arena_destroy(rejection_arena) };
    }

    unsafe {
        api::arena_destroy(arena);
        api::image_destroy(output);
        api::memory_destroy(readback);
        api::memory_destroy(backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Mixed compute-to-raster: a compute dispatch writes the vertex positions
/// that a descriptor-indexed textured draw then consumes through its root
/// descriptor (the D2.1 argument lowering feeding the M4 shape). The vertex
/// buffer starts zero, so the rendered pixels prove the compute ran and its
/// writes reached the vertex fetch.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_compute_to_raster() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 32u32;
    let out_bytes = u64::from(extent) * 256;

    // Four vertices x (x, y, z, w) float bits; the fixture computes
    // value * multiplier + count with count as both bound and addend, so the
    // upload carries the inverse and the vertices land exact.
    let words = 16u32;
    let input = linear_memory(device, 256, USAGE_STORAGE | USAGE_COPY_DST, 0);
    let vertices = linear_memory(device, 256, USAGE_STORAGE, 0);
    let upload_staging = linear_memory(device, 1024, USAGE_COPY_SRC, 1);
    let mut upload = [0u32; 256];
    let corners = [
        (-0.8f32, -0.8f32),
        (0.8, -0.8),
        (0.8, 0.8),
        (-0.8, 0.8),
    ];
    for (corner, (x, y)) in corners.iter().enumerate() {
        for (k, v) in [x, y, &0.0f32, &1.0f32].into_iter().enumerate() {
            upload[corner * 4 + k] = v.to_bits().wrapping_sub(words);
        }
    }
    // One solid 2x2 texture (constant sampling is exact at any UV).
    for row in 0..2 {
        for col in 0..2 {
            // 256-byte row pitch, matching the copy footprint.
            let word = 64 + row * 64 + col;
            upload[word..word + 1].copy_from_slice(&[u32::from_le_bytes([64, 128, 255, 255])]);
        }
    }
    // Index buffer: one quad as two triangles, clear of the texture rows.
    upload[192] = 0;
    upload[193] = 1;
    upload[194] = 2;
    upload[195] = 2;
    upload[196] = 3;
    upload[197] = 0;
    {
        let mapping = unsafe { api::memory_map(upload_staging.span(0, 1024)) }.unwrap();
        // SAFETY: the mapping covers the upload region.
        unsafe {
            ptr::copy_nonoverlapping(upload.as_ptr().cast::<u8>(), mapping.data.cast::<u8>(), 1024);
        }
        unsafe { api::memory_unmap(upload_staging) };
    }

    let image_desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: 2, y: 2, z: 1 },
        usage: IMAGE_SAMPLED | IMAGE_COPY_DST,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
    let texture_backing = unsafe {
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
    let texture = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(texture, texture_backing.span(0, requirements.size)) }.unwrap();
    let texture_view = unsafe {
        api::view_create(
            texture,
            &ViewDesc {
                header: Record::new::<ViewDesc>(VIEW_DESC),
                format: RGBA8_UNORM,
                dimension: VIEW_2D,
                usage: IMAGE_SAMPLED as u32,
                component_mapping: [0, 0, 0, 0],
                range: Subresources {
                    aspects: ASPECT_COLOR,
                    first_mip: 0,
                    mip_count: 1,
                    first_layer: 0,
                    layer_count: 1,
                },
            },
        )
    }
    .unwrap();

    let out_desc = ImageDesc {
        extent: Extent {
            x: extent,
            y: extent,
            z: 1,
        },
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        ..image_desc
    };
    let mut out_requirements = Requirements::default();
    unsafe { api::image_requirements(device, &out_desc, &mut out_requirements) }.unwrap();
    let out_backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: out_requirements.size,
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
    let output = unsafe { api::image_create_unbound(device, &out_desc) }.unwrap();
    unsafe { api::image_bind(output, out_backing.span(0, out_requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();

    let resource_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    let sampler_heap = linear_memory(device, 512, USAGE_DESCRIPTOR_HEAP, 1);
    place_resource_recipes(
        device,
        resource_heap,
        &[0],
        &[ResourceDescriptor {
            kind: DESCRIPTOR_SAMPLED_IMAGE,
            image_state: STATE_SHADER_READ,
            view: texture_view.as_raw(),
            buffer: Span {
                memory: ptr::null_mut(),
                offset: 0,
                size: 0,
            },
        }],
    );
    let sampler = SamplerDesc {
        header: Record::new::<SamplerDesc>(SAMPLER_DESC),
        min_filter: 0,
        mag_filter: 0,
        mip_filter: 0,
        address_mode: [0, 0, 0],
        compare_enable: 0,
        compare_op: 0,
        border_color: 0,
        unnormalized_coordinates: 0,
        min_lod: 0.0,
        max_lod: 1000.0,
        lod_bias: 0.0,
        max_anisotropy: 1.0,
    };
    let mut host = [[0u8; 80]; 1];
    let outputs = [HostSpan {
        data: host[0].as_mut_ptr().cast(),
        size: 80,
    }];
    unsafe {
        api::write_sampler_descriptors(
            device,
            &[sampler],
            &outputs,
            HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
        )
    }
    .unwrap();
    {
        let mapping = unsafe { api::memory_map(sampler_heap.span(0, 80)) }.unwrap();
        // SAFETY: the mapping covers the recipe slot.
        unsafe {
            ptr::copy_nonoverlapping(host[0].as_ptr(), mapping.data.cast::<u8>(), 80);
        }
        unsafe { api::memory_unmap(sampler_heap) };
    }

    let compute = argument_transform_executable(device).unwrap();
    let scene = scene_executable(device).unwrap();
    let readback = linear_memory(device, out_bytes, USAGE_COPY_DST, 2);
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();

    // Stage the inputs: vertex words, texture texels and the index buffer.
    unsafe {
        api::copy_memory(list, input.span(0, 256), upload_staging.span(0, 256));
    }
    let texture_copy = ImageCopy {
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent { x: 2, y: 2, z: 1 },
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_DST,
        reserved: 0,
    };
    let to_copy_dst = ImageBarrier {
        image: texture.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COPY_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COPY_DST,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let to_shader = ImageBarrier {
        before: ACCESS_COPY_WRITE,
        after: ACCESS_SHADER_READ,
        old_state: STATE_COPY_DST,
        new_state: STATE_SHADER_READ,
        ..to_copy_dst
    };
    let upload_requirements = api::barrier_scratch_requirements(0, 2).unwrap();
    let (mut upload_scratch, upload_offset) = aligned_scratch(upload_requirements);
    let upload_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_COMPUTE | STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { upload_scratch.as_mut_ptr().add(upload_offset) }.cast(),
        scratch_size: upload_requirements.size,
        images: &to_copy_dst,
    };
    let indices = linear_memory(device, 256, USAGE_INDEX | USAGE_COPY_DST, 0);
    unsafe {
        api::copy_memory(list, indices.span(0, 24), upload_staging.span(768, 24));
    }
    unsafe {
        api::copy_to_image(
            list,
            texture,
            upload_staging.span(256, 512),
            &texture_copy,
        );
    }
    unsafe { api::barrier(list, &upload_dependency) };
    let to_shader_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { upload_scratch.as_mut_ptr().add(upload_offset) }.cast(),
        scratch_size: upload_requirements.size,
        images: &to_shader,
    };
    unsafe { api::barrier(list, &to_shader_dependency) };

    // Compute: the argument transform writes the vertex words.
    unsafe { api::bind_executable(list, compute) };
    let input_address = unsafe { api::memory_address(input.span(0, 256)) }.unwrap();
    let vertices_address = unsafe { api::memory_address(vertices.span(0, 256)) }.unwrap();
    unsafe { api::set_root(list, STAGE_COMPUTE, 0, input_address) };
    unsafe { api::set_root(list, STAGE_COMPUTE, 1, vertices_address) };
    let control = [words, 1u32];
    unsafe {
        api::set_inline(
            list,
            STAGE_COMPUTE,
            16,
            &control.map(u32::to_ne_bytes).concat(),
        )
    };
    let launch = Launch {
        groups: Extent { x: 1, y: 1, z: 1 },
        dynamic_shared_bytes: 0,
        extensions: ptr::null(),
    };
    unsafe { api::dispatch(list, &launch) };

    // Compute writes to vertex fetch and fragment reads.
    // Symmetric sizing: covers one memory and one image barrier record.
    let barrier_requirements = api::barrier_scratch_requirements(1, 1).unwrap();
    let (mut barrier_scratch, barrier_offset) = aligned_scratch(barrier_requirements);
    let compute_to_vertex = MemoryBarrier {
        range: vertices.span(0, 256),
        before: ACCESS_SHADER_WRITE,
        after: ACCESS_VERTEX_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let compute_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COMPUTE,
        after: STAGE_VERTEX | STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &compute_to_vertex,
        scratch: unsafe { barrier_scratch.as_mut_ptr().add(barrier_offset) }.cast(),
        scratch_size: barrier_requirements.size,
        images: ptr::null(),
    };
    unsafe { api::barrier(list, &compute_dependency) };

    // Raster: the M4 shape consuming the computed vertices and the texture.
    let to_attachment = ImageBarrier {
        image: output.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let attach_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_FRAGMENT,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { barrier_scratch.as_mut_ptr().add(barrier_offset) }.cast(),
        scratch_size: barrier_requirements.size,
        images: &to_attachment,
    };
    unsafe { api::barrier(list, &attach_dependency) };
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    let render_requirements = api::render_scratch_requirements(1).unwrap();
    let (mut render_scratch, render_offset) = aligned_scratch(render_requirements);
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { render_scratch.as_mut_ptr().add(render_offset) }.cast(),
            size: render_requirements.size,
        },
    };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: extent as f32,
        height: extent as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: extent,
        scissor_height: extent,
    };
    let mut bank = [0u8; 24];
    bank[0..8].copy_from_slice(&vertices_address.to_ne_bytes());
    let resource_binding = HeapBinding {
        header: Record::new::<HeapBinding>(HEAP_BINDING),
        kind: HEAP_RESOURCE,
        flags: 0,
        storage: resource_heap.span(0, 40),
        reserved_offset: 0,
        reserved_size: 0,
    };
    let sampler_binding = HeapBinding {
        kind: HEAP_SAMPLER,
        storage: sampler_heap.span(0, 80),
        ..resource_binding
    };
    unsafe { api::render_begin(list, &render) };
    unsafe { api::bind_executable(list, scene) };
    unsafe { api::set_graphics_state(list, &viewport) };
    unsafe { api::set_root(list, STAGE_VERTEX, 0, vertices_address) };
    unsafe { api::set_inline(list, STAGE_VERTEX | STAGE_FRAGMENT, 0, &bank) };
    unsafe { api::bind_heap(list, &resource_binding) };
    unsafe { api::bind_heap(list, &sampler_binding) };
    unsafe { api::bind_indices(list, indices.span(0, 24), INDEX_U32) };
    let draw = DrawDesc {
        count: 6,
        instances: 1,
        first: 0,
        first_instance: 0,
        vertex_offset: 0,
    };
    unsafe { api::draw_indexed(list, &draw) };
    unsafe { api::render_end(list) };

    let to_copy = ImageBarrier {
        before: ACCESS_COLOR_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COLOR_ATTACHMENT,
        new_state: STATE_COPY_SRC,
        ..to_attachment
    };
    let copy_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { barrier_scratch.as_mut_ptr().add(barrier_offset) }.cast(),
        scratch_size: barrier_requirements.size,
        images: &to_copy,
    };
    unsafe { api::barrier(list, &copy_dependency) };
    let copy_out = ImageCopy {
        region: ImageRegion {
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
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };
    unsafe { api::copy_from_image(list, readback.span(0, out_bytes), output, &copy_out) };

    let list = unsafe { api::commands_end(list) };
    if list.is_err() {
        dump_validation(device);
    }
    let list = list.unwrap();
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
    dump_validation(device);

    // The quad sits at pixels [3,29)^2: the centre shows the solid texture
    // through the descriptor-indexed sample, the corner stays clear. The
    // vertices were compute-written, so both prove the mixed flow.
    let bytes = read_back(readback, out_bytes);
    let pixel = |x: usize, y: usize| -> [u8; 4] {
        let base = y * 256 + x * 4;
        bytes[base..base + 4].try_into().unwrap()
    };
    assert_eq!(pixel(16, 16), [64, 128, 255, 255], "centre");
    assert_eq!(pixel(1, 1), [0, 0, 0, 255], "corner");

    unsafe {
        api::executable_destroy(scene);
        api::executable_destroy(compute);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::view_destroy(texture_view);
        api::image_destroy(output);
        api::image_destroy(texture);
        api::memory_destroy(readback);
        api::memory_destroy(indices);
        api::memory_destroy(upload_staging);
        api::memory_destroy(vertices);
        api::memory_destroy(input);
        api::memory_destroy(sampler_heap);
        api::memory_destroy(resource_heap);
        api::memory_destroy(out_backing);
        api::memory_destroy(texture_backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// AB probe: the same draw scope recorded through a closure (round 0) and
/// inline (round 1) on one list.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_closure_ab_probe() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let extent = 64u32;
    let out_bytes = u64::from(extent) * 256;
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
        usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut requirements = Requirements::default();
    unsafe { api::image_requirements(device, &image_desc, &mut requirements) }.unwrap();
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
    let output = unsafe { api::image_create_unbound(device, &image_desc) }.unwrap();
    unsafe { api::image_bind(output, backing.span(0, requirements.size)) }.unwrap();
    let view = attachment_view(device, output, IMAGE_COLOR_ATTACHMENT as u32).unwrap();
    let executable = triangle_executable(device).unwrap();
    let arguments = linear_memory(device, 256, USAGE_INDIRECT, 1);
    let indices = linear_memory(device, 256, USAGE_INDEX | USAGE_COPY_DST, 0);
    let mut wire = [0u8; 256];
    wire[0..16].copy_from_slice(&[3u32, 1, 0, 0].map(u32::to_ne_bytes).concat());
    {
        let mapping = unsafe { api::memory_map(arguments.span(0, 256)) }.unwrap();
        unsafe {
            ptr::copy_nonoverlapping(wire.as_ptr(), mapping.data.cast::<u8>(), 256);
        }
        unsafe { api::memory_unmap(arguments) };
    }
    let _ = &indices;
    let readback = linear_memory(device, out_bytes * 2, USAGE_COPY_DST, 2);
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let to_attachment = ImageBarrier {
        image: output.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 0.0, 0.0, 1.0],
        },
    };
    let render_requirements = api::render_scratch_requirements(1).unwrap();
    let (mut render_scratch, render_offset) = aligned_scratch(render_requirements);
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: extent,
        height: extent,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { render_scratch.as_mut_ptr().add(render_offset) }.cast(),
            size: render_requirements.size,
        },
    };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: extent as f32,
        height: extent as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: extent,
        scissor_height: extent,
    };
    let draw = DrawDesc {
        count: 3,
        instances: 1,
        first: 0,
        first_instance: 0,
        vertex_offset: 0,
    };
    let copy = ImageCopy {
        region: ImageRegion {
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
        },
        row_pitch: 256,
        slice_pitch: 0,
        state: STATE_COPY_SRC,
        reserved: 0,
    };

    let mut scope = |list: api::ListHandle, round: u64, closure: bool| {
        let to_attachment = ImageBarrier {
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_COLOR,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_attachment,
        };
        let _ = closure;
        unsafe { api::barrier(list, &dependency) };
        unsafe { api::render_begin(list, &render) };
        unsafe { api::bind_executable(list, executable) };
        unsafe { api::set_graphics_state(list, &viewport) };
        unsafe { api::draw(list, &draw) };
        unsafe { api::render_end(list) };
        let to_copy = ImageBarrier {
            before: ACCESS_COLOR_WRITE,
            after: ACCESS_COPY_READ,
            old_state: STATE_COLOR_ATTACHMENT,
            new_state: STATE_COPY_SRC,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_TRANSFER,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_copy,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe {
            api::copy_from_image(
                list,
                readback.span(round * out_bytes, out_bytes),
                output,
                &copy,
            )
        };
    };

    // Round 0 through the closure; round 1 through an identical inline body.
    scope(list, 0, true);
    {
        let to_attachment = ImageBarrier {
            before: 0,
            after: ACCESS_COLOR_WRITE,
            old_state: STATE_UNDEFINED,
            new_state: STATE_COLOR_ATTACHMENT,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_COLOR,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_attachment,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe { api::render_begin(list, &render) };
        unsafe { api::bind_executable(list, executable) };
        unsafe { api::set_graphics_state(list, &viewport) };
        unsafe { api::draw(list, &draw) };
        unsafe { api::render_end(list) };
        let to_copy = ImageBarrier {
            before: ACCESS_COLOR_WRITE,
            after: ACCESS_COPY_READ,
            old_state: STATE_COLOR_ATTACHMENT,
            new_state: STATE_COPY_SRC,
            ..to_attachment
        };
        let dependency = Dependency {
            header: Record::new::<Dependency>(DEPENDENCY),
            before: STAGE_COLOR,
            after: STAGE_TRANSFER,
            global_before: 0,
            global_after: 0,
            memory_count: 0,
            image_count: 1,
            flags: 0,
            memory: ptr::null(),
            scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            scratch_size: scratch_requirements.size,
            images: &to_copy,
        };
        unsafe { api::barrier(list, &dependency) };
        unsafe {
            api::copy_from_image(list, readback.span(out_bytes, out_bytes), output, &copy)
        };
    }

    let list = unsafe { api::commands_end(list) }.unwrap();
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut submit_scratch, submit_offset) = aligned_scratch(requirements);
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
                data: submit_scratch.as_mut_ptr().add(submit_offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));

    let pixels = read_back(readback, out_bytes * 2);
    for round in [0usize, 1] {
        let base = round * out_bytes as usize + 32 * 256 + 32 * 4;
        let got: [u8; 4] = pixels[base..base + 4].try_into().unwrap();
        assert_eq!(got, [255, 0, 0, 255], "round {round} centre");
    }

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::image_destroy(output);
        api::memory_destroy(readback);
        api::memory_destroy(indices);
        api::memory_destroy(arguments);
        api::memory_destroy(backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}


/// D3 presentation path end to end: a borrowed Win32 window, the surface
/// capability queries, swapchain creation, borrowed images, acquisition with
/// a binary signal, and one native present with binary wait and fence
/// retirement. The window is created and destroyed by the test; the surface
/// only ever borrows it.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_swapchain_present() {
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, CS_HREDRAW,
        CS_VREDRAW, CW_USEDEFAULT, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    let (_discovery, device, queue, domain) =
        gpu_setup_features(1 | 16 | 65536 | RASTER | PRESENTATION);
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: HINSTANCE(std::ptr::null_mut()),
        lpszClassName: windows::core::w!("ogpu_swapchain_test"),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassW(&class) };
    assert_ne!(atom, 0);
    let window = unsafe {
        CreateWindowExW(
            Default::default(),
            windows::core::w!("ogpu_swapchain_test"),
            windows::core::w!("ogpu swapchain test"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            128,
            128,
            None,
            None,
            None,
            None,
        )
    }
    .expect("test window");

    let discovery = unsafe { api::discover_presentation(1) }.unwrap();
    let adapter = &api::adapters(&discovery)[0];
    let surface = unsafe {
        api::surface_wayland_create(
            adapter,
            &WaylandSurfaceDesc {
                header: Record::new::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC),
                display: std::ptr::null_mut(),
                surface: window.0.cast(),
            },
        )
    }
    .unwrap();

    // The surface capability queries report the DXGI facts and the measured
    // window extent.
    let mut caps_storage = SurfaceCapabilities::default();
    let mut caps_query = Query {
        header: Record::new::<Query>(SURFACE_CAPABILITIES),
        data: std::ptr::addr_of_mut!(caps_storage).cast(),
        capacity: 1,
        count: 0,
        element_size: std::mem::size_of::<SurfaceCapabilities>() as u32,
        reserved: 0,
    };
    unsafe { api::surface_query(adapter, surface, &mut caps_query) }.unwrap();
    assert_eq!(caps_query.count, 1);
    assert!(caps_storage.min_image_count >= 2);
    assert!(caps_storage.current_extent[0] > 0);

    let mut queue_storage = [SurfaceQueue::default(); 4];
    let mut queue_query = Query {
        header: Record::new::<Query>(SURFACE_QUEUES),
        data: queue_storage.as_mut_ptr().cast(),
        capacity: 4,
        count: 0,
        element_size: std::mem::size_of::<SurfaceQueue>() as u32,
        reserved: 0,
    };
    unsafe { api::surface_query(adapter, surface, &mut queue_query) }.unwrap();
    assert!(queue_storage[..queue_query.count as usize]
        .iter()
        .any(|q| q.domain == domain && q.supported != 0));

    // The swapchain binds the borrowed window through the domain's queue.
    let swapchain = unsafe {
        api::swapchain_create(
            device,
            &SwapchainDesc {
                header: Record::new::<SwapchainDesc>(SWAPCHAIN_DESC),
                surface: surface.as_raw(),
                old_swapchain: std::ptr::null_mut(),
                format: RGBA8_UNORM,
                color_space: 0,
                extent: [64, 64],
                layer_count: 1,
                min_image_count: 2,
                usage: IMAGE_COLOR_ATTACHMENT,
                transform: 1,
                alpha: 0,
                present_mode: 2,
                clipped: 1,
                domain_count: 1,
                mode_count: 0,
                domains: &domain,
                modes: std::ptr::null(),
            },
        )
    }
    .unwrap();

    // Borrowed images: the chain owns the buffers.
    let mut image_slots: [*mut Image; 16] = [std::ptr::null_mut(); 16];
    let mut image_count = 0u32;
    unsafe { api::swapchain_images(swapchain, &mut image_slots, &mut image_count) }.unwrap();
    assert!(image_count >= 2);

    // Acquisition schedules one binary signal on the chain's queue.
    let binary = unsafe { api::binary_create(device) }.unwrap();
    let (index, _suboptimal) =
        unsafe { api::acquire(swapchain, u64::MAX, Some(binary), None) }.unwrap();
    assert!((index as u32) < image_count);

    // One native present per chain: binary wait first, fence retirement after.
    let fence = unsafe { api::fence_create(device, false) }.unwrap();
    let image = PresentImage {
        swapchain: swapchain.as_raw(),
        index,
        reserved: 0,
        fence: fence.as_raw(),
    };
    let mut results = [OK; 1];
    let mut scratch = vec![0u8; 256];
    let waits = [binary.as_raw()];
    let present = PresentDesc {
        header: Record::new::<PresentDesc>(PRESENT_DESC),
        wait_count: 1,
        image_count: 1,
        waits: waits.as_ptr(),
        images: &image,
        results: results.as_mut_ptr(),
        scratch: HostSpan {
            data: scratch.as_mut_ptr().cast(),
            size: scratch.len() as u64,
        },
    };
    unsafe { api::queue_present(queue, &present) }.unwrap();
    assert_eq!(results[0], OK);
    assert!(unsafe { api::fence_wait(fence, 2_000_000_000) }.is_ok());
    // A consumed binary wait unsignals, so the same objects can acquire and
    // present again (the consumer's three-slot loop).
    unsafe { api::fence_reset(fence) }.unwrap();
    let (index, _suboptimal) =
        unsafe { api::acquire(swapchain, u64::MAX, Some(binary), None) }.unwrap();
    let image = PresentImage {
        swapchain: swapchain.as_raw(),
        index,
        reserved: 0,
        fence: fence.as_raw(),
    };
    let present = PresentDesc {
        images: &image,
        ..present
    };
    unsafe { api::queue_present(queue, &present) }.unwrap();
    assert_eq!(results[0], OK);
    assert!(unsafe { api::fence_wait(fence, 2_000_000_000) }.is_ok());

    unsafe {
        api::fence_destroy(fence);
        api::binary_destroy(binary);
        api::swapchain_destroy(swapchain);
        api::surface_destroy(surface);
        DestroyWindow(window).expect("window destroyed");
    }
}

/// Swapchain pixels must match an offscreen target: clear, then a clip-space
/// triangle, copied off the acquired image before present.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_swapchain_raster_readback() {
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, CS_HREDRAW,
        CS_VREDRAW, CW_USEDEFAULT, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    let (_discovery, device, queue, domain) =
        gpu_setup_features(1 | 16 | 65536 | RASTER | PRESENTATION);
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: HINSTANCE(std::ptr::null_mut()),
        lpszClassName: windows::core::w!("ogpu_swapchain_raster"),
        ..Default::default()
    };
    assert_ne!(unsafe { RegisterClassW(&class) }, 0);
    let window = unsafe {
        CreateWindowExW(
            Default::default(),
            windows::core::w!("ogpu_swapchain_raster"),
            windows::core::w!("ogpu swapchain raster"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            64,
            64,
            None,
            None,
            None,
            None,
        )
    }
    .expect("test window");

    let discovery = unsafe { api::discover_presentation(1) }.unwrap();
    let adapter = &api::adapters(&discovery)[0];
    let surface = unsafe {
        api::surface_wayland_create(
            adapter,
            &WaylandSurfaceDesc {
                header: Record::new::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC),
                display: std::ptr::null_mut(),
                surface: window.0.cast(),
            },
        )
    }
    .unwrap();
    let swapchain = unsafe {
        api::swapchain_create(
            device,
            &SwapchainDesc {
                header: Record::new::<SwapchainDesc>(SWAPCHAIN_DESC),
                surface: surface.as_raw(),
                old_swapchain: std::ptr::null_mut(),
                format: RGBA8_UNORM,
                color_space: 0,
                extent: [640, 480],
                layer_count: 1,
                min_image_count: 2,
                usage: IMAGE_COLOR_ATTACHMENT | IMAGE_COPY_SRC,
                transform: 1,
                alpha: 0,
                present_mode: 2,
                clipped: 0,
                domain_count: 1,
                mode_count: 0,
                domains: &domain,
                modes: std::ptr::null(),
            },
        )
    }
    .unwrap();
    let mut image_slots: [*mut Image; 16] = [std::ptr::null_mut(); 16];
    let mut image_count = 0u32;
    unsafe { api::swapchain_images(swapchain, &mut image_slots, &mut image_count) }.unwrap();
    let binary = unsafe { api::binary_create(device) }.unwrap();
    let (index, _suboptimal) =
        unsafe { api::acquire(swapchain, u64::MAX, Some(binary), None) }.unwrap();
    let image = unsafe { api::ImageHandle::from_raw(image_slots[index as usize]) }.unwrap();
    let view = attachment_view(device, image, IMAGE_COLOR_ATTACHMENT as u32).unwrap();
    let executable = triangle_executable(device).unwrap();
    let width = 640u32;
    let height = 480u32;
    let texels = u64::from(width) * u64::from(height) * 4;

    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let scratch_requirements = unsafe { api::render_scratch_requirements(1) }.unwrap();
    let (mut scratch, offset) = aligned_scratch(scratch_requirements);
    let attachment = Attachment {
        view: view.as_raw(),
        resolve_view: ptr::null_mut(),
        state: STATE_COLOR_ATTACHMENT,
        resolve_state: 0,
        load_op: CLEAR,
        store_op: STORE,
        resolve_mode: 0,
        clear: ClearValue {
            f32: [0.0, 1.0, 0.0, 1.0],
        },
    };
    let state_requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut state_scratch, state_offset) = aligned_scratch(state_requirements);
    let to_attachment = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COLOR_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COLOR_ATTACHMENT,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let state_dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_COLOR,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_attachment,
        scratch: unsafe { state_scratch.as_mut_ptr().add(state_offset) }.cast(),
        scratch_size: state_requirements.size,
    };
    unsafe { api::barrier(list, &state_dependency) };
    let render = RenderDesc {
        header: Record::new::<RenderDesc>(RENDER_DESC),
        x: 0,
        y: 0,
        width: width,
        height: height,
        layers: 1,
        view_mask: 0,
        samples: 1,
        flags: 0,
        color_count: 1,
        colors: &attachment,
        depth: ptr::null(),
        stencil: ptr::null(),
        scratch: HostSpan {
            data: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
            size: scratch_requirements.size,
        },
    };
    unsafe { api::render_begin(list, &render) };
    unsafe { api::bind_executable(list, executable) };
    let viewport = ViewportState {
        header: Record::new::<ViewportState>(VIEWPORT_STATE),
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
        scissor_x: 0,
        scissor_y: 0,
        scissor_width: width,
        scissor_height: height,
    };
    unsafe { api::set_graphics_state(list, &viewport) };
    unsafe {
        api::draw(
            list,
            &DrawDesc {
                count: 3,
                instances: 1,
                first: 0,
                first_instance: 0,
                vertex_offset: 0,
            },
        )
    };
    unsafe { api::render_end(list) };
    let (mut scratch, offset) = aligned_scratch(state_requirements);
    let to_copy = ImageBarrier {
        before: ACCESS_COLOR_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COLOR_ATTACHMENT,
        new_state: STATE_COPY_SRC,
        ..to_attachment
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        images: &to_copy,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: state_requirements.size,
    };
    unsafe { api::barrier(list, &dependency) };
    let readback = linear_memory(device, texels, USAGE_COPY_DST, 2);
    let copy = ImageCopy {
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent {
                x: width,
                y: height,
                z: 1,
            },
        },
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
    dump_validation(device);
    let pixels = read_back(readback, texels);
    let pixel = |x: usize, y: usize| -> [u8; 4] {
        let base = y * width as usize * 4 + x * 4;
        pixels[base..base + 4].try_into().unwrap()
    };
    assert_eq!(pixel(320, 240), [255, 0, 0, 255], "centre");
    assert_eq!(pixel(0, 0), [0, 255, 0, 255], "corner");

    unsafe {
        api::executable_destroy(executable);
        api::arena_destroy(arena);
        api::view_destroy(view);
        api::memory_destroy(readback);
        api::timeline_destroy(timeline);
        api::binary_destroy(binary);
        api::swapchain_destroy(swapchain);
        api::surface_destroy(surface);
        DestroyWindow(window).expect("window destroyed");
        api::device_destroy(device);
    }
}

/// Regression: a command list may only be closed while recording. Reusing a
/// retired arena slot must record and close cleanly even after the resources
/// the previous recording referenced were destroyed.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_list_reuse_after_retirement() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let memory = linear_memory(device, 256, USAGE_COPY_SRC, 1);
    let readback = linear_memory(device, 256, USAGE_COPY_DST, 2);
    let arena = single_slot_arena(device, domain);
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let submit = |queue: api::QueueHandle,
                  list: api::ListHandle,
                  value: u64,
                  scratch: *mut u8,
                  offset: usize,
                  size: u64| {
        let signal = SyncPoint {
            point: Point {
                timeline: timeline.as_raw(),
                value,
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
                    data: scratch.add(offset).cast(),
                    size,
                },
            )
        }
    };

    // One recording that uses the buffer, retired normally.
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::copy_memory(list, readback.span(0, 256), memory.span(0, 256)) };
    let list = unsafe { api::commands_end(list) }.unwrap();
    submit(
        queue,
        list,
        1,
        scratch.as_mut_ptr(),
        offset,
        requirements.size,
    );
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));

    // The used buffer is destroyed before the next recording begins.
    unsafe { api::memory_destroy(memory) };
    unsafe { api::arena_reset(arena) };

    let staging = linear_memory(device, 256, USAGE_COPY_SRC, 1);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::copy_memory(list, readback.span(0, 256), staging.span(0, 256)) };
    let list = unsafe { api::commands_end(list) }.unwrap();
    submit(
        queue,
        list,
        2,
        scratch.as_mut_ptr(),
        offset,
        requirements.size,
    );
    assert_eq!(unsafe { api::timeline_wait(timeline, 2, u64::MAX) }, Ok(()));

    unsafe {
        api::memory_destroy(staging);
        api::memory_destroy(readback);
        api::arena_destroy(arena);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Regression: index binding requires no bound executable (the contract:
/// binding persists across scopes and binds), and a transfer-to-host
/// dependency records and closes cleanly.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_binding_and_host_barrier_contract() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let indices = linear_memory(device, 256, USAGE_INDEX, 1);
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    unsafe { api::bind_indices(list, indices.span(0, 256), INDEX_U32) };
    let requirements = api::barrier_scratch_requirements(1, 0).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let to_host = MemoryBarrier {
        range: indices.span(0, 256),
        before: ACCESS_COPY_WRITE,
        after: ACCESS_HOST_READ,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
    };
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_HOST,
        global_before: 0,
        global_after: 0,
        memory_count: 1,
        image_count: 0,
        flags: 0,
        memory: &to_host,
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: requirements.size,
        images: ptr::null(),
    };
    unsafe { api::barrier(list, &dependency) };
    assert!(unsafe { api::commands_end(list) }.is_ok());
    unsafe {
        api::arena_destroy(arena);
        api::memory_destroy(indices);
        api::device_destroy(device);
    }
}

/// Regression: the native memory-compatibility rules. Textures exist only on
/// non-host-visible types; copy destinations exclude UPLOAD-type memory and
/// copy sources exclude READBACK-type memory.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_memory_compatibility_rules() {
    let (discovery, device, _queue, _domain) = gpu_setup();
    let adapter = &api::adapters(&discovery)[0];
    let upload_types: Vec<u32> = api::adapter_memory_types(adapter)
        .iter()
        .filter(|m| m.properties & 2 != 0 && m.properties & 8 == 0)
        .map(|m| m.id)
        .collect();
    let host_types: Vec<u32> = api::adapter_memory_types(adapter)
        .iter()
        .filter(|m| m.properties & 2 != 0)
        .map(|m| m.id)
        .collect();
    assert!(!host_types.is_empty());

    let probe = |usage: u64, kind: u32| -> Vec<u32> {
        let desc = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 256,
            alignment: 1,
            usage,
            memory_type: 0,
            kind,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut types = [0u32; 8];
        let mut req = Requirements {
            compatible_types: types.as_mut_ptr(),
            compatible_type_capacity: 8,
            ..Default::default()
        };
        unsafe { api::memory_requirements(device, &desc, &mut req) }.unwrap();
        types[..req.compatible_type_count as usize].to_vec()
    };

    // Textures: no host-visible type may back them.
    let image_types = probe(0, 2);
    assert!(image_types.iter().all(|t| !host_types.contains(t)));

    // Copy destinations: UPLOAD-type memory is copy sources only.
    let dst_types = probe(USAGE_COPY_DST, 1);
    assert!(dst_types.iter().all(|t| !upload_types.contains(t)));

    // Copy sources: READBACK-type memory is copy destinations only.
    let src_types = probe(USAGE_COPY_SRC, 1);
    assert!(!src_types.is_empty());

    unsafe { api::device_destroy(device) };
}

/// Regression: dependency barriers carry layout-implying access sets; the
/// ordering they express must record and close cleanly on this backend.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_layout_access_dependency() {
    let (_discovery, device, _queue, domain) = gpu_setup();
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let requirements = api::barrier_scratch_requirements(0, 0).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    // The consumer's post-render dependency: mixed layout-implying accesses
    // on both sides. The native lowering splits it into the cross product of
    // its implied-layout groups.
    let (gb, ga) = (8192u64 | 32768, 8192u64 | 32768 | 16384);
    let dependency = Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_COLOR | STAGE_DEPTH,
        after: STAGE_COLOR | STAGE_DEPTH,
        global_before: gb,
        global_after: ga,
        memory_count: 0,
        image_count: 0,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: requirements.size,
        images: ptr::null(),
    };
    unsafe { api::barrier(list, &dependency) };
    assert!(unsafe { api::commands_end(list) }.is_ok());
    unsafe {
        api::arena_destroy(arena);
        api::device_destroy(device);
    }
}

/// Tight image copies: upload an 8x8 texture with row_pitch=0, then read it
/// back both with an aligned pitch and with the contract's tight pitch.
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_tight_image_copies() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let side = 8u32;
    let bytes = 256u64;
    let desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: side, y: side, z: 1 },
        usage: IMAGE_SAMPLED | IMAGE_COPY_DST | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut req = Requirements::default();
    unsafe { api::image_requirements(device, &desc, &mut req) }.unwrap();
    let backing = unsafe {
        api::memory_create(
            device,
            &MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: req.size,
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
    let image = unsafe { api::image_create_unbound(device, &desc) }.unwrap();
    unsafe { api::image_bind(image, backing.span(0, req.size)) }.unwrap();
    let staging = linear_memory(device, bytes, USAGE_COPY_SRC, 1);
    let aligned = linear_memory(device, 2048, USAGE_COPY_DST, 2);
    let tight = linear_memory(device, bytes, USAGE_COPY_DST, 2);
    let pattern: Vec<u8> = (0..bytes).map(|i| (i % 251) as u8).collect();
    {
        let mapping = unsafe { api::memory_map(staging.span(0, bytes)) }.unwrap();
        unsafe { ptr::copy_nonoverlapping(pattern.as_ptr(), mapping.data.cast::<u8>(), bytes as usize) };
        unsafe { api::memory_unmap(staging) };
    }
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let to_dst = ImageBarrier {
        image: image.as_raw(),
        range: Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: 1,
            first_layer: 0,
            layer_count: 1,
        },
        before: 0,
        after: ACCESS_COPY_WRITE,
        old_state: STATE_UNDEFINED,
        new_state: STATE_COPY_DST,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: 1,
    };
    let to_src = ImageBarrier {
        before: ACCESS_COPY_WRITE,
        after: ACCESS_COPY_READ,
        old_state: STATE_COPY_DST,
        new_state: STATE_COPY_SRC,
        ..to_dst
    };
    let dependency = |barrier: &ImageBarrier, scratch: *mut u8, offset: usize, size: u64| Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { scratch.add(offset).cast() },
        scratch_size: size,
        images: barrier,
    };
    unsafe { api::barrier(list, &dependency(&to_dst, scratch.as_mut_ptr(), offset, requirements.size)) };
    let upload = ImageCopy {
        region: ImageRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offset: Offset { x: 0, y: 0, z: 0 },
            extent: Extent { x: side, y: side, z: 1 },
        },
        row_pitch: 0,
        slice_pitch: 0,
        state: STATE_COPY_DST,
        reserved: 0,
    };
    unsafe { api::copy_to_image(list, image, staging.span(0, bytes), &upload) };
    unsafe { api::barrier(list, &dependency(&to_src, scratch.as_mut_ptr(), offset, requirements.size)) };
    let mut read = upload;
    read.state = STATE_COPY_SRC;
    read.row_pitch = 256;
    unsafe { api::copy_from_image(list, aligned.span(0, 2048), image, &read) };
    read.row_pitch = 0;
    unsafe { api::copy_from_image(list, tight.span(0, bytes), image, &read) };
    let list = unsafe { api::commands_end(list) }.unwrap();
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let requirements = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut submit_scratch, submit_offset) = aligned_scratch(requirements);
    let signal = SyncPoint {
        point: Point { timeline: timeline.as_raw(), value: 1 },
        stages: STAGE_ALL,
    };
    unsafe {
        api::queue_submit(
            queue,
            &[list],
            &[],
            &[signal],
            HostSpan {
                data: submit_scratch.as_mut_ptr().add(submit_offset).cast(),
                size: requirements.size,
            },
        )
    }
    .unwrap();
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));
    let aligned_bytes = read_back(aligned, 2048);
    for row in 0..side as usize {
        assert_eq!(&aligned_bytes[row * 256..row * 256 + 32], &pattern[row * 32..row * 32 + 32], "aligned row {row}");
    }
    assert_eq!(read_back(tight, bytes), pattern, "tight readback");
    unsafe {
        api::arena_destroy(arena);
        api::image_destroy(image);
        api::memory_destroy(staging);
        api::memory_destroy(aligned);
        api::memory_destroy(tight);
        api::memory_destroy(backing);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}

/// Image-to-image copy and same-size nearest blit. Scaling blit and resolve
/// stay visible as UNSUPPORTED (no shader blit, no MSAA profile).
#[ignore = "requires a live D3D12 adapter; run with --ignored"]
#[test]
fn gpu_copy_image_and_blit() {
    let (_discovery, device, queue, domain) = gpu_setup();
    let side = 8u32;
    let bytes = 256u64;
    let desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: side, y: side, z: 1 },
        usage: IMAGE_COPY_DST | IMAGE_COPY_SRC,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    let mut req = Requirements::default();
    unsafe { api::image_requirements(device, &desc, &mut req) }.unwrap();
    let make = || {
        let backing = unsafe {
            api::memory_create(
                device,
                &MemoryDesc {
                    header: Record::new::<MemoryDesc>(MEMORY_DESC),
                    size: req.size,
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
        let image = unsafe { api::image_create_unbound(device, &desc) }.unwrap();
        unsafe { api::image_bind(image, backing.span(0, req.size)) }.unwrap();
        (image, backing)
    };
    let (src, src_backing) = make();
    let (dst, dst_backing) = make();
    let staging = linear_memory(device, bytes, USAGE_COPY_SRC, 1);
    let readback = linear_memory(device, bytes, USAGE_COPY_DST, 2);
    let pattern: Vec<u8> = (0..bytes).map(|i| (i % 251) as u8).collect();
    {
        let mapping = unsafe { api::memory_map(staging.span(0, bytes)) }.unwrap();
        unsafe { ptr::copy_nonoverlapping(pattern.as_ptr(), mapping.data.cast::<u8>(), bytes as usize) };
        unsafe { api::memory_unmap(staging) };
    }
    let region = ImageRegion {
        aspect: ASPECT_COLOR,
        mip: 0,
        first_layer: 0,
        layer_count: 1,
        offset: Offset { x: 0, y: 0, z: 0 },
        extent: Extent { x: side, y: side, z: 1 },
    };
    let arena = single_slot_arena(device, domain);
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let requirements = api::barrier_scratch_requirements(0, 1).unwrap();
    let (mut scratch, offset) = aligned_scratch(requirements);
    let barrier = |image: api::ImageHandle, old_state: u32, new_state: u32, access_before: u64, access_after: u64| {
        ImageBarrier {
            image: image.as_raw(),
            range: Subresources {
                aspects: ASPECT_COLOR,
                first_mip: 0,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            before: access_before,
            after: access_after,
            old_state,
            new_state,
            source_domain: DOMAIN_IGNORED,
            destination_domain: DOMAIN_IGNORED,
            discard: u32::from(old_state == STATE_UNDEFINED),
        }
    };
    let mut dep = |images: &ImageBarrier| Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before: STAGE_TRANSFER,
        after: STAGE_TRANSFER,
        global_before: 0,
        global_after: 0,
        memory_count: 0,
        image_count: 1,
        flags: 0,
        memory: ptr::null(),
        scratch: unsafe { scratch.as_mut_ptr().add(offset) }.cast(),
        scratch_size: requirements.size,
        images,
    };
    let to_dst = barrier(src, STATE_UNDEFINED, STATE_COPY_DST, 0, ACCESS_COPY_WRITE);
    unsafe { api::barrier(list, &dep(&to_dst)) };
    let upload = ImageCopy {
        region,
        row_pitch: 0,
        slice_pitch: 0,
        state: STATE_COPY_DST,
        reserved: 0,
    };
    unsafe { api::copy_to_image(list, src, staging.span(0, bytes), &upload) };
    let src_to_read = barrier(src, STATE_COPY_DST, STATE_COPY_SRC, ACCESS_COPY_WRITE, ACCESS_COPY_READ);
    unsafe { api::barrier(list, &dep(&src_to_read)) };
    let dst_to_write = barrier(dst, STATE_UNDEFINED, STATE_COPY_DST, 0, ACCESS_COPY_WRITE);
    unsafe { api::barrier(list, &dep(&dst_to_write)) };
    let transfer = ImageTransfer {
        source: region,
        destination: region,
        source_state: STATE_COPY_SRC,
        destination_state: STATE_COPY_DST,
    };
    unsafe { api::copy_image(list, dst, src, &transfer) };
    let dst_to_read = barrier(dst, STATE_COPY_DST, STATE_COPY_SRC, ACCESS_COPY_WRITE, ACCESS_COPY_READ);
    unsafe { api::barrier(list, &dep(&dst_to_read)) };
    let mut read = upload;
    read.state = STATE_COPY_SRC;
    unsafe { api::copy_from_image(list, readback.span(0, bytes), dst, &read) };
    let list = unsafe { api::commands_end(list) }.unwrap();
    let timeline = unsafe { api::timeline_create(device, 0) }.unwrap();
    let submit_req = api::submit_scratch_requirements(1, 0, 1).unwrap();
    let (mut submit_scratch, submit_offset) = aligned_scratch(submit_req);
    let signal = SyncPoint {
        point: Point { timeline: timeline.as_raw(), value: 1 },
        stages: STAGE_ALL,
    };
    unsafe {
        api::queue_submit(
            queue,
            &[list],
            &[],
            &[signal],
            HostSpan {
                data: submit_scratch.as_mut_ptr().add(submit_offset).cast(),
                size: submit_req.size,
            },
        )
    }
    .unwrap();
    assert_eq!(unsafe { api::timeline_wait(timeline, 1, u64::MAX) }, Ok(()));
    assert_eq!(read_back(readback, bytes), pattern);

    unsafe { api::arena_reset(arena) }.unwrap();
    let list = unsafe { api::commands_begin(arena, &one_shot()) }.unwrap();
    let blit = ImageBlit {
        source: BlitRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offsets: [Offset { x: 0, y: 0, z: 0 }, Offset { x: side as i32, y: side as i32, z: 1 }],
        },
        destination: BlitRegion {
            aspect: ASPECT_COLOR,
            mip: 0,
            first_layer: 0,
            layer_count: 1,
            offsets: [Offset { x: 0, y: 0, z: 0 }, Offset { x: side as i32, y: side as i32, z: 1 }],
        },
        source_state: STATE_COPY_SRC,
        destination_state: STATE_COPY_DST,
        filter: 0,
        reserved: 0,
    };
    unsafe { api::blit_image(list, dst, src, &blit) };
    let mut scaled = blit;
    scaled.destination.offsets[1] = Offset { x: 4, y: 4, z: 1 };
    unsafe { api::blit_image(list, dst, src, &scaled) };
    unsafe { api::resolve_image(list, dst, src, &transfer, 1) };
    assert_eq!(unsafe { api::commands_end(list) }, Err(UNSUPPORTED));

    unsafe {
        api::arena_destroy(arena);
        api::image_destroy(src);
        api::image_destroy(dst);
        api::memory_destroy(src_backing);
        api::memory_destroy(dst_backing);
        api::memory_destroy(staging);
        api::memory_destroy(readback);
        api::timeline_destroy(timeline);
        api::device_destroy(device);
    }
}


