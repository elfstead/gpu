//! Complete native dynamic fetch descriptions. Caller scratch, no persistent layout.
use super::*;
use std::alloc::Layout;

pub(super) fn features(
    enabled: FeatureBits,
    next: *mut std::ffi::c_void,
) -> vk::VkPhysicalDeviceVertexInputDynamicStateFeaturesEXT {
    vk::VkPhysicalDeviceVertexInputDynamicStateFeaturesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VERTEX_INPUT_DYNAMIC_STATE_FEATURES_EXT,
        pNext: next, vertexInputDynamicState: u32::from(enabled & DYNAMIC_VERTEX_INPUT != 0),
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<FeatureBits, Status> {
    if !instance
        .supports_extension(physical, c"VK_EXT_vertex_input_dynamic_state")
        .map_err(loader_error)?
    {
        return Ok(0);
    }
    let mut value = features(0, ptr::null_mut());
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut value).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    Ok(if value.vertexInputDynamicState != 0 {
        DYNAMIC_VERTEX_INPUT
    } else {
        0
    })
}
pub(super) fn load(
    instance: &Instance,
    enabled: FeatureBits,
) -> Result<vk::PFN_vkCmdSetVertexInputEXT, Status> {
    if enabled & DYNAMIC_VERTEX_INPUT == 0 {
        return Ok(None);
    }
    let command = unsafe {
        std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkCmdSetVertexInputEXT>(
            instance.proc(c"vkCmdSetVertexInputEXT"),
        )
    };
    Ok(Some(command.ok_or(UNSUPPORTED)?))
}
fn layout(bindings: u32, attributes: u32) -> Result<(Layout, usize), Status> {
    let a = Layout::array::<vk::VkVertexInputBindingDescription2EXT>(bindings as usize)
        .map_err(|_| INVALID)?;
    let b = Layout::array::<vk::VkVertexInputAttributeDescription2EXT>(attributes as usize)
        .map_err(|_| INVALID)?;
    let (layout, offset) = a.extend(b).map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), offset))
}
pub(in crate::foundation) fn requirements(
    bindings: u32,
    attributes: u32,
) -> Result<HostRequirements, Status> {
    if bindings == 0 && attributes == 0 {
        return Ok(HostRequirements {
            size: 0,
            alignment: 1,
        });
    }
    let (layout, _) = layout(bindings, attributes)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
/// Uses the immutable physical snapshot, never a driver query while recording.
pub(super) fn format(id: u32, formats: &[FormatInfo]) -> Result<vk::VkFormat, Status> {
    let native = images::format(id)?;
    if native.aspects != 1 {
        return Err(INVALID);
    }
    if formats
        .get(id.wrapping_sub(1) as usize)
        .is_none_or(|f| f.buffer_features & 1 == 0)
    {
        return Err(UNSUPPORTED);
    }
    Ok(native.native)
}
/// Scratch and arrays must be valid and disjoint. No native command is issued here.
pub(super) unsafe fn pack(
    input: &VertexInput,
    enabled: FeatureBits,
    limits: &GraphicsLimits,
    controls: &RasterControlLimits,
    formats: &[FormatInfo],
    scratch: HostSpan,
) -> Result<
    (
        *const vk::VkVertexInputBindingDescription2EXT,
        *const vk::VkVertexInputAttributeDescription2EXT,
    ),
    Status,
> {
    // parent permits the existing divisor extension but checks size/revision.
    unsafe {
        local_read::parent::<VertexInput>(&input.header, VERTEX_INPUT)?;
    }
    if enabled & DYNAMIC_VERTEX_INPUT == 0 {
        return Err(UNSUPPORTED);
    }
    if input.binding_count > limits.max_vertex_bindings
        || input.attribute_count > limits.max_vertex_attributes
    {
        return Err(UNSUPPORTED);
    }
    let bindings = unsafe { commands::slice(input.bindings, input.binding_count)? };
    let attributes = unsafe { commands::slice(input.attributes, input.attribute_count)? };
    vertex_layout(limits, bindings, attributes)?;
    let divisors = unsafe {
        raster_controls::divisor_entries(input.header.next, bindings, enabled, controls)?
    };
    // Unlike static divisor state, the native dynamic description requires the
    // non-unit-divisor feature for *any* divisor != 1, including zero.
    if enabled & VERTEX_DIVISOR == 0 && divisors.iter().any(|d| d.divisor != 1) {
        return Err(UNSUPPORTED);
    }
    let (allocation, offset) = layout(input.binding_count, input.attribute_count)?;
    let req = HostRequirements {
        size: allocation.size() as u64,
        alignment: allocation.align() as u64,
    };
    let output = commands::scratch(scratch.data, scratch.size, req)?;
    if req.size == 0 {
        return Ok((ptr::null(), ptr::null()));
    }
    let nb = output.cast::<vk::VkVertexInputBindingDescription2EXT>();
    let na = unsafe { output.add(offset) }.cast::<vk::VkVertexInputAttributeDescription2EXT>();
    for (i, binding) in bindings.iter().enumerate() {
        let divisor = divisors
            .iter()
            .find(|d| d.binding == binding.binding)
            .map_or(1, |d| d.divisor);
        unsafe {
            nb.add(i).write(vk::VkVertexInputBindingDescription2EXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_VERTEX_INPUT_BINDING_DESCRIPTION_2_EXT,
                binding: binding.binding,
                stride: binding.stride,
                inputRate: binding.rate,
                divisor,
                ..Default::default()
            });
        }
    }
    for (i, attribute) in attributes.iter().enumerate() {
        let format = format(attribute.format, formats)?;
        unsafe {
            na.add(i).write(vk::VkVertexInputAttributeDescription2EXT {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_VERTEX_INPUT_ATTRIBUTE_DESCRIPTION_2_EXT,
                location: attribute.location,
                binding: attribute.binding,
                format,
                offset: attribute.offset,
                ..Default::default()
            });
        }
    }
    Ok((nb, na))
}

pub(super) fn vertex_layout(
    limits: &GraphicsLimits,
    bindings: &[VertexBinding],
    attributes: &[VertexAttribute],
) -> Result<(), Status> {
    for (index, binding) in bindings.iter().enumerate() {
        if binding.rate > 1
            || bindings[..index]
                .iter()
                .any(|b| b.binding == binding.binding)
        {
            return Err(INVALID);
        }
        if binding.binding >= limits.max_vertex_bindings
            || binding.stride > limits.max_vertex_stride
        {
            return Err(UNSUPPORTED);
        }
    }
    for (index, attribute) in attributes.iter().enumerate() {
        if !bindings.iter().any(|b| b.binding == attribute.binding)
            || attributes[..index]
                .iter()
                .any(|a| a.location == attribute.location)
        {
            return Err(INVALID);
        }
        if attribute.location >= limits.max_vertex_attributes
            || attribute.offset > limits.max_vertex_attribute_offset
        {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_input_replaces_state_without_scratch_and_checks_feature() {
        let p = VertexInput {
            header: Record::new::<VertexInput>(VERTEX_INPUT),
            binding_count: 0,
            attribute_count: 0,
            bindings: ptr::null(),
            attributes: ptr::null(),
        };
        let args = (
            GraphicsLimits::default(),
            RasterControlLimits::default(),
            HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
        );
        assert_eq!(
            unsafe { pack(&p, DYNAMIC_VERTEX_INPUT, &args.0, &args.1, &[], args.2) },
            Ok((ptr::null(), ptr::null()))
        );
        assert_eq!(
            unsafe { pack(&p, 0, &args.0, &args.1, &[], args.2) },
            Err(UNSUPPORTED)
        );
        assert_eq!(requirements(0, 0).unwrap().size, 0);
    }
    #[test]
    fn packing_is_bounded_sparse_and_preserves_fetch_semantics() {
        let limits = GraphicsLimits {
            max_vertex_bindings: 8,
            max_vertex_attributes: 8,
            max_vertex_stride: 64,
            max_vertex_attribute_offset: 64,
            ..Default::default()
        };
        let controls = RasterControlLimits {
            max_vertex_divisor: 16,
            ..Default::default()
        };
        let mut bindings = [
            VertexBinding {
                binding: 5,
                stride: 32,
                rate: 1,
            },
            VertexBinding {
                binding: 2,
                stride: 0,
                rate: 0,
            },
        ];
        let mut attrs = [
            VertexAttribute {
                location: 3,
                binding: 5,
                format: 1,
                offset: 16,
            },
            VertexAttribute {
                location: 0,
                binding: 2,
                format: 1,
                offset: 0,
            },
        ];
        let mut entries = [VertexDivisor {
            binding: 5,
            divisor: 0,
        }];
        let ext = VertexDivisors {
            header: Record::new::<VertexDivisors>(VERTEX_DIVISORS),
            count: 1,
            reserved: 0,
            entries: entries.as_ptr(),
        };
        let mut p = VertexInput {
            header: Record {
                next: &ext.header,
                ..Record::new::<VertexInput>(VERTEX_INPUT)
            },
            binding_count: 2,
            attribute_count: 2,
            bindings: bindings.as_ptr(),
            attributes: attrs.as_ptr(),
        };
        let mut storage = [0xa5a5a5a5a5a5a5a5u64; 40];
        let req = requirements(2, 2).unwrap();
        let mut scratch = HostSpan {
            data: storage.as_mut_ptr().cast(),
            size: req.size,
        };
        let formats = [FormatInfo {
            format: 1,
            buffer_features: 1,
            ..Default::default()
        }];
        let enabled = DYNAMIC_VERTEX_INPUT | VERTEX_DIVISOR | VERTEX_ZERO_DIVISOR;
        let (b, a) = unsafe { pack(&p, enabled, &limits, &controls, &formats, scratch) }.unwrap();
        unsafe {
            assert_eq!(
                ((*b).binding, (*b).stride, (*b).inputRate, (*b).divisor),
                (5, 32, 1, 0)
            );
            assert_eq!(
                ((*b.add(1)).binding, (*b.add(1)).stride, (*b.add(1)).divisor),
                (2, 0, 1)
            );
            assert_eq!(((*a).location, (*a).binding, (*a).offset), (3, 5, 16));
            assert!((*a).pNext.is_null());
        }
        assert!(storage[(req.size as usize / 8)..]
            .iter()
            .all(|v| *v == 0xa5a5a5a5a5a5a5a5));
        assert_eq!(
            unsafe {
                pack(
                    &p,
                    enabled & !VERTEX_DIVISOR,
                    &limits,
                    &controls,
                    &formats,
                    scratch,
                )
            },
            Err(UNSUPPORTED)
        );
        assert_eq!(
            unsafe {
                pack(
                    &p,
                    enabled & !VERTEX_ZERO_DIVISOR,
                    &limits,
                    &controls,
                    &formats,
                    scratch,
                )
            },
            Err(UNSUPPORTED)
        );
        scratch.size -= 1;
        assert_eq!(
            unsafe { pack(&p, enabled, &limits, &controls, &formats, scratch) },
            Err(CAPACITY)
        );
        scratch.size += 1;
        entries[0].divisor = 2;
        bindings[0].rate = 0;
        assert_eq!(
            unsafe { pack(&p, enabled, &limits, &controls, &formats, scratch) },
            Err(INVALID)
        );
        bindings[0].rate = 1;
        attrs[1].location = 3;
        assert_eq!(
            unsafe { pack(&p, enabled, &limits, &controls, &formats, scratch) },
            Err(INVALID)
        );
        attrs[1].location = 0;
        attrs[1].format = 0;
        assert_eq!(
            unsafe { pack(&p, enabled, &limits, &controls, &formats, scratch) },
            Err(UNSUPPORTED)
        );
        attrs[1].format = 1;
        p.binding_count = 9;
        assert_eq!(
            unsafe { pack(&p, enabled, &limits, &controls, &formats, scratch) },
            Err(UNSUPPORTED)
        );
        p.binding_count = 2;
        assert_eq!(
            unsafe { pack(&p, enabled, &limits, &controls, &[], scratch) },
            Err(UNSUPPORTED)
        );
        assert_eq!(entries[0].divisor, 2);
        assert_eq!(attrs[1].format, 1);
        assert_eq!(bindings[0].rate, 1);
    }
}
