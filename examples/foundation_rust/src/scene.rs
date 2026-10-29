//! Application policy: three explicit frame slots, replacement and bounded oracle.
use super::{query, query_mode, Window};
use ogpu::foundation::{rust as gpu, types::*};
use std::{
    mem::ManuallyDrop,
    ptr,
    time::{Duration, Instant},
};
#[path = "scene_resources.rs"]
mod resources;
use resources::*;

#[derive(Clone, Copy)]
struct Camera {
    x: f32,
    y: f32,
    zoom: f32,
    lod: u32,
}
impl Camera {
    fn bounds(self, near: bool) -> [f32; 4] {
        let b = if near {
            [-0.35, -0.4, 0.75, 0.45]
        } else {
            [-0.8, -0.7, 0.65, 0.7]
        };
        [
            b[0] * self.zoom + self.x,
            b[1] * self.zoom + self.y,
            b[2] * self.zoom + self.x,
            b[3] * self.zoom + self.y,
        ]
    }
}
#[repr(C)]
struct Root {
    vertices: u64,
    image: u32,
    sampler: u32,
    lod: f32,
    overlay: u32,
}
const _: () = assert!(
    size_of::<Root>() == 24
        && std::mem::offset_of!(Root, image) == 8
        && std::mem::offset_of!(Root, sampler) == 12
        && std::mem::offset_of!(Root, lod) == 16
        && std::mem::offset_of!(Root, overlay) == 20
);

struct Frame {
    arena: gpu::ArenaHandle,
    acquire: gpu::BinaryHandle,
    ready: gpu::BinaryHandle,
    presented: gpu::FenceHandle,
    completion: u64,
    present_pending: bool,
    geometry: Buffer,
    depth: Image,
    readback: Option<Buffer>,
    check: Option<(Camera, [u32; 2], u32)>,
}
impl Frame {
    unsafe fn new(
        d: gpu::DeviceHandle,
        adapter: &ogpu::foundation::Adapter,
        domain: u32,
        size: [u32; 2],
        check: bool,
    ) -> gpu::Result<Self> {
        unsafe {
            Ok(Self {
                arena: gpu::arena_create(
                    d,
                    &ArenaDesc {
                        header: Record::new::<ArenaDesc>(ARENA_DESC),
                        domain,
                        list_capacity: 1,
                        level: 0,
                        reserved: 0,
                    },
                )?,
                acquire: gpu::binary_create(d)?,
                ready: gpu::binary_create(d)?,
                presented: gpu::fence_create(d, false)?,
                completion: 0,
                present_pending: false,
                geometry: Buffer::new(d, adapter, 512, USAGE_STORAGE | 8, 16)?,
                depth: Image::new(d, adapter, 14, size, 1, 32, 32, 2)?,
                readback: if check {
                    Some(Buffer::new(
                        d,
                        adapter,
                        (row_bytes(size[0]) * size[1] as usize + 64) as u64,
                        USAGE_COPY_DST,
                        4,
                    )?)
                } else {
                    None
                },
                check: None,
            })
        }
    }
    unsafe fn retire(&mut self, timeline: gpu::TimelineHandle) -> gpu::Result<usize> {
        unsafe {
            if self.completion != 0 {
                gpu::timeline_wait(timeline, self.completion, 10_000_000_000)?;
                self.completion = 0;
            }
            if self.present_pending {
                gpu::fence_wait(self.presented, 10_000_000_000)?;
                gpu::fence_reset(self.presented)?;
                self.present_pending = false;
            }
            let mut checked = 0;
            if let Some((camera, size, format)) = self.check.take() {
                let readback = self.readback.as_ref().unwrap();
                gpu::memory_invalidate(readback.span())?;
                let bytes = std::slice::from_raw_parts(
                    readback.mapping.data.cast::<u8>(),
                    readback.size as usize,
                );
                checked = check_pixels(bytes, camera, size, format)?;
            }
            gpu::arena_reset(self.arena)?;
            Ok(checked)
        }
    }
    unsafe fn destroy(self) {
        unsafe {
            gpu::arena_destroy(self.arena);
            self.depth.destroy();
            self.geometry.destroy();
            if let Some(readback) = self.readback {
                readback.destroy();
            }
            gpu::binary_destroy(self.acquire);
            gpu::binary_destroy(self.ready);
            gpu::fence_destroy(self.presented);
        }
    }
}
struct Generation {
    chain: gpu::SwapchainHandle,
    images: Vec<gpu::ImageHandle>,
    views: Vec<gpu::ViewHandle>,
    size: [u32; 2],
    format: u32,
}
impl Generation {
    unsafe fn new(
        d: gpu::DeviceHandle,
        adapter: &ogpu::foundation::Adapter,
        surface: gpu::SurfaceHandle,
        size: [u32; 2],
        old: Option<gpu::SwapchainHandle>,
        check: bool,
    ) -> gpu::Result<Self> {
        let caps: Vec<SurfaceCapabilities> =
            query_mode(adapter, surface, Some(1), SURFACE_CAPABILITIES);
        let caps = caps[0];
        let formats: Vec<SurfaceFormat> = query(adapter, surface, SURFACE_FORMATS);
        let format = formats
            .iter()
            .find(|f| [3, 5].contains(&f.format) && f.color_space == 0)
            .ok_or(UNSUPPORTED)?;
        let size = [0, 1].map(|i| {
            if caps.current_extent[i] == u32::MAX {
                size[i].clamp(caps.min_extent[i], caps.max_extent[i])
            } else {
                caps.current_extent[i]
            }
        });
        let modes = [1];
        unsafe {
            let chain = gpu::swapchain_create(
                d,
                &SwapchainDesc {
                    header: Record::new::<SwapchainDesc>(SWAPCHAIN_DESC),
                    surface: surface.as_raw(),
                    old_swapchain: old.map_or(ptr::null_mut(), |c| c.as_raw()),
                    format: format.format,
                    color_space: format.color_space,
                    extent: size,
                    layer_count: 1,
                    min_image_count: caps.min_image_count.max(3),
                    usage: IMAGE_COLOR_ATTACHMENT | if check { IMAGE_COPY_SRC } else { 0 },
                    transform: caps.current_transform,
                    alpha: caps.supported_alpha & caps.supported_alpha.wrapping_neg(),
                    present_mode: modes[0],
                    clipped: u32::from(!check),
                    domain_count: 0,
                    mode_count: 1,
                    domains: ptr::null(),
                    modes: modes.as_ptr(),
                },
            )?;
            let mut count = 0;
            gpu::swapchain_images(chain, &mut [], &mut count)?;
            let mut pointers = vec![ptr::null_mut(); count as usize];
            gpu::swapchain_images(chain, &mut pointers, &mut count)?;
            let images: Vec<_> = pointers
                .into_iter()
                .map(|p| gpu::ImageHandle::from_raw(p).unwrap())
                .collect();
            let mut views = Vec::new();
            for &image in &images {
                views.push(gpu::view_create(
                    image,
                    &ViewDesc {
                        header: Record::new::<ViewDesc>(VIEW_DESC),
                        format: format.format,
                        dimension: 2,
                        usage: IMAGE_COLOR_ATTACHMENT as u32,
                        component_mapping: [0; 4],
                        range: range(ASPECT_COLOR, 1),
                    },
                )?);
            }
            Ok(Self {
                chain,
                images,
                views,
                size,
                format: format.format,
            })
        }
    }
    unsafe fn destroy(self) {
        unsafe {
            for view in self.views {
                gpu::view_destroy(view);
            }
            gpu::swapchain_destroy(self.chain);
        }
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn record_frame(
    frame: &mut Frame,
    generation: &Generation,
    index: u32,
    texture: &Texture,
    pipelines: &[gpu::ExecutableHandle; 2],
    camera: Camera,
    barriers: &mut Scratch,
    rendering: &mut Scratch,
) -> gpu::Result<gpu::ListHandle> {
    unsafe {
        let vertices = frame.geometry.mapping.data.cast::<f32>();
        for surface in 0..2 {
            let b = camera.bounds(surface == 1);
            for (corner, [x, y]) in [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]
                .into_iter()
                .enumerate()
            {
                for (k, v) in [x, y, if surface == 0 { 0.7 } else { 0.3 }, 1.]
                    .into_iter()
                    .enumerate()
                {
                    vertices.add(surface * 16 + corner * 4 + k).write(v);
                }
            }
        }
        // Near first, far second: the pixel oracle must rely on depth rejection,
        // rather than accidentally passing through painter's-order rendering.
        let indices = [4u32, 5, 6, 4, 6, 7, 0, 1, 2, 0, 2, 3];
        ptr::copy_nonoverlapping(
            indices.as_ptr(),
            frame.geometry.mapping.data.cast::<u8>().add(256).cast(),
            indices.len(),
        );
        gpu::memory_flush(frame.geometry.span())?;
        if let Some(readback) = &frame.readback {
            ptr::write_bytes(readback.mapping.data, 0xa5, readback.size as usize);
            gpu::memory_flush(readback.span())?;
        }
        let list = gpu::commands_begin(frame.arena, &recording())?;
        let image = generation.images[index as usize];
        transition(
            list,
            image,
            range(ASPECT_COLOR, 1),
            STATE_UNDEFINED,
            COLOR_STATE,
            COLOR,
            COLOR,
            0,
            COLOR_WRITE,
            barriers,
        );
        transition(
            list,
            frame.depth.image,
            range(2, 1),
            STATE_UNDEFINED,
            DEPTH_STATE,
            DEPTH,
            DEPTH,
            DEPTH_WRITE,
            DEPTH_WRITE,
            barriers,
        );
        for binding in &texture.bindings {
            gpu::bind_heap(list, binding);
        }
        gpu::bind_indices(list, frame.geometry.memory.span(256, 48), 32);
        for (overlay, &pipeline) in pipelines.iter().enumerate() {
            let color = Attachment {
                view: generation.views[index as usize].as_raw(),
                resolve_view: ptr::null_mut(),
                state: COLOR_STATE,
                resolve_state: 0,
                load_op: if overlay == 0 { 1 } else { 0 },
                store_op: 0,
                resolve_mode: 0,
                clear: ClearValue {
                    f32: [0., 0., 0., 1.],
                },
            };
            let depth = Attachment {
                view: frame.depth.view.as_raw(),
                resolve_view: ptr::null_mut(),
                state: DEPTH_STATE,
                resolve_state: 0,
                load_op: if overlay == 0 { 1 } else { 0 },
                store_op: 0,
                resolve_mode: 0,
                clear: ClearValue {
                    f32: [1., 0., 0., 0.],
                },
            };
            let [w, h] = generation.size;
            gpu::render_begin(
                list,
                &RenderDesc {
                    header: Record::new::<RenderDesc>(RENDER_DESC),
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                    layers: 1,
                    view_mask: 0,
                    samples: 1,
                    flags: 0,
                    color_count: 1,
                    colors: &color,
                    depth: &depth,
                    stencil: ptr::null(),
                    scratch: rendering.span(),
                },
            );
            gpu::bind_executable(list, pipeline);
            gpu::set_graphics_state(
                list,
                &ViewportState {
                    header: Record::new::<ViewportState>(VIEWPORT_STATE),
                    x: 0.,
                    y: 0.,
                    width: w as f32,
                    height: h as f32,
                    min_depth: 0.,
                    max_depth: 1.,
                    scissor_x: if overlay == 0 { 0 } else { (w / 4) as i32 },
                    scissor_y: if overlay == 0 { 0 } else { (h / 3) as i32 },
                    scissor_width: if overlay == 0 { w } else { w / 2 },
                    scissor_height: if overlay == 0 { h } else { h / 3 },
                },
            );
            let root = Root {
                vertices: gpu::memory_address(frame.geometry.memory.span(0, 128))?,
                image: 0,
                sampler: 0,
                lod: camera.lod as f32,
                overlay: overlay as u32,
            };
            gpu::set_inline(
                list,
                VERTEX | FRAGMENT,
                0,
                std::slice::from_raw_parts(ptr::from_ref(&root).cast(), size_of::<Root>()),
            );
            gpu::draw_indexed(
                list,
                &DrawDesc {
                    count: if overlay == 0 { 12 } else { 6 },
                    instances: 1,
                    first: if overlay == 0 { 0 } else { 6 },
                    first_instance: 0,
                    vertex_offset: 0,
                },
            );
            gpu::render_end(list);
            if overlay == 0 {
                gpu::barrier(
                    list,
                    &dependency(
                        COLOR | DEPTH,
                        COLOR | DEPTH,
                        COLOR_WRITE | DEPTH_WRITE,
                        4096 | COLOR_WRITE | 16384 | DEPTH_WRITE,
                        barriers,
                    ),
                );
            }
        }
        if let Some(readback) = &frame.readback {
            transition(
                list,
                image,
                range(ASPECT_COLOR, 1),
                COLOR_STATE,
                STATE_COPY_SRC,
                COLOR,
                STAGE_TRANSFER,
                COLOR_WRITE,
                ACCESS_COPY_READ,
                barriers,
            );
            gpu::copy_from_image(
                list,
                readback.memory.span(0, readback.size - 64),
                image,
                &ImageCopy {
                    region: ImageRegion {
                        aspect: ASPECT_COLOR,
                        mip: 0,
                        first_layer: 0,
                        layer_count: 1,
                        offset: Offset { x: 0, y: 0, z: 0 },
                        extent: Extent {
                            x: generation.size[0],
                            y: generation.size[1],
                            z: 1,
                        },
                    },
                    row_pitch: row_bytes(generation.size[0]) as u64,
                    slice_pitch: 0,
                    state: STATE_COPY_SRC,
                    reserved: 0,
                },
            );
            gpu::barrier(
                list,
                &dependency(
                    STAGE_TRANSFER,
                    STAGE_HOST,
                    ACCESS_COPY_WRITE,
                    ACCESS_HOST_READ,
                    barriers,
                ),
            );
            transition(
                list,
                image,
                range(ASPECT_COLOR, 1),
                STATE_COPY_SRC,
                STATE_PRESENT,
                STAGE_TRANSFER,
                0,
                ACCESS_COPY_READ,
                0,
                barriers,
            );
            frame.check = Some((camera, generation.size, generation.format));
        } else {
            transition(
                list,
                image,
                range(ASPECT_COLOR, 1),
                COLOR_STATE,
                STATE_PRESENT,
                COLOR,
                0,
                COLOR_WRITE,
                0,
                barriers,
            );
        }
        gpu::commands_end(list)
    }
}

fn row_bytes(width: u32) -> usize {
    (width as usize * 4).next_multiple_of(256)
}

fn check_pixels(bytes: &[u8], camera: Camera, [w, h]: [u32; 2], format: u32) -> gpu::Result<usize> {
    let pitch = row_bytes(w);
    let size = pitch * h as usize;
    if bytes.len() != size + 64 || bytes[size..].iter().any(|b| *b != 0xa5) {
        return Err(INTERNAL_ERROR);
    }
    let mut checked = 0;
    for y in (3..h).step_by(11) {
        for x in (3..w).step_by(11) {
            let nx = (x as f32 + 0.5) / w as f32 * 2. - 1.;
            let ny = (y as f32 + 0.5) / h as f32 * 2. - 1.;
            let mut color = [0., 0., 0., 1.];
            let mut skip = false;
            for near in [false, true] {
                let b = camera.bounds(near);
                if [nx - b[0], nx - b[2], ny - b[1], ny - b[3]]
                    .iter()
                    .any(|v| v.abs() < 0.005)
                {
                    skip = true;
                }
                if nx > b[0] && nx < b[2] && ny > b[1] && ny < b[3] {
                    let side = 8 >> camera.lod;
                    let u = (nx - b[0]) / (b[2] - b[0]) * side as f32;
                    let v = (ny - b[1]) / (b[3] - b[1]) * side as f32;
                    if u.fract() < 0.001
                        || v.fract() < 0.001
                        || u.fract() > 0.999
                        || v.fract() > 0.999
                    {
                        skip = true;
                    }
                    color = texel(u as u32, v as u32, camera.lod).map(|c| c as f32 / 255.);
                    if near {
                        color[0] *= 0.5;
                        color[2] *= 0.5;
                    }
                }
            }
            let far = camera.bounds(false);
            if x >= w / 4
                && x < w / 4 + w / 2
                && y >= h / 3
                && y < h / 3 + h / 3
                && nx > far[0]
                && nx < far[2]
                && ny > far[1]
                && ny < far[3]
            {
                for c in 0..3 {
                    color[c] = color[c] * 0.5 + [0.25, 0.5, 1.][c] * 0.5;
                }
            }
            if skip {
                continue;
            }
            let mut expected = color.map(|c| (c * 255.).round() as i32);
            if format == 5 {
                expected.swap(0, 2);
            }
            let at = y as usize * pitch + x as usize * 4;
            if (0..4).any(|c| (bytes[at + c] as i32 - expected[c]).abs() > 2) {
                eprintln!(
                    "scene oracle ({x},{y}) actual={:?} expected={expected:?}",
                    &bytes[at..at + 4]
                );
                return Err(INTERNAL_ERROR);
            }
            checked += 1;
        }
    }
    if checked < 100 {
        return Err(INTERNAL_ERROR);
    }
    Ok(checked)
}

fn suspended(extent: [u32; 2], paused: bool) -> bool {
    paused || extent.contains(&0)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Interactive,
    Check,
    Smoke,
}

pub unsafe fn run(mode: Mode) -> gpu::Result<()> {
    let check = mode == Mode::Check;
    let bounded = mode != Mode::Interactive;
    // No automatic GPU teardown on an uncertain error. Main exits the executable.
    let discovery = gpu::discover_presentation(PRESENTATION_WAYLAND)?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let mut window = ManuallyDrop::new(Window::create());
    window.resize([640, 480]);
    window.show_and_poll();
    let surface = unsafe {
        gpu::surface_wayland_create(
            adapter,
            &WaylandSurfaceDesc {
                header: Record::new::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC),
                display: window.display,
                surface: window.surface,
            },
        )?
    };
    let supported: Vec<SurfaceQueue> = query(adapter, surface, SURFACE_QUEUES);
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| {
            q.flags & GRAPHICS != 0
                && supported
                    .iter()
                    .any(|p| p.domain == q.domain && p.supported != 0)
        })
        .ok_or(UNSUPPORTED)?
        .domain;
    let d = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.,
        }],
        RASTER | PRESENTATION | (gpu::adapter_features(adapter).available & UNIFIED_IMAGES),
    )?;
    unsafe {
        let queue = gpu::device_queue(d, domain, 0).ok_or(INVALID)?;
        let done = gpu::timeline_create(d, 0)?;
        let mut generation =
            Generation::new(d, adapter, surface, window.framebuffer(), None, check)?;
        let mut pipelines = [
            raster(d, generation.format, false)?,
            raster(d, generation.format, true)?,
        ];
        let mut barriers = Scratch::new(gpu::barrier_scratch_requirements(0, 1)?);
        let mut rendering = Scratch::new(gpu::render_scratch_requirements(1)?);
        let mut submission = Scratch::new(gpu::submit_scratch_requirements(1, 1, 2)?);
        let mut presentation = Scratch::new(gpu::present_scratch_requirements(1, 1)?);
        let mut frames = Vec::new();
        for _ in 0..3 {
            frames.push(Frame::new(d, adapter, domain, generation.size, check)?);
        }
        let list = gpu::commands_begin(frames[0].arena, &recording())?;
        let (texture, upload) = Texture::new(d, adapter, list, &mut barriers)?;
        gpu::commands_end(list)?;
        let signal = |value| SyncPoint {
            point: Point {
                timeline: done.as_raw(),
                value,
            },
            stages: STAGE_ALL,
        };
        gpu::queue_submit(queue, &[list], &[], &[signal(1)], submission.span())?;
        gpu::timeline_wait(done, 1, 10_000_000_000)?;
        gpu::arena_reset(frames[0].arena)?;
        upload.destroy();
        let mut value = 1;
        let mut rendered = 0usize;
        let mut checked = 0;
        let mut recreations = 0;
        let mut peak_pending = 0;
        let mut camera = Camera {
            x: 0.,
            y: 0.,
            zoom: 1.,
            lod: 0,
        };
        let mut paused = false;
        let mut space_down = false;
        let started = Instant::now();
        let mut last = started;
        let mut dirty = false;
        let mut resize_requested = [false; 2];
        let mut resize_observed = [false; 2];
        let mut awaiting_resize = None;
        let mut injected_pause = false;
        println!("Scene: WASD pan, Q/E zoom, 1-4 mip level, Space pause, Escape close. Explicit MAILBOX, three slots.");
        loop {
            window.poll();
            if window.closing() || window.key(256) {
                break;
            }
            if bounded && started.elapsed() > Duration::from_secs(20) {
                return Err(TIMEOUT);
            }
            let now = Instant::now();
            let dt = (now - last).as_secs_f32().min(0.05);
            last = now;
            if !bounded {
                camera.x += (i32::from(window.key(68)) - i32::from(window.key(65))) as f32 * dt;
                camera.y += (i32::from(window.key(83)) - i32::from(window.key(87))) as f32 * dt;
                camera.zoom = (camera.zoom
                    + (i32::from(window.key(69)) - i32::from(window.key(81))) as f32 * dt)
                    .clamp(0.2, 2.);
                for mip in 0..4 {
                    if window.key(49 + mip) {
                        camera.lod = mip as u32;
                    }
                }
                let space = window.key(32);
                if space && !space_down {
                    paused = !paused;
                }
                space_down = space;
            }
            if bounded {
                for (i, threshold) in [6, 12].into_iter().enumerate() {
                    if rendered == threshold && !resize_requested[i] {
                        awaiting_resize = Some((i, window.framebuffer()));
                        window.resize(if i == 0 { [480, 320] } else { [640, 480] });
                        resize_requested[i] = true;
                    }
                }
                if rendered == 18 {
                    window.request_close();
                    continue;
                }
                camera.x = ([-0.08, 0., 0.08])[rendered % 3];
                camera.y = ([-0.04, 0.04])[rendered % 2];
                camera.zoom = ([0.9, 1., 1.1])[rendered % 3];
                camera.lod = (rendered % 4) as u32;
            }
            let mut size = window.framebuffer();
            if let Some((i, previous)) = awaiting_resize {
                if size == previous {
                    window.wait_events();
                    continue;
                }
                resize_observed[i] = true;
                awaiting_resize = None;
                dirty = true;
            }
            if bounded && rendered == 9 && !injected_pause {
                // Exercise the actual suspension branch, without claiming a native
                // minimization notification (not available through Wayland GLFW).
                size = [0, 0];
                injected_pause = true;
            }
            if suspended(size, paused) {
                for frame in &mut frames {
                    checked += frame.retire(done)?;
                }
                window.wait_events();
                continue;
            }
            if size != generation.size || dirty {
                for frame in &mut frames {
                    checked += frame.retire(done)?;
                }
                let next =
                    Generation::new(d, adapter, surface, size, Some(generation.chain), check)?;
                if next.format != generation.format {
                    for p in pipelines {
                        gpu::executable_destroy(p);
                    }
                    pipelines = [
                        raster(d, next.format, false)?,
                        raster(d, next.format, true)?,
                    ];
                }
                for frame in frames.drain(..) {
                    frame.destroy();
                }
                generation.destroy();
                generation = next;
                for _ in 0..3 {
                    frames.push(Frame::new(d, adapter, domain, generation.size, check)?);
                }
                recreations += 1;
                dirty = false;
            }
            let frame = &mut frames[rendered % 3];
            checked += frame.retire(done)?;
            let (index, suboptimal) =
                match gpu::acquire(generation.chain, 0, Some(frame.acquire), None) {
                    Ok(v) => v,
                    Err(NOT_READY | TIMEOUT) => {
                        window.wait_events();
                        continue;
                    }
                    Err(OUT_OF_DATE) => {
                        dirty = true;
                        continue;
                    }
                    Err(e) => return Err(e),
                };
            let list = record_frame(
                frame,
                &generation,
                index,
                &texture,
                &pipelines,
                camera,
                &mut barriers,
                &mut rendering,
            )?;
            value += 1;
            gpu::queue_submit_with_binary(
                queue,
                &[list],
                &[],
                &[signal(value)],
                &[frame.acquire.at(COLOR)],
                &[frame.ready.at(STAGE_ALL)],
                submission.span(),
            )?;
            frame.completion = value;
            let waits = [frame.ready.as_raw()];
            let entries = [PresentImage {
                swapchain: generation.chain.as_raw(),
                index,
                reserved: 0,
                fence: frame.presented.as_raw(),
            }];
            let present = gpu::queue_present(
                queue,
                &PresentDesc {
                    header: Record::new::<PresentDesc>(PRESENT_DESC),
                    wait_count: 1,
                    image_count: 1,
                    waits: waits.as_ptr(),
                    images: entries.as_ptr(),
                    results: ptr::null_mut(),
                    scratch: presentation.span(),
                },
            );
            match present {
                Ok(changed) => {
                    dirty |= changed;
                    frame.present_pending = true;
                }
                Err(OUT_OF_DATE) => {
                    dirty = true;
                    frame.present_pending = true;
                }
                Err(e) => return Err(e),
            }
            dirty |= suboptimal;
            rendered += 1;
            peak_pending = peak_pending.max(frames.iter().filter(|f| f.completion != 0).count());
        }
        let pending_at_close = frames.iter().filter(|f| f.completion != 0).count();
        for frame in &mut frames {
            checked += frame.retire(done)?;
        }
        for frame in frames {
            frame.destroy();
        }
        generation.destroy();
        texture.destroy();
        for p in pipelines {
            gpu::executable_destroy(p);
        }
        gpu::timeline_destroy(done);
        gpu::device_destroy(d);
        gpu::surface_destroy(surface);
        ManuallyDrop::drop(&mut window);
        if bounded
            && (rendered != 18
                || peak_pending != 3
                || pending_at_close == 0
                || recreations < 2
                || resize_observed != [true; 2]
                || !injected_pause)
        {
            return Err(INTERNAL_ERROR);
        }
        println!("SCENE_PASS frames={rendered} slots=3 peak_pending={peak_pending} resize={recreations} resize_observed={resize_observed:?} readback={check} checked_pixels={checked} pending_close={pending_at_close} synthetic_suspend={injected_pause}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suspension_and_oracle_reject_corruption() {
        assert!(suspended([0, 100], false));
        assert!(suspended([100, 0], false));
        assert!(suspended([100, 100], true));
        assert!(!suspended([100, 100], false));
        let c = Camera {
            x: 0.,
            y: 0.,
            zoom: 1.,
            lod: 0,
        };
        assert_eq!(
            check_pixels(&vec![0; 64 * 64 * 4 + 64], c, [64, 64], 3),
            Err(INTERNAL_ERROR)
        );
        let mut bytes = vec![0xa5; 64 * 64 * 4 + 64];
        bytes[..64 * 64 * 4].fill(0);
        assert_eq!(check_pixels(&bytes, c, [64, 64], 3), Err(INTERNAL_ERROR));
    }
    #[test]
    fn oracle_accepts_known_clear_and_detects_pixel_and_guard_changes() {
        let camera = Camera {
            x: 4.,
            y: 4.,
            zoom: 1.,
            lod: 0,
        };
        let mut bytes = vec![0xa5; 128 * 128 * 4 + 64];
        for pixel in bytes[..128 * 128 * 4].chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0, 0, 0, 255]);
        }
        assert!(check_pixels(&bytes, camera, [128, 128], 3).unwrap() >= 100);
        assert!(check_pixels(&bytes, camera, [128, 128], 5).unwrap() >= 100);
        let sample = (3 * 128 + 3) * 4;
        bytes[sample] = 16;
        assert_eq!(
            check_pixels(&bytes, camera, [128, 128], 3),
            Err(INTERNAL_ERROR)
        );
        bytes[sample] = 0;
        bytes[128 * 128 * 4] = 0;
        assert_eq!(
            check_pixels(&bytes, camera, [128, 128], 3),
            Err(INTERNAL_ERROR)
        );
        assert_eq!(texel(7, 7, 0), [172, 24, 12, 255]);
    }
}
