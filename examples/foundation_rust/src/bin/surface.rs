//! Bounded hidden-window setup probe; no rendering or presentation acceptance.
//! GLFW is an application dependency, not part of the OGPU implementation.
use libloading::Library;
use ogpu::foundation::{rust as gpu, types::*};
use std::{
    ffi::{c_char, c_void, CStr},
    ptr,
};

struct Window {
    handle: *mut c_void,
    display: *mut c_void,
    surface: *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    terminate: unsafe extern "C" fn(),
    _library: Library,
}
impl Window {
    fn create() -> Self {
        // Deliberate executable-library selection, never accept untrusted paths.
        let path = std::env::var_os("OGPU_GLFW_LIBRARY").unwrap_or_else(|| "libglfw.so.3".into());
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
            let display = *library
                .get::<unsafe extern "C" fn() -> *mut c_void>(b"glfwGetWaylandDisplay\0")
                .unwrap();
            let surface = *library
                .get::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(b"glfwGetWaylandWindow\0")
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
            init_hint(0x00050003, 0x00060003); // GLFW_PLATFORM = WAYLAND, no fallback.
            assert_ne!(init(), 0, "GLFW Wayland initialization failed");
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
                c"OGPU surface setup probe".as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            assert!(!owner.handle.is_null(), "create hidden GLFW Wayland window");
            owner.display = display();
            owner.surface = surface(owner.handle);
            assert!(!owner.display.is_null() && !owner.surface.is_null());
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
    let mut q = Query {
        header: Record::new::<Query>(kind),
        data: ptr::null_mut(),
        capacity: 0,
        count: 0,
        element_size: size_of::<T>() as u32,
        reserved: 0,
    };
    unsafe { gpu::surface_query(adapter, surface, &mut q) }.unwrap();
    // A real application retries count/capacity races. Bound this probe as well.
    for _ in 0..8 {
        let mut values = vec![T::default(); q.count as usize];
        q.capacity = q.count;
        q.data = values.as_mut_ptr().cast();
        match unsafe { gpu::surface_query(adapter, surface, &mut q) } {
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
    let window = Window::create();
    let discovery = gpu::discover_presentation(PRESENTATION_WAYLAND).unwrap();
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .expect("modern foundation adapter");
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
    let headless = gpu::discover().unwrap();
    let other = &gpu::adapters(&headless)[0];
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
    println!("SURFACE_SETUP_PASS no_swapchain=1 no_render=1 no_present=1");
}
fn main() {
    run();
}
