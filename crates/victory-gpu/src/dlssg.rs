//! DLSS Frame Generation on VIctory's swap chain, through NVIDIA Streamline
//! (`VICTORY_DLSSG=<frames generated per real frame>`, 1 to 5; off by
//! default). The Streamline calls live in a small C++ bridge,
//! `victory_sl.dll` (`native/victory_sl`), loaded only when this is on, from
//! `VICTORY_SL_DIR` (default: beside our executable), which also holds
//! Streamline's own DLLs and `nvngx_dlssg.dll`.
//!
//! The inputs are what the title hands AMD's upscaler each frame
//! (`victory_hle::modules::ffx` records them through [`record`]): its depth
//! and motion vectors (title textures, read through their host copies at
//! present time), jitter, motion vector scale and camera. With manual
//! hooking our queue is made through Streamline's proxy device and the swap
//! chain through its proxy factory; DLSS-G then paces the frames itself.
//!
//! NVIDIA's runtime runs DLSS-G on RTX 40/50 cards; on others it reports it
//! unsupported unless something else (a third-party unlocker loaded into the
//! process) makes it run. Either way this logs what Streamline says.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D12::{D3D12_COMMAND_QUEUE_DESC, ID3D12CommandQueue, ID3D12Device};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW,
};
use windows::core::{Interface, PCSTR, PCWSTR};

/// What the title last handed its upscaler (title objects as addresses).
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub depth: usize,
    pub depth_state: u32,
    pub motion_vectors: usize,
    pub motion_state: u32,
    pub render: (u32, u32),
    pub upscale: (u32, u32),
    pub jitter: (f32, f32),
    pub motion_vector_scale: (f32, f32),
    pub near: f32,
    pub far: f32,
    pub fov_y: f32,
    pub reset: bool,
}

static INPUTS: Mutex<Option<Inputs>> = Mutex::new(None);
static RECORDED: AtomicU64 = AtomicU64::new(0);

/// Called for each upscale dispatch of the title.
pub fn record(inputs: Inputs) {
    RECORDED.fetch_add(1, Ordering::Relaxed);
    *INPUTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(inputs);
}

/// Frames to generate per real one, 0 if off.
pub fn frames() -> u32 {
    static N: OnceLock<u32> = OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("VICTORY_DLSSG").ok().and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(0).min(5)
    })
}

/// `VslFrame` of the bridge.
#[repr(C)]
struct Frame {
    depth: *mut c_void,
    depth_state: u32,
    mvec: *mut c_void,
    mvec_state: u32,
    render_width: u32,
    render_height: u32,
    output_width: u32,
    output_height: u32,
    jitter_x: f32,
    jitter_y: f32,
    mvec_scale_x: f32,
    mvec_scale_y: f32,
    near_z: f32,
    far_z: f32,
    fov_y: f32,
    depth_inverted: i32,
    reset: i32,
}

struct Bridge {
    init: unsafe extern "C" fn(PCWSTR, PCWSTR) -> i32,
    set_device: unsafe extern "C" fn(*mut c_void) -> i32,
    create_queue: unsafe extern "C" fn(*mut c_void, *const D3D12_COMMAND_QUEUE_DESC, *mut *mut c_void) -> i32,
    upgrade: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    check: unsafe extern "C" fn(u32, i32, *mut i32) -> i32,
    set_options: unsafe extern "C" fn(i32, u32, u32) -> i32,
    reflex_on: unsafe extern "C" fn() -> i32,
    frame_begin: unsafe extern "C" fn() -> i32,
    frame_inputs: unsafe extern "C" fn(*mut c_void, *const Frame) -> i32,
    present_start: unsafe extern "C" fn(),
    present_end: unsafe extern "C" fn(),
}

// SAFETY: plain function pointers into a DLL that stays loaded.
unsafe impl Send for Bridge {}
unsafe impl Sync for Bridge {}

fn dir() -> std::path::PathBuf {
    std::env::var_os("VICTORY_SL_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default()
    })
}

fn wide(s: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.as_os_str().encode_wide().chain([0]).collect()
}

/// The bridge, loaded and initialised once; `None` if off or it failed.
fn bridge() -> Option<&'static Bridge> {
    static BRIDGE: OnceLock<Option<Bridge>> = OnceLock::new();
    BRIDGE
        .get_or_init(|| {
            if frames() == 0 {
                return None;
            }
            let dir = dir();
            let path = wide(&dir.join("victory_sl.dll"));
            // SAFETY: a full path; the bridge's own DLLs are found beside it.
            let module: HMODULE = match unsafe { LoadLibraryExW(PCWSTR(path.as_ptr()), None, LOAD_WITH_ALTERED_SEARCH_PATH) } {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(dir = %dir.display(), error = %e, "dlssg: victory_sl.dll did not load; frame generation off");
                    return None;
                }
            };
            macro_rules! f {
                ($name:literal) => {{
                    // SAFETY: a name the bridge exports, as a C string.
                    let p = unsafe { GetProcAddress(module, PCSTR(concat!($name, "\0").as_ptr())) };
                    match p {
                        // SAFETY: the bridge's export has this signature.
                        Some(p) => unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, _>(p) },
                        None => {
                            tracing::warn!(name = $name, "dlssg: bridge export missing");
                            return None;
                        }
                    }
                }};
            }
            let b = Bridge {
                init: f!("vsl_init"),
                set_device: f!("vsl_set_device"),
                create_queue: f!("vsl_create_queue"),
                upgrade: f!("vsl_upgrade"),
                check: f!("vsl_check"),
                set_options: f!("vsl_set_options"),
                reflex_on: f!("vsl_reflex_on"),
                frame_begin: f!("vsl_frame_begin"),
                frame_inputs: f!("vsl_frame_inputs"),
                present_start: f!("vsl_present_start"),
                present_end: f!("vsl_present_end"),
            };
            let plugins = wide(&dir);
            let log = std::env::var_os("VICTORY_SL_LOG").map(std::path::PathBuf::from).map(|p| wide(&p));
            // SAFETY: NUL-terminated wide strings that live through the call.
            let r = unsafe { (b.init)(PCWSTR(plugins.as_ptr()), log.as_ref().map_or(PCWSTR::null(), |l| PCWSTR(l.as_ptr()))) };
            if r != 0 {
                tracing::warn!(result = r, "dlssg: slInit failed; frame generation off");
                return None;
            }
            tracing::info!(frames = frames(), dir = %dir.display(), "dlssg: Streamline is up");
            Some(b)
        })
        .as_ref()
}

/// Makes the host's direct queue, through Streamline when frame generation
/// is on (its proxy device must make the queue the swap chain presents on).
pub(crate) fn create_queue(device: &ID3D12Device, desc: &D3D12_COMMAND_QUEUE_DESC) -> windows::core::Result<ID3D12CommandQueue> {
    if let Some(b) = bridge() {
        // SAFETY: our live device.
        let set = unsafe { (b.set_device)(device.as_raw()) };
        let mut queue: *mut c_void = std::ptr::null_mut();
        // SAFETY: our device and description; `queue` receives a reference.
        let r = unsafe { (b.create_queue)(device.as_raw(), desc, &mut queue) };
        if set == 0 && r == 0 && !queue.is_null() {
            tracing::info!("dlssg: the host queue is made through Streamline");
            // SAFETY: a queue the call made, whose reference we now own.
            return Ok(unsafe { ID3D12CommandQueue::from_raw(queue) });
        }
        tracing::warn!(set, result = r, "dlssg: the queue could not be made through Streamline; a native one is used");
    }
    // SAFETY: plain creation.
    unsafe { device.CreateCommandQueue(desc) }
}

/// Replaces a DXGI factory by Streamline's proxy, so the swap chain made
/// from it is one DLSS-G drives.
pub(crate) fn upgrade<T: Interface>(object: T) -> T {
    let Some(b) = bridge() else { return object };
    let mut raw = object.into_raw();
    // SAFETY: `raw` holds our reference; the call swaps it for the proxy's.
    let r = unsafe { (b.upgrade)(&mut raw) };
    if r != 0 {
        tracing::warn!(result = r, "dlssg: slUpgradeInterface failed");
    }
    // SAFETY: whichever object `raw` now names, we own one reference to it.
    unsafe { T::from_raw(raw) }
}

/// After the swap chain is made: asks Streamline whether DLSS-G may run here
/// and turns it on.
pub(crate) fn swap_chain_ready(device: &ID3D12Device, width: u32, height: u32) {
    let Some(b) = bridge() else { return };
    // SAFETY: plain query.
    let luid = unsafe { device.GetAdapterLuid() };
    let mut max = 0;
    // SAFETY: plain calls into the bridge.
    let supported = unsafe { (b.check)(luid.LowPart, luid.HighPart, &mut max) };
    // SAFETY: as above.
    let reflex = unsafe { (b.reflex_on)() };
    let wanted = frames().min(if max > 0 { max as u32 } else { frames() });
    // SAFETY: as above.
    let set = unsafe { (b.set_options)(wanted as i32, width, height) };
    tracing::info!(supported, max_generated = max, reflex, set, frames = wanted, "dlssg: frame generation asked for");
}

/// The start of a frame's present (before its copy is recorded).
pub(crate) fn frame_begin() {
    if let Some(b) = bridge() {
        // SAFETY: plain call.
        unsafe { (b.frame_begin)() };
    }
}

/// Tags the title's depth and motion vectors and sets the camera constants,
/// recorded on `list` (Streamline copies the tagged textures there).
pub(crate) fn frame_inputs(list: &windows::Win32::Graphics::Direct3D12::ID3D12GraphicsCommandList, output: (u32, u32)) {
    let Some(b) = bridge() else { return };
    let Some(i) = *INPUTS.lock().unwrap_or_else(|e| e.into_inner()) else {
        return;
    };
    let depth = crate::resource::host_texture_now(i.depth as crate::object::This);
    let mvec = crate::resource::host_texture_now(i.motion_vectors as crate::object::This);
    let (Some((depth, depth_state)), Some((mvec, mvec_state))) = (depth, mvec) else {
        static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !ONCE.swap(true, Ordering::Relaxed) {
            tracing::warn!("dlssg: the title's depth or motion vectors have no host copy yet");
        }
        return;
    };
    // The title names its upscaler's states; the host copies say theirs.
    let _ = (i.depth_state, i.motion_state);
    let f = Frame {
        depth: depth.as_raw(),
        depth_state,
        mvec: mvec.as_raw(),
        mvec_state,
        render_width: i.render.0,
        render_height: i.render.1,
        output_width: output.0,
        output_height: output.1,
        jitter_x: i.jitter.0,
        jitter_y: i.jitter.1,
        // FSR's scale takes vectors to render pixels; SL wants [-1, 1].
        mvec_scale_x: if i.render.0 > 0 { i.motion_vector_scale.0 / i.render.0 as f32 } else { 1.0 },
        mvec_scale_y: if i.render.1 > 0 { i.motion_vector_scale.1 / i.render.1 as f32 } else { 1.0 },
        near_z: i.near,
        far_z: i.far,
        fov_y: i.fov_y,
        // Reversed depth when the title's near plane is beyond its far one.
        depth_inverted: i32::from(i.near > i.far),
        reset: i32::from(i.reset),
    };
    // SAFETY: our open list, live resources that outlive the call.
    let r = unsafe { (b.frame_inputs)(list.as_raw(), &f) };
    static LOGGED: AtomicU64 = AtomicU64::new(0);
    let n = LOGGED.fetch_add(1, Ordering::Relaxed);
    if n < 3 || r != 0 && n.is_power_of_two() {
        tracing::info!(result = r, depth_state, mvec_state, ?i, "dlssg: frame inputs tagged");
    }
}

pub(crate) fn present_start() {
    if let Some(b) = bridge() {
        // SAFETY: plain call.
        unsafe { (b.present_start)() };
    }
}

pub(crate) fn present_end() {
    if let Some(b) = bridge() {
        // SAFETY: plain call.
        unsafe { (b.present_end)() };
    }
}

/// Whether frame generation is on and its bridge loaded.
pub(crate) fn active() -> bool {
    bridge().is_some()
}
