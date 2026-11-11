"""Makes the versions from the Cargo workspace's manifests available to BUILD files.

Cargo sets `CARGO_PKG_VERSION` from each package's own version, and mirrord uses it, for example,
to pick the agent's image tag, and as the protocol version that `mirrord-protocol` negotiates,
which is versioned apart from the rest of the workspace. rules_rust sets it from the `version`
attribute instead, which defaults to `0.0.0`, and the `@crates` hub doesn't provide the versions of
the workspace's own crates. So this repository rule reads them from the manifests:

- `VERSION` is `version` in `[workspace.package]` of the root `Cargo.toml`, the version that
  mirrord releases under, which the UI takes.
- `VERSIONS` maps each workspace member's directory, like `mirrord/cli`, to the member's version,
  which `rust.bzl` gives the member's Rust targets. A member that inherits the workspace's version
  gets `VERSION`, and one without a version gets `0.0.0`, as with Cargo.

The rule writes them to `@mirrord_version//:version.bzl`. It has to be a repository rule, because a
BUILD file can only load a file that exists while Bazel reads the BUILD file. Bazel runs a
repository rule as soon as a BUILD file loads from its repository, while a build action only runs
after Bazel has read the BUILD files. Bazel runs the rule again when a manifest that it read
changes, or when a directory that a member pattern lists gains or loses an entry.

The rule only understands the parts of TOML that the workspace's manifests use: `key = "value"`
lines, `version.workspace = true` or `version = { workspace = true }`, and a `members` array that
can span several lines. A member
pattern can only use `*` as its whole last component, like `mirrord/*`.
"""

def _sections(text):
    """Returns the `key = value` lines of `text`, a TOML file, by the table that holds them.

    A value that opens an array and continues on the next lines, like `members = [`, is joined
    into one line.
    """
    sections = {}
    section = ""
    pending = None
    for line in text.splitlines():
        line = line.split("#")[0].strip()
        if pending != None:
            pending[1] += " " + line
            if "]" in line:
                sections.setdefault(section, {})[pending[0]] = pending[1].strip()
                pending = None
            continue
        if not line:
            continue
        if line.startswith("["):
            section = line.strip("[]").strip()
            continue
        key, _, value = line.partition("=")
        key, value = key.strip(), value.strip()
        if value.startswith("[") and "]" not in value:
            pending = [key, value]
            continue
        sections.setdefault(section, {})[key] = value

    return sections

def _strings(array):
    """Returns the strings in `array`, a TOML array of strings like `["a", "b"]`."""
    return [item.strip().strip('"') for item in array.strip("[] ").split(",") if item.strip()]

def _member_directories(rctx, root, patterns):
    """Returns the member directories that `patterns`, Cargo's `members`, name under `root`."""
    directories = []
    for pattern in patterns:
        if "*" not in pattern:
            directories.append(pattern)
            continue

        parent, _, last = pattern.rpartition("/")
        if last != "*" or "*" in parent:
            fail("`{}`: only `*` as the whole last component is supported".format(pattern))
        for entry in rctx.path(root + "/" + parent).readdir():
            if entry.get_child("Cargo.toml").exists:
                directories.append(parent + "/" + entry.basename)

    return directories

def _cargo_version_impl(rctx):
    root_manifest = rctx.path(rctx.attr.cargo_toml)
    root = str(root_manifest.dirname)
    workspace = _sections(rctx.read(root_manifest))

    version = workspace.get("workspace.package", {}).get("version", "").strip('"')
    if not version:
        fail("`[workspace.package]` in {} has no `version`".format(rctx.attr.cargo_toml))

    versions = {}
    patterns = _strings(workspace.get("workspace", {}).get("members", "[]"))
    for directory in _member_directories(rctx, root, patterns):
        package = _sections(rctx.read(root + "/" + directory + "/Cargo.toml")).get("package", {})
        if package.get("version.workspace") == "true" or "workspace" in package.get("version", ""):
            versions[directory] = version
        else:
            versions[directory] = package.get("version", '"0.0.0"').strip('"')

    rctx.file("BUILD.bazel", "")
    rctx.file("version.bzl", "VERSION = {}\nVERSIONS = {}\n".format(
        repr(version),
        repr(versions),
    ))
    return rctx.repo_metadata(reproducible = True)

cargo_version = repository_rule(
    implementation = _cargo_version_impl,
    attrs = {
        "cargo_toml": attr.label(allow_single_file = True, mandatory = True),
    },
)
