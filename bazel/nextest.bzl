"""`nextest_test`, a test that runs a `rust_test`'s binary with cargo-nextest.

A `rust_test` on its own runs the libtest binary that it builds, which runs all of the crate's tests
on threads of one process. Under Cargo, the workspace runs its tests with cargo-nextest instead,
which runs each test in a process of its own, retries the tests that fail, and writes a JUnit
report with a test case for each test. Some tests rely on that: they change the process's
environment or working directory, which would race with the other tests on libtest's threads.

`nextest_test` runs the binary with the same nextest, configured by the same
`.config/nextest.toml`, so the tests behave as they do under `cargo nextest run`. nextest only runs
the binaries of a Cargo workspace, which it learns about from two JSON files that
`cargo nextest run` takes in place of a build (https://nexte.st/docs/ci-features/archiving/):

- `--cargo-metadata`, the output of `cargo metadata`. nextest takes the workspace's root, each
  package's directory and its targets from it. It also checks every package and binary that a
  filter in `.config/nextest.toml` names against it, so it must describe the whole workspace,
  even though each `nextest_test` runs a single binary. `cargo_metadata` below writes it, with
  only the fields that nextest reads, so that it only changes when a package or a target is added,
  removed or renamed, or a version changes. Otherwise any change to any `Cargo.toml`, like a new
  dependency, would run every test again.
- `--binaries-metadata`, the list of test binaries that a build would write. The test's runner
  writes it with the one binary that the test runs.

The runner, `//bazel/nextest_runner`, is the test's executable. It's a Rust program, so that the
tests also run on Windows machines, and it also translates Bazel's test protocol for nextest
(https://bazel.build/reference/test-encyclopedia):

- `--test_filter` runs only the tests whose names contain the filter.
- `shard_count` splits the tests across shards by a hash of their names (`--partition hash:`).
- The JUnit report that nextest writes becomes the target's `test.xml`.
- `--test_arg` passes options to `cargo nextest run`, for example `--test_arg=-E
  --test_arg='test(/^fs::/)'` or `--test_arg=--run-ignored=all`.
- `--test_env=NEXTEST_PROFILE=ci` picks a profile of `.config/nextest.toml`, and the other
  `NEXTEST_*` variables, like `NEXTEST_RETRIES`, override its settings.
"""

def _cargo_metadata_impl(ctx):
    cargo = ctx.toolchains["@rules_rust//cargo:toolchain_type"]
    output = ctx.actions.declare_file(ctx.label.name + ".json")

    # The runner writes the workspace's root as placeholders, because the action runs in a sandbox
    # whose path differs for each build, and each test puts back the root of its own runfiles.
    ctx.actions.run(
        executable = ctx.executable._runner,
        arguments = [
            "metadata",
            "--cargo",
            cargo.cargo.path,
            "--manifest",
            ctx.file.workspace_manifest.path,
            "--output",
            output.path,
        ],
        inputs = ctx.files.srcs + [ctx.file.workspace_manifest],
        tools = cargo.all_files,
        outputs = [output],
        mnemonic = "CargoMetadata",
        progress_message = "Describing the Cargo workspace for nextest",
    )

    return [DefaultInfo(files = depset([output]))]

cargo_metadata = rule(
    implementation = _cargo_metadata_impl,
    doc = "The workspace's packages and their targets, as `cargo metadata --no-deps` writes them.",
    attrs = {
        "srcs": attr.label_list(
            doc = "Each package's `Cargo.toml` and the sources of its targets that Cargo " +
                  "discovers, like `src/lib.rs` and `tests/*.rs`.",
            allow_files = True,
        ),
        "workspace_manifest": attr.label(
            doc = "The workspace's root `Cargo.toml`.",
            allow_single_file = True,
            default = "//:Cargo.toml",
        ),
        "_runner": attr.label(
            default = "//bazel/nextest_runner",
            cfg = "exec",
            executable = True,
        ),
    },
    toolchains = ["@rules_rust//cargo:toolchain_type"],
)

def _rlocation_path(ctx, file):
    """Returns the path of `file` in the runfiles, like `$(rlocationpath)`."""
    if file.short_path.startswith("../"):
        return file.short_path[len("../"):]

    return ctx.workspace_name + "/" + file.short_path

def _nextest_test_impl(ctx):
    test = ctx.attr.test
    binary = test[DefaultInfo].files_to_run.executable
    nextest = ctx.file._nextest

    # The test's executable is the runner, under the test's name. Windows only runs a program whose
    # name ends with `.exe`.
    runner = ctx.executable._runner
    windows = ctx.target_platform_has_constraint(ctx.attr._windows[platform_common.ConstraintValueInfo])
    executable = ctx.actions.declare_file(ctx.label.name + "_nextest" + (".exe" if windows else ""))

    ctx.actions.symlink(output = executable, target_file = runner, is_executable = True)

    # The runner's settings, which it reads from its environment.
    settings = {
        "BINARY": _rlocation_path(ctx, binary),
        "BINARY_ID": ctx.attr.binary_id,
        "BINARY_KIND": ctx.attr.kind,
        "BINARY_NAME": ctx.attr.binary_name,
        "CARGO_METADATA": _rlocation_path(ctx, ctx.file.cargo_metadata),
        "CONFIG": _rlocation_path(ctx, ctx.file.config),
        "NEXTEST": _rlocation_path(ctx, nextest),
        "PACKAGE_DIRECTORY": ctx.label.package,

        # The platform that the tests run on, which `platform()` filters and `cfg()` overrides in
        # `.config/nextest.toml` match.
        "TRIPLE": ctx.toolchains["@rules_rust//rust:toolchain_type"].target_flag_value,

        # nextest only checks that the workspace's `Cargo.toml` exists. An empty one stands in for
        # it, so that a change to the real one, like a new workspace dependency, doesn't run every
        # test again.
        "WORKSPACE_MANIFEST": ctx.workspace_name + "/Cargo.toml",
    }

    runfiles = ctx.runfiles(files = [
        binary,
        ctx.file.cargo_metadata,
        ctx.file.config,
        nextest,
        runner,
    ] + ctx.files.manifest, root_symlinks = {
        ctx.workspace_name + "/Cargo.toml": ctx.file._workspace_manifest_stand_in,
    }).merge(test[DefaultInfo].default_runfiles)

    # The `env` of the `rust_test`, with its `$(rlocationpath)`s already expanded, applies to the
    # tests, which inherit it from nextest. nextest sets `CARGO_MANIFEST_DIR` itself, to the
    # package's directory in the runfiles, which is also the tests' working directory.
    environment = {}
    inherited_environment = []

    if RunEnvironmentInfo in test:
        environment = dict(test[RunEnvironmentInfo].environment)
        inherited_environment = test[RunEnvironmentInfo].inherited_environment

    environment.pop("CARGO_MANIFEST_DIR", None)
    environment.update({"BAZEL_NEXTEST_" + name: value for name, value in settings.items()})

    return [
        DefaultInfo(executable = executable, runfiles = runfiles),
        RunEnvironmentInfo(
            environment = environment,
            inherited_environment = inherited_environment,
        ),
    ]

_nextest_test = rule(
    implementation = _nextest_test_impl,
    doc = """Runs the tests of a `rust_test` with cargo-nextest, as `cargo nextest run` would.

    The target must be in the package of the `Cargo.toml` that the test belongs to, because the
    runner finds the Cargo package by the target's package.""",
    test = True,
    attrs = {
        "binary_id": attr.string(
            doc = "The binary's ID in nextest, like `mirrord-layer-tests::c_stat`, which " +
                  "`binary_id()` filters and the JUnit report's test suites take.",
            mandatory = True,
        ),
        "binary_name": attr.string(
            doc = "The name of the binary's Cargo target, which `binary()` filters.",
            mandatory = True,
        ),
        "cargo_metadata": attr.label(
            doc = "The workspace's `cargo_metadata`.",
            allow_single_file = True,
            default = "//:cargo_metadata",
        ),
        "config": attr.label(
            doc = "nextest's configuration, which sets the profiles, retries and test groups.",
            allow_single_file = True,
            default = "//:.config/nextest.toml",
        ),
        "kind": attr.string(
            doc = "The kind of the binary's Cargo target, which `kind()` filters.",
            values = ["bin", "lib", "proc-macro", "test"],
            mandatory = True,
        ),
        "manifest": attr.label(
            doc = "The package's `Cargo.toml`, which tests that read it relative to their " +
                  "working directory, the package's directory, need in their runfiles.",
            allow_single_file = True,
        ),
        "test": attr.label(
            doc = "The `rust_test` whose binary nextest runs. Its runfiles and `env` apply to the " +
                  "tests.",
            executable = True,
            cfg = "target",
            mandatory = True,
        ),
        "_runner": attr.label(
            default = "//bazel/nextest_runner",
            cfg = "target",
            executable = True,
        ),
        "_windows": attr.label(default = "@platforms//os:windows"),
        "_workspace_manifest_stand_in": attr.label(
            allow_single_file = True,
            default = "//bazel/nextest_runner:workspace.Cargo.toml",
        ),
        # The release archive's program is a source file, so it can't be an `executable` attribute.
        "_nextest": attr.label(
            allow_single_file = True,
            cfg = "target",
            default = "//third_party/nextest",
        ),
    },
    toolchains = ["@rules_rust//rust:toolchain_type"],
)

def nextest_test(name, args = [], **kwargs):
    """Runs the tests of a `rust_test` with cargo-nextest, as `cargo nextest run` would.

    The test's executable is the runner, whose `test` subcommand runs the tests, so the target's
    arguments start with it. Bazel adds the arguments from `--test_arg` after them.

    Args:
      name: The test's name.
      args: Arguments for `cargo nextest run`.
      **kwargs: The attributes of `_nextest_test`.
    """
    _nextest_test(name = name, args = ["test"] + args, **kwargs)
