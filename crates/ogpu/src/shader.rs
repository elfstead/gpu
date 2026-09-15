//! Artifact validation, independent of shader translation or native pipeline creation.
use crate::{boundary::required, Error, INVALID_ARGUMENT, UNSUPPORTED};
use std::{
    collections::BTreeSet,
    ffi::{c_char, c_void, CStr},
};

pub const SHADER_SPIRV: u32 = 0;
pub const SHADER_MSL: u32 = 1;
pub const SHADER_METALLIB: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OgpuSpecializationConstant {
    pub id: u32,
    pub bits: u32,
}

#[repr(C)]
pub struct OgpuShaderDesc {
    pub code: *const c_void,
    pub code_size: u64,
    pub entry_point: *const c_char,
    pub constants: *const OgpuSpecializationConstant,
    pub constant_count: u32,
    pub format: u32,
    pub local_size: [u32; 3],
    pub reserved: u32,
}

pub(crate) struct Shader<'a> {
    pub(crate) code: &'a [u8],
    pub(crate) entry: &'a str,
    pub(crate) constants: &'a [OgpuSpecializationConstant],
    pub(crate) format: u32,
    pub(crate) local_size: [u32; 3],
}

impl<'a> Shader<'a> {
    // SAFETY: descriptor and all non-NULL input pointers obey the C header contract.
    pub(crate) unsafe fn read(desc: *const OgpuShaderDesc) -> Result<Self, Error> {
        required(desc)?;
        let d = unsafe { &*desc };
        required(d.code)?;
        if d.reserved != 0 || d.code_size == 0 || d.code_size > isize::MAX as u64 {
            return Err(Error::new(INVALID_ARGUMENT, "Invalid shader description"));
        }
        match d.format {
            SHADER_SPIRV => {
                if d.code as usize % 4 != 0
                    || d.code_size < 20
                    || d.code_size % 4 != 0
                    || d.local_size != [0; 3]
                {
                    return Err(Error::new(
                        INVALID_ARGUMENT,
                        "Invalid SPIR-V size/alignment/local size",
                    ));
                }
            }
            SHADER_MSL | SHADER_METALLIB => {
                if d.local_size.contains(&0) {
                    return Err(Error::new(
                        INVALID_ARGUMENT,
                        "Native compute artifacts need a local size",
                    ));
                }
                if d.constant_count != 0 {
                    return Err(Error::new(
                        UNSUPPORTED,
                        "Native Metal artifacts must be pre-specialized",
                    ));
                }
            }
            _ => return Err(Error::new(INVALID_ARGUMENT, "Unknown shader format")),
        }
        let entry = if d.entry_point.is_null() {
            "main"
        } else {
            unsafe { CStr::from_ptr(d.entry_point) }
                .to_str()
                .map_err(|_| Error::new(INVALID_ARGUMENT, "Entry point is not UTF-8"))?
        };
        if entry.is_empty() {
            return Err(Error::new(INVALID_ARGUMENT, "Empty entry point"));
        }
        let constants = if d.constant_count == 0 {
            &[][..]
        } else {
            required(d.constants)?;
            if d.constant_count as u64 > isize::MAX as u64 / 8 || d.constants as usize % 4 != 0 {
                return Err(Error::new(INVALID_ARGUMENT, "Invalid specialization array"));
            }
            unsafe { std::slice::from_raw_parts(d.constants, d.constant_count as usize) }
        };
        let mut ids = BTreeSet::new();
        if constants.iter().any(|c| !ids.insert(c.id)) {
            return Err(Error::new(INVALID_ARGUMENT, "Duplicate specialization ID"));
        }
        let code = unsafe { std::slice::from_raw_parts(d.code.cast(), d.code_size as usize) };
        Ok(Self {
            code,
            entry,
            constants,
            format: d.format,
            local_size: d.local_size,
        })
    }

    #[cfg(any(target_os = "linux", feature = "spirv-to-msl", test))]
    pub(crate) fn spirv(&self) -> Result<&'a [u32], Error> {
        if self.format != SHADER_SPIRV {
            return Err(Error::new(UNSUPPORTED, "This backend requires SPIR-V"));
        }
        // Current Vulkan executable preparation fixes the entry name. Metal's
        // adapter accepts the same contract; native artifacts have arbitrary names.
        if self.entry != "main" {
            return Err(Error::new(UNSUPPORTED, "SPIR-V entry point must be main"));
        }
        debug_assert_eq!(self.local_size, [0; 3]);
        let words =
            unsafe { std::slice::from_raw_parts(self.code.as_ptr().cast(), self.code.len() / 4) };
        if words[0] != 0x0723_0203 {
            return Err(Error::new(INVALID_ARGUMENT, "Invalid SPIR-V magic"));
        }
        Ok(words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_artifacts_are_not_spirv_and_need_explicit_launch_metadata() {
        let code = b"kernel void test() {}";
        let mut desc = OgpuShaderDesc {
            code: code.as_ptr().cast(),
            code_size: code.len() as u64,
            entry_point: c"test".as_ptr(),
            constants: std::ptr::null(),
            constant_count: 0,
            format: SHADER_MSL,
            local_size: [1; 3],
            reserved: 0,
        };
        unsafe {
            let artifact = Shader::read(&desc).unwrap();
            assert_eq!(artifact.entry, "test");
            assert_eq!(artifact.spirv().unwrap_err().status, UNSUPPORTED);
            desc.local_size[0] = 0;
            assert!(Shader::read(&desc).is_err());
            desc.local_size[0] = 1;
            desc.constant_count = 1;
            assert!(matches!(
                Shader::read(&desc),
                Err(Error {
                    status: UNSUPPORTED,
                    ..
                })
            ));
            desc.constant_count = 0;
            desc.format = 77;
            assert!(Shader::read(&desc).is_err());
        }
    }
}
