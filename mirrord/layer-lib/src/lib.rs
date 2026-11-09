//! Code that the CLI and the layer share. Each of them enables the feature for its own half, and
//! both halves are compiled when Cargo or Bazel builds them together.

/// Describes how the CLI starts the user's process.
#[cfg(feature = "cli-execution")]
pub fn cli_execution() -> &'static str {
    "the CLI starts the process with the layer loaded"
}

/// Returns the ID of the process that a hooked function runs in.
#[cfg(feature = "layer-hook-execution")]
pub fn hooked_process_id() -> i32 {
    unsafe { libc::getpid() }
}
