//! The mirrord CLI, reduced to what shows how it builds: it bundles the layer for its platform and
//! the UI, and reports them, together with its version.
//!
//! With the `standalone` feature, which Bazel's `//mirrord/cli:cli_standalone` turns on, the CLI
//! embeds the files that it bundles. Without it, the CLI reads them at run time from its runfiles
//! instead, so changing the layer or the UI doesn't build the CLI again.

use std::borrow::Cow;

use mirrord_bundle::bundled_file;

/// The layer that the CLI loads into the user's process. On macOS, it's the universal layer.
fn layer() -> Cow<'static, [u8]> {
    bundled_file!("MIRRORD_LAYER_FILE")
}

/// The arm64 layer, which the universal layer's arm64e shim loads.
#[cfg(target_os = "macos")]
fn layer_arm64() -> Cow<'static, [u8]> {
    bundled_file!("MIRRORD_LAYER_FILE_MACOS_ARM64")
}

/// The utilities that the CLI copies to run SIP-protected programs on macOS.
#[cfg(target_os = "macos")]
fn compressed_sip_binaries() -> Cow<'static, [u8]> {
    bundled_file!("MIRRORD_SIP_BINARIES")
}

/// Returns the UI's file at `path`, relative to the UI's root.
fn ui_file(path: &str) -> Option<Cow<'static, [u8]>> {
    #[cfg(feature = "standalone")]
    {
        /// The built UI (`packages/ui`), which `mirrord ui` serves.
        #[derive(rust_embed::Embed)]
        #[folder = "$MIRRORD_UI_DIST"]
        struct Ui;

        Ui::get(path).map(|file| file.data)
    }

    #[cfg(not(feature = "standalone"))]
    {
        let root =
            mirrord_bundle::bundled_file_path("MIRRORD_UI_DIST", option_env!("MIRRORD_UI_DIST"));
        std::fs::read(root.join(path)).ok().map(Cow::Owned)
    }
}

fn main() {
    println!("mirrord {}", env!("CARGO_PKG_VERSION"));
    println!("{}", mirrord_layer_lib::cli_execution());
    let index = ui_file("index.html").expect("the UI has an index page");
    println!("ui index {} bytes", index.len());

    let layer = layer();
    let ring_digest = ring::digest::digest(&ring::digest::SHA256, &layer);
    let aws_lc_digest = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, &layer);
    println!(
        "layer {} bytes, ring sha256 {:x?}, aws-lc eq: {}",
        layer.len(),
        ring_digest.as_ref().iter().take(4).collect::<Vec<_>>(),
        ring_digest.as_ref() == aws_lc_digest.as_ref()
    );
    #[cfg(target_os = "macos")]
    println!(
        "arm64 layer {} bytes, SIP binaries {} bytes",
        layer_arm64().len(),
        compressed_sip_binaries().len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bazel bundles the layer for the CLI's own platform, so its file format matches the target
    /// OS.
    #[test]
    fn bundles_the_layer_for_the_target_os() {
        let magic: &[u8] = if cfg!(target_os = "macos") {
            // A universal binary.
            &[0xca, 0xfe, 0xba, 0xbe]
        } else if cfg!(windows) {
            b"MZ"
        } else {
            b"\x7fELF"
        };
        assert!(layer().starts_with(magic));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundles_the_arm64_layer_on_macos() {
        assert!(layer_arm64().starts_with(&[0xcf, 0xfa, 0xed, 0xfe]));
    }

    /// The UI reads the version from the same `Cargo.toml` as the CLI.
    #[test]
    fn bundles_the_ui_of_the_same_version() {
        let index = ui_file("index.html").expect("the UI has an index page");
        let script = std::str::from_utf8(&index)
            .unwrap()
            .split('"')
            .find(|part| part.ends_with(".js"))
            .expect("the index page loads a script")
            .trim_start_matches('/')
            .to_owned();
        let script = ui_file(&script).expect("the index page's script is bundled");
        let script = std::str::from_utf8(&script).unwrap();
        assert!(script.contains(env!("CARGO_PKG_VERSION")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundles_the_sip_binaries_on_macos() {
        assert!(compressed_sip_binaries().starts_with(&[0x1f, 0x8b]));
    }

    #[test]
    fn ring_and_aws_lc_agree() {
        let layer = layer();
        let ring = ring::digest::digest(&ring::digest::SHA256, &layer);
        let aws_lc = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, &layer);
        assert_eq!(ring.as_ref(), aws_lc.as_ref());
    }
}
