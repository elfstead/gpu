//! Borrowed native viewport/scissor arrays. No active-count or slot tracking.
use super::*;
pub(super) fn range(
    first: u32,
    count: u32,
    flags: u32,
    enabled: FeatureBits,
    limits: &ViewportLimits,
) -> Result<(), Status> {
    if flags & !SET_COUNT != 0 {
        return Err(UNSUPPORTED);
    }
    if flags & SET_COUNT != 0 && (first != 0 || count == 0) {
        return Err(INVALID);
    }
    let end = first.checked_add(count).ok_or(INVALID)?;
    if end > limits.max_viewports || end > 1 && enabled & MULTI_VIEWPORT == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn counts(
    state: Option<ViewportCounts>,
    dynamic: u64,
    enabled: FeatureBits,
    limits: &ViewportLimits,
) -> Result<[u32; 2], Status> {
    let mut counts = state.map_or([1, 1], |s| [s.viewports, s.scissors]);
    for (v, bit) in counts
        .iter_mut()
        .zip([DYNAMIC_VIEWPORT_COUNT_STATE, DYNAMIC_SCISSOR_COUNT_STATE])
    {
        if dynamic & bit != 0 {
            *v = 0;
        } else {
            range(0, *v, SET_COUNT, enabled, limits)?;
        }
    }
    if counts[0] != 0 && counts[1] != 0 && counts[0] != counts[1] {
        return Err(INVALID);
    }
    Ok(counts)
}
pub(super) fn viewport(
    v: &Viewport,
    p: &GraphicsLimits,
    enabled: FeatureBits,
) -> Result<(), Status> {
    if ![v.x, v.y, v.width, v.height, v.min_depth, v.max_depth]
        .iter()
        .all(|f| f.is_finite())
        || v.width <= 0.0
        || v.height == 0.0
        || enabled & UNRESTRICTED_DEPTH_RANGE == 0
            && (!(0.0..=1.0).contains(&v.min_depth) || !(0.0..=1.0).contains(&v.max_depth))
    {
        return Err(INVALID);
    }
    if v.width > p.max_viewport[0] as f32
        || v.height.abs() > p.max_viewport[1] as f32
        || v.x < p.viewport_bounds[0]
        || v.x + v.width > p.viewport_bounds[1]
        || v.y.min(v.y + v.height) < p.viewport_bounds[0]
        || v.y.max(v.y + v.height) > p.viewport_bounds[1]
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn scissor(s: &Scissor) -> Result<(), Status> {
    if s.x < 0
        || s.y < 0
        || u64::from(s.width) + s.x as u64 > i32::MAX as u64
        || u64::from(s.height) + s.y as u64 > i32::MAX as u64
    {
        return Err(INVALID);
    }
    Ok(())
}
const _: () = {
    assert!(size_of::<Viewport>() == size_of::<vk::VkViewport>());
    assert!(align_of::<Viewport>() == align_of::<vk::VkViewport>());
    assert!(std::mem::offset_of!(Viewport, x) == std::mem::offset_of!(vk::VkViewport, x));
    assert!(std::mem::offset_of!(Viewport, y) == std::mem::offset_of!(vk::VkViewport, y));
    assert!(std::mem::offset_of!(Viewport, width) == std::mem::offset_of!(vk::VkViewport, width));
    assert!(std::mem::offset_of!(Viewport, height) == std::mem::offset_of!(vk::VkViewport, height));
    assert!(
        std::mem::offset_of!(Viewport, min_depth) == std::mem::offset_of!(vk::VkViewport, minDepth)
    );
    assert!(
        std::mem::offset_of!(Viewport, max_depth) == std::mem::offset_of!(vk::VkViewport, maxDepth)
    );
    assert!(size_of::<Scissor>() == size_of::<vk::VkRect2D>());
    assert!(align_of::<Scissor>() == align_of::<vk::VkRect2D>());
    assert!(
        std::mem::offset_of!(Scissor, x)
            == std::mem::offset_of!(vk::VkRect2D, offset) + std::mem::offset_of!(vk::VkOffset2D, x)
    );
    assert!(
        std::mem::offset_of!(Scissor, y)
            == std::mem::offset_of!(vk::VkRect2D, offset) + std::mem::offset_of!(vk::VkOffset2D, y)
    );
    assert!(
        std::mem::offset_of!(Scissor, width)
            == std::mem::offset_of!(vk::VkRect2D, extent)
                + std::mem::offset_of!(vk::VkExtent2D, width)
    );
    assert!(
        std::mem::offset_of!(Scissor, height)
            == std::mem::offset_of!(vk::VkRect2D, extent)
                + std::mem::offset_of!(vk::VkExtent2D, height)
    );
};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_and_ranges_preserve_explicit_modes_and_independent_enabling() {
        let limits = ViewportLimits {
            max_viewports: 16,
            subpixel_bits: 8,
        };
        assert_eq!(range(0, 1, SET_COUNT, FeatureBits::ZERO, &limits), Ok(()));
        assert_eq!(
            range(0, 2, SET_COUNT, FeatureBits::ZERO, &limits),
            Err(UNSUPPORTED)
        );
        assert_eq!(range(0, 2, SET_COUNT, MULTI_VIEWPORT, &limits), Ok(()));
        assert_eq!(range(15, 1, 0, MULTI_VIEWPORT, &limits), Ok(()));
        assert_eq!(
            range(1, 1, SET_COUNT, MULTI_VIEWPORT, &limits),
            Err(INVALID)
        );
        assert_eq!(
            range(0, 0, SET_COUNT, MULTI_VIEWPORT, &limits),
            Err(INVALID)
        );
        assert_eq!(range(16, 0, 0, MULTI_VIEWPORT, &limits), Ok(()));
        assert_eq!(range(u32::MAX, 1, 0, MULTI_VIEWPORT, &limits), Err(INVALID));
        assert_eq!(range(16, 1, 0, MULTI_VIEWPORT, &limits), Err(UNSUPPORTED));
        let s = ViewportCounts {
            header: Record::new::<ViewportCounts>(VIEWPORT_COUNTS),
            viewports: 99,
            scissors: 1,
        };
        assert_eq!(
            counts(
                Some(s),
                DYNAMIC_VIEWPORT_COUNT_STATE,
                FeatureBits::ZERO,
                &limits
            ),
            Ok([0, 1])
        );
        assert_eq!(
            counts(Some(s), 0, MULTI_VIEWPORT, &limits),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            counts(
                Some(ViewportCounts { viewports: 2, ..s }),
                0,
                MULTI_VIEWPORT,
                &limits
            ),
            Err(INVALID)
        );
    }
    #[test]
    fn flipped_viewport_reversed_depth_and_empty_scissors_are_native_values() {
        let p = GraphicsLimits {
            max_viewport: [1024, 1024],
            viewport_bounds: [-2048.0, 2048.0],
            ..Default::default()
        };
        let v = Viewport {
            x: -10.0,
            y: 100.0,
            width: 20.0,
            height: -100.0,
            min_depth: 1.0,
            max_depth: 0.0,
        };
        assert_eq!(viewport(&v, &p, FeatureBits::ZERO), Ok(()));
        assert_eq!(
            viewport(&Viewport { height: 0.0, ..v }, &p, FeatureBits::ZERO),
            Err(INVALID)
        );
        assert_eq!(
            viewport(&Viewport { x: f32::NAN, ..v }, &p, FeatureBits::ZERO),
            Err(INVALID)
        );
        assert_eq!(
            viewport(
                &Viewport {
                    max_depth: 1.1,
                    ..v
                },
                &p,
                FeatureBits::ZERO
            ),
            Err(INVALID)
        );
        assert_eq!(scissor(&Scissor::default()), Ok(()));
        assert_eq!(
            scissor(&Scissor {
                x: -1,
                ..Default::default()
            }),
            Err(INVALID)
        );
        assert_eq!(
            scissor(&Scissor {
                x: i32::MAX,
                width: 1,
                ..Default::default()
            }),
            Err(INVALID)
        );
    }
}
