//! Cold argument mapping and update preflight for the descriptor-heap backend.
//! No captured values, referenced resources, interning cache or hot allocation.
use super::*;
use std::ffi::c_void;

const SHADER_STAGES: [u64; 5] = [4, 8, 16, TASK_STAGE, MESH_STAGE];

#[derive(Clone, Copy, PartialEq, Eq)]
struct AddressField {
    offset: u32,
    alignment: u32,
}
#[derive(Clone, Copy, Default)]
struct Boundary {
    addresses_before: u32,
    inside_address: bool,
}

pub struct ArgumentLayout {
    pub(super) device: *const Device,
    pub(super) execution_class: u32,
    byte_size: u32,
    mappings: [Vec<vk::VkDescriptorSetAndBindingMappingEXT>; 5],
    addresses: Vec<AddressField>,
    boundaries: Vec<Boundary>,
    sequence_fields: Vec<u32>,
}

impl ArgumentLayout {
    /// # Safety
    /// Live device and readable field array. The device outlives this layout and
    /// every executable/recorded use borrowing it. No shader reflection is implied.
    pub(in crate::foundation) unsafe fn create(
        device: &Device,
        desc: &ArgumentLayoutDesc,
    ) -> Result<Box<Self>, Status> {
        device.ready()?;
        if desc.field_count != 0
            && (desc.fields.is_null()
                || desc.field_count as u64 > isize::MAX as u64 / size_of::<ArgumentField>() as u64)
        {
            return Err(INVALID);
        }
        let fields = if desc.field_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(desc.fields, desc.field_count as usize) }
        };
        Ok(Box::new(Self::prepare(
            device,
            desc,
            fields,
            device.snapshot.execution_limits.max_inline_bytes,
            device.snapshot.features.enabled,
            device
                .snapshot
                .descriptor_properties
                .uniform_address_alignment,
        )?))
    }

    fn prepare(
        device: *const Device,
        desc: &ArgumentLayoutDesc,
        fields: &[ArgumentField],
        max_push: u64,
        enabled: FeatureBits,
        uniform_alignment: u64,
    ) -> Result<Self, Status> {
        if desc.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let permitted = match desc.execution_class {
            ARGUMENT_COMPUTE => 4,
            ARGUMENT_GRAPHICS if enabled & RASTER != 0 => {
                8 | 16
                    | if enabled & TASK_SHADER != 0 {
                        TASK_STAGE
                    } else {
                        0
                    }
                    | if enabled & MESH_SHADER != 0 {
                        MESH_STAGE
                    } else {
                        0
                    }
            }
            _ => return Err(UNSUPPORTED),
        };
        if desc.byte_size % 4 != 0
            || fields.len() != desc.field_count as usize
            || fields.is_empty() != (desc.byte_size == 0)
        {
            return Err(INVALID);
        }
        if u64::from(desc.byte_size) > max_push {
            return Err(UNSUPPORTED);
        }
        let mut previous_end = 0;
        for field in fields {
            let end = field.offset.checked_add(field.size).ok_or(INVALID)?;
            if field.size == 0
                || field.offset % 4 != 0
                || field.size % 4 != 0
                || field.offset < previous_end
                || end > desc.byte_size
                || (field.space == ARGUMENT_DIRECT) != (field.binding == ARGUMENT_DIRECT)
            {
                return Err(INVALID);
            }
            if field.stages == 0 || field.stages & !permitted != 0 {
                return Err(UNSUPPORTED);
            }
            match field.kind {
                ARGUMENT_INLINE => {
                    if field.address_alignment != 0 {
                        return Err(INVALID);
                    }
                }
                ARGUMENT_UNIFORM_ADDRESS | ARGUMENT_READ_ADDRESS | ARGUMENT_READ_WRITE_ADDRESS => {
                    if field.offset % 8 != 0
                        || field.size != 8
                        || !field.address_alignment.is_power_of_two()
                        || field.space == ARGUMENT_DIRECT
                    {
                        return Err(INVALID);
                    }
                    // PUSH_ADDRESS uniform resources retain the native uniform
                    // alignment obligation. Storage-address shader accesses retain
                    // their artifact alignment, not descriptor-offset alignment.
                    if field.kind == ARGUMENT_UNIFORM_ADDRESS
                        && u64::from(field.address_alignment) < uniform_alignment
                    {
                        return Err(UNSUPPORTED);
                    }
                }
                _ => return Err(UNSUPPORTED),
            }
            previous_end = end;
        }

        let mut result = Self {
            device,
            execution_class: desc.execution_class,
            byte_size: desc.byte_size,
            mappings: std::array::from_fn(|_| Vec::new()),
            addresses: Vec::new(),
            boundaries: Vec::new(),
            sequence_fields: Vec::new(),
        };
        result
            .addresses
            .try_reserve_exact(fields.iter().filter(|f| f.kind != ARGUMENT_INLINE).count())
            .map_err(|_| OUT_OF_MEMORY)?;
        for (index, stage) in SHADER_STAGES.iter().enumerate() {
            let mappings = &mut result.mappings[index];
            let count = fields
                .iter()
                .filter(|f| f.stages & stage != 0 && f.space != ARGUMENT_DIRECT)
                .count();
            mappings
                .try_reserve_exact(count)
                .map_err(|_| OUT_OF_MEMORY)?;
            for field in fields
                .iter()
                .filter(|f| f.stages & stage != 0 && f.space != ARGUMENT_DIRECT)
            {
                let resource = match field.kind {
                    ARGUMENT_INLINE | ARGUMENT_UNIFORM_ADDRESS => vk::VkSpirvResourceTypeFlagBitsEXT_VK_SPIRV_RESOURCE_TYPE_UNIFORM_BUFFER_BIT_EXT,
                    ARGUMENT_READ_ADDRESS => vk::VkSpirvResourceTypeFlagBitsEXT_VK_SPIRV_RESOURCE_TYPE_READ_ONLY_STORAGE_BUFFER_BIT_EXT,
                    ARGUMENT_READ_WRITE_ADDRESS => vk::VkSpirvResourceTypeFlagBitsEXT_VK_SPIRV_RESOURCE_TYPE_READ_WRITE_STORAGE_BUFFER_BIT_EXT,
                    _ => unreachable!(),
                };
                // Cold ambiguity check; disjoint stage/type mappings may share
                // source binding coordinates without requiring a global registry.
                if mappings.iter().any(|m| {
                    m.descriptorSet == field.space
                        && m.firstBinding == field.binding
                        && m.resourceMask & resource != 0
                }) {
                    return Err(INVALID);
                }
                mappings.push(vk::VkDescriptorSetAndBindingMappingEXT {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DESCRIPTOR_SET_AND_BINDING_MAPPING_EXT,
                    descriptorSet: field.space,
                    firstBinding: field.binding,
                    bindingCount: 1,
                    resourceMask: resource,
                    source: if field.kind == ARGUMENT_INLINE {
                        vk::VkDescriptorMappingSourceEXT_VK_DESCRIPTOR_MAPPING_SOURCE_PUSH_DATA_EXT
                    } else {
                        vk::VkDescriptorMappingSourceEXT_VK_DESCRIPTOR_MAPPING_SOURCE_PUSH_ADDRESS_EXT
                    },
                    sourceData: if field.kind == ARGUMENT_INLINE {
                        vk::VkDescriptorMappingSourceDataEXT { pushDataOffset: field.offset }
                    } else {
                        vk::VkDescriptorMappingSourceDataEXT { pushAddressOffset: field.offset }
                    },
                    ..Default::default()
                });
            }
        }
        for field in fields.iter().filter(|f| f.kind != ARGUMENT_INLINE) {
            result.addresses.push(AddressField {
                offset: field.offset,
                alignment: field.address_alignment,
            });
        }
        result
            .sequence_fields
            .try_reserve_exact(
                fields
                    .iter()
                    .filter(|f| f.kind == ARGUMENT_INLINE && f.size == 4)
                    .count(),
            )
            .map_err(|_| OUT_OF_MEMORY)?;
        for field in fields
            .iter()
            .filter(|f| f.kind == ARGUMENT_INLINE && f.size == 4)
        {
            result.sequence_fields.push(field.offset);
        }
        // No table for the common all-inline path. Otherwise two lookups locate
        // the affected address fields; recording never scans unrelated fields.
        if !result.addresses.is_empty() {
            let count = (desc.byte_size / 4) as usize + 1;
            result
                .boundaries
                .try_reserve_exact(count)
                .map_err(|_| OUT_OF_MEMORY)?;
            let mut next = 0;
            for word in 0..count {
                let offset = (word * 4) as u32;
                while next < result.addresses.len() && result.addresses[next].offset + 8 <= offset {
                    next += 1;
                }
                result.boundaries.push(Boundary {
                    addresses_before: next as u32,
                    inside_address: next < result.addresses.len()
                        && result.addresses[next].offset < offset,
                });
            }
        }
        Ok(result)
    }

    // Cold compatibility check. Binding-preservation keys describe native state
    // storage, not artifact mappings. No global layout registry or hot scan.
    pub(super) fn generated_compatible(&self, other: &Self) -> bool {
        if ptr::eq(self, other) {
            return true;
        }
        self.device == other.device
            && self.execution_class == other.execution_class
            && self.byte_size == other.byte_size
            && self.addresses == other.addresses
            && self.mappings.iter().zip(&other.mappings).all(|(a, b)| {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(a, b)| {
                        a.descriptorSet == b.descriptorSet && a.firstBinding == b.firstBinding
                        && a.bindingCount == b.bindingCount && a.resourceMask == b.resourceMask
                        && a.source == b.source
                        // Both sources used by this layout store one u32 offset.
                        && unsafe { a.sourceData.pushDataOffset == b.sourceData.pushDataOffset }
                    })
            })
    }

    fn update_range(&self, offset: u32, size: u32) -> Result<std::ops::Range<usize>, Status> {
        if offset % 4 != 0
            || size % 4 != 0
            || offset > self.byte_size
            || size > self.byte_size - offset
        {
            return Err(INVALID);
        }
        if size == 0 || self.addresses.is_empty() {
            return Ok(0..0);
        }
        let first = self.boundaries[(offset / 4) as usize];
        let last = self.boundaries[((offset + size) / 4) as usize];
        if first.inside_address || last.inside_address {
            return Err(INVALID);
        }
        Ok(first.addresses_before as usize..last.addresses_before as usize)
    }

    pub(super) fn generated_range(
        &self,
        offset: u32,
        size: u32,
        sequence: bool,
    ) -> Result<(), Status> {
        self.update_range(offset, size)?;
        if size == 0
            || sequence && (size != 4 || self.sequence_fields.binary_search(&offset).is_err())
        {
            return Err(INVALID);
        }
        Ok(())
    }

    pub(in crate::foundation) fn byte_size(&self) -> u32 {
        self.byte_size
    }

    pub(in crate::foundation) fn state_info(&self) -> ArgumentStateInfo {
        ArgumentStateInfo {
            // All descriptor-heap push data uses one native storage domain.
            // Tokens compare only within this device, never across devices.
            domain: 1,
            binding_key: 1,
            offset_base: 0,
            byte_size: self.byte_size,
            bind_rule: ARGUMENT_BIND_PRESERVES,
            reserved: 0,
        }
    }

    pub(in crate::foundation) fn mapping_info(
        &self,
        stage: u32,
        next: *const c_void,
    ) -> Result<vk::VkShaderDescriptorSetAndBindingMappingInfoEXT, Status> {
        let index = SHADER_STAGES
            .iter()
            .position(|&s| s == u64::from(stage))
            .ok_or(UNSUPPORTED)?;
        let mappings = &self.mappings[index];
        Ok(vk::VkShaderDescriptorSetAndBindingMappingInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_DESCRIPTOR_SET_AND_BINDING_MAPPING_INFO_EXT,
            pNext: next,
            mappingCount: mappings.len() as u32,
            pMappings: if mappings.is_empty() { ptr::null() } else { mappings.as_ptr() },
        })
    }

    /// # Safety
    /// Nonempty data covers size readable bytes, possibly unaligned. Only native
    /// address fields are inspected; pointees and inline pointer bits are trusted.
    pub(in crate::foundation) unsafe fn validate_update(
        &self,
        offset: u32,
        size: u32,
        data: *const c_void,
    ) -> Result<(), Status> {
        let range = self.update_range(offset, size)?;
        if size != 0 && data.is_null() {
            return Err(INVALID);
        }
        for address in &self.addresses[range] {
            let value = unsafe {
                data.cast::<u8>()
                    .add((address.offset - offset) as usize)
                    .cast::<u64>()
                    .read_unaligned()
            };
            if value % u64::from(address.alignment) != 0 {
                return Err(INVALID);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn field(offset: u32, kind: u32, binding: u32) -> ArgumentField {
        ArgumentField {
            offset,
            size: 8,
            kind,
            address_alignment: if kind == ARGUMENT_INLINE { 0 } else { 16 },
            stages: 4,
            space: 0,
            binding,
        }
    }
    fn desc(fields: &[ArgumentField], bytes: u32) -> ArgumentLayoutDesc {
        ArgumentLayoutDesc {
            execution_class: ARGUMENT_COMPUTE,
            byte_size: bytes,
            field_count: fields.len() as u32,
            flags: 0,
            fields: fields.as_ptr(),
        }
    }
    fn prepare(fields: &[ArgumentField], bytes: u32) -> Result<ArgumentLayout, Status> {
        ArgumentLayout::prepare(ptr::null(), &desc(fields, bytes), fields, 256, 0, 16)
    }
    fn status(result: Result<ArgumentLayout, Status>) -> Status {
        result.err().unwrap_or(OK)
    }

    #[test]
    fn layout_rejects_ambiguous_or_unrepresentable_fields() {
        let fields = [
            field(0, ARGUMENT_INLINE, 0),
            field(8, ARGUMENT_READ_ADDRESS, 1),
        ];
        assert_eq!(status(prepare(&fields, 16)), OK);
        for (index, replacement, expected) in [
            (
                0,
                ArgumentField {
                    size: 0,
                    ..fields[0]
                },
                INVALID,
            ),
            (
                0,
                ArgumentField {
                    size: 12,
                    ..fields[0]
                },
                INVALID,
            ),
            (
                0,
                ArgumentField {
                    address_alignment: 4,
                    ..fields[0]
                },
                INVALID,
            ),
            (
                0,
                ArgumentField {
                    stages: 0,
                    ..fields[0]
                },
                UNSUPPORTED,
            ),
            (
                0,
                ArgumentField {
                    stages: 8,
                    ..fields[0]
                },
                UNSUPPORTED,
            ),
            (
                0,
                ArgumentField {
                    space: ARGUMENT_DIRECT,
                    ..fields[0]
                },
                INVALID,
            ),
            (
                1,
                ArgumentField {
                    offset: 4,
                    ..fields[1]
                },
                INVALID,
            ),
            (
                1,
                ArgumentField {
                    offset: u32::MAX - 7,
                    ..fields[1]
                },
                INVALID,
            ),
            (
                1,
                ArgumentField {
                    size: 4,
                    ..fields[1]
                },
                INVALID,
            ),
            (
                1,
                ArgumentField {
                    address_alignment: 3,
                    ..fields[1]
                },
                INVALID,
            ),
            (
                1,
                ArgumentField {
                    kind: 9,
                    ..fields[1]
                },
                UNSUPPORTED,
            ),
            (
                1,
                ArgumentField {
                    space: ARGUMENT_DIRECT,
                    binding: ARGUMENT_DIRECT,
                    ..fields[1]
                },
                INVALID,
            ),
            (
                1,
                ArgumentField {
                    kind: ARGUMENT_UNIFORM_ADDRESS,
                    address_alignment: 8,
                    ..fields[1]
                },
                UNSUPPORTED,
            ),
            // INLINE and UNIFORM_ADDRESS refer to the same native resource type.
            (
                1,
                ArgumentField {
                    kind: ARGUMENT_UNIFORM_ADDRESS,
                    binding: 0,
                    ..fields[1]
                },
                INVALID,
            ),
        ] {
            let mut candidate = fields;
            candidate[index] = replacement;
            assert_eq!(status(prepare(&candidate, 16)), expected, "{candidate:?}");
        }
        assert_eq!(status(prepare(&fields, 12)), INVALID);
        assert_eq!(status(prepare(&fields, 260)), UNSUPPORTED);
        assert_eq!(status(prepare(&[], 0)), OK);
        assert_eq!(status(prepare(&[], 4)), INVALID);
        // Native storage-address alignment follows the artifact, not a blanket
        // uniform or storage descriptor-offset minimum.
        assert_eq!(
            status(prepare(
                &[ArgumentField {
                    address_alignment: 1,
                    ..fields[1]
                }],
                16
            )),
            OK
        );
    }

    #[test]
    fn native_mapping_preserves_resource_role_stage_and_byte_offset() {
        let fields = [
            field(0, ARGUMENT_INLINE, 2),
            field(8, ARGUMENT_UNIFORM_ADDRESS, 3),
            field(16, ARGUMENT_READ_ADDRESS, 3),
            field(24, ARGUMENT_READ_WRITE_ADDRESS, 3),
        ];
        let layout = prepare(&fields, 32).unwrap();
        let next = ptr::dangling::<u32>().cast();
        let info = layout.mapping_info(4, next).unwrap();
        assert_eq!(info.pNext, next);
        assert_eq!(info.mappingCount, 4);
        let mappings = unsafe { std::slice::from_raw_parts(info.pMappings, 4) };
        for (i, m) in mappings.iter().enumerate() {
            assert_eq!(
                (m.descriptorSet, m.firstBinding, m.bindingCount),
                (0, fields[i].binding, 1)
            );
            assert_eq!(m.resourceMask, [0x20, 0x20, 0x40, 0x80][i]);
            assert_eq!(m.source, if i == 0 { 5 } else { 6 });
            assert_eq!(unsafe { m.sourceData.pushDataOffset }, fields[i].offset);
        }
        let mut fields = [field(0, ARGUMENT_INLINE, 0), field(8, ARGUMENT_INLINE, 0)];
        fields[0].stages = 8;
        fields[1].stages = 16;
        let mut description = desc(&fields, 16);
        description.execution_class = ARGUMENT_GRAPHICS;
        let graphics =
            ArgumentLayout::prepare(ptr::null(), &description, &fields, 256, RASTER, 16).unwrap();
        assert_eq!(
            graphics.mapping_info(8, ptr::null()).unwrap().mappingCount,
            1
        );
        assert_eq!(
            graphics.mapping_info(16, ptr::null()).unwrap().mappingCount,
            1
        );
        assert_eq!(
            graphics.mapping_info(4, ptr::null()).unwrap().mappingCount,
            0
        );
        assert_eq!(graphics.state_info().domain, layout.state_info().domain);
        assert_eq!(
            graphics.state_info().binding_key,
            layout.state_info().binding_key
        );
        assert_eq!(graphics.state_info().bind_rule, ARGUMENT_BIND_PRESERVES);
    }

    #[test]
    fn generated_identity_requires_a_declared_inline_word() {
        let fields = [
            field(0, ARGUMENT_READ_WRITE_ADDRESS, 0),
            ArgumentField {
                size: 4,
                ..field(8, ARGUMENT_INLINE, 1)
            },
            field(16, ARGUMENT_INLINE, 2),
        ];
        let layout = prepare(&fields, 32).unwrap();
        assert_eq!(layout.generated_range(8, 4, true), Ok(()));
        for (offset, size) in [
            (0, 4),
            (4, 4),
            (8, 8),
            (12, 4),
            (16, 4),
            (20, 4),
            (28, 4),
            (32, 4),
            (u32::MAX - 3, 8),
        ] {
            assert_eq!(layout.generated_range(offset, size, true), Err(INVALID));
        }
    }

    #[test]
    fn generated_compatibility_is_mapping_equality_not_binding_preservation() {
        let fields = [
            field(0, ARGUMENT_READ_WRITE_ADDRESS, 0),
            field(8, ARGUMENT_INLINE, 1),
        ];
        let layout = prepare(&fields, 16).unwrap();
        assert!(layout.generated_compatible(&prepare(&fields, 16).unwrap()));
        for (index, replacement) in [
            (
                0,
                ArgumentField {
                    kind: ARGUMENT_READ_ADDRESS,
                    ..fields[0]
                },
            ),
            (
                0,
                ArgumentField {
                    address_alignment: 32,
                    ..fields[0]
                },
            ),
            (
                0,
                ArgumentField {
                    binding: 2,
                    ..fields[0]
                },
            ),
            (
                1,
                ArgumentField {
                    space: ARGUMENT_DIRECT,
                    binding: ARGUMENT_DIRECT,
                    ..fields[1]
                },
            ),
        ] {
            let mut changed = fields;
            changed[index] = replacement;
            let other = prepare(&changed, 16).unwrap();
            assert_eq!(
                layout.state_info().binding_key,
                other.state_info().binding_key
            );
            assert!(!layout.generated_compatible(&other));
        }
        assert!(!layout.generated_compatible(&prepare(&fields, 20).unwrap()));
        let mut other = prepare(&fields, 16).unwrap();
        other.device = ptr::dangling();
        assert!(!layout.generated_compatible(&other));
        other.device = layout.device;
        other.execution_class = ARGUMENT_GRAPHICS;
        assert!(!layout.generated_compatible(&other));
        // Direct inline declarations may be partitioned differently: the caller
        // owns their artifact semantics; they introduce no native resource map.
        let direct = ArgumentField {
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
            ..field(0, ARGUMENT_INLINE, 0)
        };
        assert!(prepare(&[direct], 8).unwrap().generated_compatible(
            &prepare(
                &[
                    ArgumentField { size: 4, ..direct },
                    ArgumentField {
                        offset: 4,
                        size: 4,
                        ..direct
                    }
                ],
                8
            )
            .unwrap()
        ));
    }

    #[test]
    fn partial_updates_match_address_atomicity_for_every_range() {
        let fields = [
            field(0, ARGUMENT_INLINE, 0),
            field(8, ARGUMENT_READ_ADDRESS, 1),
            field(24, ARGUMENT_READ_WRITE_ADDRESS, 2),
        ];
        let layout = prepare(&fields, 40).unwrap();
        let data = [0u8; 41];
        for offset in 0..=44u32 {
            for size in 0..=44u32 {
                let expected = offset % 4 == 0
                    && size % 4 == 0
                    && offset + size <= 40
                    && (size == 0
                        || [8, 24].into_iter().all(|a| {
                            let overlaps = offset < a + 8 && offset + size > a;
                            !overlaps || (offset <= a && offset + size >= a + 8)
                        }));
                // Deliberately unaligned host data; device-address alignment is
                // a constraint on its value, not on this host slice.
                let result =
                    unsafe { layout.validate_update(offset, size, data.as_ptr().add(1).cast()) };
                assert_eq!(result.is_ok(), expected, "offset={offset} size={size}");
                assert_eq!(
                    layout.generated_range(offset, size, false).is_ok(),
                    expected && size != 0,
                    "generated offset={offset} size={size}"
                );
            }
        }
        assert_eq!(
            unsafe { layout.validate_update(0, 4, ptr::null()) },
            Err(INVALID)
        );
        assert_eq!(
            unsafe { layout.validate_update(40, 0, ptr::null()) },
            Ok(())
        );
        assert_eq!(
            unsafe { layout.validate_update(u32::MAX - 3, 8, data.as_ptr().cast()) },
            Err(INVALID)
        );
        let mut bytes = [0u8; 41];
        bytes[25..33].copy_from_slice(&3u64.to_ne_bytes());
        assert_eq!(
            unsafe { layout.validate_update(0, 40, bytes.as_ptr().add(1).cast()) },
            Err(INVALID)
        );
        assert_eq!(
            unsafe { layout.validate_update(0, 16, bytes.as_ptr().add(1).cast()) },
            Ok(())
        );
        let direct = [ArgumentField {
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
            ..field(0, ARGUMENT_INLINE, 0)
        }];
        let inline = prepare(&direct, 8).unwrap();
        assert_eq!(inline.addresses.capacity(), 0);
        assert_eq!(inline.boundaries.capacity(), 0);
        assert_eq!(
            unsafe { inline.validate_update(0, 8, [3u64].as_ptr().cast()) },
            Ok(())
        );
    }

    thread_local! {
        static PUSHES: Cell<u32> = const { Cell::new(0) };
        static PUSH_RANGE: Cell<(u32, usize)> = const { Cell::new((0, 0)) };
    }
    unsafe extern "C" fn push(_: vk::VkCommandBuffer, info: *const vk::VkPushDataInfoEXT) {
        let info = unsafe { &*info };
        PUSHES.set(PUSHES.get() + 1);
        PUSH_RANGE.set((info.offset, info.data.size));
    }

    #[test]
    #[ignore = "requires modern Vulkan; explicit layouts, update call count, rejection atomicity and recording poison"]
    fn gpu_prepared_arguments() {
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
            let mut device = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                0,
            )
            .unwrap();
            device.f.vkCmdPushDataEXT = Some(push);
            let fields = [
                ArgumentField {
                    space: ARGUMENT_DIRECT,
                    binding: ARGUMENT_DIRECT,
                    ..field(0, ARGUMENT_INLINE, 0)
                },
                field(8, ARGUMENT_READ_ADDRESS, 0),
                field(16, ARGUMENT_READ_WRITE_ADDRESS, 1),
            ];
            let layout = unsafe { ArgumentLayout::create(&device, &desc(&fields, 24)) }.unwrap();
            let other = unsafe { ArgumentLayout::create(&device, &desc(&fields, 24)) }.unwrap();
            let code: Vec<u32> =
                include_bytes!("../../../../examples/shaders/foundation-compute.comp.spv")
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
                entry: c"transform".as_ptr(),
                specialization: ptr::null(),
                subgroup: ptr::null(),
            };
            let requirements = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: Features::default(),
                local_size: [64, 1, 1],
                shared_memory: 0,
            };
            let description = ExecutableDesc {
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
            // Pipeline creation borrows the explicitly prepared layout.
            let executable = unsafe { Executable::create(&device, &description, &layout) }.unwrap();
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
                    replay_mode: 1,
                    level: 0,
                    inheritance: ptr::null(),
                })
                .unwrap();
            let e = unsafe { &*list };
            let data = [3u64, 16, 32];
            PUSHES.set(0);
            assert_eq!(
                unsafe { e.set_arguments(&layout, 0, 24, data.as_ptr().cast()) },
                Err(INVALID)
            );
            e.bind_executable(&executable).unwrap();
            assert_eq!(
                unsafe { e.set_arguments(&other, 0, 24, data.as_ptr().cast()) },
                Err(INVALID)
            );
            assert_eq!(
                unsafe { e.set_arguments(&layout, 12, 4, data.as_ptr().cast()) },
                Err(INVALID)
            );
            let bad = [3u64, 16, 3];
            assert_eq!(
                unsafe { e.set_arguments(&layout, 0, 24, bad.as_ptr().cast()) },
                Err(INVALID)
            );
            assert_eq!(PUSHES.get(), 0);
            unsafe { e.set_arguments(&layout, 0, 24, data.as_ptr().cast()) }.unwrap();
            assert_eq!((PUSHES.get(), PUSH_RANGE.get()), (1, (0, 24)));
            unsafe { e.set_arguments(&layout, 8, 8, data[1..].as_ptr().cast()) }.unwrap();
            assert_eq!((PUSHES.get(), PUSH_RANGE.get()), (2, (8, 8)));
            unsafe { e.set_arguments(&layout, 24, 0, ptr::null()) }.unwrap();
            assert_eq!(PUSHES.get(), 2);
            unsafe {
                crate::foundation::commands_api::encode(list, |e| {
                    e.set_arguments(&layout, 0, 24, bad.as_ptr().cast())
                });
            }
            assert_eq!(
                unsafe { e.set_arguments(&layout, 0, 24, data.as_ptr().cast()) },
                Err(INVALID)
            );
            assert_eq!(PUSHES.get(), 2);
            assert_eq!(e.end(), Err(INVALID));
            arena.reset(false).unwrap();
            tested += 1;
        }
        assert!(tested > 0, "No suitable device, not a passing skip");
    }
}

#[cfg(test)]
#[path = "argument_execution.rs"]
mod execution;
