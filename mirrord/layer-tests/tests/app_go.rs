use std::{env::consts::EXE_SUFFIX, process::Command};

use mirrord_layer_tests::{run, runfile};

#[test]
fn go_versions() {
    for version in ["1.25", "1.26", "1.27"] {
        let path = format!(
            "_main/mirrord/layer-tests/tests/apps/app_go/app_go_{}{EXE_SUFFIX}",
            version.replace('.', "_")
        );
        let got = run(&mut Command::new(runfile(&path)));
        assert!(got.starts_with(&format!("go{version}")), "{path}: {got}");
    }
}
