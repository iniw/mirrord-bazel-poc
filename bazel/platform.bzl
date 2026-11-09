"""A filegroup whose files build for a given OS and CPU, such as one slice of a universal binary.

The filegroup builds its files with a transition, which changes a flag for those files only: here,
it sets `--platforms` to `//platforms:{cpu}_{os}`. See
https://bazel.build/extending/config#user-defined-transitions.

When the build already targets that OS and CPU, the files build for the build's own platform
instead. They're then the same build as the build's other targets, and share their outputs, while
a transition to another platform, even an equivalent one, would build them again in other output
directories.
"""

_PLATFORMS_FLAG = "//command_line_option:platforms"

def _platform_transition_impl(settings, attr):
    if not attr.target_platform:
        return {_PLATFORMS_FLAG: settings[_PLATFORMS_FLAG]}
    return {_PLATFORMS_FLAG: str(attr.target_platform)}

_platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = [_PLATFORMS_FLAG],
    outputs = [_PLATFORMS_FLAG],
)

def _platform_filegroup_impl(ctx):
    return [DefaultInfo(files = depset(transitive = [src[DefaultInfo].files for src in ctx.attr.srcs]))]

_platform_filegroup = rule(
    implementation = _platform_filegroup_impl,
    attrs = {
        "srcs": attr.label_list(cfg = _platform_transition),
        "target_platform": attr.label(),
    },
)

def platform_filegroup(name, srcs, os, cpu, **kwargs):
    """Builds `srcs` for `os` and `cpu`.

    Args:
      name: The filegroup's name.
      srcs: The targets whose files the filegroup provides.
      os: The OS to build `srcs` for: `linux`, `macos` or `windows`.
      cpu: The CPU to build `srcs` for: `aarch64` or `x86_64`.
      **kwargs: The attributes common to all rules, like `tags`.
    """
    _platform_filegroup(
        name = name,
        srcs = srcs,
        target_platform = select({
            "//platforms:is_{}_{}".format(os, cpu): None,
            "//conditions:default": "//platforms:{}_{}".format(cpu, os),
        }),
        **kwargs
    )
