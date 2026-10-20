//! Explicit query storage: native query heaps with caller-owned reset, issue
//! and resolve order. D3D12 has no native query reset (a slot is implicitly
//! reusable after its resolve), so `queries_reset` validates the range and
//! records no native command. Result values are native 64-bit query data; the
//! contract's 32-bit result width would require a format conversion this
//! profile does not hide, so resolves require `RESULT_64` at the native
//! record stride.
use std::cell::Cell;

use super::*;
use crate::foundation::types::{
    QueryPoolDesc, Span, Status, INVALID, PIPELINE_STATISTICS, PRECISE_OCCLUSION, RASTER,
    UNSUPPORTED, QUERY_POOL_DESC,
};

/// Native result record strides: one u64 per query, or the full native
/// pipeline-statistics struct (11 counters) per query.
const RESULT_BYTES: u64 = 8;
const STATISTICS_COUNTERS: u32 = 11;
const STATISTICS_BYTES: u64 = RESULT_BYTES * 11;

pub struct QueryPool {
    pub(super) device: *const Device,
    pub(super) heap: d3d::ID3D12QueryHeap,
    /// 1 timestamp, 2 occlusion, 3 pipeline statistics.
    pub(super) kind: u32,
    pub(super) statistics: u64,
    count: u32,
}

impl QueryPool {
    /// # Safety
    /// Live device with declared query features; device outlives the pool.
    pub(in crate::foundation) fn create(
        d: &Device,
        desc: &QueryPoolDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<QueryPoolDesc>(QUERY_POOL_DESC)?;
        if desc.count == 0 {
            return Err(INVALID);
        }
        if desc.kind != 3 && desc.statistics != 0 {
            return Err(UNSUPPORTED);
        }
        if desc.kind == 3 {
            if desc.statistics == 0 {
                return Err(INVALID);
            }
            if desc.statistics & !d.snapshot.query_limits.statistics != 0 {
                return Err(UNSUPPORTED);
            }
            // Native pipeline-statistics resolution writes every counter; a
            // subset mask would change the record layout and stays visible.
            if desc.statistics.count_ones() != STATISTICS_COUNTERS {
                return Err(UNSUPPORTED);
            }
        }
        let heap_type = match desc.kind {
            1 if d
                .snapshot
                .queues
                .iter()
                .any(|q| q.count > 0 && q.timestamp_bits != 0) =>
            {
                d3d::D3D12_QUERY_HEAP_TYPE_TIMESTAMP
            }
            2 if d.snapshot.features.enabled & RASTER != 0 => {
                d3d::D3D12_QUERY_HEAP_TYPE_OCCLUSION
            }
            3 if d.snapshot.features.enabled & PIPELINE_STATISTICS != 0 => {
                d3d::D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS
            }
            _ => return Err(UNSUPPORTED),
        };
        let heap_desc = d3d::D3D12_QUERY_HEAP_DESC {
            Type: heap_type,
            Count: desc.count,
            NodeMask: 0,
        };
        // SAFETY: well-formed heap description; the output adopts one reference.
        let mut heap: Option<d3d::ID3D12QueryHeap> = None;
        unsafe { d.device.CreateQueryHeap(&heap_desc, &mut heap) }
            .map_err(super::map_error)?;
        let heap = heap.ok_or(BACKEND_ERROR)?;
        Ok(Box::new(Self {
            device: d,
            heap,
            kind: desc.kind,
            statistics: desc.statistics,
            count: desc.count,
        }))
    }

    pub(super) fn range(&self, d: &Device, first: u32, count: u32) -> Result<(), Status> {
        if self.device != d || count == 0 || first >= self.count || count > self.count - first {
            return Err(INVALID);
        }
        Ok(())
    }
}

/// One active query per type per list, matching the contract's begin/end rules.
#[derive(Clone, Copy)]
pub(super) struct ActiveQuery {
    pub(super) pool: *const QueryPool,
    pub(super) index: u32,
    pub(super) inside: bool,
}

impl super::List {
    /// # Safety
    /// Live recording slot and pool; recorded lifetime rules apply.
    pub(in crate::foundation) fn queries_reset(
        &self,
        pool: &QueryPool,
        first: u32,
        count: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        self.outside_render()?;
        pool.range(d, first, count)?;
        if self.active_query(pool, first, count) {
            return Err(INVALID);
        }
        // D3D12 query heaps have no native reset; slots are implicitly
        // reusable after resolve. The contract's reset discipline is the
        // caller's ordering obligation and records nothing here.
        Ok(())
    }

    pub(in crate::foundation) fn query(
        &self,
        pool: &QueryPool,
        index: u32,
        begin: bool,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        pool.range(d, index, 1)?;
        if !matches!(pool.kind, 2 | 3) {
            return Err(INVALID);
        }
        let queue_flags = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?
            .flags;
        if (pool.kind == 2 || pool.statistics & !1024 != 0) && queue_flags & super::GRAPHICS == 0
            || pool.statistics & 1024 != 0 && queue_flags & super::COMPUTE == 0
        {
            return Err(UNSUPPORTED);
        }
        // Begin flags: 0, or PRECISE for occlusion. D3D12 occlusion queries
        // count samples exactly natively, which satisfies either promise.
        if flags & !1 != 0 {
            return Err(UNSUPPORTED);
        }
        if flags & 1 != 0
            && (pool.kind != 2 || d.snapshot.features.enabled & PRECISE_OCCLUSION == 0)
        {
            return Err(UNSUPPORTED);
        }
        let query_type = if pool.kind == 2 {
            d3d::D3D12_QUERY_TYPE_OCCLUSION
        } else {
            d3d::D3D12_QUERY_TYPE_PIPELINE_STATISTICS
        };
        let active = &self.active_queries[(pool.kind - 2) as usize];
        if begin {
            if active.get().is_some() {
                return Err(INVALID);
            }
            // SAFETY: recording slot and live pool heap.
            unsafe {
                self.command.BeginQuery(&pool.heap, query_type, index);
            }
            active.set(Some(ActiveQuery {
                pool,
                index,
                inside: self.rendering.get(),
            }));
        } else {
            let previous = active.get().ok_or(INVALID)?;
            if previous.pool != pool
                || previous.index != index
                || previous.inside != self.rendering.get()
            {
                return Err(INVALID);
            }
            // SAFETY: recording slot and live pool heap.
            unsafe {
                self.command.EndQuery(&pool.heap, query_type, index);
            }
            active.set(None);
        }
        Ok(())
    }

    pub(in crate::foundation) fn timestamp(
        &self,
        pool: &QueryPool,
        index: u32,
        scope: u64,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        pool.range(d, index, 1)?;
        if pool.kind != 1 {
            return Err(INVALID);
        }
        // The scope must lower to one supported native stage bit; HOST, zero
        // and multi-bit lowerings are rejected rather than approximated.
        let native = super::commands::stages(scope)?;
        if native.0.count_ones() != 1 {
            return Err(INVALID);
        }
        if d.snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?
            .timestamp_bits
            == 0
        {
            return Err(UNSUPPORTED);
        }
        // SAFETY: recording slot and live pool heap.
        unsafe {
            self.command
                .EndQuery(&pool.heap, d3d::D3D12_QUERY_TYPE_TIMESTAMP, index);
        }
        Ok(())
    }

    /// # Safety
    /// Destination span is live and writable through all recorded uses.
    pub(in crate::foundation) unsafe fn queries_resolve(
        &self,
        pool: &QueryPool,
        first: u32,
        count: u32,
        dst: Span,
        stride: u64,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        pool.range(d, first, count)?;
        if self.active_query(pool, first, count) {
            return Err(INVALID);
        }
        // Result flags: WAIT is the native in-queue resolve ordering; RESULT_64
        // is required because native query data is 64-bit. AVAILABILITY and
        // PARTIAL have no native form and stay visible.
        if flags & !5 != 0 {
            return Err(UNSUPPORTED);
        }
        if flags & 1 == 0 {
            return Err(UNSUPPORTED);
        }
        let record = if pool.kind == 3 {
            STATISTICS_BYTES
        } else {
            RESULT_BYTES
        };
        // Native resolution packs results with no stride control.
        if count > 1 && stride != record {
            return Err(UNSUPPORTED);
        }
        let size = u64::from(count - 1)
            .checked_mul(if count > 1 { stride } else { 0 })
            .and_then(|v| v.checked_add(record))
            .ok_or(INVALID)?;
        if dst.size < size {
            return Err(INVALID);
        }
        let memory = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) =
            memory.command_range(d, self.domain, dst.offset, dst.size, 2)?;
        if offset % RESULT_BYTES != 0 {
            return Err(INVALID);
        }
        let query_type = match pool.kind {
            1 => d3d::D3D12_QUERY_TYPE_TIMESTAMP,
            2 => d3d::D3D12_QUERY_TYPE_OCCLUSION,
            _ => d3d::D3D12_QUERY_TYPE_PIPELINE_STATISTICS,
        };
        // SAFETY: recording slot and live pool/destination for recorded uses.
        unsafe {
            self.command.ResolveQueryData(
                &pool.heap,
                query_type,
                first,
                count,
                buffer,
                offset,
            );
        }
        Ok(())
    }

    /// Whether any active query overlaps the pool range.
    fn active_query(&self, pool: &QueryPool, first: u32, count: u32) -> bool {
        self.active_queries.iter().any(|active| {
            active.get().is_some_and(|active| {
                active.pool == pool && active.index >= first && active.index - first < count
            })
        })
    }
}

/// Active-query cells for the two begin/end query types (occlusion, statistics).
pub(super) fn active_cells() -> [Cell<Option<ActiveQuery>>; 2] {
    [Cell::new(None), Cell::new(None)]
}
