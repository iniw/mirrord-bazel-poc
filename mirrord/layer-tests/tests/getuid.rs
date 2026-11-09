//! Runs the same program in C, Python and Node with the layer preloaded, and checks that the
//! layer's hook replaced `getuid` in each of them.

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    process::Command,
};

use mirrord_layer_tests::{run, runfile};

/// The variable that makes the dynamic loader load the layer into a program.
const PRELOAD: &str = if cfg!(target_os = "macos") {
    "DYLD_INSERT_LIBRARIES"
} else {
    "LD_PRELOAD"
};

/// The apps, by their paths in the runfiles. `js_binary` puts its launcher in a directory of its
/// own.
///
/// The Python app starts through a launcher that begins with `#!/usr/bin/env bash`, and macOS
/// removes `DYLD_INSERT_LIBRARIES` from the environment of SIP-protected programs like
/// `/usr/bin/env`, so the interpreter would start without the layer. mirrord's CLI avoids that by
/// running patched copies of such programs, which this test doesn't do, so on macOS it leaves the
/// Python app out. The Node app's launcher is a native program, which keeps the variable.
const APPS: &[&str] = if cfg!(target_os = "macos") {
    &["getuid_c", "getuid_node_/getuid_node"]
} else {
    &["getuid_c", "getuid_py", "getuid_node_/getuid_node"]
};

#[test]
fn layer_hooks_getuid() {
    let layer = runfile(&format!(
        "_main/mirrord/layer/{DLL_PREFIX}mirrord_layer{DLL_SUFFIX}"
    ));
    for app in APPS {
        let path = runfile(&format!(
            "_main/mirrord/layer-tests/tests/apps/getuid/{app}"
        ));
        let out = run(Command::new(path).env(PRELOAD, &layer));
        assert_eq!(out, "4242\n", "{app}");
    }
}
