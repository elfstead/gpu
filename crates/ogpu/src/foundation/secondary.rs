//! Native secondary recording/execution; caller owns inherited compatibility,
//! child lifetimes, parent invalidation, replay and all translation scratch.
use super::*;

pub(in crate::foundation) fn execute_scratch(count: u32) -> Result<HostRequirements, Status> {
    let layout = array_layout::<vk::VkCommandBuffer>(count)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
pub(in crate::foundation) fn inheritance_scratch(count: u32) -> Result<HostRequirements, Status> {
    let layout = array_layout::<vk::VkFormat>(count)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}

fn attachment_format(d: &Device, format: u32, aspect: u32) -> Result<vk::VkFormat, Status> {
    if format == 0 {
        return Ok(vk::VkFormat_VK_FORMAT_UNDEFINED);
    }
    let f = images::format(format)?;
    if f.aspects & aspect == 0 {
        return Err(INVALID);
    }
    let mut p = vk::VkFormatProperties::default();
    unsafe {
        (d.f.vkGetPhysicalDeviceFormatProperties.unwrap())(d.physical, f.native, &mut p);
    }
    let required = if aspect == 1 {
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BIT
    } else {
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_DEPTH_STENCIL_ATTACHMENT_BIT
    };
    if (p.linearTilingFeatures | p.optimalTilingFeatures) & required == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(f.native)
}

#[derive(Default)]
pub(super) struct PreparedInheritance {
    pub native: vk::VkCommandBufferInheritanceInfo,
    pub rendering: Option<vk::VkCommandBufferInheritanceRenderingInfo>,
    resource: Option<vk::VkBindHeapInfoEXT>,
    sampler: Option<vk::VkBindHeapInfoEXT>,
    heaps: vk::VkCommandBufferInheritanceDescriptorHeapInfoEXT,
}
impl PreparedInheritance {
    pub fn heap_mask(&self) -> u32 {
        u32::from(self.resource.is_some()) | (u32::from(self.sampler.is_some()) << 1)
    }
    // Link only after placing this aggregate at its final stack address. The
    // returned borrow prevents moving it while the native begin reads the chain.
    pub fn link(&mut self) -> &vk::VkCommandBufferInheritanceInfo {
        self.native.pNext = self
            .rendering
            .as_ref()
            .map_or(ptr::null(), |r| ptr::from_ref(r).cast());
        if self.heap_mask() != 0 {
            self.heaps = vk::VkCommandBufferInheritanceDescriptorHeapInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_INHERITANCE_DESCRIPTOR_HEAP_INFO_EXT,
                pNext: self.native.pNext,
                pResourceHeapBindInfo: self.resource.as_ref().map_or(ptr::null(), ptr::from_ref),
                pSamplerHeapBindInfo: self.sampler.as_ref().map_or(ptr::null(), ptr::from_ref),
            };
            self.native.pNext = ptr::from_ref(&self.heaps).cast();
        }
        &self.native
    }
}
unsafe fn heap(
    d: &Device,
    domain: u32,
    record: *const Record,
    kind: u32,
) -> Result<Option<vk::VkBindHeapInfoEXT>, Status> {
    if record.is_null() {
        return Ok(None);
    }
    let binding = unsafe { executable_record::<HeapBinding>(record, HEAP_BINDING)? };
    if binding.kind != kind {
        return Err(INVALID);
    }
    Ok(Some(unsafe { d.heap_binding(domain, binding)? }))
}
pub(super) unsafe fn inheritance(
    d: &Device,
    domain: u32,
    desc: &RecordingDesc,
) -> Result<PreparedInheritance, Status> {
    let mut prepared = PreparedInheritance::default();
    prepared.native.sType = vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_INHERITANCE_INFO;
    if desc.inheritance.is_null() {
        return Ok(prepared);
    }
    if desc.level != 1 {
        return Err(INVALID);
    }
    let input = unsafe { executable_record::<Inheritance>(desc.inheritance, INHERITANCE)? };
    if input.occlusion > 1 || input.query_flags & !1 != 0 {
        return Err(UNSUPPORTED);
    }
    let enabled = d.snapshot.features.enabled;
    if (input.occlusion != 0 || input.query_flags != 0) && enabled & INHERITED_QUERIES == 0
        || input.statistics != 0 && enabled & PIPELINE_STATISTICS == 0
        || input.statistics & !d.snapshot.query_limits.statistics != 0
    {
        return Err(UNSUPPORTED);
    }
    prepared.native.occlusionQueryEnable = input.occlusion;
    prepared.native.queryFlags = input.query_flags;
    prepared.native.pipelineStatistics = input.statistics as u32;
    prepared.resource = unsafe { heap(d, domain, input.resource_heap, 1)? };
    prepared.sampler = unsafe { heap(d, domain, input.sampler_heap, 2)? };
    if input.rendering.is_null() {
        return Ok(prepared);
    }
    if enabled & RASTER == 0 {
        return Err(UNSUPPORTED);
    }
    let r = unsafe { executable_record::<RenderInheritance>(input.rendering, RENDER_INHERITANCE)? };
    if r.view_mask != 0 || r.reserved != 0 || r.color_count > d.snapshot.graphics_limits.max_colors
    {
        return Err(UNSUPPORTED);
    }
    samples(r.samples, 127)?;
    if r.depth_format != 0 && r.stencil_format != 0 && r.depth_format != r.stencil_format {
        return Err(INVALID);
    }
    let colors = unsafe { slice(r.color_formats, r.color_count)? };
    let formats = scratch(
        r.scratch.data,
        r.scratch.size,
        inheritance_scratch(r.color_count)?,
    )?
    .cast::<vk::VkFormat>();
    for (i, color) in colors.iter().enumerate() {
        unsafe {
            formats.add(i).write(attachment_format(d, *color, 1)?);
        }
    }
    prepared.rendering = Some(vk::VkCommandBufferInheritanceRenderingInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_INHERITANCE_RENDERING_INFO,
        colorAttachmentCount: r.color_count,
        pColorAttachmentFormats: formats,
        depthAttachmentFormat: attachment_format(d, r.depth_format, 2)?,
        stencilAttachmentFormat: attachment_format(d, r.stencil_format, 4)?,
        rasterizationSamples: r.samples,
        ..Default::default()
    });
    Ok(prepared)
}

impl List {
    pub(in crate::foundation) unsafe fn execute(
        &self,
        count: u32,
        lists: *const *mut List,
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        if self.level != 0 {
            return Err(UNSUPPORTED);
        }
        if count == 0 {
            return Ok(());
        }
        if self.rendering.get() && !self.secondary_contents.get() {
            return Err(INVALID);
        }
        if self.active_queries.iter().any(|q| q.get().is_some())
            && d.snapshot.features.enabled & INHERITED_QUERIES == 0
        {
            return Err(UNSUPPORTED);
        }
        let lists = unsafe { slice(lists, count)? };
        let native = scratch(storage.data, storage.size, execute_scratch(count)?)?
            .cast::<vk::VkCommandBuffer>();
        for (i, pointer) in lists.iter().enumerate() {
            let child = unsafe { pointer.as_ref() }.ok_or(INVALID)?;
            if child.device != self.device
                || child.domain != self.domain
                || child.level != 1
                || child.state.get() != 2
                || child.continuation != self.rendering.get()
                || self.mode == 3 && child.mode != 3
            {
                return Err(INVALID);
            }
            if let Some(query) = self.active_queries[0].get() {
                if child.inherited_occlusion == 0 || query.flags & !child.inherited_query_flags != 0
                {
                    return Err(INVALID);
                }
            }
            if let Some(query) = self.active_queries[1].get() {
                if unsafe { &*query.pool }.statistics & !child.inherited_statistics != 0 {
                    return Err(INVALID);
                }
            }
            unsafe {
                native.add(i).write(child.command);
            }
        }
        unsafe {
            (d.f.vkCmdExecuteCommands.unwrap())(self.command, count, native);
        }
        // Native state is not inherited/restored. Do not pretend a stale bind is live.
        self.executable.set(ptr::null());
        self.graphics.set(ptr::null());
        self.viewport_set.set(false);
        self.index_count.set(None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    thread_local! {
        static EXECUTE: Cell<vk::PFN_vkCmdExecuteCommands> = const { Cell::new(None) };
        static CALLS: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn execute(
        command: vk::VkCommandBuffer,
        count: u32,
        children: *const vk::VkCommandBuffer,
    ) {
        CALLS.set(CALLS.get() + 1);
        unsafe {
            EXECUTE.get().unwrap()(command, count, children);
        }
    }
    #[test]
    fn inheritance_chain_links_only_final_storage() {
        let mut prepared = PreparedInheritance::default();
        assert_eq!(prepared.heap_mask(), 0);
        assert!(prepared.link().pNext.is_null());
        prepared.rendering = Some(vk::VkCommandBufferInheritanceRenderingInfo {
            colorAttachmentCount: 2,
            ..Default::default()
        });
        for mask in 0..4 {
            prepared.resource = (mask & 1 != 0).then(vk::VkBindHeapInfoEXT::default);
            prepared.sampler = (mask & 2 != 0).then(vk::VkBindHeapInfoEXT::default);
            // Moving before link must not preserve dangling self-pointers.
            let mut moved = prepared;
            assert_eq!(moved.heap_mask(), mask);
            let head = moved.link().pNext;
            let rendering = ptr::from_ref(moved.rendering.as_ref().unwrap()).cast();
            if mask == 0 {
                assert_eq!(head, rendering);
            } else {
                assert_eq!(head, ptr::from_ref(&moved.heaps).cast());
                assert_eq!(moved.heaps.pNext, rendering);
                assert_eq!(
                    moved.heaps.pResourceHeapBindInfo,
                    moved.resource.as_ref().map_or(ptr::null(), ptr::from_ref)
                );
                assert_eq!(
                    moved.heaps.pSamplerHeapBindInfo,
                    moved.sampler.as_ref().map_or(ptr::null(), ptr::from_ref)
                );
            }
            prepared = moved;
        }
        prepared.resource = None;
        prepared.sampler = None;
        prepared.rendering = None;
        assert!(prepared.link().pNext.is_null());
    }
    #[test]
    fn secondary_scratch_layouts() {
        assert_eq!(
            execute_scratch(0).unwrap(),
            HostRequirements {
                size: 0,
                alignment: 1
            }
        );
        assert_eq!(
            inheritance_scratch(0).unwrap(),
            HostRequirements {
                size: 0,
                alignment: 1
            }
        );
        assert_eq!(
            execute_scratch(3).unwrap().size,
            3 * size_of::<vk::VkCommandBuffer>() as u64
        );
        assert_eq!(
            inheritance_scratch(7).unwrap().size,
            7 * size_of::<vk::VkFormat>() as u64
        );
    }
    #[test]
    #[ignore = "requires modern Vulkan; secondary levels, atomic validation, query inheritance and simultaneous parent execution"]
    fn gpu_foundation_secondaries() {
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
            let features = snapshot.features.available & (INHERITED_QUERIES | PIPELINE_STATISTICS);
            let adapter = Adapter {
                state: AdapterState {
                    instance: instance.clone(),
                    physical,
                },
                snapshot,
            };
            let mut device = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                features,
            )
            .unwrap();
            EXECUTE.set(device.f.vkCmdExecuteCommands);
            device.f.vkCmdExecuteCommands = Some(execute);
            CALLS.set(0);
            let ad = ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 3,
                level: 1,
                reserved: 0,
            };
            let mut children = Arena::create(&device, &ad).unwrap();
            let mut parent = Arena::create(&device, &ArenaDesc { level: 0, ..ad }).unwrap();
            let mut rd = RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 3,
                level: 1,
                inheritance: ptr::null(),
            };
            assert_eq!(parent.begin(&rd), Err(INVALID));
            let a = children.begin(&rd).unwrap();
            unsafe { &*a }.end().unwrap();
            rd.replay_mode = 2;
            let serial = children.begin(&rd).unwrap();
            unsafe { &*serial }.end().unwrap();
            rd.replay_mode = 3;
            let inh = Inheritance {
                header: Record::new::<Inheritance>(INHERITANCE),
                occlusion: 0,
                query_flags: 0,
                statistics: if features & PIPELINE_STATISTICS != 0 {
                    1024
                } else {
                    0
                },
                rendering: ptr::null(),
                resource_heap: ptr::null(),
                sampler_heap: ptr::null(),
            };
            rd.inheritance = &inh.header;
            let b = children.begin(&rd).unwrap();
            unsafe { &*b }.end().unwrap();
            rd.level = 0;
            assert_eq!(parent.begin(&rd), Err(INVALID));
            rd.inheritance = ptr::null();
            let p = parent.begin(&rd).unwrap();
            let primary = unsafe { &*p };
            let mut data = [0u64; 2];
            let storage = HostSpan {
                data: data.as_mut_ptr().cast(),
                size: size_of_val(&data) as u64,
            };
            assert_eq!(
                unsafe {
                    primary.execute(
                        0,
                        ptr::null(),
                        HostSpan {
                            data: ptr::null_mut(),
                            size: 0,
                        },
                    )
                },
                Ok(())
            );
            assert_eq!(CALLS.get(), 0);
            assert_eq!(
                unsafe { primary.execute(1, &serial, storage) },
                Err(INVALID)
            );
            assert_eq!(unsafe { primary.execute(1, &p, storage) }, Err(INVALID));
            assert_eq!(
                unsafe { primary.execute(2, [a, ptr::null_mut()].as_ptr(), storage) },
                Err(INVALID)
            );
            assert_eq!(
                unsafe { primary.execute(2, [a, b].as_ptr(), HostSpan { size: 0, ..storage }) },
                Err(CAPACITY)
            );
            assert_eq!(CALLS.get(), 0); // A valid prefix never leaks native execution.
            unsafe { primary.execute(2, [a, b].as_ptr(), storage) }.unwrap();
            assert_eq!(CALLS.get(), 1);
            primary.end().unwrap();
            let timeline = Timeline::create(&device, 0).unwrap();
            let point = SyncPoint {
                point: Point {
                    timeline: ptr::from_ref(&*timeline).cast_mut(),
                    value: 1,
                },
                stages: 1,
            };
            let sr = submit_scratch(1, 0, 1).unwrap();
            let mut submit_data = vec![0u64; sr.size.div_ceil(8) as usize];
            let mut submit = SubmitDesc {
                header: Record::new::<SubmitDesc>(SUBMIT_DESC),
                list_count: 1,
                wait_count: 0,
                signal_count: 1,
                lists: &a,
                waits: ptr::null(),
                signals: &point,
                scratch: submit_data.as_mut_ptr().cast(),
                scratch_size: sr.size,
            };
            let queue = device.queue(domain, 0).unwrap();
            assert_eq!(unsafe { queue.submit(&submit) }, Err(INVALID)); // No direct secondary submission.
            submit.lists = &p;
            unsafe { queue.submit(&submit) }.unwrap();
            timeline.wait(1, 10_000_000_000).unwrap();
            parent.reset(false).unwrap(); // Discard all parents before mutating children.
            let query_pool = if features & PIPELINE_STATISTICS != 0 {
                Some(
                    QueryPool::create(
                        &device,
                        &QueryPoolDesc {
                            header: Record::new::<QueryPoolDesc>(QUERY_POOL_DESC),
                            kind: 3,
                            count: 1,
                            statistics: 1024,
                        },
                    )
                    .unwrap(),
                )
            } else {
                None
            };
            if let Some(pool) = &query_pool {
                let p = parent.begin(&rd).unwrap();
                let primary = unsafe { &*p };
                primary.queries_reset(pool, 0, 1).unwrap();
                primary.query(pool, 0, true, 0).unwrap();
                if features & INHERITED_QUERIES != 0 {
                    assert_eq!(unsafe { primary.execute(1, &a, storage) }, Err(INVALID));
                    unsafe { primary.execute(1, &b, storage) }.unwrap();
                } else {
                    assert_eq!(unsafe { primary.execute(1, &b, storage) }, Err(UNSUPPORTED));
                }
                primary.query(pool, 0, false, 0).unwrap();
                primary.end().unwrap();
                println!(
                    "Secondary inherited queries enabled: {}",
                    features & INHERITED_QUERIES != 0
                );
                parent.reset(false).unwrap();
            }
            children.reset(false).unwrap();
            rd.level = 1;
            let child = children.begin(&rd).unwrap();
            assert_eq!(
                unsafe { (&*child).execute(0, ptr::null(), storage) },
                Err(UNSUPPORTED)
            );
            unsafe { &*child }.cancel();
            drop(parent);
            drop(children);
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }
}
