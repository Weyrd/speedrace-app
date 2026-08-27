pub const EVENT_CAPTURE_RESTART: &str = "CaptureHook_Restart";
pub const EVENT_HOOK_INIT: &str = "CaptureHook_Initialize";
pub const WINDOW_HOOK_KEEPALIVE: &str = "CaptureHook_KeepAlive";
pub const SHMEM_HOOK_INFO: &str = "CaptureHook_HookInfo";
pub const SHMEM_TEXTURE: &str = "CaptureHook_Texture";

pub const CAPTURE_TYPE_TEXTURE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3d8Offsets {
    pub present: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3d9Offsets {
    pub present: u32,
    pub present_ex: u32,
    pub present_swap: u32,
    pub d3d9_clsoff: u32,
    pub is_d3d9ex_clsoff: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DxgiOffsets {
    pub present: u32,
    pub resize: u32,
    pub present1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DdrawOffsets {
    pub surface_create: u32,
    pub surface_restore: u32,
    pub surface_release: u32,
    pub surface_unlock: u32,
    pub surface_blt: u32,
    pub surface_flip: u32,
    pub surface_set_palette: u32,
    pub palette_set_entries: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DxgiOffsets2 {
    pub release: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct D3d12Offsets {
    pub execute_command_lists: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GraphicsOffsets {
    pub d3d8: D3d8Offsets,
    pub d3d9: D3d9Offsets,
    pub dxgi: DxgiOffsets,
    pub ddraw: DdrawOffsets,
    pub dxgi2: DxgiOffsets2,
    pub d3d12: D3d12Offsets,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HookInfo {
    pub hook_ver_major: u32,
    pub hook_ver_minor: u32,
    pub capture_type: u32,
    pub window: u32,
    pub format: u32,
    pub cx: u32,
    pub cy: u32,
    pub unused_base_cx: u32,
    pub unused_base_cy: u32,
    pub pitch: u32,
    pub map_id: u32,
    pub map_size: u32,
    pub flip: u8,
    pub frame_interval: u64,
    pub unused_use_scale: u8,
    pub force_shmem: u8,
    pub capture_overlay: u8,
    pub allow_srgb_alias: u8,
    pub offsets: GraphicsOffsets,
    pub reserved: [u32; 126],
}

const _: () = assert!(core::mem::size_of::<HookInfo>() == 648);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ShtexData {
    pub tex_handle: u32,
}

pub fn name_with_id(base: &str, pid: u32) -> String {
    format!("{base}{pid}")
}
