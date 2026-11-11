//! Tests that only pass when each test runs in a process of its own, as cargo-nextest runs them,
//! both under Cargo and under Bazel (see `//bazel:nextest.bzl`). libtest runs the tests of a binary
//! on threads of one process, which share its environment and working directory.

use std::{env, fs, path::Path, thread, time::Duration};

/// Sets an environment variable, waits for the other tests to start, and checks that the variable
/// still holds the value that this test set.
fn set_and_check_variable(value: &str) {
    // SAFETY: nextest runs each test in a process of its own, so no other thread reads or writes
    // the environment.
    unsafe { env::set_var("PROCESS_ISOLATION_VALUE", value) };
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        env::var("PROCESS_ISOLATION_VALUE").unwrap(),
        value,
        "another test in this process changed the environment"
    );
}

#[test]
fn environment_a() {
    set_and_check_variable("a");
}

#[test]
fn environment_b() {
    set_and_check_variable("b");
}

/// Changes the working directory, and checks that it's still the one that this test chose.
fn change_and_check_working_directory(name: &str) {
    let directory = env::temp_dir().join(format!("process_isolation_{name}"));
    fs::create_dir_all(&directory).unwrap();
    env::set_current_dir(&directory).unwrap();
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        env::current_dir().unwrap().canonicalize().unwrap(),
        directory.canonicalize().unwrap(),
        "another test in this process changed the working directory"
    );
}

#[test]
fn working_directory_a() {
    change_and_check_working_directory("a");
}

#[test]
fn working_directory_b() {
    change_and_check_working_directory("b");
}

/// nextest runs each test in its package's directory, as Cargo does, so a test can read the
/// package's files by their paths relative to it.
#[test]
fn runs_in_the_package_directory() {
    let manifest = fs::read_to_string(Path::new("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("name = \"mirrord-layer-tests\""),
        "{manifest}"
    );
}

/// Cargo, nextest and rules_rust all set `CARGO_MANIFEST_DIR` when the test runs, so a test can
/// find the package's files from it. `env!("CARGO_MANIFEST_DIR")` would read it when rustc compiles
/// the test, which under Bazel is the package's directory in a sandbox that's gone by the time the
/// test runs. rules_rust fails the build of a crate that keeps that path.
#[test]
fn finds_the_package_from_the_run_time_manifest_directory() {
    let directory = env::var("CARGO_MANIFEST_DIR").unwrap();
    assert!(Path::new(&directory).join("Cargo.toml").exists(), "{directory}");
}

/// Fails on its first attempt under nextest, so that nextest retries it and reports it as flaky.
#[test]
fn passes_on_retry() {
    assert_ne!(
        env::var("NEXTEST_ATTEMPT").as_deref(),
        Ok("1"),
        "fails on the first attempt"
    );
}
