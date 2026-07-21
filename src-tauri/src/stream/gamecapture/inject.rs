use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;

use windows::core::{s, w, BOOL};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, IsWow64Process, OpenProcess, WaitForSingleObject,
    LPTHREAD_START_ROUTINE, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
    PROCESS_VM_READ, PROCESS_VM_WRITE,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const HOST_IS_64: bool = cfg!(target_pointer_width = "64");

pub(crate) fn target_is_64bit(pid: u32) -> Result<bool, String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid)
            .map_err(|e| format!("OpenProcess({pid}) for bitness: {e}"))?;
        let mut wow64 = BOOL(0);
        let r = IsWow64Process(process, &mut wow64);
        let _ = CloseHandle(process);
        r.map_err(|e| format!("IsWow64Process: {e}"))?;
        Ok(!wow64.as_bool())
    }
}

pub(crate) fn inject(
    pid: u32,
    thread_id: u32,
    process_is_64bit: bool,
    anticheat: bool,
) -> Result<(), String> {
    let hook = super::resolve_binary(if process_is_64bit {
        "graphics-hook64.dll"
    } else {
        "graphics-hook32.dll"
    })?;

    let matching = HOST_IS_64 == process_is_64bit;
    if matching && !anticheat {
        inject_direct(pid, &hook)
    } else {
        let helper = super::resolve_binary(if process_is_64bit {
            "inject-helper64.exe"
        } else {
            "inject-helper32.exe"
        })?;
        let id = if anticheat { thread_id } else { pid };
        let status = std::process::Command::new(&helper)
            .arg(&hook)
            .arg(if anticheat { "1" } else { "0" })
            .arg(id.to_string())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("spawn inject-helper: {e}"))?;
        if !status.success() {
            return Err(format!("inject-helper failed (code {:?})", status.code()));
        }
        Ok(())
    }
}

fn inject_direct(pid: u32, hook: &Path) -> Result<(), String> {
    let wpath: Vec<u16> = hook
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let bytes = std::mem::size_of_val(wpath.as_slice());
    unsafe {
        let process = OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            false,
            pid,
        )
        .map_err(|e| format!("OpenProcess({pid}) for inject: {e}"))?;
        let out = inject_into(process, &wpath, bytes);
        let _ = CloseHandle(process);
        out
    }
}

unsafe fn inject_into(process: HANDLE, wpath: &[u16], bytes: usize) -> Result<(), String> {
    let remote = VirtualAllocEx(
        process,
        None,
        bytes,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );
    if remote.is_null() {
        return Err("VirtualAllocEx failed".into());
    }
    let out = write_and_run(process, remote, wpath, bytes);
    let _ = VirtualFreeEx(process, remote, 0, MEM_RELEASE);
    out
}

unsafe fn write_and_run(
    process: HANDLE,
    remote: *mut c_void,
    wpath: &[u16],
    bytes: usize,
) -> Result<(), String> {
    WriteProcessMemory(
        process,
        remote,
        wpath.as_ptr() as *const c_void,
        bytes,
        None,
    )
    .map_err(|e| format!("WriteProcessMemory: {e}"))?;

    let k32 = GetModuleHandleW(w!("kernel32.dll")).map_err(|e| format!("GetModuleHandleW: {e}"))?;
    let Some(load) = GetProcAddress(k32, s!("LoadLibraryW")) else {
        return Err("LoadLibraryW not found".into());
    };
    let entry: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(load);
    let start: LPTHREAD_START_ROUTINE = Some(entry);
    let thread = CreateRemoteThread(
        process,
        None,
        0,
        start,
        Some(remote as *const c_void),
        0,
        None,
    )
    .map_err(|e| format!("CreateRemoteThread: {e}"))?;
    WaitForSingleObject(thread, 10_000);
    let mut exit = 0u32;
    let _ = GetExitCodeThread(thread, &mut exit);
    let _ = CloseHandle(thread);
    // exit = low 32 bits of the loaded HMODULE 0 => LoadLibraryW returned NULL
    if exit == 0 {
        return Err("LoadLibraryW returned null in target".into());
    }
    Ok(())
}
