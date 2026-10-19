//! D3D12 executable cache: advisory preparation-time storage of native cached
//! pipeline-state blobs (`CachedPSO` in, `GetCachedBlob` out), exported as a
//! self-describing container.
//!
//! The exported container carries the identity header the contract requires
//! (format size/version, vendor, device, cache UUID); importing with a
//! different identity is `UNSUPPORTED` and is never silently retried as an
//! empty cache. Entries are keyed by the canonical preparation identity of
//! their executable and compared exactly, so a cached blob is only ever
//! offered to the preparation it was produced from. Caching is advisory: a
//! rejected cached blob falls back to an uncached creation and export may
//! truncate at entry boundaries into a valid partial cache.
use std::{
    cell::RefCell,
    ffi::c_void,
    ptr,
    slice,
};

use super::Device;
use crate::foundation::types::{
    Bytes, ExecutableCacheDesc, ExecutableDesc, Record, Status, CACHE_CALLER_SYNCHRONIZATION,
    CACHE_CONTROL, CACHE_NATIVE_SYNCHRONIZATION, EXECUTABLE_CACHE_DESC, INVALID, OUT_OF_MEMORY,
    UNSUPPORTED,
};

/// Container magic and format version.
const MAGIC: [u8; 8] = *b"OGPUCAC1";
const VERSION: u32 = 1;
const HEADER_BYTES: usize = 32;

/// One advisory entry: the preparation identity and its native cached blob.
struct Entry {
    identity: Vec<u8>,
    blob: Vec<u8>,
}

pub struct ExecutableCache {
    identity: [u8; 16],
    entries: RefCell<Vec<Entry>>,
}

/// Serialize little-endian scalars into a byte sink.
fn push(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(bytes);
}

/// Adapter identity header: vendor, device and cache UUID (the adapter LUID).
fn adapter_identity(d: &Device) -> Result<[u8; 16], Status> {
    // SAFETY: live adapter; the descriptor is written by the call.
    let desc = unsafe { d.adapter.GetDesc1() }.map_err(super::map_error)?;
    let mut identity = [0u8; 16];
    identity[0..4].copy_from_slice(&desc.VendorId.to_le_bytes());
    identity[4..8].copy_from_slice(&desc.DeviceId.to_le_bytes());
    identity[8..12].copy_from_slice(&desc.AdapterLuid.LowPart.to_le_bytes());
    identity[12..16].copy_from_slice(&(desc.AdapterLuid.HighPart as u32).to_le_bytes());
    Ok(identity)
}

impl ExecutableCache {
    /// # Safety
    /// Live device and initial cache bytes; declared synchronization applies.
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ExecutableCacheDesc,
    ) -> Result<Box<Self>, Status> {
        desc.header.validate::<ExecutableCacheDesc>(EXECUTABLE_CACHE_DESC)?;
        if desc.reserved != 0 {
            return Err(INVALID);
        }
        match desc.synchronization {
            CACHE_NATIVE_SYNCHRONIZATION => {}
            CACHE_CALLER_SYNCHRONIZATION => {
                if d.snapshot.features.enabled & CACHE_CONTROL == 0 {
                    // Caller synchronization requires the CACHE_CONTROL feature.
                    return Err(UNSUPPORTED);
                }
            }
            _ => return Err(INVALID),
        }
        let identity = adapter_identity(d)?;
        let mut entries = Vec::new();
        if desc.initial.size != 0 {
            let Bytes { data, size } = desc.initial;
            if data.is_null() {
                return Err(INVALID);
            }
            // SAFETY: initial bytes are borrowed during creation per contract.
            let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), size) };
            entries = Self::parse(bytes, &identity)?;
        }
        Ok(Box::new(Self {
            identity,
            entries: RefCell::new(entries),
        }))
    }

    /// Parse an exported container, checking the identity header.
    fn parse(bytes: &[u8], identity: &[u8; 16]) -> Result<Vec<Entry>, Status> {
        if bytes.len() < HEADER_BYTES || bytes[0..8] != MAGIC {
            return Err(UNSUPPORTED);
        }
        let mut cursor = 8;
        let word = |cursor: &mut usize| -> Result<u32, Status> {
            let end = *cursor + 4;
            let value = bytes.get(*cursor..end).ok_or(UNSUPPORTED)?;
            *cursor = end;
            Ok(u32::from_le_bytes(value.try_into().unwrap()))
        };
        if word(&mut cursor)? != VERSION {
            return Err(UNSUPPORTED);
        }
        if &bytes[cursor..cursor + 16] != identity.as_slice() {
            // Identity mismatch is UNSUPPORTED, never retried as empty.
            return Err(UNSUPPORTED);
        }
        cursor += 16;
        let count = word(&mut cursor)? as usize;
        let mut entries = Vec::new();
        entries.try_reserve_exact(count).map_err(|_| OUT_OF_MEMORY)?;
        for _ in 0..count {
            let take = |cursor: &mut usize| -> Result<usize, Status> {
                let end = *cursor + 8;
                let value = bytes.get(*cursor..end).ok_or(UNSUPPORTED)?;
                *cursor = end;
                Ok(u64::from_le_bytes(value.try_into().unwrap()) as usize)
            };
            let identity_len = take(&mut cursor)?;
            let blob_len = take(&mut cursor)?;
            let identity_end = cursor + identity_len;
            let blob_end = identity_end + blob_len;
            if blob_end > bytes.len() {
                return Err(UNSUPPORTED);
            }
            entries.push(Entry {
                identity: bytes[cursor..identity_end].to_vec(),
                blob: bytes[identity_end..blob_end].to_vec(),
            });
            cursor = blob_end;
        }
        Ok(entries)
    }

    /// Full container size: header plus every complete entry.
    fn container_size(entries: &[Entry]) -> usize {
        HEADER_BYTES
            + entries
                .iter()
                .map(|entry| 16 + entry.identity.len() + entry.blob.len())
                .sum::<usize>()
    }

    /// Serialize whole entries up to `limit` bytes; a truncated container is a
    /// valid partial cache whose count matches its written entries.
    fn encode(&self, limit: usize) -> Vec<u8> {
        let entries = self.entries.borrow();
        let mut out = Vec::new();
        let mut written = 0u32;
        push(&mut out, &MAGIC);
        push(&mut out, &VERSION.to_le_bytes());
        push(&mut out, &self.identity);
        push(&mut out, &written.to_le_bytes());
        for entry in entries.iter() {
            let bytes = 16 + entry.identity.len() + entry.blob.len();
            if out.len() + bytes > limit {
                break;
            }
            push(&mut out, &(entry.identity.len() as u64).to_le_bytes());
            push(&mut out, &(entry.blob.len() as u64).to_le_bytes());
            push(&mut out, &entry.identity);
            push(&mut out, &entry.blob);
            written += 1;
        }
        out[28..32].copy_from_slice(&written.to_le_bytes());
        out
    }

    /// # Safety
    /// Live cache; exclusive mutation/export access; writable output storage.
    pub(in crate::foundation) unsafe fn data(
        &self,
        size: *mut usize,
        data: *mut c_void,
    ) -> Result<(), Status> {
        if size.is_null() {
            return Err(INVALID);
        }
        if data.is_null() {
            // Query the maximum export size.
            unsafe { *size = Self::container_size(&self.entries.borrow()) };
            return Ok(());
        }
        let capacity = unsafe { *size };
        let out = self.encode(capacity);
        if out.len() > capacity {
            return Err(INVALID);
        }
        // SAFETY: caller storage covers the reported capacity; sizes disjoint.
        unsafe {
            ptr::copy_nonoverlapping(out.as_ptr(), data.cast::<u8>(), out.len());
            *size = out.len();
        }
        Ok(())
    }

    /// # Safety
    /// Live same-device caches; exclusive destination, no source mutation.
    pub(in crate::foundation) unsafe fn merge(
        &self,
        count: u32,
        sources: *const *mut ExecutableCache,
    ) -> Result<(), Status> {
        if count == 0 {
            return Ok(());
        }
        if sources.is_null() {
            return Err(INVALID);
        }
        // SAFETY: trusted source array of the declared count.
        let sources = unsafe { slice::from_raw_parts(sources, count as usize) };
        let mut destination = self.entries.borrow_mut();
        for source in sources {
            // SAFETY: live cache pointers by caller contract.
            let source = unsafe { source.as_ref() }.ok_or(INVALID)?;
            if std::ptr::eq(source, self) {
                return Err(INVALID);
            }
            let entries = source.entries.borrow();
            for entry in entries.iter() {
                if !destination
                    .iter()
                    .any(|existing| existing.identity == entry.identity)
                {
                    let mut identity = Vec::new();
                    let mut blob = Vec::new();
                    identity.try_reserve_exact(entry.identity.len()).map_err(|_| OUT_OF_MEMORY)?;
                    blob.try_reserve_exact(entry.blob.len()).map_err(|_| OUT_OF_MEMORY)?;
                    identity.extend_from_slice(&entry.identity);
                    blob.extend_from_slice(&entry.blob);
                    destination.push(Entry { identity, blob });
                }
            }
        }
        Ok(())
    }

    /// Advisory lookup of a cached blob for one preparation identity.
    pub(super) fn lookup(&self, identity: &[u8]) -> Option<Vec<u8>> {
        self.entries
            .borrow()
            .iter()
            .find(|entry| entry.identity == identity)
            .map(|entry| entry.blob.clone())
    }

    /// Advisory update of one preparation's cached blob.
    pub(super) fn store(&self, identity: Vec<u8>, blob: Vec<u8>) {
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.identity == identity)
        {
            entry.blob = blob;
            return;
        }
        entries.push(Entry { identity, blob });
    }
}

/// Canonical preparation identity: every byte that shapes the native pipeline
/// state. Entries compare exactly, so a cached blob is only offered to the
/// preparation it came from.
pub(super) fn identity(desc: &ExecutableDesc) -> Vec<u8> {
    let mut out = Vec::new();
    push(&mut out, &desc.kind.to_le_bytes());
    push(&mut out, &desc.shader_count.to_le_bytes());
    for shader in unsafe {
        slice::from_raw_parts(desc.shaders, desc.shader_count as usize)
    } {
        push(&mut out, &shader.stage.to_le_bytes());
        push(&mut out, &shader.format.to_le_bytes());
        push(&mut out, &(shader.code.size as u64).to_le_bytes());
        push(&mut out, unsafe {
            slice::from_raw_parts(shader.code.data.cast::<u8>(), shader.code.size)
        });
        if !shader.entry.is_null() {
            let length = unsafe { std::ffi::CStr::from_ptr(shader.entry) }
                .to_bytes_with_nul()
                .len();
            push(&mut out, unsafe {
                slice::from_raw_parts(shader.entry.cast::<u8>(), length)
            });
        }
        for record in [
            shader.interface_metadata,
            shader.specialization,
            shader.subgroup,
        ] {
            push_record(&mut out, record);
        }
    }
    push(&mut out, &desc.dynamic_state.to_le_bytes());
    for record in [desc.static_state, desc.requirements] {
        push_record(&mut out, record);
    }
    out
}

/// Serialize one record (the header byte_size covers the complete record).
fn push_record(out: &mut Vec<u8>, record: *const Record) {
    if record.is_null() {
        push(out, &0u64.to_le_bytes());
        return;
    }
    // SAFETY: complete records behind the pointers by caller contract.
    let size = unsafe { (*record).byte_size } as usize;
    // SAFETY: the record header declares its complete extent.
    let bytes = unsafe { slice::from_raw_parts(record.cast::<u8>(), size) };
    push(out, &(bytes.len() as u64).to_le_bytes());
    push(out, bytes);
}
