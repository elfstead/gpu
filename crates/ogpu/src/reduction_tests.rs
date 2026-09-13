//! A workload test of the existing API, not a runtime reduction primitive.
use super::*;
use crate::compute::batch::{COMPUTE_READ, COMPUTE_WRITE};

const GUARD: u32 = 0xa5a5a5a5;

fn stage_counts(mut count: u32) -> Vec<u32> {
    let mut stages = Vec::new();
    loop {
        count = count.div_ceil(128).max(1);
        stages.push(count);
        if count == 1 {
            return stages;
        }
    }
}

#[test]
fn reduction_stage_boundaries() {
    assert_eq!(stage_counts(0), [1]);
    assert_eq!(stage_counts(1), [1]);
    assert_eq!(stage_counts(128), [1]);
    assert_eq!(stage_counts(129), [2, 1]);
    assert_eq!(stage_counts(16384), [128, 1]);
    assert_eq!(stage_counts(16385), [129, 2, 1]);
    assert_eq!(stage_counts(1048579), [8193, 65, 1]);
    // Planning must not overflow at count + 127, even though this size is not run.
    assert_eq!(stage_counts(u32::MAX), [33554432, 262144, 2048, 16, 1]);
}

fn bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|n| n.to_ne_bytes()).collect()
}

fn guarded(device: Rc<Device>, values: &[u32]) -> Buffer {
    let buffer = Buffer::new(device, (values.len() + 2) * 4).unwrap();
    let mut contents = Vec::with_capacity(values.len() + 2);
    contents.push(GUARD);
    contents.extend_from_slice(values);
    contents.push(GUARD);
    buffer.write(0, &bytes(&contents)).unwrap();
    buffer
}

fn read_guarded(buffer: &Buffer) -> Vec<u32> {
    let mut output = vec![0; buffer.size];
    unsafe {
        buffer.read(0, output.as_mut_ptr(), output.len()).unwrap();
    }
    let values: Vec<u32> = output
        .chunks_exact(4)
        .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(values.first(), Some(&GUARD), "Prefix guard overwritten");
    assert_eq!(values.last(), Some(&GUARD), "Suffix guard overwritten");
    values[1..values.len() - 1].to_vec()
}

fn check_reduction(device: Rc<Device>, kernel: Rc<Kernel>, input: &[u32]) {
    let expected_total = input.iter().map(|&n| u64::from(n)).sum::<u64>() as u32;
    let mut allocations = vec![guarded(device.clone(), input)];
    let mut count = input.len() as u32;
    let mut batch = Batch::new(device.clone()).unwrap();
    for groups in stage_counts(count) {
        let output = guarded(device.clone(), &vec![GUARD; groups as usize]);
        let mut root = [0u8; 24];
        root[..8]
            .copy_from_slice(&(allocations.last().unwrap().address().unwrap() + 4).to_ne_bytes());
        root[8..16].copy_from_slice(&(output.address().unwrap() + 4).to_ne_bytes());
        root[16..20].copy_from_slice(&count.to_ne_bytes());
        batch
            .dispatch(kernel.clone(), [groups, 1, 1], &root)
            .unwrap();
        root.fill(0); // Every level must have its own copied argument payload.
        allocations.push(output); // Addresses do not retain scratch; this vector does.
        if groups != 1 {
            batch.barrier(COMPUTE_WRITE, COMPUTE_READ).unwrap();
        }
        count = groups;
    }
    // SAFETY: every referenced allocation is owned here until the chain completes,
    // with distinct per-level outputs and explicit producer/consumer dependencies.
    let mut completion = unsafe { batch.submit().unwrap() };
    drop(batch);
    completion.wait().unwrap(); // Only wait; no per-level CPU readback while executing.
    drop(completion);

    assert_eq!(read_guarded(&allocations[0]), input, "Input was modified");
    let mut expected = input.to_vec();
    for (level, buffer) in allocations.iter().skip(1).enumerate() {
        expected = if expected.is_empty() {
            vec![0]
        } else {
            expected
                .chunks(128)
                .map(|chunk| chunk.iter().map(|&n| u64::from(n)).sum::<u64>() as u32)
                .collect()
        };
        assert_eq!(
            read_guarded(buffer),
            expected,
            "Incorrect partials at level {level}"
        );
    }
    assert_eq!(expected, [expected_total]);
}

#[test]
#[ignore = "requires Vulkan; checks shared-memory reduction and all intermediate levels"]
fn gpu_reduction() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<u32> = include_bytes!("../../../examples/shaders/reduce.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let counts = [
        0u32, 1, 2, 63, 64, 65, 127, 128, 129, 255, 256, 257, 4099, 16383, 16384, 16385, 1048579,
    ];
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &words, 24, &[]).unwrap() });
        for count in counts {
            for pattern in 0..4 {
                let input: Vec<u32> = (0..count)
                    .map(|i| match pattern {
                        0 => 0,
                        1 => 1,
                        2 => u32::MAX,
                        _ => i.wrapping_mul(1664525).wrapping_add(1013904223) ^ (i >> 5),
                    })
                    .collect();
                check_reduction(device.clone(), kernel.clone(), &input);
            }
        }
        let info = instance.device_info(physical).unwrap();
        println!(
            "Verified {} reductions, including every intermediate level and guards, on {}",
            counts.len() * 4,
            unsafe { std::ffi::CStr::from_ptr(info.name.as_ptr()) }.to_string_lossy()
        );
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
}
