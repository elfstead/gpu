//! Bounded two-layer MLP using native cooperative matrices. No scalar fallback.
use ogpu::foundation::{rust as gpu, types::*};
use std::{mem, ptr};
#[path = "../submission.rs"]
mod submission;
const N: usize = 16;
const COUNT: usize = N * N;
const SIZE: usize = 20480;
const X: usize = 2048;
const W1: usize = 4096;
const W2: usize = 6144;
const B1: usize = 8192;
const B2: usize = 10240;
const PRE: usize = 12288;
const HIDDEN: usize = 14336;
const Y: usize = 16384;
const ROOT1: usize = 256;
const ROOT2: usize = 512;
const REQUIRED: [u32; 5] = [
    feature_ids::COOPERATIVE_MATRIX,
    feature_ids::FLOAT16,
    feature_ids::VULKAN_MEMORY_MODEL,
    feature_ids::SUBGROUP_SIZE_CONTROL,
    feature_ids::FULL_SUBGROUPS,
];
const LINEAR: &[u8] = include_bytes!("../../../shaders/foundation-matrix-ml.linear.comp.spv");
const ACTIVATION: &[u8] =
    include_bytes!("../../../shaders/foundation-matrix-ml.activation.comp.spv");

// Exact finite fixture values only. Fail rather than silently round a reference.
fn half_exact(value: f64) -> u16 {
    let bits = (value as f32).to_bits();
    assert_eq!(value as f32 as f64, value);
    if value == 0.0 {
        return 0;
    }
    let exponent = ((bits >> 23) & 255) as i32 - 127 + 15;
    assert!((1..31).contains(&exponent));
    assert_eq!(bits & 8191, 0);
    ((bits >> 16) as u16 & 0x8000) | ((exponent as u16) << 10) | ((bits >> 13) as u16 & 1023)
}
struct Reference {
    x: Vec<f64>,
    w1: Vec<f64>,
    w2: Vec<f64>,
    b1: Vec<f64>,
    b2: Vec<f64>,
    pre: Vec<f64>,
    hidden: Vec<f64>,
    y: Vec<f64>,
}
fn linear(a: &[f64], b: &[f64], bias: &[f64]) -> Vec<f64> {
    (0..COUNT)
        .map(|i| {
            bias[i]
                + (0..N)
                    .map(|k| a[(i / N) * N + k] * b[k * N + i % N])
                    .sum::<f64>()
        })
        .collect()
}
fn reference(seed: usize) -> Reference {
    let values = |mul, modulus, center: i32, scale: f64| {
        (0..COUNT)
            .map(|i| (((i * mul + seed) % modulus) as i32 - center) as f64 / scale)
            .collect::<Vec<_>>()
    };
    let x = values(3, 9, 4, 8.0);
    let w1 = values(7, 5, 2, 8.0);
    let w2 = values(11, 7, 3, 8.0);
    let b1 = (0..COUNT)
        .map(|i| ((i % N) as i32 % 7 - 3) as f64 / 16.0)
        .collect::<Vec<_>>();
    let b2 = (0..COUNT)
        .map(|i| ((i % N) as i32 % 5 - 2) as f64 / 16.0)
        .collect::<Vec<_>>();
    let pre = linear(&x, &w1, &b1);
    let hidden = pre.iter().map(|v| v.max(0.0)).collect::<Vec<_>>();
    let y = linear(&hidden, &w2, &b2);
    Reference {
        x,
        w1,
        w2,
        b1,
        b2,
        pre,
        hidden,
        y,
    }
}
fn selected(t: &MatrixTuple) -> bool {
    [t.m, t.n, t.k] == [16; 3]
        && [t.a, t.b] == [COMPONENT_FLOAT16; 2]
        && [t.c, t.result] == [COMPONENT_FLOAT32; 2]
        && t.saturating == 0
        && t.scope == MATRIX_SCOPE_SUBGROUP
        && t.stages & STAGE_COMPUTE != 0
}
unsafe fn query<T: Copy + Default>(
    adapter: &ogpu::foundation::Adapter,
    kind: u32,
) -> gpu::Result<Vec<T>> {
    let mut q = Query {
        header: Record::new::<Query>(kind),
        reserved: 0,
        element_size: mem::size_of::<T>() as u32,
        count: 0,
        capacity: 0,
        data: ptr::null_mut(),
    };
    unsafe {
        gpu::adapter_query(adapter, &mut q)?;
    }
    let mut values = vec![T::default(); q.count as usize];
    q.data = values.as_mut_ptr().cast();
    q.capacity = q.count;
    unsafe {
        gpu::adapter_query(adapter, &mut q)?;
    }
    Ok(values)
}
pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
    let tuples = unsafe { query::<MatrixTuple>(adapter, MATRIX_TUPLES)? };
    let missing: Vec<_> = REQUIRED
        .iter()
        .filter(|f| !available.contains(f))
        .copied()
        .collect();
    if !missing.is_empty() || !tuples.iter().any(selected) {
        println!("MATRIX_ML_UNAVAILABLE missing={missing:?} exact_16x16x16_half_float_tuple={} execution=not-tested", tuples.iter().any(selected));
        return Err(UNSUPPORTED);
    }
    let limits = unsafe { query::<SubgroupLimits>(adapter, SUBGROUP_LIMITS)? }[0];
    let width = limits.default_size;
    if limits.required_size_stages & STAGE_COMPUTE == 0
        || width < limits.min_size
        || width > limits.max_size
    {
        return Err(UNSUPPORTED);
    }
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let requests = [QueueRequest {
        domain,
        count: 1,
        priority: 1.0,
    }];
    let device = gpu::device_create(adapter, &requests, &REQUIRED)?;
    // Objects remain retained through timeline completion. An uncertain failure
    // exits without destroying possibly pending resources.
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let mut md = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: SIZE as u64,
            alignment: 8,
            usage: USAGE_STORAGE,
            memory_type: 0,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut req = Requirements::default();
        gpu::memory_requirements(device, &md, &mut req)?;
        let mut compatible = vec![0; req.compatible_type_count as usize];
        req.compatible_types = compatible.as_mut_ptr();
        req.compatible_type_capacity = compatible.len() as u32;
        gpu::memory_requirements(device, &md, &mut req)?;
        md.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| m.properties & MEMORY_HOST_VISIBLE != 0 && compatible.contains(&m.id))
            .ok_or(UNSUPPORTED)?
            .id;
        let memory = gpu::memory_create(device, &md)?;
        let span = memory.span(0, SIZE as u64);
        let map = gpu::memory_map(span)?;
        let address = gpu::memory_address(span)?;
        let field = ArgumentField {
            offset: 0,
            size: 8,
            kind: ARGUMENT_INLINE,
            address_alignment: 0,
            stages: STAGE_COMPUTE,
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
        };
        let arguments = gpu::argument_layout_create(
            device,
            &ArgumentLayoutDesc {
                execution_class: ARGUMENT_COMPUTE,
                byte_size: 8,
                field_count: 1,
                flags: 0,
                fields: &field,
            },
        )?;
        let entries = [SpecializationEntry {
            id: 0,
            offset: 0,
            size: 4,
        }];
        let spec = Specialization {
            header: Record::new::<Specialization>(SPECIALIZATION),
            count: 1,
            reserved: 0,
            entries: entries.as_ptr(),
            data: Bytes {
                data: ptr::from_ref(&width).cast(),
                size: 4,
            },
        };
        let subgroup = SubgroupState {
            header: Record::new::<SubgroupState>(SUBGROUP_STATE),
            operations: 0,
            required_size: width,
            flags: 2,
            reserved: 0,
        };
        let mut pipelines = Vec::new();
        for (i, (code, entry)) in [(LINEAR, c"linearMain"), (ACTIVATION, c"activationMain")]
            .into_iter()
            .enumerate()
        {
            let code: Vec<u32> = code
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let shader = Shader {
                stage: 4,
                format: SHADER_SPIRV,
                code: Bytes {
                    data: code.as_ptr().cast(),
                    size: code.len() * 4,
                },
                entry: entry.as_ptr(),
                specialization: if i == 0 { &spec.header } else { ptr::null() },
                subgroup: if i == 0 {
                    &subgroup.header
                } else {
                    ptr::null()
                },
            };
            let features = if i == 0 {
                REQUIRED.as_slice()
            } else {
                &[feature_ids::FLOAT16]
            };
            let requirements = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: Features {
                    ids: features.as_ptr(),
                    count: features.len() as u32,
                    reserved: 0,
                },
                local_size: [if i == 0 { width } else { 64 }, 1, 1],
                shared_memory: 0,
            };
            let ed = ExecutableDesc {
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
            pipelines.push(gpu::executable_create(device, &ed, arguments)?);
        }
        let arena = gpu::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )?;
        let list = gpu::commands_begin(
            arena,
            &RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            },
        )?;
        let br = gpu::barrier_scratch_requirements(device, 1, 0, 0)?;
        assert!(br.alignment <= 8);
        let mut barriers = vec![0u64; br.size.div_ceil(8) as usize];
        for (step, (pipeline, root, groups)) in [
            (pipelines[0], ROOT1, 1),
            (pipelines[1], ROOT1, 4),
            (pipelines[0], ROOT2, 1),
        ]
        .into_iter()
        .enumerate()
        {
            gpu::bind_executable(list, pipeline);
            gpu::set_arguments(list, arguments, 0, &(address + root as u64).to_ne_bytes());
            gpu::dispatch(
                list,
                &Launch {
                    groups: Extent {
                        x: groups,
                        y: 1,
                        z: 1,
                    },
                    dynamic_shared_bytes: 0,
                    extensions: ptr::null(),
                },
            );
            let scope = BarrierScope {
                stages_before: STAGE_COMPUTE,
                stages_after: if step == 2 { STAGE_HOST } else { STAGE_COMPUTE },
                access_before: ACCESS_SHADER_WRITE,
                access_after: if step == 2 {
                    ACCESS_HOST_READ
                } else {
                    ACCESS_SHADER_READ
                },
            };
            gpu::barrier(
                list,
                &Dependency {
                    global_count: 1,
                    global: &scope,
                    ..Default::default()
                },
                HostSpan {
                    data: barriers.as_mut_ptr().cast(),
                    size: br.size,
                },
            );
        }
        let list = gpu::commands_end(list)?;
        let done = gpu::timeline_create(device, 0)?;
        let sr = gpu::submit_scratch_requirements(
            device,
            &[SubmitDesc {
                list_count: 1,
                signal_count: 1,
                ..Default::default()
            }],
        )?;
        assert!(sr.alignment <= 8);
        let mut scratch = vec![0u64; sr.size.div_ceil(8) as usize];
        for (phase, seed) in [1, 4, 1].into_iter().enumerate() {
            let r = reference(seed);
            let bytes = std::slice::from_raw_parts_mut(map.data.cast::<u8>(), SIZE);
            bytes.fill(0xcd);
            for (offset, values) in [(X, &r.x), (W1, &r.w1), (W2, &r.w2)] {
                for (i, v) in values.iter().enumerate() {
                    bytes[offset + i * 2..offset + i * 2 + 2]
                        .copy_from_slice(&half_exact(*v).to_ne_bytes());
                }
            }
            for (offset, values) in [(B1, &r.b1), (B2, &r.b2)] {
                for (i, v) in values.iter().enumerate() {
                    bytes[offset + i * 4..offset + i * 4 + 4]
                        .copy_from_slice(&(*v as f32).to_ne_bytes());
                }
            }
            for (root, fields) in [
                (ROOT1, [X, W1, B1, PRE, HIDDEN]),
                (ROOT2, [HIDDEN, W2, B2, Y, HIDDEN]),
            ] {
                for (i, offset) in fields.into_iter().enumerate() {
                    bytes[root + i * 8..root + i * 8 + 8]
                        .copy_from_slice(&(address + offset as u64).to_ne_bytes());
                }
            }
            let before = bytes.to_vec();
            gpu::memory_flush(span)?;
            let signal = SyncPoint {
                point: Point {
                    timeline: done.as_raw(),
                    value: phase as u64 + 1,
                },
                stages: STAGE_ALL,
            };
            submission::one(
                queue,
                &[list],
                &[],
                &[signal],
                &[],
                &[],
                HostSpan {
                    data: scratch.as_mut_ptr().cast(),
                    size: sr.size,
                },
            )?;
            gpu::timeline_wait(done, phase as u64 + 1, 10_000_000_000)?;
            gpu::memory_invalidate(span)?;
            let bytes = std::slice::from_raw_parts(map.data.cast::<u8>(), SIZE);
            for (offset, values) in [(PRE, &r.pre), (Y, &r.y)] {
                for (i, expected) in values.iter().enumerate() {
                    let got = f32::from_ne_bytes(
                        bytes[offset + i * 4..offset + i * 4 + 4]
                            .try_into()
                            .unwrap(),
                    );
                    assert_eq!(
                        got as f64, *expected,
                        "phase {phase}, offset {offset}, element {i}"
                    );
                }
            }
            for (i, expected) in r.hidden.iter().enumerate() {
                assert_eq!(
                    u16::from_ne_bytes(
                        bytes[HIDDEN + i * 2..HIDDEN + i * 2 + 2]
                            .try_into()
                            .unwrap()
                    ),
                    half_exact(*expected)
                );
            }
            for i in 0..SIZE {
                if ![(PRE, 1024), (HIDDEN, 512), (Y, 1024)]
                    .iter()
                    .any(|(s, n)| (*s..*s + *n).contains(&i))
                {
                    assert_eq!(bytes[i], before[i], "input/root/guard byte {i}");
                }
            }
        }
        gpu::arena_destroy(arena);
        gpu::timeline_destroy(done);
        for p in pipelines {
            gpu::executable_destroy(p);
        }
        gpu::argument_layout_destroy(arguments);
        gpu::memory_unmap(memory);
        gpu::memory_destroy(memory);
        gpu::device_destroy(device);
    }
    println!("MATRIX_ML_PASS shape=16x16x16 layers=2 phases=A/B/A full_intermediates=exact guards=pass native_dispatches=3 scalar_fallback=none");
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(e) = run() {
        eprintln!("MATRIX_ML_STATUS={e}");
        std::process::exit(if e == UNSUPPORTED { 77 } else { 1 });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ml_reference_exercises_activation_and_exact_storage() {
        assert_eq!(half_exact(1.0), 0x3c00);
        assert_eq!(half_exact(-0.5), 0xb800);
        let a = reference(1);
        let b = reference(4);
        assert_ne!(a.y, b.y);
        assert_eq!(a.y, reference(1).y);
        for r in [a, b] {
            assert!(r.pre.iter().any(|v| *v < 0.0));
            assert!(r.pre.iter().any(|v| *v > 0.0));
            for v in r.x.iter().chain(&r.w1).chain(&r.w2).chain(&r.hidden) {
                half_exact(*v);
            }
            for v in r.pre.iter().chain(&r.y) {
                assert_eq!(*v as f32 as f64, *v);
            }
        }
    }
    #[test]
    fn selection_is_joint_not_a_shape_or_precision_substitution() {
        let t = MatrixTuple {
            m: 16,
            n: 16,
            k: 16,
            a: COMPONENT_FLOAT16,
            b: COMPONENT_FLOAT16,
            c: COMPONENT_FLOAT32,
            result: COMPONENT_FLOAT32,
            scope: MATRIX_SCOPE_SUBGROUP,
            stages: STAGE_COMPUTE,
            ..Default::default()
        };
        assert!(selected(&t));
        for bad in [
            MatrixTuple { m: 8, ..t },
            MatrixTuple {
                a: COMPONENT_BFLOAT16,
                ..t
            },
            MatrixTuple {
                scope: MATRIX_SCOPE_WORKGROUP,
                ..t
            },
            MatrixTuple {
                result: COMPONENT_FLOAT16,
                ..t
            },
            MatrixTuple { saturating: 1, ..t },
            MatrixTuple { stages: 8, ..t },
        ] {
            assert!(!selected(&bad));
        }
    }
}
