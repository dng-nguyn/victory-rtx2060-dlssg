//! `amd_fidelityfx_loader_xbox.dll`: AMD's FidelityFX API, which the title
//! calls to upscale its frame. Only `ffxDispatch` is ours: it records what the
//! title hands the upscaler (its depth, motion vectors, the upscaled colour,
//! jitter and camera) for frame generation, then calls the title's own
//! loader. `ffxCreateContext`, `ffxConfigure`, `ffxQuery` and
//! `ffxDestroyContext` stay the title's (the HLE registry is asked first, the
//! title's DLLs after, so only the names listed here are taken over).
//!
//! The structures follow AMD's published `ffx_api.h` / `ffx_upscale.h`
//! (FidelityFX SDK 1.1). Each descriptor starts with a header (`type`, `next`)
//! and the title chains them; the upscale dispatch is the one with type
//! `DISPATCH_DESC_TYPE_UPSCALE`. The layout is checked at run time: the first
//! dispatches are logged with every size, so a wrong offset shows at once.
#![allow(non_snake_case)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::{HleModule, hle_export};

/// `ffxApiHeader`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub kind: u64,
    pub next: *const Header,
}

/// `FfxApiResourceDescription`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceDescription {
    pub kind: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u32,
    pub flags: u32,
    pub usage: u32,
}

/// `FfxApiResource`: the title's `ID3D12Resource` (a D3D12.X object), its
/// description and its state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Resource {
    pub resource: *mut c_void,
    pub description: ResourceDescription,
    pub state: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Float2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

/// `ffxDispatchDescUpscale`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DispatchUpscale {
    pub header: Header,
    pub command_list: *mut c_void,
    pub color: Resource,
    pub depth: Resource,
    pub motion_vectors: Resource,
    pub exposure: Resource,
    pub reactive: Resource,
    pub transparency_and_composition: Resource,
    pub output: Resource,
    pub jitter_offset: Float2,
    pub motion_vector_scale: Float2,
    pub render_size: Dimensions,
    pub upscale_size: Dimensions,
    pub frame_time_delta: f32,
    pub enable_sharpening: bool,
    pub sharpness: f32,
    pub pre_exposure: f32,
    pub view_space_to_meters_factor: f32,
    pub camera_near: f32,
    pub camera_far: f32,
    pub camera_fov_angle_vertical: f32,
    pub flags: u32,
}

/// `FFX_API_DISPATCH_DESC_TYPE_UPSCALE` (`ffx_upscale.h`).
pub const DISPATCH_DESC_TYPE_UPSCALE: u64 = 0x0001_0001;

/// What the title last handed the upscaler: what frame generation reads.
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    /// Title objects (D3D12.X resources), as addresses.
    pub depth: usize,
    pub motion_vectors: usize,
    pub output: usize,
    pub depth_desc: ResourceDescription,
    pub motion_desc: ResourceDescription,
    pub output_desc: ResourceDescription,
    pub jitter: Float2,
    pub motion_vector_scale: Float2,
    pub render_size: Dimensions,
    pub upscale_size: Dimensions,
    pub frame_time_delta: f32,
    pub camera_near: f32,
    pub camera_far: f32,
    pub fov_y: f32,
    /// Counts upscale dispatches; a frame generator sees a new frame by it.
    pub serial: u64,
}

static LATEST: Mutex<Option<Inputs>> = Mutex::new(None);
static UPSCALES: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_DISPATCH: AtomicUsize = AtomicUsize::new(0);

/// The title loader's own `ffxDispatch`, found once its DLL is mapped.
pub fn set_original_dispatch(address: usize) {
    ORIGINAL_DISPATCH.store(address, Ordering::Release);
    tracing::info!(address = format_args!("{address:#x}"), "ffx: the title's ffxDispatch is wrapped");
}

/// The newest upscale inputs, if the title has upscaled yet.
pub fn latest() -> Option<Inputs> {
    *LATEST.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads an upscale dispatch into [`Inputs`]; `serial` numbers it.
pub fn inputs_of(d: &DispatchUpscale, serial: u64) -> Inputs {
    Inputs {
        depth: d.depth.resource as usize,
        motion_vectors: d.motion_vectors.resource as usize,
        output: d.output.resource as usize,
        depth_desc: d.depth.description,
        motion_desc: d.motion_vectors.description,
        output_desc: d.output.description,
        jitter: d.jitter_offset,
        motion_vector_scale: d.motion_vector_scale,
        render_size: d.render_size,
        upscale_size: d.upscale_size,
        frame_time_delta: d.frame_time_delta,
        camera_near: d.camera_near,
        camera_far: d.camera_far,
        fov_y: d.camera_fov_angle_vertical,
        serial,
    }
}

/// The upscale descriptor in the chain starting at `desc`, if there is one.
///
/// # Safety
/// `desc` is null or the start of a chain of valid headers.
pub unsafe fn find_upscale(mut desc: *const Header) -> Option<*const DispatchUpscale> {
    // A chain is a handful of links; the bound guards a cycle.
    for _ in 0..64 {
        // SAFETY: the caller's contract, one link at a time.
        let header = unsafe { desc.as_ref() }?;
        if header.kind == DISPATCH_DESC_TYPE_UPSCALE {
            return Some(desc.cast());
        }
        desc = header.next;
    }
    None
}

type Dispatch = unsafe extern "system" fn(*mut c_void, *const Header) -> i32;

/// `ffxReturnCode_t ffxDispatch(ffxContext*, const ffxDispatchDescHeader*)`.
extern "system" fn ffxDispatch(context: *mut c_void, desc: *const Header) -> i32 {
    // SAFETY: the title passes a valid chain to its own loader.
    if let Some(up) = unsafe { find_upscale(desc) } {
        let serial = UPSCALES.fetch_add(1, Ordering::Relaxed) + 1;
        // SAFETY: `find_upscale` found a header of this type.
        let d = unsafe { &*up };
        let inputs = inputs_of(d, serial);
        if serial <= 3 || serial.is_power_of_two() {
            tracing::info!(
                serial,
                render = format_args!("{}x{}", d.render_size.width, d.render_size.height),
                upscale = format_args!("{}x{}", d.upscale_size.width, d.upscale_size.height),
                color = ?d.color.description,
                depth = ?d.depth.description,
                motion = ?d.motion_vectors.description,
                output = ?d.output.description,
                jitter = ?d.jitter_offset,
                mv_scale = ?d.motion_vector_scale,
                dt = d.frame_time_delta,
                near = d.camera_near,
                far = d.camera_far,
                fov = d.camera_fov_angle_vertical,
                flags = format_args!("{:#x}", d.flags),
                "ffx: upscale dispatch"
            );
        }
        *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(inputs);
        #[cfg(windows)]
        victory_gpu::dlssg::record(victory_gpu::dlssg::Inputs {
            depth: inputs.depth,
            depth_state: d.depth.state,
            motion_vectors: inputs.motion_vectors,
            motion_state: d.motion_vectors.state,
            render: (d.render_size.width, d.render_size.height),
            upscale: (d.upscale_size.width, d.upscale_size.height),
            jitter: (d.jitter_offset.x, d.jitter_offset.y),
            motion_vector_scale: (d.motion_vector_scale.x, d.motion_vector_scale.y),
            near: d.camera_near,
            far: d.camera_far,
            fov_y: d.camera_fov_angle_vertical,
            // FFX_UPSCALE_FLAG... none says "reset"; the first frame is one.
            reset: serial == 1,
        });
    } else {
        // SAFETY: as above; only the first header is read.
        let kind = unsafe { desc.as_ref() }.map_or(0, |h| h.kind);
        static OTHER: AtomicU64 = AtomicU64::new(0);
        if OTHER.fetch_add(1, Ordering::Relaxed) < 8 {
            tracing::info!(kind = format_args!("{kind:#x}"), "ffx: other dispatch");
        }
    }
    let original = ORIGINAL_DISPATCH.load(Ordering::Acquire);
    if original == 0 {
        tracing::error!("ffx: ffxDispatch called before the title's loader was found");
        return 1; // FFX_API_RETURN_ERROR
    }
    // SAFETY: the title loader's own export, with the title's arguments.
    unsafe { std::mem::transmute::<usize, Dispatch>(original)(context, desc) }
}

pub(crate) fn module() -> HleModule {
    HleModule {
        name: "ffx",
        dlls: &["amd_fidelityfx_loader_xbox.dll"],
        exports: vec![hle_export!(ffxDispatch)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout of AMD's `ffx_api_types.h` on x64.
    #[test]
    fn layouts_match_ffx_api() {
        assert_eq!(size_of::<Header>(), 16);
        assert_eq!(size_of::<ResourceDescription>(), 32);
        assert_eq!(size_of::<Resource>(), 48);
        assert_eq!(std::mem::offset_of!(DispatchUpscale, command_list), 16);
        assert_eq!(std::mem::offset_of!(DispatchUpscale, color), 24);
        assert_eq!(std::mem::offset_of!(DispatchUpscale, jitter_offset), 24 + 7 * 48);
        assert_eq!(std::mem::offset_of!(DispatchUpscale, render_size), 24 + 7 * 48 + 16);
    }

    #[test]
    fn the_upscale_link_is_found_in_a_chain() {
        let mut up: DispatchUpscale = unsafe { std::mem::zeroed() };
        up.header.kind = DISPATCH_DESC_TYPE_UPSCALE;
        up.render_size = Dimensions { width: 1280, height: 720 };
        let first = Header { kind: 0x1234, next: (&raw const up).cast() };
        let found = unsafe { find_upscale(&first) }.expect("found");
        assert_eq!(unsafe { (*found).render_size.width }, 1280);
        let lone = Header { kind: 0x1234, next: std::ptr::null() };
        assert!(unsafe { find_upscale(&lone) }.is_none());
        assert!(unsafe { find_upscale(std::ptr::null()) }.is_none());
    }
}
