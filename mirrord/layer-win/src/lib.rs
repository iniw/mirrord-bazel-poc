//! The library that mirrord loads into the user's process on Windows.
//!
//! When it loads, it replaces `GetTickCount` with MinHook, the hooking library that the real
//! Windows layer uses.
#![cfg(windows)]

use std::ffi::{c_char, c_void};

use minhook_detours_rs::guard::DetourGuard;

/// The tick count that the hooked `GetTickCount` returns, so that a test can tell that the hook is
/// in place.
const HOOKED_TICK_COUNT: u32 = 4242;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(module_name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, procedure_name: *const c_char) -> *mut c_void;
}

/// Import-table injection resolves this marker through ordinal 1, which `BUILD.bazel` assigns.
#[unsafe(no_mangle)]
pub extern "system" fn mirrord_stork_marker() {}

#[ctor::ctor]
fn install_hooks() {
    // The address of the function in kernel32 itself. Taking the address of an imported function
    // would give the stub in this library that jumps to it, and hooking that stub would miss the
    // calls from every other module.
    // SAFETY: Both arguments are null-terminated strings, and kernel32 is loaded in every process.
    let get_tick_count = unsafe {
        GetProcAddress(
            GetModuleHandleA(c"kernel32.dll".as_ptr()),
            c"GetTickCount".as_ptr(),
        )
    };
    assert!(!get_tick_count.is_null(), "kernel32 exports `GetTickCount`");

    let mut guard = DetourGuard::new().expect("MinHook should initialize");
    guard
        .create_and_enable_hook::<extern "system" fn() -> u32>(
            get_tick_count,
            get_tick_count_detour as *mut c_void,
        )
        .expect("MinHook should hook `GetTickCount`");
    // Dropping the guard would remove the hook, and the layer keeps its hooks while it's loaded.
    std::mem::forget(guard);
}

extern "system" fn get_tick_count_detour() -> u32 {
    HOOKED_TICK_COUNT
}
