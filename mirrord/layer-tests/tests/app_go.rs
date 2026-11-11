//! Runs the Go app, built with each Go version, with the layer preloaded.

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

/// Checks that each build of the app keeps the symbol table that the real layer finds Go's
/// runtime by, and that the layer loads into it and hooks libc's `getuid`. The layer can only be
/// preloaded into a dynamically linked program, which a Go app is only when it's built with cgo, as
/// `go build` builds it.
#[test]
fn go_versions() {
    let layer = runfile(&format!(
        "_main/mirrord/layer/{DLL_PREFIX}mirrord_layer{DLL_SUFFIX}"
    ));
    for version in ["1.25", "1.26", "1.27"] {
        let path = format!(
            "_main/mirrord/layer-tests/tests/apps/app_go/app_go_{}",
            version.replace('.', "_")
        );
        let got = run(Command::new(runfile(&path)).env(PRELOAD, &layer));
        let fields: Vec<&str> = got.split_whitespace().collect();
        assert!(fields[0].starts_with(&format!("go{version}")), "{path}: {got}");
        assert_eq!(fields[1..], ["1", "true", "4242"], "{path}: {got}");
    }
}
