{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs) lib;
    in
    {
      devShells = lib.genAttrs lib.systems.flakeExposed (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};

          # Bazel downloads prebuilt toolchains, such as rustc, Node.js and Python, which load their
          # libraries through `/lib64/ld-linux-x86-64.so.2` and look for them in `/usr/lib`. NixOS
          # has neither, so on Linux the `bazel` command runs Bazel in a bubblewrap namespace that
          # provides both, with the libraries below. Every action and `bazel run` target runs in
          # that namespace too, because the Bazel server and the programs that it starts inherit
          # it.
          bazel =
            if pkgs.stdenv.hostPlatform.isLinux then
              pkgs.buildFHSEnv {
                name = "bazel";
                targetPkgs = pkgs: [
                  # libgcc_s and libstdc++.
                  pkgs.stdenv.cc.cc.lib
                  pkgs.zlib
                ];
                runScript = lib.getExe pkgs.bazel_9;
              }
            else
              pkgs.bazel_9;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              bazel
              bazel-buildtools
              starpls

              # Bazel downloads its own toolchains: these tools only update the lockfiles that it
              # reads. Cargo updates `Cargo.lock`, pnpm `pnpm-lock.yaml`, Go the test apps' `go.sum`,
              # and uv the Python test apps' `requirements.txt`.
              cargo
              go
              nodejs
              pnpm_10
              uv

              # Cargo also checks the Rust workspace, with rustc and clippy, for `cargo check`,
              # `cargo clippy` and rust-analyzer (see `.cargo/config.toml`). The build scripts that
              # Bazel replaces or gives its own tools run under Cargo as they are: containerd-client's
              # needs `protoc`, and frida-gum-sys's runs bindgen, which needs libclang and the C
              # headers that `bindgenHook` points it at.
              clippy
              protobuf
              rust-analyzer
              rustPlatform.bindgenHook
              rustc
              rustfmt
            ];
          };
        }
      );
    };
}
