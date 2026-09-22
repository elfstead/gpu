use super::*;

#[test]
fn host_atom_ranges_cover_atoms_without_overflow() {
    assert_eq!(host_atom_range(1000, 260, 1, 256).unwrap(), (256, 256));
    assert_eq!(host_atom_range(1000, 900, 100, 256).unwrap(), (768, 232));
    assert_eq!(
        host_atom_range(u64::MAX, u64::MAX - 1, 1, 256).unwrap(),
        (u64::MAX - 255, 255)
    );
    for (a, o, s, g) in [
        (100, 99, 2, 16),
        (100, 0, 0, 16),
        (100, 0, 1, 0),
        (100, 100, 1, 16),
    ] {
        assert!(host_atom_range(a, o, s, g).is_err());
    }
    for atom in [1, 16, 64, 256, 512] {
        let stride = 388u64.div_ceil(atom) * atom;
        assert_eq!(
            host_atom_range(stride * 2, 0, 388, atom).unwrap(),
            (0, stride)
        );
        assert_eq!(
            host_atom_range(stride * 2, stride, 388, atom).unwrap(),
            (stride, stride)
        );
    }
}

thread_local! {
    static STATUS: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
    static FLUSH_ONLY: Cell<bool> = const { Cell::new(false) };
    static CALLS: RefCell<Vec<(bool,u64,u64)>> = const { RefCell::new(Vec::new()) };
}
unsafe extern "C" fn flush(
    _: vk::VkDevice,
    count: u32,
    ranges: *const vk::VkMappedMemoryRange,
) -> vk::VkResult {
    assert_eq!(count, 1);
    let r = unsafe { *ranges };
    CALLS.with_borrow_mut(|calls| calls.push((true, r.offset, r.size)));
    STATUS.get()
}
unsafe extern "C" fn invalidate(
    _: vk::VkDevice,
    count: u32,
    ranges: *const vk::VkMappedMemoryRange,
) -> vk::VkResult {
    assert_eq!(count, 1);
    let r = unsafe { *ranges };
    CALLS.with_borrow_mut(|calls| calls.push((false, r.offset, r.size)));
    if FLUSH_ONLY.get() {
        vk::VkResult_VK_SUCCESS
    } else {
        STATUS.get()
    }
}

#[test]
#[ignore = "requires Vulkan; borrowed view, mocked noncoherent cache/error paths on real backing"]
fn gpu_host_views() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::create_configured(instance.clone(), physical, false, |f| {
            f.vkFlushMappedMemoryRanges = Some(flush);
            f.vkInvalidateMappedMemoryRanges = Some(invalidate);
        }) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        STATUS.set(vk::VkResult_VK_SUCCESS);
        FLUSH_ONLY.set(false);
        CALLS.with_borrow_mut(Vec::clear);
        let atom = d.limits.nonCoherentAtomSize;
        let mut b = Buffer::new(d.clone(), (atom * 2 + 3) as usize).unwrap();
        let v = b.host_view().unwrap();
        assert_eq!(v.data, b.mapped.cast());
        assert_eq!(v.size_bytes, b.size as u64);
        assert!(v.alignment > 0 && (v.data as usize).is_multiple_of(v.alignment as usize));
        assert_eq!(b.host_view().unwrap().data, v.data);
        b.coherent = false; // Cache calls below are mocks, never pass fake properties to Vulkan.
        assert_eq!(b.host_view().unwrap().access_granularity, atom);
        assert_eq!(b.host_view().unwrap().coherent, 0);
        b.host_cache(atom + 1, 1, true).unwrap();
        b.host_cache(atom + 1, 1, false).unwrap();
        let tail = host_atom_range(b.allocated, atom * 2, 3, atom).unwrap();
        b.host_cache(atom * 2, 3, true).unwrap();
        assert_eq!(
            CALLS.with_borrow(Clone::clone),
            vec![
                (true, atom, atom),
                (false, atom, atom),
                (true, tail.0, tail.1)
            ]
        );
        let before = CALLS.with_borrow(Vec::len);
        b.host_cache(b.size as u64, 0, true).unwrap();
        assert!(b.host_cache(b.size as u64, 1, true).is_err());
        assert!(b.host_cache(u64::MAX, 2, false).is_err());
        assert_eq!(CALLS.with_borrow(Vec::len), before);
        let device = Buffer::placed(d.clone(), 16, Placement::Device).unwrap();
        assert_eq!(device.host_view().unwrap_err().status, INVALID_ARGUMENT);
        assert_eq!(
            device.host_cache(0, 0, true).unwrap_err().status,
            INVALID_ARGUMENT
        );
        unsafe {
            ptr::write_bytes(b.mapped, 0x55, b.size);
        }
        b.write(1, &[0x22]).unwrap(); // Partial update preserves neighboring bytes.
        assert_eq!(
            unsafe { std::slice::from_raw_parts(b.mapped, 3) },
            &[0x55, 0x22, 0x55]
        );
        for failure in [
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            vk::VkResult_VK_ERROR_UNKNOWN,
        ] {
            STATUS.set(failure);
            assert_eq!(b.host_cache(0, 1, true).unwrap_err().vk, failure);
            let mut destination = [0x77];
            assert!(unsafe { b.read(0, destination.as_mut_ptr(), 1) }.is_err());
            assert_eq!(destination, [0x77]);
            STATUS.set(vk::VkResult_VK_SUCCESS);
            b.host_cache(0, 1, true).unwrap();
            b.host_cache(0, 1, false).unwrap();
        }
        // A flush failure occurs after caller stores: never claim rollback.
        FLUSH_ONLY.set(true);
        STATUS.set(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
        assert!(b.write(1, &[0x33]).is_err());
        assert_eq!(unsafe { b.mapped.add(1).read() }, 0x33);
        FLUSH_ONLY.set(false);
        STATUS.set(vk::VkResult_VK_SUCCESS);
        b.host_cache(1, 1, true).unwrap();
        b.coherent = true;
        let before = CALLS.with_borrow(Vec::len);
        assert_eq!(b.host_view().unwrap().access_granularity, 1);
        assert_eq!(b.host_view().unwrap().coherent, 1);
        b.host_cache(1, 1, true).unwrap();
        b.host_cache(1, 1, false).unwrap();
        assert_eq!(CALLS.with_borrow(Vec::len), before);
        b.coherent = false;
        STATUS.set(vk::VkResult_VK_ERROR_DEVICE_LOST);
        assert!(b.host_cache(0, 1, true).is_err());
        assert!(d.lost.get());
        STATUS.set(vk::VkResult_VK_SUCCESS);
        assert!(b.host_view().is_err() && b.host_cache(0, 0, false).is_err());
        tested += 1;
    }
    assert!(tested > 0);
}
