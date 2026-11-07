//! Native GPU predicates and primitive queries; no CPU decisions or emulation.
use super::*;
pub(super) const CONDITIONAL: FeatureBits =
    FeatureBits::union(&[CONDITIONAL_RENDERING, INHERITED_CONDITIONAL_RENDERING]);
pub(super) const PRIMITIVES: FeatureBits = FeatureBits::union(&[
    PRIMITIVES_GENERATED_QUERY,
    PRIMITIVES_GENERATED_DISCARD,
    PRIMITIVES_GENERATED_STREAMS,
]);
pub(super) struct Native {
    conditional: vk::VkPhysicalDeviceConditionalRenderingFeaturesEXT,
    primitives: vk::VkPhysicalDevicePrimitivesGeneratedQueryFeaturesEXT,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            conditional: vk::VkPhysicalDeviceConditionalRenderingFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_CONDITIONAL_RENDERING_FEATURES_EXT,
                conditionalRendering: u32::from(enabled & CONDITIONAL_RENDERING != 0),
                inheritedConditionalRendering: u32::from(enabled & INHERITED_CONDITIONAL_RENDERING != 0),
                ..Default::default()
            },
            primitives: vk::VkPhysicalDevicePrimitivesGeneratedQueryFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PRIMITIVES_GENERATED_QUERY_FEATURES_EXT,
                primitivesGeneratedQuery: u32::from(enabled & PRIMITIVES_GENERATED_QUERY != 0),
                primitivesGeneratedQueryWithRasterizerDiscard: u32::from(enabled & PRIMITIVES_GENERATED_DISCARD != 0),
                primitivesGeneratedQueryWithNonZeroStreams: u32::from(enabled & PRIMITIVES_GENERATED_STREAMS != 0),
                ..Default::default()
            },
        }
    }
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        enabled: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if enabled & CONDITIONAL != 0 {
            self.conditional.pNext = next;
            next = ptr::from_mut(&mut self.conditional).cast();
        }
        if enabled & PRIMITIVES != 0 {
            self.primitives.pNext = next;
            next = ptr::from_mut(&mut self.primitives).cast();
        }
        next
    }
    fn available(&self) -> FeatureBits {
        let mut bits = FeatureBits::ZERO;
        for (value, bit) in [
            (self.conditional.conditionalRendering, CONDITIONAL_RENDERING),
            (
                self.conditional.inheritedConditionalRendering,
                INHERITED_CONDITIONAL_RENDERING,
            ),
            (
                self.primitives.primitivesGeneratedQuery,
                PRIMITIVES_GENERATED_QUERY,
            ),
            (
                self.primitives
                    .primitivesGeneratedQueryWithRasterizerDiscard,
                PRIMITIVES_GENERATED_DISCARD,
            ),
            (
                self.primitives.primitivesGeneratedQueryWithNonZeroStreams,
                PRIMITIVES_GENERATED_STREAMS,
            ),
        ] {
            if value != 0 {
                bits |= bit;
            }
        }
        bits
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<FeatureBits, Status> {
    let mut present = FeatureBits::ZERO;
    for (mask, name) in [
        (CONDITIONAL, c"VK_EXT_conditional_rendering"),
        (PRIMITIVES, c"VK_EXT_primitives_generated_query"),
    ] {
        if instance
            .supports_extension(physical, name)
            .map_err(loader_error)?
        {
            present |= mask;
        }
    }
    let mut native = Native::new(FeatureBits::ZERO);
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: native.link(ptr::null_mut(), present),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    Ok(native.available())
}
pub(super) struct Commands {
    pub begin: vk::PFN_vkCmdBeginConditionalRendering2EXT,
    pub end: vk::PFN_vkCmdEndConditionalRenderingEXT,
    pub query_begin: vk::PFN_vkCmdBeginQueryIndexedEXT,
    pub query_end: vk::PFN_vkCmdEndQueryIndexedEXT,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! load {
            ($bit:expr,$name:literal,$ty:ty) => {{
                if enabled & $bit == 0 {
                    None
                } else {
                    let f = unsafe {
                        std::mem::transmute::<vk::PFN_vkVoidFunction, $ty>(instance.proc($name))
                    };
                    if f.is_none() {
                        return Err(UNSUPPORTED);
                    }
                    f
                }
            }};
        }
        Ok(Self {
            begin: load!(
                CONDITIONAL_RENDERING,
                c"vkCmdBeginConditionalRendering2EXT",
                vk::PFN_vkCmdBeginConditionalRendering2EXT
            ),
            end: load!(
                CONDITIONAL_RENDERING,
                c"vkCmdEndConditionalRenderingEXT",
                vk::PFN_vkCmdEndConditionalRenderingEXT
            ),
            query_begin: load!(
                PRIMITIVES_GENERATED_QUERY,
                c"vkCmdBeginQueryIndexedEXT",
                vk::PFN_vkCmdBeginQueryIndexedEXT
            ),
            query_end: load!(
                PRIMITIVES_GENERATED_QUERY,
                c"vkCmdEndQueryIndexedEXT",
                vk::PFN_vkCmdEndQueryIndexedEXT
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_native_controls_do_not_enable_capture_or_arithmetic() {
        assert_eq!(
            validate_feature_dependencies(PRIMITIVES_GENERATED_QUERY),
            Ok(())
        );
        assert_eq!(
            validate_feature_dependencies(PRIMITIVES_GENERATED_DISCARD),
            Err(INVALID)
        );
        assert_eq!(validate_feature_dependencies(PRIMITIVES), Ok(()));
        assert_eq!(
            validate_feature_dependencies(INHERITED_CONDITIONAL_RENDERING),
            Err(INVALID)
        );
        assert_eq!(validate_feature_dependencies(CONDITIONAL), Ok(()));
        let mut n = Native::new(PRIMITIVES_GENERATED_QUERY | CONDITIONAL_RENDERING);
        assert_eq!(
            n.available(),
            PRIMITIVES_GENERATED_QUERY | CONDITIONAL_RENDERING
        );
        assert_eq!(n.conditional.inheritedConditionalRendering, 0);
        assert_eq!(
            n.primitives.primitivesGeneratedQueryWithRasterizerDiscard,
            0
        );
        assert_eq!(n.primitives.primitivesGeneratedQueryWithNonZeroStreams, 0);
        let tail = ptr::dangling_mut();
        let head = n.link(tail, PRIMITIVES_GENERATED_QUERY | CONDITIONAL_RENDERING);
        assert_eq!(head, ptr::from_mut(&mut n.primitives).cast());
        assert_eq!(n.primitives.pNext, ptr::from_mut(&mut n.conditional).cast());
        assert_eq!(n.conditional.pNext, tail);
        assert_eq!(n.link(tail, FeatureBits::ZERO), tail);
    }
}
