//! Runs the built CLI as a user would.

use std::{env::consts::EXE_SUFFIX, process::Command};

#[test]
fn reports_its_version_and_the_embedded_layer() {
    let runfiles = runfiles::Runfiles::create().unwrap();
    let path = runfiles::rlocation!(runfiles, format!("_main/mirrord/cli/mirrord{EXE_SUFFIX}"))
        .filter(|path| path.exists())
        .expect("the CLI isn't in the runfiles; add `:cli` to the test's `data`");

    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Bazel gives this test crate the workspace version too, so it must match the CLI's, which
    // the agent's image tag comes from.
    assert_ne!(env!("CARGO_PKG_VERSION"), "0.0.0");
    assert!(
        stdout.starts_with(&format!("mirrord {}\n", env!("CARGO_PKG_VERSION"))),
        "{stdout}"
    );
    assert!(stdout.contains("aws-lc eq: true\n"), "{stdout}");
}
