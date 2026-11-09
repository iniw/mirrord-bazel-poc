//! Provides the files that a crate bundles, like the CLI's layer and UI. A standalone binary embeds
//! them at compile time, and other binaries read them at run time from their runfiles, so that
//! changing a file doesn't build the crate that bundles it again.
//!
//! A crate that bundles files declares its own `standalone` feature, and has a Bazel target that
//! turns it on, like `//mirrord/cli:cli_standalone`. Bazel passes the path of each file to rustc in
//! an environment variable: its path in the build (`$(execpath)`) to that target, and its runfiles
//! path (`$(rlocationpath)`) to the others.

use std::path::PathBuf;

/// Returns the file whose path Bazel passes to rustc in the environment variable `$variable`, as a
/// `Cow<'static, [u8]>`: embedded at compile time with the `standalone` feature, and read at run
/// time without it.
///
/// The macro expands in the crate that calls it, so `standalone` and `$variable` are that crate's
/// feature and that crate's rustc environment.
#[macro_export]
macro_rules! bundled_file {
    ($variable:literal) => {{
        #[cfg(feature = "standalone")]
        let file: ::std::borrow::Cow<'static, [u8]> =
            ::std::borrow::Cow::Borrowed(include_bytes!(env!($variable)));

        #[cfg(not(feature = "standalone"))]
        let file: ::std::borrow::Cow<'static, [u8]> = {
            let path = $crate::bundled_file_path($variable, option_env!($variable));
            ::std::borrow::Cow::Owned(
                ::std::fs::read(&path)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display())),
            )
        };

        file
    }};
}

/// Returns where a program without the `standalone` feature finds a bundled file at run time.
///
/// That's the path in the environment variable `variable` if it's set, which overrides the file
/// like the real CLI's `MIRRORD_LAYER_FILE` does. Otherwise, it's the file in the program's
/// runfiles at `runfiles_path`, which Bazel passes to rustc in the same variable. Cargo builds have
/// no runfiles, so they need the variable.
pub fn bundled_file_path(variable: &str, runfiles_path: Option<&str>) -> PathBuf {
    if let Some(path) = std::env::var_os(variable) {
        return PathBuf::from(path);
    }

    let runfiles_path = runfiles_path.unwrap_or_else(|| {
        panic!("only Bazel builds have runfiles, so this build needs `{variable}` to be set")
    });
    let runfiles = runfiles::Runfiles::create().unwrap_or_else(|error| {
        panic!("the program has no runfiles ({error}), so it needs `{variable}` to be set")
    });
    runfiles::rlocation!(runfiles, runfiles_path)
        .filter(|path| path.exists())
        .unwrap_or_else(|| panic!("{runfiles_path} isn't in the program's runfiles"))
}
