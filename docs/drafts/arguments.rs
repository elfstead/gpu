//! C1 argument record mirror. Design only; not part of the runtime or SDK.
//! See ../minimal-core-arguments.md and ogpu_arguments.h.
//! These records do not route native Rust operations through the C ABI.
use std::ffi::{c_char, c_void};
use crate::presentation::Record;

#[repr(C)] pub struct ArgumentLayout { _private: [u8; 0] }
#[repr(C)] pub struct ExecutableCache { _private: [u8; 0] }
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Bytes { pub data: *const c_void, pub size: usize }
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ExecutionLimits {
    pub max_inline_bytes: u64,
    pub max_groups: [u32; 3],
    pub max_local_size: [u32; 3],
    pub max_local_invocations: u32,
    pub max_shared_memory: u32,
    pub argument_kinds: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Shader {
    pub stage: u32,
    pub format: u32,
    pub code: Bytes,
    pub entry: *const c_char,
    pub specialization: *const Record,
    pub subgroup: *const Record,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ExecutableDesc {
    pub header: Record,
    pub kind: u32,
    pub shader_count: u32,
    pub shaders: *const Shader,
    pub static_state: *const Record,
    pub dynamic_state: u64,
    pub requirements: *const Record,
    pub cache: *mut ExecutableCache,
    pub compile_flags: u32,
    pub reserved: u32,
}

pub const ARGUMENT_INLINE: u32 = 0;
pub const ARGUMENT_UNIFORM_ADDRESS: u32 = 1;
pub const ARGUMENT_READ_ADDRESS: u32 = 2;
pub const ARGUMENT_READ_WRITE_ADDRESS: u32 = 3;
pub const ARGUMENT_COMPUTE: u32 = 0;
pub const ARGUMENT_GRAPHICS: u32 = 1;
pub const ARGUMENT_DIRECT: u32 = u32::MAX;
pub const ARGUMENT_BIND_PRESERVES: u32 = 0;
pub const ARGUMENT_BIND_INVALIDATES_ON_CHANGE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ArgumentField {
    pub offset: u32,
    pub size: u32,
    pub kind: u32,
    pub address_alignment: u32,
    pub stages: u64,
    pub space: u32,
    pub binding: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ArgumentLayoutDesc {
    pub execution_class: u32,
    pub byte_size: u32,
    pub field_count: u32,
    pub flags: u32,
    pub fields: *const ArgumentField,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ArgumentStateInfo {
    pub domain: u64,
    pub binding_key: u64,
    pub offset_base: u32,
    pub byte_size: u32,
    pub bind_rule: u32,
    pub reserved: u32,
}
