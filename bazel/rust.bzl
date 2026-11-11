"""The Rust rules, with the attributes that Cargo would take from each crate's `Cargo.toml`.

A `.bzl` file holds Starlark code that BUILD files `load()`. The public functions here are macros:
when Bazel reads a BUILD file that calls one, the macro runs and declares targets with rules_rs's
copies of rules_rust's rules, much as a CMake function declares targets with `add_library`. See
https://bazel.build/extending/macros.

rules_rust doesn't read `Cargo.toml`, so each target has to state the crate name, edition, version,
features and lints that Cargo would take from it. These macros take them from the `@crates` hub,
which rules_rs generates from `Cargo.toml`, and from `@mirrord_version`. Every Rust target in the
workspace uses these macros instead of the rules from rules_rs, so that it builds with the same
settings as under Cargo, and its BUILD file doesn't repeat them.

The macros also add what the workspace needs on top of Cargo's settings. They fail the build on
warnings, and turn off a linker warning that the toolchain's flags cause (see `RUSTC_FLAGS`). They
also give each crate a `<name>_doc` target that runs rustdoc on it, which
`bazel build --config=doc //...` builds.
"""

load("@crates//:data.bzl", "DEP_DATA")
load("@crates//:defs.bzl", "aliases", "crate_name", "edition", "lint_config")
load("@mirrord_version//:version.bzl", "VERSIONS")
load("@rules_rs//rs:rust_binary.bzl", _rust_binary = "rust_binary")
load("@rules_rs//rs:rust_library.bzl", _rust_library = "rust_library")
load("@rules_rs//rs:rust_shared_library.bzl", _rust_shared_library = "rust_shared_library")
load("@rules_rs//rs:rust_test.bzl", _rust_test = "rust_test")
load("@rules_rust//rust:defs.bzl", "rust_doc")
load(":nextest.bzl", "nextest_test")

# The flags that every Rust target of the workspace gets, like `build.rustflags` in
# `.cargo/config.toml`. Third-party crates and rules_rust's own Rust code don't get them.
#
# `-Dwarnings` is like `build.warnings = "deny"` in Cargo: every warning in the workspace's crates
# fails the build.
#
# `-Wno-unused-command-line-argument` turns off clang's warning about unused arguments in links.
# The llvm toolchain passes `--unwindlib=none` (Linux) or `-rtlib=compiler-rt` (macOS) to every
# link. rustc links through clang with `-nodefaultlibs`, which makes clang ignore those flags and
# warn that they're unused, and rustc turns the warning into a `linker_messages` warning. The flags
# don't change the result, so the warning can be turned off safely. Links for MSVC don't go through
# clang, because the llvm toolchain's `lld-link` links them directly, and `lld-link` doesn't know
# the flag, so builds for MSVC leave it out. On Linux and macOS, `.bazelrc` passes the flag to the
# links of the tools that run during the build, like proc macros.
RUSTC_FLAGS = ["-Dwarnings"] + select({
    "@llvm//constraints/windows/abi:msvc": [],
    "//conditions:default": ["-Clink-arg=-Wno-unused-command-line-argument"],
})

def _crate_features():
    """Returns the package's features, unified across the workspace as Cargo unifies them.

    rules_rs resolves them, but unlike the edition, the `@crates` hub has no function that returns
    them, so this reads them from the hub's data.
    """
    dep_data = DEP_DATA.get(native.package_name())
    if not dep_data:
        return []

    features = dep_data["crate_features"]
    features_by_platform = dep_data["crate_features_by_platform"]
    if not features_by_platform:
        return features
    return features + select(features_by_platform | {"//conditions:default": []})

def _version():
    """Returns the version of the package's crate from its `Cargo.toml`, as Cargo gives it.

    Most crates take the workspace's version, but some have their own, like `mirrord-protocol`,
    whose version is the protocol version that the agent and the CLI negotiate.
    """
    package = native.package_name()
    if package not in VERSIONS:
        fail("{} isn't a member of the Cargo workspace, so it has no version".format(package))
    return VERSIONS[package]

def _cargo_attrs(kwargs, crate_name_from_cargo = True):
    """Returns `kwargs` with the attributes that the package's `Cargo.toml` decides.

    An attribute in `kwargs` overrides the one from `Cargo.toml`, except `rustc_flags` and
    `crate_features`, which are added to the ones here.
    """
    kwargs = dict(kwargs)
    attrs = {
        "aliases": aliases(),
        "rustc_flags": RUSTC_FLAGS + kwargs.pop("rustc_flags", []),
        "crate_features": _crate_features() + kwargs.pop("crate_features", []),
        "edition": edition(),
        "lint_config": lint_config(),
        "version": _version(),
    }
    if crate_name_from_cargo:
        attrs["crate_name"] = crate_name()
    return attrs | kwargs

def _cargo_files():
    """Declares the package's `cargo_files`, unless an earlier macro in the package did.

    `cargo_files` holds the files that `cargo metadata` reads to describe the package: its
    `Cargo.toml` and the sources of the targets that Cargo discovers. `//:cargo_metadata` takes
    them from every package, for nextest (see `//bazel:nextest.bzl`).
    """
    if native.existing_rule("cargo_files"):
        return
    native.filegroup(
        name = "cargo_files",
        srcs = native.glob(
            [
                "Cargo.toml",
                "build.rs",
                "src/lib.rs",
                "src/main.rs",
                "src/bin/*.rs",
                "src/bin/*/main.rs",
                "tests/*.rs",
                "tests/*/main.rs",
                "examples/*.rs",
                "examples/*/main.rs",
                "benches/*.rs",
                "benches/*/main.rs",
            ],
            allow_empty = True,
        ),
        visibility = ["//:__pkg__"],
    )

def _rust_doc(name, crate = None, **kwargs):
    rust_doc(
        name = name + "_doc",
        crate = crate or ":" + name,
        # rustdoc doesn't take the crate's features from the crate, so it would leave out the items
        # behind them.
        crate_features = _crate_features(),
        rustdoc_flags = ["-Dwarnings"],
        # `.bazelrc` leaves out targets with this tag, unless `--config=doc` selects only them.
        tags = ["rustdoc"],
        target_compatible_with = kwargs.get("target_compatible_with"),
        visibility = ["//visibility:private"],
    )

def rust_binary(name, doc = True, **kwargs):
    """A binary crate, and with `doc`, its `<name>_doc` target.

    Args:
      name: The binary's name.
      doc: Whether to document the crate. A second binary of the same sources, like
        `//mirrord/cli:cli_standalone`, would document the same crate again.
      **kwargs: The attributes of rules_rust's `rust_binary`.
    """
    _cargo_files()
    _rust_binary(name = name, **_cargo_attrs(kwargs))
    if doc:
        _rust_doc(name, **kwargs)

def rust_library(name, **kwargs):
    _cargo_files()
    _rust_library(name = name, **_cargo_attrs(kwargs))
    _rust_doc(name, **kwargs)

def rust_shared_library(name, **kwargs):
    """A shared library crate, like a `cdylib`, and its `<name>_doc` target.

    Args:
      name: The library's name.
      **kwargs: The attributes of rules_rust's `rust_shared_library`.
    """
    _cargo_files()
    attrs = _cargo_attrs(kwargs)
    _rust_shared_library(name = name, **attrs)

    # `rust_doc` only takes targets that provide a crate to other Rust code, which a shared library
    # doesn't. So its docs come from a library of the same sources, which only the docs build
    # compiles.
    _rust_library(
        name = name + "_doc_crate",
        tags = ["manual"],
        visibility = ["//visibility:private"],
        **{
            key: value
            for key, value in attrs.items()
            if key not in ["cc_runtime_linkage", "tags", "visibility"]
        }
    )
    _rust_doc(name, crate = ":" + name + "_doc_crate", **kwargs)

# The attributes that apply to a test as a whole, which `rust_test` gives the `nextest_test` that
# Bazel runs rather than the `rust_test` that builds the binary.
_TEST_ATTRS = [
    "args",
    "flaky",
    "local",
    "shard_count",
    "size",
    "timeout",
    "visibility",
]

def rust_test(name, **kwargs):
    """A test crate: a file in `tests/`, or a crate's own `#[test]` functions with `crate`.

    The crate of a file in `tests/` is named after the file, as Cargo names it. A test of a crate's
    own functions takes everything but the version and the features from that crate. It gets the
    features from `Cargo.toml`, as the crate does, and `crate_features` must add the ones that the
    crate's target adds.

    `<name>` runs the tests with cargo-nextest, as `cargo nextest run` does (see
    `//bazel:nextest.bzl`), so each test runs in a process of its own, and `.config/nextest.toml`
    sets the retries and the timeouts. `<name>_libtest` builds the test binary, and runs it on its
    own, with all tests in one process. `.bazelrc` leaves `<name>_libtest` out of `bazel test`,
    because the tests would run twice.

    A test is small unless it says otherwise. Bazel would make it medium, with a 5-minute timeout,
    and then warn that the tests finish far faster than that.

    Args:
      name: The test's name.
      **kwargs: The attributes of rules_rust's `rust_test`.
    """
    _cargo_files()
    kwargs.setdefault("size", "small")
    test_attrs = {attr: kwargs.pop(attr) for attr in _TEST_ATTRS if attr in kwargs}
    tags = kwargs.pop("tags", [])
    libtest = name + "_libtest"

    package_data = DEP_DATA.get(native.package_name(), {})
    package = crate_name().replace("_", "-")
    if "crate" not in kwargs:
        kind = "test"
        binary_name = name
        binary_id = package + "::" + name
    elif package_data.get("binaries"):
        # The tests of a binary crate. The package's only binary is the crate, as in `mirrord/cli`.
        kind = "bin"
        binary_name = package_data["binaries"].keys()[0]
        binary_id = package + "::bin/" + binary_name
    else:
        kind = "lib"
        binary_name = crate_name()
        binary_id = package

    # The binary's target takes the test's size, so that running it on its own doesn't warn.
    libtest_attrs = {
        "size": test_attrs.get("size"),
        "tags": tags + ["libtest"],
        "visibility": ["//visibility:private"],
    }
    if "crate" in kwargs:
        _rust_test(
            name = libtest,
            crate_features = _crate_features() + kwargs.pop("crate_features", []),
            rustc_flags = RUSTC_FLAGS + kwargs.pop("rustc_flags", []),
            version = _version(),
            **(kwargs | libtest_attrs)
        )
    else:
        _rust_test(
            name = libtest,
            # The crate is named after the test, as Cargo names it, rather than after this target.
            crate_name = name.replace("-", "_"),
            **_cargo_attrs(kwargs | libtest_attrs, crate_name_from_cargo = False)
        )

    nextest_test(
        name = name,
        binary_id = binary_id,
        binary_name = binary_name,
        kind = kind,
        manifest = ":Cargo.toml",
        tags = tags,
        target_compatible_with = kwargs.get("target_compatible_with"),
        test = ":" + libtest,
        **test_attrs
    )
