//! Creation-time scalar specialization; each shader stage owns its ID namespace.
use super::*;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SpecializationConstant {
    pub id: u32,
    pub bits: u32,
}

pub(super) struct Specialization {
    entries: Vec<vk::VkSpecializationMapEntry>,
    data: Vec<u32>,
}

impl Specialization {
    pub(super) fn new(constants: &[SpecializationConstant]) -> Result<Self, Error> {
        if constants.len() > (u32::MAX / 4) as usize {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Too many specialization constants",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut entries = Vec::with_capacity(constants.len());
        let mut data = Vec::with_capacity(constants.len());
        for (i, constant) in constants.iter().enumerate() {
            if !ids.insert(constant.id) {
                return Err(Error::new(INVALID_ARGUMENT, "Duplicate specialization ID"));
            }
            entries.push(vk::VkSpecializationMapEntry {
                constantID: constant.id,
                offset: i as u32 * 4,
                size: 4,
            });
            data.push(constant.bits);
        }
        Ok(Self { entries, data })
    }

    pub(super) fn info(&self) -> vk::VkSpecializationInfo {
        vk::VkSpecializationInfo {
            mapEntryCount: self.entries.len() as u32,
            pMapEntries: self.entries.as_ptr(),
            dataSize: self.data.len() * 4,
            pData: self.data.as_ptr().cast(),
        }
    }
}
