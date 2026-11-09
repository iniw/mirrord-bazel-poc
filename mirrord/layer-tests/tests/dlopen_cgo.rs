use std::{env::consts::DLL_SUFFIX, process::Command};

use mirrord_layer_tests::{run, runfile};

#[test]
fn loads_go_library() {
    let library = runfile(&format!(
        "_main/mirrord/layer-tests/tests/apps/dlopen_cgo/go_server_/libgo_server{DLL_SUFFIX}"
    ));
    let loader = runfile("_main/mirrord/layer-tests/tests/apps/dlopen_cgo/loader");
    let out = run(Command::new(loader).arg(library));
    assert_eq!(out, "served from cgo\n");
}
