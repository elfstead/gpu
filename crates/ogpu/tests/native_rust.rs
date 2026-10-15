#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/main.rs"]
mod consumer;

#[test]
#[ignore = "requires modern Vulkan; native Rust consumer, no C adapter calls"]
fn gpu_native_rust_consumer() {
    consumer::run().unwrap();
}

/// Compile-check native equivalents and require a disposition for every exported
/// C operation. This is surface coverage, not evidence that every feature executed.
#[test]
fn native_surface_covers_current_c_operations() {
    use ogpu::foundation::rust as gpu;
    use std::collections::BTreeSet;
    let mut covered = BTreeSet::new();
    macro_rules! operations {
        ($($name:ident),* $(,)?) => { $(
            let _ = gpu::$name;
            assert!(covered.insert(stringify!($name)));
        )* };
    }
    operations!(
        adapter_query,
        arena_create,
        arena_destroy,
        arena_reserve,
        arena_reset,
        arena_trim,
        barrier,
        barrier_scratch_requirements,
        bind_executable,
        bind_heap,
        bind_indices,
        bind_vertices,
        blit_image,
        clear_image,
        commands_begin,
        commands_cancel,
        commands_end,
        copy_from_image,
        copy_image,
        copy_memory,
        copy_to_image,
        descriptor_scratch_requirements,
        device_create,
        device_destroy,
        device_query,
        device_queue,
        dispatch,
        dispatch_indirect,
        draw,
        draw_indexed,
        draw_indexed_indirect,
        draw_indirect,
        executable_cache_create,
        executable_cache_data,
        executable_cache_destroy,
        executable_cache_merge,
        executable_create,
        executable_destroy,
        execute_lists,
        execute_scratch_requirements,
        fill_memory,
        image_bind,
        image_create,
        image_create_unbound,
        image_destroy,
        image_limits_query,
        image_requirements,
        inheritance_scratch_requirements,
        memory_address,
        memory_budget_query,
        memory_create,
        memory_create_dedicated_image,
        memory_destroy,
        memory_flush,
        memory_invalidate,
        memory_map,
        memory_requirements,
        memory_unmap,
        queries_reset,
        queries_resolve,
        query_begin,
        query_end,
        query_pool_create,
        query_pool_destroy,
        queue_submit,
        queue_submit_batch,
        render_begin,
        render_end,
        render_scratch_requirements,
        resolve_image,
        set_blend_constants,
        set_cull,
        set_depth_compare,
        set_depth_test,
        set_depth_write,
        set_front_face,
        set_graphics_state,
        set_inline,
        set_root,
        set_stencil_compare_mask,
        set_stencil_ops,
        set_stencil_reference,
        set_stencil_test,
        set_stencil_write_mask,
        split_acquire,
        split_create,
        split_destroy,
        split_release,
        split_reset,
        split_scratch_requirements,
        submit_batch_scratch_requirements,
        submit_scratch_requirements,
        timeline_create,
        timeline_destroy,
        timeline_poll,
        timeline_wait,
        timestamp,
        vertex_scratch_requirements,
        view_create,
        view_destroy,
        write_resource_descriptors,
        write_sampler_descriptors,
    );
    // Rust owns discovery with Box; adapters() provides len/get/iteration without
    // a second count/index API, and drop releases discovery without a GPU wait.
    let _ = gpu::discover;
    let _ = gpu::adapters;
    let _ = std::mem::drop::<Box<ogpu::foundation::Discovery>>;
    let _ = gpu::timeline_signal;
    covered.extend([
        "discovery_create",
        "discovery_count",
        "discovery_adapter",
        "discovery_destroy",
        "timeline_signal_host",
        // C-only table lookup; compile-check its native operations below.
        "presentation_api",
    ]);
    let _ = gpu::discover_presentation;
    let _ = gpu::presentation_support;
    let _ = gpu::surface_wayland_create;
    let _ = gpu::surface_destroy;
    let _ = gpu::surface_query;
    let header = include_str!("../../../include/ogpu_next.h");
    let declared: BTreeSet<_> = header
        .split("ogpu_next_")
        .skip(1)
        .filter_map(|tail| {
            let end = tail.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
            (tail.as_bytes().get(end) == Some(&b'(')).then_some(&tail[..end])
        })
        .collect();
    assert_eq!(
        covered, declared,
        "update both host interfaces for a new operation"
    );
    let presentation_table: BTreeSet<_> = header
        .split("typedef struct ogpu_next_presentation_functions {")
        .nth(1)
        .unwrap()
        .split("} ogpu_next_presentation_functions;")
        .next()
        .unwrap()
        .split("(*")
        .skip(1)
        .map(|tail| tail.split(')').next().unwrap())
        .collect();
    assert_eq!(
        presentation_table,
        BTreeSet::from([
            "discovery_create",
            "adapter_support",
            "surface_wayland_create",
            "surface_destroy",
            "surface_query"
        ]),
        "extend native Rust coverage when the optional table grows"
    );
}

#[test]
fn native_operations_do_not_call_c_adapters() {
    // This application legitimately calls GLFW's C API, but never OGPU's C ABI.
    assert!(
        !include_str!("../../../examples/foundation_rust/src/bin/surface.rs")
            .contains("ogpu_next_")
    );
    for source in [
        include_str!("../src/foundation/rust.rs"),
        include_str!("../src/foundation/rust/resources.rs"),
        include_str!("../src/foundation/rust/commands.rs"),
        include_str!("../src/foundation/rust/presentation.rs"),
        include_str!("../../../examples/foundation_rust/src/main.rs"),
        include_str!("../../../examples/foundation_rust/src/extended.rs"),
    ] {
        // Unit tests intentionally compare a C entry point; production does not.
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("ogpu_next_"));
        assert!(!production.contains("extern \"C\""));
    }
}
