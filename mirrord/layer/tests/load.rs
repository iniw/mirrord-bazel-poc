//! Loads the built layer the way a program would and calls its exported functions.

use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};

#[test]
fn exports_layer_pid() {
    let runfiles = runfiles::Runfiles::create().unwrap();
    let path = runfiles::rlocation!(
        runfiles,
        format!("_main/mirrord/layer/{DLL_PREFIX}mirrord_layer{DLL_SUFFIX}")
    )
    .filter(|path| path.exists())
    .expect("the layer isn't in the runfiles; add `:layer` to the test's `data`");

    let layer = unsafe { libloading::Library::new(path) }.unwrap();
    let layer_pid = unsafe { layer.get::<extern "C" fn() -> i32>(b"layer_pid") }.unwrap();
    assert_eq!(layer_pid() as u32, std::process::id());

    // Frida's threads and hooks keep using the layer's code, so unloading it would crash the
    // process. Programs never unload the layer either.
    std::mem::forget(layer);
}
