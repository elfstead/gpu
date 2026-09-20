use super::*;

#[test]
fn heap_alignment_checks_overflow_and_invalid_properties() {
    assert_eq!(align_up(65, 64).unwrap(), 128);
    assert_eq!(align_up(64, 64).unwrap(), 64);
    assert!(align_up(u64::MAX, 64).is_err());
    assert!(align_up(0, 0).is_err());
    assert!(align_up(8, 3).is_err());
}

thread_local! {
    static FAIL: Cell<u32> = const { Cell::new(0) };
    static RESOURCE: Cell<vk::PFN_vkWriteResourceDescriptorsEXT> = const { Cell::new(None) };
    static SAMPLER: Cell<vk::PFN_vkWriteSamplerDescriptorsEXT> = const { Cell::new(None) };
    static FLUSH: Cell<vk::PFN_vkFlushMappedMemoryRanges> = const { Cell::new(None) };
    static INVALIDATE: Cell<vk::PFN_vkInvalidateMappedMemoryRanges> = const { Cell::new(None) };
    static ALLOCATE: Cell<vk::PFN_vkAllocateMemory> = const { Cell::new(None) };
    static BIND: Cell<vk::PFN_vkBindBufferMemory> = const { Cell::new(None) };
    static FORMAT: Cell<vk::PFN_vkGetPhysicalDeviceFormatProperties> = const { Cell::new(None) };
    static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static GATE: Cell<vk::VkSemaphore> = const { Cell::new(ptr::null_mut()) };
}
const OOM: vk::VkResult = vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
unsafe extern "C" fn resource(
    d: vk::VkDevice,
    n: u32,
    r: *const vk::VkResourceDescriptorInfoEXT,
    o: *const vk::VkHostAddressRangeEXT,
) -> vk::VkResult {
    let status = unsafe { (RESOURCE.get().unwrap())(d, n, r, o) };
    if FAIL.get() == 1 {
        OOM
    } else {
        status
    }
}
unsafe extern "C" fn sampler(
    d: vk::VkDevice,
    n: u32,
    r: *const vk::VkSamplerCreateInfo,
    o: *const vk::VkHostAddressRangeEXT,
) -> vk::VkResult {
    let status = unsafe { (SAMPLER.get().unwrap())(d, n, r, o) };
    if FAIL.get() == 2 {
        OOM
    } else {
        status
    }
}
unsafe extern "C" fn flush(
    d: vk::VkDevice,
    n: u32,
    r: *const vk::VkMappedMemoryRange,
) -> vk::VkResult {
    if FAIL.get() == 3 {
        OOM
    } else {
        unsafe { (FLUSH.get().unwrap())(d, n, r) }
    }
}
unsafe extern "C" fn invalidate(
    d: vk::VkDevice,
    n: u32,
    r: *const vk::VkMappedMemoryRange,
) -> vk::VkResult {
    if FAIL.get() == 8 {
        OOM
    } else {
        unsafe { (INVALIDATE.get().unwrap())(d, n, r) }
    }
}
unsafe extern "C" fn allocate(
    d: vk::VkDevice,
    i: *const vk::VkMemoryAllocateInfo,
    a: *const vk::VkAllocationCallbacks,
    o: *mut vk::VkDeviceMemory,
) -> vk::VkResult {
    if FAIL.get() == 4 {
        OOM
    } else {
        unsafe { (ALLOCATE.get().unwrap())(d, i, a, o) }
    }
}
unsafe extern "C" fn bind(
    d: vk::VkDevice,
    b: vk::VkBuffer,
    m: vk::VkDeviceMemory,
    o: u64,
) -> vk::VkResult {
    if FAIL.get() == 5 {
        OOM
    } else {
        unsafe { (BIND.get().unwrap())(d, b, m, o) }
    }
}
unsafe extern "C" fn format(
    p: vk::VkPhysicalDevice,
    f: vk::VkFormat,
    o: *mut vk::VkFormatProperties,
) {
    unsafe {
        (FORMAT.get().unwrap())(p, f, o);
        if FAIL.get() == 6 {
            (*o).optimalTilingFeatures = 0;
        }
    }
}
unsafe extern "C" fn submit(
    q: vk::VkQueue,
    n: u32,
    s: *const vk::VkSubmitInfo2,
    f: vk::VkFence,
) -> vk::VkResult {
    if FAIL.get() == 7 {
        return OOM;
    }
    assert_eq!(n, 1);
    let mut info = unsafe { *s };
    let gate = vk::VkSemaphoreSubmitInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
        semaphore: GATE.get(),
        value: 1,
        stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        ..Default::default()
    };
    if !GATE.get().is_null() {
        info.waitSemaphoreInfoCount = 1;
        info.pWaitSemaphoreInfos = &gate;
    }
    unsafe { (SUBMIT.get().unwrap())(q, n, &info, f) }
}

struct Gate {
    device: Rc<Device>,
    semaphore: vk::VkSemaphore,
    signal: vk::PFN_vkSignalSemaphore,
    completion: Option<Completion>,
    opened: bool,
}
impl Gate {
    fn open(&mut self) {
        if !self.opened {
            let info = vk::VkSemaphoreSignalInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SIGNAL_INFO,
                semaphore: self.semaphore,
                value: 1,
                ..Default::default()
            };
            assert_eq!(
                unsafe { (self.signal.unwrap())(self.device.handle, &info) },
                vk::VkResult_VK_SUCCESS
            );
            self.opened = true;
            GATE.set(ptr::null_mut());
        }
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.open();
        self.completion = None;
        unsafe {
            (self.device.f.vkDestroySemaphore.unwrap())(
                self.device.handle,
                self.semaphore,
                ptr::null(),
            );
        }
    }
}

fn snapshot(heap: &Heap) -> Vec<u8> {
    unsafe {
        std::slice::from_raw_parts(
            heap.buffer.mapped.add(heap.offset),
            heap.capacity * heap.stride,
        )
        .to_vec()
    }
}

#[test]
#[ignore = "requires graphics Vulkan; independent heaps, gated mutation, atomic writes and poisoned flush"]
fn gpu_heaps() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        FAIL.set(0);
        GATE.set(ptr::null_mut());
        let d = match Device::create_configured(instance.clone(), physical, true, |f| {
            RESOURCE.set(f.vkWriteResourceDescriptorsEXT);
            f.vkWriteResourceDescriptorsEXT = Some(resource);
            SAMPLER.set(f.vkWriteSamplerDescriptorsEXT);
            f.vkWriteSamplerDescriptorsEXT = Some(sampler);
            FLUSH.set(f.vkFlushMappedMemoryRanges);
            f.vkFlushMappedMemoryRanges = Some(flush);
            INVALIDATE.set(f.vkInvalidateMappedMemoryRanges);
            f.vkInvalidateMappedMemoryRanges = Some(invalidate);
            ALLOCATE.set(f.vkAllocateMemory);
            f.vkAllocateMemory = Some(allocate);
            BIND.set(f.vkBindBufferMemory);
            f.vkBindBufferMemory = Some(bind);
            FORMAT.set(f.vkGetPhysicalDeviceFormatProperties);
            f.vkGetPhysicalDeviceFormatProperties = Some(format);
            SUBMIT.set(f.vkQueueSubmit2);
            f.vkQueueSubmit2 = Some(submit);
        }) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        assert!(ImageHeap::new(d.clone(), 0).is_err());
        assert!(SamplerHeap::new(d.clone(), 0).is_err());
        for point in [4, 5] {
            FAIL.set(point);
            assert!(ImageHeap::new(d.clone(), 2).is_err());
            assert!(SamplerHeap::new(d.clone(), 2).is_err());
            assert_eq!(
                Rc::strong_count(&d),
                1,
                "partial heap allocation leaked device ownership"
            );
        }
        FAIL.set(0);
        let target = Rc::new(Image::new(d.clone(), ImageDesc::rgba8(2, 3)).unwrap());
        let other = Rc::new(Image::new(d.clone(), ImageDesc::rgba8(2, 3)).unwrap());
        let foreign = Device::new_graphics(instance.clone(), physical).unwrap();
        let foreign_target = Rc::new(Image::new(foreign.clone(), ImageDesc::rgba8(1, 1)).unwrap());
        let mut images = Rc::new(ImageHeap::new(d.clone(), 2).unwrap());
        let mut samplers = Rc::new(SamplerHeap::new(d.clone(), 2).unwrap());
        let desc = SamplerDesc::default();
        let h = Rc::get_mut(&mut images).unwrap();
        h.write(
            0,
            vec![(target.clone(), SAMPLED), (target.clone(), STORAGE)],
        )
        .unwrap();
        let before = snapshot(&h.heap);
        for entries in [
            vec![(other.clone(), SAMPLED), (foreign_target, STORAGE)],
            vec![(other.clone(), SAMPLED), (target.clone(), 99)],
        ] {
            assert!(h.write(0, entries).is_err());
            assert_eq!(snapshot(&h.heap), before);
        }
        assert!(h.write(2, vec![(target.clone(), SAMPLED)]).is_err());
        assert!(h.clear(u32::MAX, 1).is_err());
        h.write(2, vec![]).unwrap();
        FAIL.set(1);
        assert!(h.write(0, vec![(other.clone(), SAMPLED)]).is_err());
        assert_eq!(snapshot(&h.heap), before);
        assert!(Rc::ptr_eq(h.entries[0].as_ref().unwrap(), &target));
        let coherent = h.heap.buffer.coherent;
        h.heap.buffer.coherent = false;
        FAIL.set(8);
        assert!(h.write(0, vec![(other.clone(), SAMPLED)]).is_err());
        assert_eq!(snapshot(&h.heap), before);
        assert!(Rc::ptr_eq(h.entries[0].as_ref().unwrap(), &target));
        assert!(!h.heap.poisoned);
        h.heap.buffer.coherent = coherent;
        FAIL.set(0);
        h.clear(1, 1).unwrap();
        assert!(h.entries[1].is_none());
        h.write(1, vec![(other.clone(), STORAGE)]).unwrap();
        let weak = Rc::downgrade(&other);
        drop(other);
        h.clear(1, 1).unwrap();
        assert!(weak.upgrade().is_none());
        let s = Rc::get_mut(&mut samplers).unwrap();
        s.write(0, &[desc]).unwrap();
        let before = snapshot(&s.heap);
        assert!(s
            .write(
                0,
                &[
                    desc,
                    SamplerDesc {
                        address_v: 2,
                        ..desc
                    }
                ]
            )
            .is_err());
        assert!(s.write(2, &[desc]).is_err());
        s.write(2, &[]).unwrap();
        FAIL.set(2);
        assert!(s.write(0, &[desc]).is_err());
        assert_eq!(snapshot(&s.heap), before);
        let coherent = s.heap.buffer.coherent;
        s.heap.buffer.coherent = false;
        FAIL.set(8);
        assert!(s.write(0, &[desc]).is_err());
        assert_eq!(snapshot(&s.heap), before);
        assert!(!s.heap.poisoned);
        s.heap.buffer.coherent = coherent;
        FAIL.set(6);
        // Filtering is a promise of a sampled image, not of a sampler alone.
        s.write(
            0,
            &[SamplerDesc {
                mag_filter: 1,
                ..desc
            }],
        )
        .unwrap();
        for format in [0, 1] {
            let image = ImageDesc {
                format,
                usage: graphics::SAMPLED,
                ..ImageDesc::rgba8(2, 3)
            };
            assert_eq!(
                Image::check_support(&d, image).unwrap_err().status,
                UNSUPPORTED
            );
            assert!(matches!(Image::new(d.clone(), image), Err(e) if e.status == UNSUPPORTED));
            Image::new(
                d.clone(),
                ImageDesc {
                    usage: graphics::COPY_SRC,
                    ..image
                },
            )
            .unwrap();
        }
        FAIL.set(0);
        assert!(Batch::new(foreign.clone())
            .unwrap()
            .bind_images(images.clone())
            .is_err());
        assert!(Batch::new(foreign)
            .unwrap()
            .bind_samplers(samplers.clone())
            .is_err());
        // Abandoned and rejected recordings release both heap kinds.
        for rejected in [false, true] {
            let mut batch = Batch::new(d.clone()).unwrap();
            batch.bind_images(images.clone()).unwrap();
            batch.bind_samplers(samplers.clone()).unwrap();
            assert!(Rc::get_mut(&mut images).is_none() && Rc::get_mut(&mut samplers).is_none());
            if rejected {
                FAIL.set(7);
                assert!(unsafe { batch.submit() }.is_err());
                FAIL.set(0);
            }
            drop(batch);
            assert!(Rc::get_mut(&mut images).is_some() && Rc::get_mut(&mut samplers).is_some());
        }
        let ty = vk::VkSemaphoreTypeCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
            semaphoreType: vk::VkSemaphoreType_VK_SEMAPHORE_TYPE_TIMELINE,
            ..Default::default()
        };
        let create = vk::VkSemaphoreCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
            pNext: (&ty as *const vk::VkSemaphoreTypeCreateInfo).cast(),
            ..Default::default()
        };
        let mut semaphore = ptr::null_mut();
        assert_eq!(
            unsafe {
                (d.f.vkCreateSemaphore.unwrap())(d.handle, &create, ptr::null(), &mut semaphore)
            },
            vk::VkResult_VK_SUCCESS
        );
        let mut gate = Gate {
            device: d.clone(),
            semaphore,
            signal: unsafe {
                std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkSignalSemaphore>(
                    instance.proc(c"vkSignalSemaphore"),
                )
            },
            completion: None,
            opened: false,
        };
        // A completed-but-unobserved submission and an abandoned recording each
        // retain their own heap uses. Retiring the gated submission cannot clear them.
        let mut earlier_batch = Batch::new(d.clone()).unwrap();
        earlier_batch.bind_images(images.clone()).unwrap();
        earlier_batch.bind_samplers(samplers.clone()).unwrap();
        let mut earlier = unsafe { earlier_batch.submit().unwrap() };
        let mut recording = Batch::new(d.clone()).unwrap();
        recording.bind_images(images.clone()).unwrap();
        recording.bind_samplers(samplers.clone()).unwrap();
        GATE.set(semaphore);
        let mut batch = Batch::new(d.clone()).unwrap();
        batch.bind_images(images.clone()).unwrap();
        batch.bind_samplers(samplers.clone()).unwrap();
        gate.completion = Some(unsafe { batch.submit().unwrap() });
        assert!(
            batch.bind_images(images.clone()).is_err()
                && batch.bind_samplers(samplers.clone()).is_err()
        );
        assert!(!gate.completion.as_mut().unwrap().poll().unwrap());
        assert!(
            d.command_storage.borrow().is_empty(),
            "Pending/unobserved storage is unavailable"
        );
        assert!(Rc::get_mut(&mut images).is_none() && Rc::get_mut(&mut samplers).is_none());
        gate.open();
        gate.completion.as_mut().unwrap().wait().unwrap();
        assert!(Rc::get_mut(&mut images).is_none() && Rc::get_mut(&mut samplers).is_none());
        assert!(earlier.poll().unwrap());
        assert!(Rc::get_mut(&mut images).is_none() && Rc::get_mut(&mut samplers).is_none());
        drop(recording);
        Rc::get_mut(&mut images).unwrap().clear(0, 2).unwrap();
        Rc::get_mut(&mut samplers)
            .unwrap()
            .write(0, &[desc])
            .unwrap();
        // Both receipts survive the successful heap edits.
        assert!(earlier.poll().unwrap());
        assert!(gate.completion.as_mut().unwrap().poll().unwrap());
        assert_eq!(d.command_storage.borrow().len(), 2);
        // Reuse the reset storage with different real heaps, then release those heaps
        // while their consumed batch/receipt survive. Validation covers reserved ranges.
        for _ in 0..4 {
            let mut replacement_images = Rc::new(ImageHeap::new(d.clone(), 1).unwrap());
            Rc::get_mut(&mut replacement_images)
                .unwrap()
                .write(0, vec![(target.clone(), SAMPLED)])
                .unwrap();
            let mut replacement_samplers = Rc::new(SamplerHeap::new(d.clone(), 1).unwrap());
            Rc::get_mut(&mut replacement_samplers)
                .unwrap()
                .write(0, &[desc])
                .unwrap();
            let mut replacement = Batch::new(d.clone()).unwrap();
            replacement.bind_images(replacement_images.clone()).unwrap();
            replacement
                .bind_samplers(replacement_samplers.clone())
                .unwrap();
            let mut done = unsafe { replacement.submit().unwrap() };
            assert_eq!(d.command_storage.borrow().len(), 1);
            done.wait().unwrap();
            assert_eq!(d.command_storage.borrow().len(), 2);
            Rc::get_mut(&mut replacement_images)
                .unwrap()
                .clear(0, 1)
                .unwrap();
            Rc::get_mut(&mut replacement_samplers)
                .unwrap()
                .write(0, &[desc])
                .unwrap();
            let weak_images = Rc::downgrade(&replacement_images);
            let weak_samplers = Rc::downgrade(&replacement_samplers);
            drop(replacement_images);
            drop(replacement_samplers);
            assert!(weak_images.upgrade().is_none() && weak_samplers.upgrade().is_none());
            assert!(done.poll().unwrap());
        }
        gate.completion = None;
        // Force the noncoherent maintenance path on this coherent-capable device;
        // the injected error occurs after copying live bytes and is terminal per heap.
        let h = Rc::get_mut(&mut images).unwrap();
        h.heap.buffer.coherent = false;
        FAIL.set(3);
        assert!(h.write(0, vec![(target.clone(), SAMPLED)]).is_err());
        FAIL.set(0);
        assert!(h.heap.poisoned && h.clear(0, 1).is_err());
        assert!(Batch::new(d.clone())
            .unwrap()
            .bind_images(images.clone())
            .is_err());
        let s = Rc::get_mut(&mut samplers).unwrap();
        s.heap.buffer.coherent = false;
        FAIL.set(3);
        assert!(s.write(0, &[desc]).is_err());
        FAIL.set(0);
        assert!(s.heap.poisoned && s.write(0, &[desc]).is_err());
        assert!(Batch::new(d.clone())
            .unwrap()
            .bind_samplers(samplers.clone())
            .is_err());
        tested += 1;
    }
    assert!(tested > 0, "No image-capable device");
}
