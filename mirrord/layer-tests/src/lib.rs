use std::{path::PathBuf, process::Command};

/// Returns the absolute path of the runfile at `path`, a runfiles path such as
/// `_main/mirrord/layer-tests/tests/apps/app_go/app_go_1_26`.
///
/// The target that builds the file must be in the test's `data`.
pub fn runfile(path: &str) -> PathBuf {
    let runfiles = runfiles::Runfiles::create().unwrap();
    runfiles::rlocation!(runfiles, path)
        .filter(|path| path.exists())
        .unwrap_or_else(|| {
            panic!("{path} isn't in the runfiles; add its target to the test's `data`")
        })
}

/// Runs `cmd`, asserts that it succeeds, and returns its stdout.
pub fn run(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}
