use super::{ArgumentCache, Buffer, DrawState, DrawStateCache, Raster};

// Identity-only addresses, never dereferenced; no driver is needed for these tests.
fn state() -> DrawState {
    DrawState {
        raster: std::ptr::dangling::<Raster>(),
        indices: Some((std::ptr::dangling::<Buffer>(), 16, 256, 1)),
    }
}

#[test]
fn identical_bindings_skip_independent_of_arguments() {
    let mut cache = DrawStateCache::default();
    assert!(cache.needs_bind(1, state()));
    assert!(!cache.needs_bind(512, state()));
    // Indirect record/count addresses and contents are intentionally not keys:
    // each native draw is still emitted with its own descriptor and identity.
}

#[test]
fn each_changed_binding_rebinds() {
    for change in 0..6 {
        let mut cache = DrawStateCache::default();
        assert!(cache.needs_bind(1, state()));
        let mut next = state();
        match change {
            0 => next.raster = std::ptr::null(),
            1 => next.indices.as_mut().unwrap().0 = std::ptr::null(),
            2 => next.indices.as_mut().unwrap().1 += 4,
            3 => next.indices.as_mut().unwrap().2 -= 4,
            4 => next.indices.as_mut().unwrap().3 = 0,
            5 => next.indices = None,
            _ => unreachable!(),
        }
        assert!(cache.needs_bind(1, next));
        assert!(cache.needs_bind(1, state()));
    }
}

#[test]
fn empty_ranges_do_not_install_state() {
    let mut cache = DrawStateCache::default();
    assert!(!cache.needs_bind(0, state()));
    assert!(cache.needs_bind(1, state()));
    let mut different = state();
    different.raster = std::ptr::null();
    assert!(!cache.needs_bind(0, different));
    assert!(!cache.needs_bind(1, state()));
}

#[test]
fn boundaries_and_new_recordings_rebind() {
    let mut cache = DrawStateCache::default();
    assert!(cache.needs_bind(1, state()));
    assert!(!cache.needs_bind(1, state()));
    cache.invalidate();
    assert!(!cache.needs_bind(0, state()));
    assert!(cache.needs_bind(1, state()));
    assert!(DrawStateCache::default().needs_bind(1, state()));
}

#[test]
fn argument_elision_compares_range_and_copied_bytes() {
    let mut cache = ArgumentCache::default();
    let a = [1, 2, 3, 4];
    let b = a;
    assert!(cache.needs_push(0, &a));
    assert!(!cache.needs_push(0, &b));
    assert!(!cache.needs_push(4, &[]));
    assert!(!cache.needs_push(0, &b));
    assert!(cache.needs_push(4, &a));
    assert!(cache.needs_push(0, &b));
    assert!(cache.needs_push(0, &[1, 2, 3, 5]));
    assert!(cache.needs_push(0, &a));
    assert!(ArgumentCache::default().needs_push(0, &a));
}

#[test]
fn argument_update_bounds_alignment_and_empty() {
    use crate::{contract::argument_range as range, INVALID_ARGUMENT, OUT_OF_RANGE};
    assert!(range(0, 256, 256).is_ok());
    assert!(range(252, 4, 256).is_ok());
    assert!(range(256, 0, 256).is_ok());
    assert!(range(0, 0, 0).is_ok());
    for (offset, bytes) in [(1, 4), (0, 3), (3, 0)] {
        assert_eq!(
            range(offset, bytes, 256).unwrap_err().status,
            INVALID_ARGUMENT
        );
    }
    for (offset, bytes) in [(256, 4), (260, 0), (u32::MAX - 3, 8), (4, usize::MAX - 3)] {
        assert_eq!(range(offset, bytes, 256).unwrap_err().status, OUT_OF_RANGE);
    }
}
