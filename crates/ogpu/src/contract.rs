//! Backend-independent rules. Native encoders and synchronization stay in their backends.
use crate::{Error, INVALID_ARGUMENT, OUT_OF_RANGE, UNSUPPORTED};
use std::rc::Rc;

pub(crate) fn range(size: usize, offset: u64, length: u64) -> Result<(usize, usize), Error> {
    let bad = || Error::new(OUT_OF_RANGE, "Buffer range out of bounds");
    let offset = usize::try_from(offset).map_err(|_| bad())?;
    let length = usize::try_from(length).map_err(|_| bad())?;
    if offset > size || length > size - offset {
        return Err(bad());
    }
    Ok((offset, length))
}

pub(crate) fn copy_ranges(
    source_size: usize,
    source_offset: u64,
    destination_size: usize,
    destination_offset: u64,
    length: u64,
    same_buffer: bool,
) -> Result<(usize, usize, usize), Error> {
    let (source, length) = range(source_size, source_offset, length)?;
    let (destination, _) = range(destination_size, destination_offset, length as u64)?;
    // Empty copies are validated no-ops, including identical empty ranges.
    if length != 0 && same_buffer && source < destination + length && destination < source + length
    {
        return Err(Error::new(INVALID_ARGUMENT, "Copy ranges overlap"));
    }
    Ok((source, destination, length))
}

pub(crate) fn root_size(bytes: u32, limit: u64) -> Result<(), Error> {
    if bytes % 4 != 0 || u64::from(bytes) > limit {
        return Err(Error::new(INVALID_ARGUMENT, "Invalid root-data size"));
    }
    Ok(())
}

pub(crate) fn dispatch(
    groups: [u32; 3],
    limits: [u32; 3],
    bytes: usize,
    expected: u32,
) -> Result<(), Error> {
    if bytes != expected as usize
        || groups
            .into_iter()
            .zip(limits)
            .any(|(n, limit)| n == 0 || n > limit)
    {
        return Err(Error::new(
            INVALID_ARGUMENT,
            "Invalid dispatch size or argument byte count",
        ));
    }
    Ok(())
}

pub(crate) fn access(mask: u32, graphics: bool) -> Result<(), Error> {
    if mask == 0 || mask & !511 != 0 {
        return Err(Error::new(INVALID_ARGUMENT, "Invalid access mask"));
    }
    if !graphics && mask & (4 | 8 | 16 | 128 | 256) != 0 {
        return Err(Error::new(
            UNSUPPORTED,
            "Graphics access on a compute-only queue",
        ));
    }
    Ok(())
}

pub(crate) fn compile_flags(flags: u32) -> Result<bool, Error> {
    if flags & !1 != 0 {
        return Err(Error::new(INVALID_ARGUMENT, "Invalid command-list flags"));
    }
    Ok(flags & 1 != 0)
}

pub(crate) fn recording<T>(value: &mut Option<T>) -> Result<&mut T, Error> {
    value.as_mut().ok_or_else(submitted)
}

pub(crate) fn take_recording<T>(value: &mut Option<T>) -> Result<T, Error> {
    value.take().ok_or_else(submitted)
}

fn submitted() -> Error {
    Error::new(INVALID_ARGUMENT, "Batch already submitted")
}

pub(crate) fn retain<T>(objects: &mut Vec<Rc<T>>, object: Rc<T>) {
    if !objects.iter().any(|other| Rc::ptr_eq(other, &object)) {
        objects.push(object);
    }
}

/// Owns partial preparation, then accepted work, then only its durable outcome.
/// Only the native backend can establish terminal completion. Transient query
/// failures must NOT call finish; Drop of the backend receipt must drain first.
pub(crate) struct Submission<R, O> {
    pub(crate) resources: Option<R>,
    pub(crate) pending: bool,
    pub(crate) outcome: Option<O>,
}

impl<R, O> Submission<R, O> {
    pub(crate) fn preparing(resources: R) -> Self {
        Self {
            resources: Some(resources),
            pending: false,
            outcome: None,
        }
    }
    pub(crate) fn accept(&mut self) {
        assert!(self.resources.is_some() && !self.pending && self.outcome.is_none());
        self.pending = true;
    }
    pub(crate) fn finish(&mut self, outcome: O) {
        assert!(self.pending);
        self.pending = false;
        self.outcome = Some(outcome);
        self.retire();
    }
    pub(crate) fn retire(&mut self) {
        assert!(!self.pending, "Cannot retire in-flight resources");
        drop(self.resources.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_and_byte_copies() {
        assert_eq!(range(16, 16, 0).unwrap(), (16, 0));
        assert!(range(16, 16, 1).is_err());
        assert!(range(16, u64::MAX, 1).is_err());
        assert!(range(16, 1, u64::MAX).is_err());
        assert!(copy_ranges(16, 1, 16, 2, 3, true).is_err());
        assert!(copy_ranges(16, 2, 16, 1, 3, true).is_err());
        assert_eq!(copy_ranges(16, 1, 16, 4, 3, true).unwrap(), (1, 4, 3));
        assert!(copy_ranges(16, 1, 16, 2, 3, false).is_ok());
        assert!(copy_ranges(16, 16, 16, 16, 0, true).is_ok());
        assert!(copy_ranges(16, 17, 16, 16, 0, true).is_err());
    }
    #[test]
    fn root_grid_and_access_limits() {
        for size in [0, 4, 4096] {
            root_size(size, 4096).unwrap();
        }
        for size in [1, 4097, 4100] {
            assert!(root_size(size, 4096).is_err());
        }
        dispatch([2, 3, 4], [2, 3, 4], 8, 8).unwrap();
        assert!(dispatch([0, 1, 1], [2; 3], 8, 8).is_err());
        assert!(dispatch([3, 1, 1], [2; 3], 8, 8).is_err());
        assert!(dispatch([1; 3], [2; 3], 4, 8).is_err());
        access(1 | 2 | 32 | 64, false).unwrap();
        assert_eq!(access(256, false).unwrap_err().status, UNSUPPORTED);
        assert_eq!(access(512, true).unwrap_err().status, INVALID_ARGUMENT);
        assert!(access(0, true).is_err());
    }
    #[test]
    fn recording_is_consumed_once_even_if_submission_fails() {
        let mut recording_state = Some(vec![1]);
        recording(&mut recording_state).unwrap().push(2);
        assert_eq!(take_recording(&mut recording_state).unwrap(), [1, 2]);
        assert!(recording(&mut recording_state).is_err());
        assert!(take_recording(&mut recording_state).is_err());
    }
    #[test]
    fn terminal_success_and_failure_retire_once_and_cache_outcome() {
        for result in [Ok(()), Err("native error")] {
            let object = Rc::new(());
            let weak = Rc::downgrade(&object);
            let mut retained = vec![];
            retain(&mut retained, object.clone());
            retain(&mut retained, object);
            assert_eq!(retained.len(), 1);
            let mut receipt = Submission::preparing(retained);
            receipt.accept();
            // A pending poll / transient native error leaves ownership unchanged.
            assert!(receipt.pending && weak.upgrade().is_some());
            receipt.finish(result);
            assert!(weak.upgrade().is_none());
            assert!(!receipt.pending && receipt.resources.is_none());
            assert_eq!(receipt.outcome, Some(result));
            receipt.retire();
            assert_eq!(receipt.outcome, Some(result));
        }
    }
    #[test]
    fn unaccepted_preparation_releases_resources() {
        let object = Rc::new(());
        let weak = Rc::downgrade(&object);
        let receipt = Submission::<_, ()>::preparing(object);
        drop(receipt);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    #[should_panic(expected = "Cannot retire in-flight resources")]
    fn pending_resources_cannot_be_retired() {
        let mut receipt = Submission::<_, ()>::preparing(());
        receipt.accept();
        receipt.retire();
    }
}
