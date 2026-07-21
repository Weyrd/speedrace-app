use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE,
    MEMORY_MAPPED_VIEW_ADDRESS,
};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenEventW, SetEvent, SYNCHRONIZATION_ACCESS_RIGHTS,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use super::protocol::{self, HookInfo};

const SYNCHRONIZE: u32 = 0x0010_0000;
const EVENT_MODIFY_STATE: u32 = 0x0002;
const FILE_MAP_RW: u32 = 0x0004 | 0x0002; // FILE_MAP_READ or FILE_MAP_WRITE

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn win_target(hwnd: u64) -> (u32, u32) {
    use windows::Win32::Foundation::HWND;
    let h = HWND(hwnd as *mut std::ffi::c_void);
    let mut pid = 0u32;
    let tid = unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    (pid, tid)
}

fn open_event(name: &str) -> Option<HANDLE> {
    unsafe {
        OpenEventW(
            SYNCHRONIZATION_ACCESS_RIGHTS(EVENT_MODIFY_STATE | SYNCHRONIZE),
            false,
            PCWSTR(wide(name).as_ptr()),
        )
    }
    .ok()
}

fn open_map(name: &str) -> Option<HANDLE> {
    unsafe { OpenFileMappingW(FILE_MAP_RW, false, PCWSTR(wide(name).as_ptr())) }.ok()
}

fn named(base: &str, pid: u32) -> String {
    protocol::name_with_id(base, pid)
}

fn ensure_hooked(hwnd: u64) -> Result<(u32, u32, HANDLE), String> {
    let (pid, tid) = win_target(hwnd);
    if pid == 0 {
        return Err("window has no process".into());
    }
    let is64 = super::inject::target_is_64bit(pid)?;
    let keepalive = unsafe {
        CreateMutexW(
            None,
            false,
            PCWSTR(wide(&named(protocol::WINDOW_HOOK_KEEPALIVE, pid)).as_ptr()),
        )
    }
    .map_err(|e| format!("create keepalive: {e}"))?;

    if open_event(&named(protocol::EVENT_CAPTURE_RESTART, pid)).is_none() {
        super::inject::inject(pid, tid, is64, false)?;
    }
    Ok((pid, tid, keepalive))
}

pub(crate) struct ArmedSession {
    pub pid: u32,
    pub keepalive: HANDLE,
    map: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    hi: *mut HookInfo,
}

pub(crate) fn inject_and_arm(hwnd: u64, fps: u32) -> Result<ArmedSession, String> {
    let (pid, _tid, keepalive) = ensure_hooked(hwnd)?;
    let armed = (|| {
        let is64 = super::inject::target_is_64bit(pid)?;
        let info_name = named(protocol::SHMEM_HOOK_INFO, pid);
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        let map = loop {
            if let Some(m) = open_map(&info_name) {
                break m;
            }
            if std::time::Instant::now() >= deadline {
                return Err("hook_info never appeared (injection failed?)".to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let view = unsafe {
            MapViewOfFile(
                map,
                FILE_MAP_READ | FILE_MAP_WRITE,
                0,
                0,
                std::mem::size_of::<HookInfo>(),
            )
        };
        if view.Value.is_null() {
            unsafe {
                let _ = CloseHandle(map);
            }
            return Err("MapViewOfFile(hook_info) failed".into());
        }
        let hi = view.Value as *mut HookInfo;
        let offsets = super::offsets::load(is64)?;
        unsafe {
            (*hi).offsets = offsets;
            (*hi).capture_overlay = 0;
            (*hi).force_shmem = 0;
            (*hi).unused_use_scale = 0;
            (*hi).allow_srgb_alias = 1;
            (*hi).frame_interval = 1_000_000_000u64 / fps.max(1) as u64;
        }
        if let Some(init) = open_event(&named(protocol::EVENT_HOOK_INIT, pid)) {
            unsafe {
                let _ = SetEvent(init);
                let _ = CloseHandle(init);
            }
        }
        if let Some(r) = open_event(&named(protocol::EVENT_CAPTURE_RESTART, pid)) {
            unsafe {
                let _ = SetEvent(r);
                let _ = CloseHandle(r);
            }
        }
        Ok((map, view, hi))
    })();
    match armed {
        Ok((map, view, hi)) => Ok(ArmedSession {
            pid,
            keepalive,
            map,
            view,
            hi,
        }),
        Err(e) => {
            release_session(pid, keepalive);
            Err(e)
        }
    }
}

impl ArmedSession {
    pub(crate) fn try_texture(&self, hwnd: u64) -> Option<(u32, u32, u32)> {
        let (ctype, map_id, cx, cy) = unsafe {
            (
                (*self.hi).capture_type,
                (*self.hi).map_id,
                (*self.hi).cx,
                (*self.hi).cy,
            )
        };
        if cx == 0 || cy == 0 || ctype != protocol::CAPTURE_TYPE_TEXTURE {
            if let Some(r) = open_event(&named(protocol::EVENT_CAPTURE_RESTART, self.pid)) {
                unsafe {
                    let _ = SetEvent(r);
                    let _ = CloseHandle(r);
                }
            }
            return None;
        }
        let tex_map_name = format!("{}_{}_{}", protocol::SHMEM_TEXTURE, hwnd, map_id);
        let tex_map = open_map(&tex_map_name)?;
        let tex_view = unsafe {
            MapViewOfFile(
                tex_map,
                FILE_MAP_READ,
                0,
                0,
                std::mem::size_of::<protocol::ShtexData>(),
            )
        };
        if tex_view.Value.is_null() {
            unsafe {
                let _ = CloseHandle(tex_map);
            }
            return None;
        }
        let tex = unsafe { (*(tex_view.Value as *const protocol::ShtexData)).tex_handle };
        unsafe {
            let _ = UnmapViewOfFile(tex_view);
            let _ = CloseHandle(tex_map);
        }
        if tex == 0 {
            None
        } else {
            Some((tex, cx, cy))
        }
    }

    pub(crate) fn release(self) {
        unsafe {
            let _ = UnmapViewOfFile(self.view);
            let _ = CloseHandle(self.map);
        }
        release_session(self.pid, self.keepalive);
    }
}

pub(crate) fn release_session(_pid: u32, keepalive: HANDLE) {
    unsafe {
        let _ = CloseHandle(keepalive);
    }
}
