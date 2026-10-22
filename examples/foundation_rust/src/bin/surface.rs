//! Wayland setup/lifecycle probes and an interactive or bounded textured scene.
//! GLFW is an application dependency, not part of the OGPU implementation.
use libloading::Library;
use ogpu::foundation::{rust as gpu, types::*};
use std::{
    ffi::{c_char, c_void, CStr},
    ptr,
};
#[path = "../presentation.rs"]
mod presentation;
#[path = "../scene.rs"]
mod scene;

struct Window {
    handle: *mut c_void,
    display: *mut c_void,
    surface: *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    terminate: unsafe extern "C" fn(),
    _library: Library,
}
impl Window {
    fn framebuffer(&self) -> [u32; 2] {
        let (mut w, mut h) = (0, 0);
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32)>(
                    b"glfwGetFramebufferSize\0",
                )
                .unwrap()(self.handle, &mut w, &mut h);
        }
        [w.max(0) as u32, h.max(0) as u32]
    }
    fn resize(&self, size: [u32; 2]) {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(*mut c_void, i32, i32)>(b"glfwSetWindowSize\0")
                .unwrap()(self.handle, size[0] as i32, size[1] as i32);
        }
    }
    fn key(&self, key: i32) -> bool {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(*mut c_void, i32) -> i32>(b"glfwGetKey\0")
                .unwrap()(self.handle, key)
                == 1
        }
    }
    fn closing(&self) -> bool {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"glfwWindowShouldClose\0")
                .unwrap()(self.handle)
                != 0
        }
    }
    fn request_close(&self) {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(*mut c_void, i32)>(b"glfwSetWindowShouldClose\0")
                .unwrap()(self.handle, 1);
        }
    }
    fn wait_events(&self) {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(f64)>(b"glfwWaitEventsTimeout\0")
                .unwrap()(0.01);
        }
    }
    fn show_and_poll(&self) {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn(*mut c_void)>(b"glfwShowWindow\0")
                .unwrap()(self.handle);
            self.poll();
        }
    }
    fn poll(&self) {
        unsafe {
            self._library
                .get::<unsafe extern "C" fn()>(b"glfwPollEvents\0")
                .unwrap()();
        }
    }
    fn create() -> Self {
        // Deliberate executable-library selection, never accept untrusted paths.
        #[cfg(not(target_os = "windows"))]
        let default_library = "libglfw.so.3";
        #[cfg(target_os = "windows")]
        let default_library = "glfw3.dll";
        let path = std::env::var_os("OGPU_GLFW_LIBRARY").unwrap_or_else(|| default_library.into());
        unsafe {
            let library = Library::new(path).expect("load GLFW 3.4 (set OGPU_GLFW_LIBRARY)");
            let init = *library
                .get::<unsafe extern "C" fn() -> i32>(b"glfwInit\0")
                .unwrap();
            let init_hint = *library
                .get::<unsafe extern "C" fn(i32, i32)>(b"glfwInitHint\0")
                .unwrap();
            let hint = *library
                .get::<unsafe extern "C" fn(i32, i32)>(b"glfwWindowHint\0")
                .unwrap();
            let create = *library
                .get::<unsafe extern "C" fn(
                    i32,
                    i32,
                    *const c_char,
                    *mut c_void,
                    *mut c_void,
                ) -> *mut c_void>(b"glfwCreateWindow\0")
                .unwrap();
            // The surface record's provisional D3D12 binding: the window
            // handle travels in `surface` with `display` null; the Wayland
            // path carries its native pair.
            #[cfg(not(target_os = "windows"))]
            let display = *library
                .get::<unsafe extern "C" fn() -> *mut c_void>(b"glfwGetWaylandDisplay\0")
                .unwrap();
            #[cfg(target_os = "windows")]
            let display: *mut c_void = std::ptr::null_mut();
            #[cfg(not(target_os = "windows"))]
            let surface = *library
                .get::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(b"glfwGetWaylandWindow\0")
                .unwrap();
            #[cfg(target_os = "windows")]
            let surface = *library
                .get::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(b"glfwGetWin32Window\0")
                .unwrap();
            let destroy = *library
                .get::<unsafe extern "C" fn(*mut c_void)>(b"glfwDestroyWindow\0")
                .unwrap();
            let terminate = *library
                .get::<unsafe extern "C" fn()>(b"glfwTerminate\0")
                .unwrap();
            type ErrorCallback = Option<unsafe extern "C" fn(i32, *const c_char)>;
            let callback = *library
                .get::<unsafe extern "C" fn(ErrorCallback) -> ErrorCallback>(
                    b"glfwSetErrorCallback\0",
                )
                .unwrap();
            unsafe extern "C" fn error(code: i32, message: *const c_char) {
                if !message.is_null() {
                    eprintln!(
                        "GLFW {code}: {}",
                        unsafe { CStr::from_ptr(message) }.to_string_lossy()
                    );
                }
            }
            callback(Some(error));
            // GLFW_PLATFORM: WAYLAND on Linux, WIN32 on Windows; no fallback.
            #[cfg(not(target_os = "windows"))]
            init_hint(0x00050003, 0x00060003);
            #[cfg(target_os = "windows")]
            init_hint(0x00050003, 0x00060001);
            assert_ne!(init(), 0, "GLFW platform initialization failed");
            let mut owner = Self {
                handle: ptr::null_mut(),
                display: ptr::null_mut(),
                surface: ptr::null_mut(),
                destroy,
                terminate,
                _library: library,
            };
            hint(0x00022001, 0); // CLIENT_API = NO_API
            hint(0x00020004, 0); // VISIBLE = false; no interactive acceptance claim.
            owner.handle = create(
                320,
                240,
                c"OGPU presentation probe".as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            assert!(!owner.handle.is_null(), "create hidden GLFW window");
            #[cfg(not(target_os = "windows"))]
            {
                owner.display = display();
                owner.surface = surface(owner.handle);
                assert!(!owner.display.is_null() && !owner.surface.is_null());
            }
            #[cfg(target_os = "windows")]
            {
                // The provisional D3D12 binding carries the window handle in
                // `surface` with `display` null.
                owner.surface = surface(owner.handle);
                assert!(!owner.surface.is_null());
            }
            owner
        }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                (self.destroy)(self.handle);
            }
            (self.terminate)();
        }
    }
}
struct Surface(gpu::SurfaceHandle);
struct Device(gpu::DeviceHandle);
impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            gpu::device_destroy(self.0);
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            gpu::surface_destroy(self.0);
        }
    }
}
fn query<T: Copy + Default>(
    adapter: &ogpu::foundation::Adapter,
    surface: gpu::SurfaceHandle,
    kind: u32,
) -> Vec<T> {
    query_mode(adapter, surface, None, kind)
}
fn query_mode<T: Copy + Default>(
    adapter: &ogpu::foundation::Adapter,
    surface: gpu::SurfaceHandle,
    mode: Option<u32>,
    kind: u32,
) -> Vec<T> {
    let mut q = Query {
        header: Record::new::<Query>(kind),
        data: ptr::null_mut(),
        capacity: 0,
        count: 0,
        element_size: size_of::<T>() as u32,
        reserved: 0,
    };
    let call = |q: &mut Query| unsafe {
        match mode {
            Some(mode) => gpu::surface_mode_query(adapter, surface, mode, q),
            None => gpu::surface_query(adapter, surface, q),
        }
    };
    call(&mut q).unwrap();
    // A real application retries count/capacity races. Bound this probe as well.
    for _ in 0..8 {
        let mut values = vec![T::default(); q.count as usize];
        q.capacity = q.count;
        q.data = values.as_mut_ptr().cast();
        match call(&mut q) {
            Ok(()) => {
                values.truncate(q.count as usize);
                return values;
            }
            Err(CAPACITY) => continue,
            other => panic!("surface query {kind}: {other:?}"),
        }
    }
    panic!("surface query {kind} did not stabilize");
}

pub fn run() {
    let discovery = gpu::discover_presentation(PRESENTATION_WAYLAND).unwrap();
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .expect("modern foundation adapter");
    // Feature enabling is ordinary device creation, before any native window.
    // No presentation queue is implicitly selected; this setup-only device runs
    // no GPU work and can be destroyed immediately.
    assert_ne!(gpu::adapter_features(adapter).available & PRESENTATION, 0);
    let queue = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.flags & 2 != 0)
        .unwrap();
    let requests = [QueueRequest {
        domain: queue.domain,
        count: 1,
        priority: 1.0,
    }];
    for features in [0, PRESENTATION] {
        let device = Device(gpu::device_create(adapter, &requests, features).unwrap());
        let mut info = FeatureInfo::default();
        let mut q = Query {
            header: Record::new::<Query>(FEATURES),
            data: ptr::from_mut(&mut info).cast(),
            capacity: 1,
            count: 0,
            element_size: size_of::<FeatureInfo>() as u32,
            reserved: 0,
        };
        unsafe { gpu::device_query(device.0, &mut q) }.unwrap();
        assert_eq!(info.enabled, features);
        assert_ne!(info.available & PRESENTATION, 0);
        drop(device);
    }
    let window = Window::create();
    let desc = WaylandSurfaceDesc {
        header: Record::new::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC),
        display: window.display,
        surface: window.surface,
    };
    let surface = Surface(unsafe { gpu::surface_wayland_create(adapter, &desc) }.unwrap());
    let queues: Vec<SurfaceQueue> = query(adapter, surface.0, SURFACE_QUEUES);
    assert!(queues.iter().any(|q| q.supported != 0));
    let caps: Vec<SurfaceCapabilities> = query(adapter, surface.0, SURFACE_CAPABILITIES);
    let formats: Vec<SurfaceFormat> = query(adapter, surface.0, SURFACE_FORMATS);
    let modes: Vec<u32> = query(adapter, surface.0, SURFACE_PRESENT_MODES);
    let support = gpu::presentation_support(adapter).unwrap();
    assert_eq!(caps.len(), 1);
    assert!(caps[0].min_image_count > 0);
    assert!(formats.iter().any(|f| f.format != 0));
    assert!(modes.contains(&2)); // Native FIFO baseline.
    for &mode in &modes {
        let per_mode: Vec<SurfaceCapabilities> =
            query_mode(adapter, surface.0, Some(mode), SURFACE_CAPABILITIES);
        let scaling: Vec<SurfaceScaling> =
            query_mode(adapter, surface.0, Some(mode), SURFACE_SCALING);
        let compatible: Vec<u32> =
            query_mode(adapter, surface.0, Some(mode), SURFACE_COMPATIBLE_MODES);
        assert_eq!(per_mode.len(), 1);
        assert_eq!(scaling.len(), 1);
        assert!(compatible.contains(&mode));
        assert!(compatible.iter().all(|m| modes.contains(m)));
        if compatible.len() > 1 {
            let mut value = u32::MAX;
            let mut q = Query {
                header: Record::new::<Query>(SURFACE_COMPATIBLE_MODES),
                data: ptr::from_mut(&mut value).cast(),
                capacity: 1,
                count: 91,
                element_size: size_of::<u32>() as u32,
                reserved: 0,
            };
            assert_eq!(
                unsafe { gpu::surface_mode_query(adapter, surface.0, mode, &mut q) },
                Err(CAPACITY)
            );
            assert_eq!(q.count as usize, compatible.len());
            assert_eq!(value, u32::MAX);
        }
        println!("SURFACE_MODE mode={mode} min_images={} max_images={} scaling={scaling:?} compatible={compatible:?}", per_mode[0].min_image_count, per_mode[0].max_image_count);
    }
    assert_eq!(
        support,
        PresentationSupport {
            swapchain: 1,
            swapchain_maintenance: 1
        }
    );
    println!(
        "SURFACE_SETUP queues={queues:?} capabilities={caps:?} formats={formats:?} modes={modes:?}"
    );

    // CAPACITY changes count only; bad schema and wrong instance change nothing.
    let sentinel = SurfaceQueue {
        domain: 99,
        supported: 99,
    };
    let mut output = [sentinel];
    let mut q = Query {
        header: Record::new::<Query>(SURFACE_QUEUES),
        data: output.as_mut_ptr().cast(),
        capacity: 1,
        count: 91,
        element_size: size_of::<SurfaceQueue>() as u32,
        reserved: 0,
    };
    if queues.len() > 1 {
        assert_eq!(
            unsafe { gpu::surface_query(adapter, surface.0, &mut q) },
            Err(CAPACITY)
        );
        assert_eq!(q.count as usize, queues.len());
        assert_eq!(output, [sentinel]);
    }
    q.element_size -= 1;
    q.count = 91;
    assert_eq!(
        unsafe { gpu::surface_query(adapter, surface.0, &mut q) },
        Err(INVALID)
    );
    assert_eq!(q.count, 91);
    q.header.kind = SURFACE_CAPABILITIES;
    q.element_size = size_of::<SurfaceCapabilities>() as u32;
    q.capacity = 0;
    assert_eq!(
        unsafe { gpu::surface_mode_query(adapter, surface.0, u32::MAX, &mut q) },
        Err(UNSUPPORTED)
    );
    assert_eq!(q.count, 91);
    q.header.kind = SURFACE_SCALING;
    q.element_size = size_of::<SurfaceScaling>() as u32;
    assert_eq!(
        unsafe { gpu::surface_query(adapter, surface.0, &mut q) },
        Err(UNSUPPORTED)
    );
    q.header.kind = SURFACE_FORMATS;
    q.element_size = size_of::<SurfaceFormat>() as u32;
    assert_eq!(
        unsafe { gpu::surface_mode_query(adapter, surface.0, modes[0], &mut q) },
        Err(UNSUPPORTED)
    );
    assert_eq!(q.count, 91);
    let headless = gpu::discover().unwrap();
    let other = &gpu::adapters(&headless)[0];
    assert_eq!(gpu::adapter_features(other).available & PRESENTATION, 0);
    let headless_queue = gpu::adapter_queues(other)
        .iter()
        .find(|q| q.flags & 2 != 0)
        .unwrap();
    let requests = [QueueRequest {
        domain: headless_queue.domain,
        count: 1,
        priority: 1.0,
    }];
    assert_eq!(
        gpu::device_create(other, &requests, PRESENTATION),
        Err(UNSUPPORTED)
    );
    let _ = gpu::presentation_support(other).unwrap(); // No window/WSI opt-in needed.
    assert_eq!(
        unsafe { gpu::surface_query(other, surface.0, &mut q) },
        Err(INVALID)
    );
    assert_eq!(q.count, 91);
    assert_eq!(
        unsafe { gpu::surface_wayland_create(other, &desc) },
        Err(UNSUPPORTED)
    );
    drop(headless);
    // Surface keeps its instance alive, but does not keep the native window.
    drop(discovery);
    drop(surface);
    drop(window);
    println!(
        "SURFACE_SETUP_PASS device_enable=1 mode_queries=1 no_swapchain=1 no_render=1 no_present=1"
    );
}
fn main() {
    if let Some(mode) = std::env::args().find_map(|arg| match arg.as_str() {
        "--scene" => Some(scene::Mode::Interactive),
        "--scene-check" => Some(scene::Mode::Check),
        "--scene-smoke" => Some(scene::Mode::Smoke),
        _ => None,
    }) {
        if let Err(status) = unsafe { scene::run(mode) } {
            eprintln!("scene failed: {status}; retaining potentially pending resources until process exit");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--present") {
        // On uncertain execution failure, exit without unwinding GPU/native-window
        // owners. This probe is not a general application's recovery framework.
        if let Err(status) = unsafe { presentation::run() } {
            eprintln!("presentation probe failed: {status}");
            std::process::exit(1);
        }
    } else {
        run();
    }
}
