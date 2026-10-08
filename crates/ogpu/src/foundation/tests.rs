use super::*;

fn fixture() -> Snapshot {
    Snapshot {
        info: AdapterInfo {
            name: [0; 256],
            backend: 1,
            vendor_id: 0,
            device_id: 0,
            device_type: 0,
            native_api_version: 0,
        },
        queues: vec![
            QueueInfo {
                domain: 3,
                count: 2,
                flags: COMPUTE | TRANSFER,
                ..Default::default()
            },
            QueueInfo {
                domain: 8,
                count: 1,
                flags: TRANSFER,
                ..Default::default()
            },
        ],
        memory_types: vec![],
        memory_heaps: vec![],
        features: FeatureInfo::default(),
        memory_limits: MemoryLimits::default(),
    }
}
fn query<T>(kind: u32, values: &mut [T]) -> Query {
    Query {
        header: Record::new::<Query>(kind),
        data: values.as_mut_ptr().cast(),
        capacity: values.len() as u32,
        count: 91,
        element_size: size_of::<T>() as u32,
        reserved: 0,
    }
}
#[test]
fn query_capacity_is_atomic_and_reports_full_count() {
    let snapshot = fixture();
    let sentinel = QueueInfo {
        domain: 99,
        ..Default::default()
    };
    let mut output = [sentinel; 2];
    let mut q = query(QUEUES, &mut output[..1]);
    assert_eq!(unsafe { snapshot.query(&mut q) }, Err(CAPACITY));
    assert_eq!(q.count, 2);
    assert_eq!(output, [sentinel; 2]);
    q.capacity = 0;
    q.data = ptr::null_mut();
    assert_eq!(unsafe { snapshot.query(&mut q) }, Ok(()));
    assert_eq!(q.count, 2);
    q.capacity = 2;
    q.data = output.as_mut_ptr().cast();
    assert_eq!(unsafe { snapshot.query(&mut q) }, Ok(()));
    assert_eq!(output.as_slice(), snapshot.queues);
}
#[test]
fn malformed_queries_preserve_outputs() {
    let snapshot = fixture();
    let sentinel = QueueInfo {
        domain: 99,
        ..Default::default()
    };
    for (case, expected) in [
        (0, INVALID),
        (1, INVALID),
        (2, UNSUPPORTED),
        (3, UNSUPPORTED),
        (4, INVALID),
        (5, INVALID),
        (6, INVALID),
        (7, INVALID),
    ] {
        let mut output = [sentinel; 2];
        let mut q = query(QUEUES, &mut output);
        match case {
            0 => q.element_size -= 1,
            1 => q.header.byte_size -= 1,
            2 => q.header.kind = 999,
            3 => q.header.version = 999,
            4 => q.header.flags = 1,
            5 => q.header.next = ptr::from_ref(&q.header),
            6 => q.reserved = 1,
            7 => q.data = ptr::null_mut(),
            _ => unreachable!(),
        }
        assert_eq!(unsafe { snapshot.query(&mut q) }, Err(expected));
        assert_eq!(q.count, 91);
        assert_eq!(output, [sentinel; 2]);
    }
    assert_eq!(unsafe { snapshot.query(ptr::null_mut()) }, Err(INVALID));
    // A short record must be rejected before reading the larger query object.
    let mut short = Record::new::<Record>(QUEUES);
    assert_eq!(
        unsafe { snapshot.query(ptr::from_mut(&mut short).cast()) },
        Err(INVALID)
    );
}
#[test]
fn queue_requests_are_exact_and_do_not_alias_or_clamp() {
    let snapshot = fixture();
    let good = QueueRequest {
        domain: 3,
        count: 2,
        priority: 1.0,
    };
    assert_eq!(validate_requests(&snapshot.queues, &[good]), Ok(()));
    assert_eq!(validate_requests(&snapshot.queues, &[]), Err(INVALID));
    assert_eq!(
        validate_requests(&snapshot.queues, &[good, good]),
        Err(INVALID)
    );
    for priority in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert_eq!(
            validate_requests(&snapshot.queues, &[QueueRequest { priority, ..good }]),
            Err(INVALID)
        );
    }
    assert_eq!(
        validate_requests(&snapshot.queues, &[QueueRequest { count: 0, ..good }]),
        Err(INVALID)
    );
    assert_eq!(
        validate_requests(&snapshot.queues, &[QueueRequest { count: 3, ..good }]),
        Err(UNSUPPORTED)
    );
    assert_eq!(
        validate_requests(&snapshot.queues, &[QueueRequest { domain: 0, ..good }]),
        Err(UNSUPPORTED)
    );
}
#[test]
fn c_layouts_and_boundary_rules() {
    assert_eq!((size_of::<Record>(), align_of::<Record>()), (24, 8));
    assert_eq!(size_of::<Query>(), 48);
    assert_eq!(std::mem::offset_of!(Query, data), 24);
    assert_eq!(size_of::<DeviceDesc>(), 48);
    assert_eq!(std::mem::offset_of!(DeviceDesc, required_features), 40);
    assert_eq!(size_of::<AdapterInfo>(), 276);
    assert_eq!(size_of::<QueueInfo>(), 32);
    assert_eq!(size_of::<MemoryTypeInfo>(), 16);
    assert_eq!(size_of::<MemoryHeapInfo>(), 16);
    assert_eq!(size_of::<FeatureInfo>(), 32);
    assert_eq!(size_of::<QueueRequest>(), 12);
    assert_eq!(size_of::<Point>(), 16);
    assert_eq!(size_of::<MemoryDesc>(), 72);
    assert_eq!(size_of::<Span>(), 24);
    assert_eq!(size_of::<Requirements>(), 40);
    assert_eq!(size_of::<Mapping>(), 40);
    assert_eq!(size_of::<MemoryLimits>(), 32);
    unsafe {
        let mut discovery = ptr::dangling_mut();
        assert_eq!(
            ogpu_next_discovery_create(VERSION + 1, &mut discovery),
            VERSION_MISMATCH
        );
        assert!(discovery.is_null());
        let mut device = ptr::dangling_mut();
        assert_eq!(
            ogpu_next_device_create(ptr::null(), ptr::null(), &mut device),
            INVALID
        );
        assert!(device.is_null());
        let mut timeline = ptr::dangling_mut();
        assert_eq!(
            ogpu_next_timeline_create(ptr::null_mut(), 0, &mut timeline),
            INVALID
        );
        assert!(timeline.is_null());
        assert_eq!(ogpu_next_discovery_count(ptr::null()), 0);
        assert!(ogpu_next_discovery_adapter(ptr::null(), 0).is_null());
        assert!(ogpu_next_device_queue(ptr::null(), 0, 0).is_null());
        ogpu_next_timeline_destroy(ptr::null_mut());
        ogpu_next_device_destroy(ptr::null_mut());
        ogpu_next_discovery_destroy(ptr::null_mut());
    }
}

#[test]
#[ignore = "requires modern Vulkan; foundation discovery, explicit queues and concurrent timelines"]
fn gpu_foundation_setup() {
    unsafe {
        let mut discovery = ptr::null_mut();
        assert_eq!(ogpu_next_discovery_create(VERSION, &mut discovery), OK);
        let mut tested = 0;
        for index in 0..ogpu_next_discovery_count(discovery) {
            let adapter = ogpu_next_discovery_adapter(discovery, index);
            let mut features = [FeatureInfo::default()];
            assert_eq!(
                ogpu_next_adapter_query(adapter, &mut query(FEATURES, &mut features)),
                OK
            );
            if features[0].baseline_supported == 0 {
                continue;
            }
            let mut count = query::<QueueInfo>(QUEUES, &mut []);
            assert_eq!(ogpu_next_adapter_query(adapter, &mut count), OK);
            let mut queues = vec![QueueInfo::default(); count.count as usize];
            assert_eq!(
                ogpu_next_adapter_query(adapter, &mut query(QUEUES, &mut queues)),
                OK
            );
            let requests: Vec<_> = queues
                .iter()
                .filter(|q| q.count > 0)
                .map(|q| QueueRequest {
                    domain: q.domain,
                    count: q.count.min(2),
                    priority: 0.5,
                })
                .collect();
            for enabled in [0, features[0].available] {
                let desc = DeviceDesc {
                    header: Record::new::<DeviceDesc>(DEVICE_DESC),
                    queues: requests.as_ptr(),
                    queue_request_count: requests.len() as u32,
                    reserved: 0,
                    required_features: enabled,
                };
                let mut device = ptr::null_mut();
                assert_eq!(ogpu_next_device_create(adapter, &desc, &mut device), OK);
                let mut enabled_info = [FeatureInfo::default()];
                assert_eq!(
                    ogpu_next_device_query(device, &mut query(FEATURES, &mut enabled_info)),
                    OK
                );
                assert_eq!(enabled_info[0].enabled, enabled);
                assert_eq!(enabled_info[0].device_scope, 1);
                let mut returned = vec![QueueInfo::default(); queues.len()];
                assert_eq!(
                    ogpu_next_device_query(device, &mut query(QUEUES, &mut returned)),
                    OK
                );
                let mut handles = Vec::new();
                for request in &requests {
                    assert_eq!(
                        returned
                            .iter()
                            .find(|q| q.domain == request.domain)
                            .unwrap()
                            .count,
                        request.count
                    );
                    for i in 0..request.count {
                        let handle = ogpu_next_device_queue(device, request.domain, i);
                        assert!(!handle.is_null());
                        assert!(!handles.contains(&handle));
                        handles.push(handle);
                    }
                    assert!(
                        ogpu_next_device_queue(device, request.domain, request.count).is_null()
                    );
                }
                // Independent native creations/queries/waits on one device,
                // with no device mutex and no retained completion objects.
                let d = &*device;
                std::thread::scope(|scope| {
                    for _ in 0..4 {
                        scope.spawn(move || {
                            let mut t = ptr::null_mut();
                            assert_eq!(
                                ogpu_next_timeline_create(ptr::from_ref(d).cast_mut(), 7, &mut t),
                                OK
                            );
                            let mut value = 0;
                            assert_eq!(ogpu_next_timeline_poll(t, &mut value), OK);
                            assert_eq!(value, 7);
                            let point = Point {
                                timeline: t,
                                value: 8,
                            };
                            assert_eq!(ogpu_next_timeline_wait(point, 0), TIMEOUT);
                            assert_eq!(ogpu_next_timeline_signal_host(point), OK);
                            assert_eq!(ogpu_next_timeline_wait(point, 1_000_000_000), OK);
                            assert_eq!(ogpu_next_timeline_poll(t, &mut value), OK);
                            assert_eq!(value, 8);
                            ogpu_next_timeline_destroy(t);
                        });
                    }
                });
                ogpu_next_device_destroy(device);
            }
            let mut device = ptr::null_mut();
            let desc = DeviceDesc {
                header: Record::new::<DeviceDesc>(DEVICE_DESC),
                queues: requests.as_ptr(),
                queue_request_count: requests.len() as u32,
                reserved: 0,
                required_features: 1 << 63,
            };
            assert_eq!(
                ogpu_next_device_create(adapter, &desc, &mut device),
                UNSUPPORTED
            );
            assert!(device.is_null());
            tested += 1;
        }
        ogpu_next_discovery_destroy(discovery);
        assert!(
            tested > 0,
            "No suitable physical device; not a passing skip"
        );
    }
}
