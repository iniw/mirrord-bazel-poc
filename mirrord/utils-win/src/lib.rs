//! Helpers for mirrord on Windows.
//!
//! The crate also adds mirrord's logo to the resources of every binary that links it, for the
//! diagnostics dialog to show. Bazel compiles the logo's resource script (see `BUILD.bazel`).

/// The ID of the logo's resource in `src/diagnostics/logo.rc`.
pub const LOGO_RESOURCE_ID: u16 = 100;
