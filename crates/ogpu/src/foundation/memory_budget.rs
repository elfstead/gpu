//! Explicit live physical budget estimates; no allocation or pressure policy.
use super::*;

fn values(
    heaps: usize,
    memory: &vk::VkPhysicalDeviceMemoryProperties,
    budget: &vk::VkPhysicalDeviceMemoryBudgetPropertiesEXT,
) -> Result<[MemoryBudget; 16], Status> {
    if heaps > 16 || memory.memoryHeapCount as usize != heaps {
        return Err(BACKEND_ERROR);
    }
    let mut result = [MemoryBudget::default(); 16];
    for (i, out) in result[..heaps].iter_mut().enumerate() {
        if budget.heapBudget[i] == 0 || budget.heapBudget[i] > memory.memoryHeaps[i].size {
            return Err(BACKEND_ERROR);
        }
        *out = MemoryBudget {
            budget: budget.heapBudget[i],
            usage: budget.heapUsage[i], // Deliberately not clamped to budget.
            heap: i as u32,
            reserved: 0,
        };
    }
    Ok(result)
}

unsafe fn query(
    supported: bool,
    heaps: usize,
    output: *mut Query,
    sample: impl FnOnce() -> Result<[MemoryBudget; 16], Status>,
) -> Result<(), Status> {
    if output.is_null() {
        return Err(INVALID);
    }
    unsafe { output.cast::<Record>().read() }.validate::<Query>(MEMORY_BUDGETS)?;
    let output = unsafe { &mut *output };
    if output.reserved != 0
        || output.element_size != size_of::<MemoryBudget>() as u32
        || (output.capacity != 0 && output.data.is_null())
    {
        return Err(INVALID);
    }
    if !supported {
        return Err(UNSUPPORTED);
    }
    // Vulkan's fixed native bound is private, not a public API heap-count limit.
    if heaps > 16 {
        return Err(BACKEND_ERROR);
    }
    if output.capacity == 0 || (output.capacity as usize) < heaps {
        output.count = heaps as u32;
        return if output.capacity == 0 {
            Ok(())
        } else {
            Err(CAPACITY)
        };
    }
    let values = sample()?;
    unsafe { super::super::copy_out(output, &values[..heaps]) }
}

impl Device {
    pub(in crate::foundation) unsafe fn memory_budget_query(
        &self,
        output: *mut Query,
    ) -> Result<(), Status> {
        self.ready()?;
        let heaps = self.snapshot.memory_heaps.len();
        unsafe {
            query(self.snapshot.memory_budget_supported, heaps, output, || {
                let mut budget = vk::VkPhysicalDeviceMemoryBudgetPropertiesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_BUDGET_PROPERTIES_EXT,
                ..Default::default()
            };
                let mut memory = vk::VkPhysicalDeviceMemoryProperties2 {
                    sType:
                        vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_PROPERTIES_2,
                    pNext: ptr::from_mut(&mut budget).cast(),
                    ..Default::default()
                };
                (self.f.vkGetPhysicalDeviceMemoryProperties2.unwrap())(self.physical, &mut memory);
                values(heaps, &memory.memoryProperties, &budget)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_budget_values_preserve_pressure_and_validate_native_counts() {
        let mut memory = vk::VkPhysicalDeviceMemoryProperties {
            memoryHeapCount: 2,
            ..Default::default()
        };
        memory.memoryHeaps[0].size = 1000;
        memory.memoryHeaps[1].size = 2000;
        let mut budget = vk::VkPhysicalDeviceMemoryBudgetPropertiesEXT::default();
        budget.heapBudget[0] = 700;
        budget.heapBudget[1] = 1000;
        budget.heapUsage[0] = 900;
        budget.heapUsage[1] = 0;
        let got = values(2, &memory, &budget).unwrap();
        assert_eq!(
            got[0],
            MemoryBudget {
                budget: 700,
                usage: 900,
                heap: 0,
                reserved: 0
            }
        );
        assert_eq!(
            got[1],
            MemoryBudget {
                budget: 1000,
                usage: 0,
                heap: 1,
                reserved: 0
            }
        );
        assert_eq!(values(17, &memory, &budget), Err(BACKEND_ERROR));
        assert_eq!(values(1, &memory, &budget), Err(BACKEND_ERROR));
        budget.heapBudget[1] = 2001;
        assert_eq!(values(2, &memory, &budget), Err(BACKEND_ERROR));
        budget.heapBudget[1] = 0;
        assert_eq!(values(2, &memory, &budget), Err(BACKEND_ERROR));
    }
    #[test]
    fn budget_query_samples_only_full_calls_and_preserves_failed_outputs() {
        assert_eq!(size_of::<MemoryBudget>(), 24);
        assert_eq!(std::mem::offset_of!(MemoryBudget, heap), 16);
        let sentinel = MemoryBudget {
            budget: 42,
            usage: 43,
            heap: 44,
            reserved: 45,
        };
        let mut data = [sentinel; 3];
        let mut q = Query {
            header: Record::new::<Query>(MEMORY_BUDGETS),
            data: data.as_mut_ptr().cast(),
            capacity: 2,
            count: 91,
            element_size: 24,
            reserved: 0,
        };
        unsafe {
            assert_eq!(
                query(false, 2, &mut q, || panic!("unsupported must not sample")),
                Err(UNSUPPORTED)
            );
            assert_eq!(q.count, 91);
            assert_eq!(data, [sentinel; 3]);
            q.element_size = 23;
            assert_eq!(query(true, 2, &mut q, || panic!()), Err(INVALID));
            assert_eq!(q.count, 91);
            q.element_size = 24;
            q.capacity = 1;
            assert_eq!(
                query(true, 2, &mut q, || panic!("short capacity must not sample")),
                Err(CAPACITY)
            );
            assert_eq!(q.count, 2);
            assert_eq!(data, [sentinel; 3]);
            q.capacity = 0;
            q.data = ptr::null_mut();
            assert_eq!(
                query(true, 2, &mut q, || panic!("count only must not sample")),
                Ok(())
            );
            assert_eq!(q.count, 2);
            q.capacity = 2;
            q.count = 91;
            assert_eq!(query(true, 2, &mut q, || panic!()), Err(INVALID));
            assert_eq!(q.count, 91);
            q.data = data.as_mut_ptr().cast();
            assert_eq!(
                query(true, 2, &mut q, || Err(BACKEND_ERROR)),
                Err(BACKEND_ERROR)
            );
            assert_eq!(q.count, 91);
            assert_eq!(data, [sentinel; 3]);
            for usage in [7, 11] {
                let mut sampled = [MemoryBudget::default(); 16];
                sampled[0].usage = usage;
                sampled[1].heap = 1;
                assert_eq!(query(true, 2, &mut q, || Ok(sampled)), Ok(()));
                assert_eq!(&data[..2], &sampled[..2]);
                assert_eq!(data[2], sentinel);
            }
            q.header.kind = MEMORY_HEAPS;
            assert_eq!(query(true, 2, &mut q, || panic!()), Err(UNSUPPORTED));
            q.header = Record::new::<Record>(MEMORY_BUDGETS);
            assert_eq!(query(true, 2, &mut q, || panic!()), Err(INVALID));
        }
    }
}
