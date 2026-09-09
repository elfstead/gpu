//! Narrow Vulkan loader/query boundary. No Vulkan types escape into the public ABI.
use crate::{Error, OgpuCapabilities, OgpuDeviceInfo, INTERNAL_ERROR, LOADER_ERROR, UNSUPPORTED};
use libloading::Library;
use ogpu_vulkan_sys as vk;
use std::{
    ffi::{c_char, CStr},
    ptr,
};

const V1_1: u32 = version(1, 1);
const V1_2: u32 = version(1, 2);
const V1_3: u32 = version(1, 3);
const fn version(major: u32, minor: u32) -> u32 {
    (major << 22) | (minor << 12)
}

// VKAPI_CALL is the C ABI on our initial Linux x86-64 target. Assignment from the
// generated PFN types below is checked by Rust (no transmute for these aliases).
type GetProc = unsafe extern "C" fn(vk::VkInstance, *const c_char) -> vk::PFN_vkVoidFunction;
type Destroy = unsafe extern "C" fn(vk::VkInstance, *const vk::VkAllocationCallbacks);

// All invocations name a Vulkan command together with its generated matching PFN type.
macro_rules! command {
    ($get:expr, $instance:expr, $name:literal, $ty:ty) => {{
        // SAFETY: get belongs to the retained Vulkan library; the instance is live or NULL.
        let erased = unsafe { $get($instance, concat!($name, "\0").as_ptr().cast()) };
        // SAFETY: Vulkan defines vkGetInstanceProcAddr's erased result as this exact PFN.
        let typed: $ty = unsafe { std::mem::transmute(erased) };
        typed.ok_or_else(|| Error::new(LOADER_ERROR, concat!("Missing Vulkan command: ", $name)))?
    }};
}

pub(crate) struct Instance {
    handle: vk::VkInstance,
    get: GetProc,
    destroy: Destroy,
    api_version: u32,
    _library: Library,
}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: this owns a live instance, no device children exist, and the library is
        // retained until after Drop. The same NULL allocator was used for creation.
        unsafe {
            (self.destroy)(self.handle, ptr::null());
        }
    }
}

impl Instance {
    pub(crate) fn new() -> Result<Self, Error> {
        // Deliberate developer override. Treat it like any executable/library search path:
        // callers must not accept it from an untrusted party.
        let path =
            std::env::var_os("OGPU_VULKAN_LIBRARY").unwrap_or_else(|| "libvulkan.so.1".into());
        // SAFETY: loading the system Vulkan loader (or caller-selected library) executes code.
        let library = unsafe { Library::new(&path) }
            .map_err(|e| Error::new(LOADER_ERROR, format!("Cannot load {path:?}: {e}")))?;
        // Resolve destruction BEFORE creating an instance, so all later failures have RAII
        // cleanup. The Vulkan loader exports Vulkan 1.0 entry points, including destruction.
        let (get, destroy) = unsafe {
            let get = *library
                .get::<vk::PFN_vkGetInstanceProcAddr>(b"vkGetInstanceProcAddr\0")
                .map_err(|e| Error::new(LOADER_ERROR, e.to_string()))?;
            let destroy = *library
                .get::<vk::PFN_vkDestroyInstance>(b"vkDestroyInstance\0")
                .map_err(|e| Error::new(LOADER_ERROR, e.to_string()))?;
            (
                get.ok_or_else(|| Error::new(LOADER_ERROR, "Missing vkGetInstanceProcAddr"))?,
                destroy.ok_or_else(|| Error::new(LOADER_ERROR, "Missing vkDestroyInstance"))?,
            )
        };
        // An absent version command denotes a Vulkan 1.0 loader, not a corrupt loader.
        let version_fn: vk::PFN_vkEnumerateInstanceVersion = unsafe {
            std::mem::transmute(get(ptr::null_mut(), c"vkEnumerateInstanceVersion".as_ptr()))
        };
        let Some(version_fn) = version_fn else {
            return Err(Error::new(
                UNSUPPORTED,
                "Vulkan 1.1 or newer loader required",
            ));
        };
        let mut supported = 0;
        // SAFETY: valid output and a correctly typed loader entry point.
        check("vkEnumerateInstanceVersion", unsafe {
            version_fn(&mut supported)
        })?;
        if supported < V1_1 {
            return Err(Error::new(
                UNSUPPORTED,
                "Vulkan 1.1 or newer loader required",
            ));
        }
        // apiVersion declares our application ceiling, not the loader's ceiling.
        // A 1.1+ instance implementation may expose newer physical devices.
        let api_version = V1_3;
        let create = command!(
            get,
            ptr::null_mut(),
            "vkCreateInstance",
            vk::PFN_vkCreateInstance
        );
        let app = vk::VkApplicationInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_APPLICATION_INFO,
            apiVersion: api_version,
            ..Default::default()
        };
        let info = vk::VkInstanceCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
            pApplicationInfo: &app,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        // SAFETY: initialized structures and valid lifetimes; no extensions/layers requested.
        check("vkCreateInstance", unsafe {
            create(&info, ptr::null(), &mut handle)
        })?;
        Ok(Self {
            handle,
            get,
            destroy,
            api_version,
            _library: library,
        })
    }

    pub(crate) fn devices(&self) -> Result<Vec<OgpuDeviceInfo>, Error> {
        let enumerate_devices = command!(
            self.get,
            self.handle,
            "vkEnumeratePhysicalDevices",
            vk::PFN_vkEnumeratePhysicalDevices
        );
        // SAFETY: enumerate supplies properly sized, initialized output storage.
        let devices = enumerate("vkEnumeratePhysicalDevices", |count, data| unsafe {
            enumerate_devices(self.handle, count, data)
        })?;
        devices
            .into_iter()
            .map(|device| self.device_info(device))
            .collect()
    }

    fn device_info(&self, device: vk::VkPhysicalDevice) -> Result<OgpuDeviceInfo, Error> {
        let properties = command!(
            self.get,
            self.handle,
            "vkGetPhysicalDeviceProperties",
            vk::PFN_vkGetPhysicalDeviceProperties
        );
        let extensions = command!(
            self.get,
            self.handle,
            "vkEnumerateDeviceExtensionProperties",
            vk::PFN_vkEnumerateDeviceExtensionProperties
        );
        let features = command!(
            self.get,
            self.handle,
            "vkGetPhysicalDeviceFeatures2",
            vk::PFN_vkGetPhysicalDeviceFeatures2
        );
        let queues = command!(
            self.get,
            self.handle,
            "vkGetPhysicalDeviceQueueFamilyProperties",
            vk::PFN_vkGetPhysicalDeviceQueueFamilyProperties
        );
        let mut props = vk::VkPhysicalDeviceProperties::default();
        // SAFETY: device was returned by this live instance and the output is writable.
        unsafe {
            properties(device, &mut props);
        }
        let exts = enumerate(
            "vkEnumerateDeviceExtensionProperties",
            |count, data| unsafe { extensions(device, ptr::null(), count, data) },
        )?;
        let supported = |name: &CStr| exts.iter().any(|e| name_matches(&e.extensionName, name));
        let api = props.apiVersion.min(self.api_version);
        // A separate query per optional structure deliberately avoids a self-referential
        // pNext chain. Only query structures supported by core version or extension name.
        // The feature BOOL, not the extension name, determines the public capability.
        macro_rules! query {
            ($ty:ty, $stype:ident, $condition:expr) => {{
                let mut extension = <$ty>::default();
                if $condition {
                    extension.sType = vk::$stype;
                    let mut root = vk::VkPhysicalDeviceFeatures2 {
                        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
                        pNext: (&mut extension as *mut $ty).cast(),
                        ..Default::default()
                    };
                    // SAFETY: this initialized supported structure extends Features2;
                    // both structures remain stationary and live throughout the query.
                    unsafe {
                        features(device, &mut root);
                    }
                }
                extension
            }};
        }
        let bda = query!(
            vk::VkPhysicalDeviceBufferDeviceAddressFeatures,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_BUFFER_DEVICE_ADDRESS_FEATURES,
            api >= V1_2 || supported(c"VK_KHR_buffer_device_address")
        );
        let timeline = query!(
            vk::VkPhysicalDeviceTimelineSemaphoreFeatures,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_FEATURES,
            api >= V1_2 || supported(c"VK_KHR_timeline_semaphore")
        );
        let sync = query!(
            vk::VkPhysicalDeviceSynchronization2Features,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SYNCHRONIZATION_2_FEATURES,
            api >= V1_3 || supported(c"VK_KHR_synchronization2")
        );
        let storage8 = query!(
            vk::VkPhysicalDevice8BitStorageFeatures,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_8BIT_STORAGE_FEATURES,
            api >= V1_2 || supported(c"VK_KHR_8bit_storage")
        );
        let storage16 = query!(
            vk::VkPhysicalDevice16BitStorageFeatures,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES,
            api >= V1_1 || supported(c"VK_KHR_16bit_storage")
        );
        let narrow = query!(
            vk::VkPhysicalDeviceShaderFloat16Int8Features,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_FLOAT16_INT8_FEATURES,
            api >= V1_2 || supported(c"VK_KHR_shader_float16_int8")
        );
        let heap = query!(
            vk::VkPhysicalDeviceDescriptorHeapFeaturesEXT,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_FEATURES_EXT,
            supported(c"VK_EXT_descriptor_heap")
        );
        let addresses = query!(
            vk::VkPhysicalDeviceDeviceAddressCommandsFeaturesKHR,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_ADDRESS_COMMANDS_FEATURES_KHR,
            supported(c"VK_KHR_device_address_commands")
        );
        let untyped = query!(
            vk::VkPhysicalDeviceShaderUntypedPointersFeaturesKHR,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_UNTYPED_POINTERS_FEATURES_KHR,
            supported(c"VK_KHR_shader_untyped_pointers")
        );
        let matrix = query!(
            vk::VkPhysicalDeviceCooperativeMatrixFeaturesKHR,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COOPERATIVE_MATRIX_FEATURES_KHR,
            supported(c"VK_KHR_cooperative_matrix")
        );
        let bf16 = query!(
            vk::VkPhysicalDeviceShaderBfloat16FeaturesKHR,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_BFLOAT16_FEATURES_KHR,
            supported(c"VK_KHR_shader_bfloat16")
        );
        let float8 = query!(
            vk::VkPhysicalDeviceShaderFloat8FeaturesEXT,
            VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_FLOAT8_FEATURES_EXT,
            supported(c"VK_EXT_shader_float8")
        );
        let microscaling = query!(vk::VkPhysicalDeviceShaderOCPMicroscalingTypesFeaturesEXT, VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_OCP_MICROSCALING_TYPES_FEATURES_EXT, supported(c"VK_EXT_shader_ocp_microscaling_types"));
        let mut core = vk::VkPhysicalDeviceFeatures2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
            ..Default::default()
        };
        // SAFETY: live device and a correctly initialized writable root structure.
        unsafe {
            features(device, &mut core);
        }
        let mut queue_count = 0;
        // SAFETY: first call queries the stable number of queue families.
        unsafe {
            queues(device, &mut queue_count, ptr::null_mut());
        }
        let mut families = vec![vk::VkQueueFamilyProperties::default(); queue_count as usize];
        if queue_count != 0 {
            // SAFETY: output allocation holds queue_count initialized entries.
            unsafe {
                queues(device, &mut queue_count, families.as_mut_ptr());
            }
            if queue_count as usize > families.len() {
                return Err(Error::new(
                    INTERNAL_ERROR,
                    "Queue family count exceeded capacity",
                ));
            }
            families.truncate(queue_count as usize);
        }
        let has_queue = |flag| {
            u32::from(
                families
                    .iter()
                    .any(|q| q.queueCount > 0 && q.queueFlags & flag != 0),
            )
        };
        let flag = |value| u32::from(value == vk::VK_TRUE);
        let capabilities = OgpuCapabilities {
            graphics_queue: has_queue(vk::VkQueueFlagBits_VK_QUEUE_GRAPHICS_BIT),
            compute_queue: has_queue(vk::VkQueueFlagBits_VK_QUEUE_COMPUTE_BIT),
            buffer_device_address: flag(bda.bufferDeviceAddress),
            timeline_semaphore: flag(timeline.timelineSemaphore),
            synchronization2: flag(sync.synchronization2),
            descriptor_heap: flag(heap.descriptorHeap),
            device_address_commands: flag(addresses.deviceAddressCommands),
            shader_untyped_pointers: flag(untyped.shaderUntypedPointers),
            cooperative_matrix: flag(matrix.cooperativeMatrix),
            storage_buffer_8bit_access: flag(storage8.storageBuffer8BitAccess),
            storage_buffer_16bit_access: flag(storage16.storageBuffer16BitAccess),
            shader_float16: flag(narrow.shaderFloat16),
            shader_int8: flag(narrow.shaderInt8),
            shader_int16: flag(core.features.shaderInt16),
            shader_int64: flag(core.features.shaderInt64),
            shader_float64: flag(core.features.shaderFloat64),
            shader_bfloat16: flag(bf16.shaderBFloat16Type),
            shader_bfloat16_cooperative_matrix: flag(bf16.shaderBFloat16CooperativeMatrix),
            shader_float8: flag(float8.shaderFloat8),
            shader_float8_cooperative_matrix: flag(float8.shaderFloat8CooperativeMatrix),
            shader_float4: flag(microscaling.shaderFloat4),
            shader_float6: flag(microscaling.shaderFloat6),
            shader_float8_unsigned_e8m0: flag(microscaling.shaderFloat8UnsignedE8M0),
            shader_mx_int8: flag(microscaling.shaderMXInt8),
        };
        let mut name = props.deviceName;
        name[255] = 0;
        Ok(OgpuDeviceInfo {
            name,
            vendor_id: props.vendorID,
            device_id: props.deviceID,
            device_type: match props.deviceType {
                vk::VkPhysicalDeviceType_VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU => 1,
                vk::VkPhysicalDeviceType_VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU => 2,
                vk::VkPhysicalDeviceType_VK_PHYSICAL_DEVICE_TYPE_VIRTUAL_GPU => 3,
                vk::VkPhysicalDeviceType_VK_PHYSICAL_DEVICE_TYPE_CPU => 4,
                _ => 0,
            },
            vulkan_api_major: (props.apiVersion >> 22) & 0x7f,
            vulkan_api_minor: (props.apiVersion >> 12) & 0x3ff,
            vulkan_api_patch: props.apiVersion & 0xfff,
            capabilities,
        })
    }
}

fn name_matches(bytes: &[c_char], name: &CStr) -> bool {
    let expected = name.to_bytes_with_nul();
    bytes.len() >= expected.len() && bytes.iter().zip(expected).all(|(&a, &b)| a as u8 == b)
}

fn check(operation: &str, result: vk::VkResult) -> Result<(), Error> {
    if result == vk::VkResult_VK_SUCCESS {
        Ok(())
    } else {
        Err(Error::vulkan(operation, result))
    }
}

/// Handles count changes without exposing uninitialized elements or looping forever.
fn enumerate<T: Default + Clone>(
    operation: &str,
    mut call: impl FnMut(*mut u32, *mut T) -> vk::VkResult,
) -> Result<Vec<T>, Error> {
    for _ in 0..8 {
        let mut count = 0;
        let result = call(&mut count, ptr::null_mut());
        if result == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        check(operation, result)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut items = vec![T::default(); count as usize];
        let result = call(&mut count, items.as_mut_ptr());
        if result == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        check(operation, result)?;
        if count as usize > items.len() {
            return Err(Error::new(
                INTERNAL_ERROR,
                "Vulkan enumeration count exceeded capacity",
            ));
        }
        items.truncate(count as usize);
        return Ok(items);
    }
    Err(Error::vulkan(operation, vk::VkResult_VK_INCOMPLETE))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_enumeration_is_success() {
        assert!(enumerate::<u32>("test", |_, _| vk::VkResult_VK_SUCCESS)
            .ok()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn first_call_error_is_preserved() {
        let error = enumerate::<u32>("test", |_, _| vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY)
            .err()
            .unwrap();
        assert_eq!(error.vk, vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
    }
    #[test]
    fn incomplete_enumeration_is_retried_and_truncated() {
        let mut reads = 0;
        let values = enumerate::<u32>("test", |count, data| unsafe {
            if data.is_null() {
                *count = 2;
                return vk::VkResult_VK_SUCCESS;
            }
            reads += 1;
            if reads == 1 {
                return vk::VkResult_VK_INCOMPLETE;
            }
            *count = 1;
            *data = 42;
            vk::VkResult_VK_SUCCESS
        })
        .ok()
        .unwrap();
        assert_eq!(reads, 2);
        assert_eq!(values, [42]);
    }
    #[test]
    fn incomplete_is_bounded() {
        let mut calls = 0;
        assert!(enumerate::<u32>("test", |_, _| {
            calls += 1;
            vk::VkResult_VK_INCOMPLETE
        })
        .is_err());
        assert_eq!(calls, 8);
    }
    #[test]
    fn extension_match_is_exact_and_bounded() {
        let bytes: Vec<_> = b"VK_KHR_cooperative_matrix\0"
            .iter()
            .map(|&b| b as c_char)
            .collect();
        assert!(name_matches(&bytes, c"VK_KHR_cooperative_matrix"));
        assert!(!name_matches(&bytes, c"VK_KHR_cooperative"));
        assert!(!name_matches(
            &bytes[..bytes.len() - 1],
            c"VK_KHR_cooperative_matrix"
        ));
    }
}
