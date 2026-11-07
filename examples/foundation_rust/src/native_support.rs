//! Consumer-side native device creation. No core extension-negotiation policy.
use ogpu::foundation::{rust as gpu, types::*};
use ogpu_vulkan_sys as vk;
use std::{ffi::CStr, ptr};
pub fn check(value: vk::VkResult) -> gpu::Result<()> {
    if value == vk::VkResult_VK_SUCCESS {
        Ok(())
    } else {
        eprintln!("NATIVE_RESULT={value}");
        Err(BACKEND_ERROR)
    }
}
pub struct Loader {
    _library: libloading::Library,
    pub get:
        unsafe extern "C" fn(vk::VkInstance, *const std::ffi::c_char) -> vk::PFN_vkVoidFunction,
    pub instance: vk::VkInstance,
}
impl Loader {
    pub unsafe fn new(instance: *mut std::ffi::c_void) -> gpu::Result<Self> {
        let path =
            std::env::var_os("OGPU_VULKAN_LIBRARY").unwrap_or_else(|| "libvulkan.so.1".into());
        let library = unsafe { libloading::Library::new(path) }.map_err(|_| BACKEND_ERROR)?;
        let get = unsafe {
            *library
                .get::<vk::PFN_vkGetInstanceProcAddr>(b"vkGetInstanceProcAddr\0")
                .map_err(|_| BACKEND_ERROR)?
        }
        .ok_or(UNSUPPORTED)?;
        Ok(Self {
            _library: library,
            get,
            instance: instance.cast(),
        })
    }
    pub unsafe fn proc(&self, name: &CStr) -> vk::PFN_vkVoidFunction {
        unsafe { (self.get)(self.instance, name.as_ptr()) }
    }
    pub unsafe fn create_device(
        &self,
        physical: vk::VkPhysicalDevice,
        domain: u32,
        extra: &[&CStr],
    ) -> gpu::Result<vk::VkDevice> {
        unsafe {
            let enumerate: vk::PFN_vkEnumerateDeviceExtensionProperties =
                std::mem::transmute(self.proc(c"vkEnumerateDeviceExtensionProperties"));
            let enumerate = enumerate.ok_or(UNSUPPORTED)?;
            let mut count = 0;
            check(enumerate(
                physical,
                ptr::null(),
                &mut count,
                ptr::null_mut(),
            ))?;
            let mut found = vec![vk::VkExtensionProperties::default(); count as usize];
            check(enumerate(
                physical,
                ptr::null(),
                &mut count,
                found.as_mut_ptr(),
            ))?;
            for name in extra {
                if !found[..count as usize]
                    .iter()
                    .any(|e| CStr::from_ptr(e.extensionName.as_ptr()) == *name)
                {
                    return Err(UNSUPPORTED);
                }
            }
            let create: vk::PFN_vkCreateDevice = std::mem::transmute(self.proc(c"vkCreateDevice"));
            let mut untyped = vk::VkPhysicalDeviceShaderUntypedPointersFeaturesKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_UNTYPED_POINTERS_FEATURES_KHR,
                shaderUntypedPointers: 1, ..Default::default()
            };
            let mut address = vk::VkPhysicalDeviceDeviceAddressCommandsFeaturesKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_ADDRESS_COMMANDS_FEATURES_KHR,
                pNext: ptr::from_mut(&mut untyped).cast(), deviceAddressCommands: 1,
            };
            let mut heap = vk::VkPhysicalDeviceDescriptorHeapFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_FEATURES_EXT,
                pNext: ptr::from_mut(&mut address).cast(), descriptorHeap: 1, ..Default::default()
            };
            let mut v14 = vk::VkPhysicalDeviceVulkan14Features {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_4_FEATURES,
                pNext: ptr::from_mut(&mut heap).cast(),
                maintenance5: 1,
                ..Default::default()
            };
            let mut v13 = vk::VkPhysicalDeviceVulkan13Features {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
                pNext: ptr::from_mut(&mut v14).cast(),
                synchronization2: 1,
                maintenance4: 1,
                ..Default::default()
            };
            let mut v12 = vk::VkPhysicalDeviceVulkan12Features {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
                pNext: ptr::from_mut(&mut v13).cast(),
                bufferDeviceAddress: 1,
                timelineSemaphore: 1,
                ..Default::default()
            };
            let storage = vk::VkPhysicalDevice16BitStorageFeatures {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES,
                pNext: ptr::from_mut(&mut v12).cast(),
                storageBuffer16BitAccess: 1,
                ..Default::default()
            };
            let priority = 1.0;
            let queue = vk::VkDeviceQueueCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                queueFamilyIndex: domain,
                queueCount: 1,
                pQueuePriorities: &priority,
                ..Default::default()
            };
            let mut extensions = vec![
                c"VK_EXT_descriptor_heap".as_ptr(),
                c"VK_KHR_device_address_commands".as_ptr(),
                c"VK_KHR_shader_untyped_pointers".as_ptr(),
            ];
            extensions.extend(extra.iter().map(|name| name.as_ptr()));
            let mut device = ptr::null_mut();
            check(create.ok_or(UNSUPPORTED)?(
                physical,
                &vk::VkDeviceCreateInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                    pNext: ptr::from_ref(&storage).cast(),
                    queueCreateInfoCount: 1,
                    pQueueCreateInfos: &queue,
                    enabledExtensionCount: extensions.len() as u32,
                    ppEnabledExtensionNames: extensions.as_ptr(),
                    ..Default::default()
                },
                ptr::null(),
                &mut device,
            ))?;
            Ok(device)
        }
    }
}
