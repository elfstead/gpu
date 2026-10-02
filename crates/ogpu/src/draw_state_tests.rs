use super::{Buffer, DrawState, DrawStateCache, Raster};

// Identity-only addresses, never dereferenced; no driver is needed for these tests.
fn state(root: &[u8]) -> DrawState<'_> {
    DrawState {
        raster: std::ptr::dangling::<Raster>(),
        indices: Some((std::ptr::dangling::<Buffer>(), 16, 256, 1)),
        root,
    }
}

#[test]
fn identical_state_and_copied_root_skip_bindings() {
    let mut cache = DrawStateCache::default();
    let a = [1, 2, 3, 4];
    let b = a;
    assert!(cache.needs_bind(1, state(&a)));
    assert!(!cache.needs_bind(512, state(&b)));
    // Indirect record/count addresses and contents are intentionally not keys:
    // each native draw is still emitted with its own descriptor and identity.
}

#[test]
fn each_changed_binding_rebinds() {
    let root = [1, 2, 3, 4];
    let other_root = [1, 2, 3, 5];
    for change in 0..8 {
        let mut cache = DrawStateCache::default();
        assert!(cache.needs_bind(1, state(&root)));
        let mut next = state(&root);
        match change {
            0 => next.raster = std::ptr::null(),
            1 => next.indices.as_mut().unwrap().0 = std::ptr::null(),
            2 => next.indices.as_mut().unwrap().1 += 4,
            3 => next.indices.as_mut().unwrap().2 -= 4,
            4 => next.indices.as_mut().unwrap().3 = 0,
            5 => next.indices = None,
            6 => next.root = &other_root,
            7 => next.root = &[],
            _ => unreachable!(),
        }
        assert!(cache.needs_bind(1, next));
        assert!(cache.needs_bind(1, state(&root)));
    }
}

#[test]
fn empty_ranges_do_not_install_state() {
    let mut cache = DrawStateCache::default();
    assert!(!cache.needs_bind(0, state(&[9])));
    assert!(cache.needs_bind(1, state(&[9])));
    assert!(!cache.needs_bind(0, state(&[8])));
    assert!(!cache.needs_bind(1, state(&[9])));
    assert!(cache.needs_bind(1, state(&[8])));
}

#[test]
fn boundaries_and_new_recordings_rebind() {
    let mut cache = DrawStateCache::default();
    assert!(cache.needs_bind(1, state(&[])));
    assert!(!cache.needs_bind(1, state(&[])));
    cache.invalidate();
    assert!(!cache.needs_bind(0, state(&[])));
    assert!(cache.needs_bind(1, state(&[])));
    assert!(DrawStateCache::default().needs_bind(1, state(&[])));
}
