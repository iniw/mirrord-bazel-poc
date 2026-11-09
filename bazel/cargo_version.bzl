"""Makes the workspace version from `Cargo.toml` available to BUILD files.

Cargo sets `CARGO_PKG_VERSION` from each package's version, and mirrord uses it, for example, to
pick the agent's image tag. rules_rust sets it from the `version` attribute instead, which defaults
to `0.0.0`, and the `@crates` hub doesn't provide the versions of the workspace's own crates. So
this repository rule reads `version` from `[workspace.package]` in the root `Cargo.toml`, the one
place that holds it, and `rust.bzl` passes it to every Rust target.

The rule writes the version to `@mirrord_version//:version.bzl`, from which BUILD files load it as
`VERSION`. It has to be a repository rule, because a BUILD file can only load a file that exists
while Bazel reads the BUILD file. Bazel runs a repository rule as soon as a BUILD file loads from
its repository, while a build action only runs after Bazel has read the BUILD files.
"""

def _cargo_version_impl(rctx):
    version = None
    in_workspace_package = False
    for line in rctx.read(rctx.attr.cargo_toml).splitlines():
        line = line.strip()
        if line.startswith("["):
            in_workspace_package = line == "[workspace.package]"
            continue
        key, _, value = line.partition("=")
        if in_workspace_package and key.strip() == "version":
            version = value.strip().strip('"')

    if not version:
        fail("`[workspace.package]` in {} has no `version`".format(rctx.attr.cargo_toml))

    rctx.file("BUILD.bazel", "")
    rctx.file("version.bzl", 'VERSION = "{}"\n'.format(version))
    return rctx.repo_metadata(reproducible = True)

cargo_version = repository_rule(
    implementation = _cargo_version_impl,
    attrs = {
        "cargo_toml": attr.label(allow_single_file = True, mandatory = True),
    },
)
