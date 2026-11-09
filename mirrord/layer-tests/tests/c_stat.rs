use std::{env::consts::EXE_SUFFIX, process::Command};

use mirrord_layer_tests::{run, runfile};

#[test]
fn stat() {
    let out = run(&mut Command::new(runfile(&format!(
        "_main/mirrord/layer-tests/tests/apps/c_stat/c_stat{EXE_SUFFIX}"
    ))));
    // MSVC's C runtime ends lines with `\r\n`, even in a pipe.
    assert_eq!(out.trim_end(), "stat=0");
}
