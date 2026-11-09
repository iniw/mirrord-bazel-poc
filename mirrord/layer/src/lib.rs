//! The library that mirrord loads into the user's process on Linux and macOS.
//!
//! When it loads, it replaces libc's `getuid` with Frida, the way the real layer hooks the libc
//! functions that it intercepts.

use std::{ffi::c_void, ptr, sync::LazyLock};

use frida_gum::{Gum, Module, NativePointer, interceptor::Interceptor};

/// The user ID that the hooked `getuid` returns, so that tests can tell that the hook is in place.
const HOOKED_UID: libc::uid_t = 4242;

static GUM: LazyLock<Gum> = LazyLock::new(Gum::obtain);

#[unsafe(no_mangle)]
pub extern "C" fn layer_pid() -> i32 {
    unsafe { libc::getpid() }
}

#[unsafe(no_mangle)]
pub extern "C" fn hello() -> i32 {
    println!("Wassup!");
    0
}

/// Replaces libc's `getuid` with [`getuid_detour`] as soon as the layer is loaded.
#[ctor::ctor]
fn hook_getuid() {
    let mut interceptor = Interceptor::obtain(&GUM);
    let getuid = Module::find_global_export_by_name("getuid").expect("libc exports `getuid`");
    interceptor
        .replace(
            getuid,
            NativePointer(getuid_detour as *mut c_void),
            NativePointer(ptr::null_mut()),
        )
        .expect("Frida should replace `getuid`");
}

extern "C" fn getuid_detour() -> libc::uid_t {
    HOOKED_UID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_pid_is_the_process_id() {
        assert_eq!(layer_pid() as u32, std::process::id());
    }

    /// `mirrord-layer-lib`'s half for the layer is enabled.
    #[test]
    fn shares_code_with_the_cli() {
        assert_eq!(mirrord_layer_lib::hooked_process_id(), layer_pid());
    }

    #[test]
    fn hello_succeeds() {
        assert_eq!(hello(), 0);
    }

    /// The test binary includes the layer's constructor, so `getuid` is hooked here too.
    #[test]
    fn getuid_is_hooked() {
        assert_eq!(unsafe { libc::getuid() }, HOOKED_UID);
    }
}
