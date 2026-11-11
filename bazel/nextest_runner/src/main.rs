//! Runs the tests of a `rust_test` binary with cargo-nextest under Bazel, and writes the Cargo
//! metadata that nextest needs to do so. `//bazel:nextest.bzl` explains the design.
//!
//! The program has two modes, one for each subcommand:
//!
//! - `metadata` runs `cargo metadata --no-deps` for the workspace, as a build action of
//!   `cargo_metadata`. It keeps only what nextest reads, and replaces the workspace's root with
//!   placeholders, because the root is a sandbox path that's gone after the action.
//! - `test` is the executable of a `nextest_test`, whose `args` start with it. It reads its
//!   settings from the `BAZEL_NEXTEST_*` variables that the rule sets, translates Bazel's test
//!   protocol (<https://bazel.build/reference/test-encyclopedia>) into nextest's options, and runs
//!   nextest. The arguments after it, from `--test_arg`, go to `cargo nextest run`.
//!
//! It's a Rust program rather than a shell script so that it runs on Windows machines, which may
//! not have a shell.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};

/// The placeholder for the workspace's root as a file system path.
const ROOT_PLACEHOLDER: &str = "@@WORKSPACE_ROOT@@";

/// The placeholder for the workspace's root as the path of a `file://` URL, which Cargo's package
/// IDs hold. It differs from the file system path on Windows, where it starts with `/C:/`.
const ROOT_URL_PLACEHOLDER: &str = "@@WORKSPACE_ROOT_URL@@";

/// The prefix of the variables that `nextest_test` passes its settings in.
const SETTINGS_PREFIX: &str = "BAZEL_NEXTEST_";

/// The fields of a package in `cargo metadata`'s output that nextest reads. nextest checks every
/// `package()` and `binary()` filter in `.config/nextest.toml` against the names of the workspace's
/// packages and their targets, so the file must list all of them, but it reads nothing else of the
/// packages that it doesn't run. Dropping the rest keeps the file the same when it changes, so
/// Bazel doesn't run every test again.
const PACKAGE_FIELDS: &[&str] = &[
    "name",
    "version",
    "id",
    "source",
    "manifest_path",
    "edition",
    "targets",
];

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand)]
enum Mode {
    /// Writes the workspace's Cargo metadata for nextest.
    Metadata {
        /// The `cargo` program.
        #[arg(long)]
        cargo: PathBuf,

        /// The workspace's root `Cargo.toml`.
        #[arg(long)]
        manifest: PathBuf,

        /// The file to write.
        #[arg(long)]
        output: PathBuf,
    },

    /// Runs a test binary with nextest, as the executable of a `nextest_test`.
    Test(TestSettings),
}

/// The settings of a `nextest_test`, which the rule passes in the environment. The files are
/// runfiles paths, like `_main/Cargo.toml`.
#[derive(Args)]
struct TestSettings {
    /// The cargo-nextest program.
    #[arg(long, env = "BAZEL_NEXTEST_NEXTEST")]
    nextest: String,

    /// The test binary.
    #[arg(long, env = "BAZEL_NEXTEST_BINARY")]
    binary: String,

    /// The binary's ID in nextest, like `mirrord-layer-tests::c_stat`.
    #[arg(long, env = "BAZEL_NEXTEST_BINARY_ID")]
    binary_id: String,

    /// The name of the binary's Cargo target.
    #[arg(long, env = "BAZEL_NEXTEST_BINARY_NAME")]
    binary_name: String,

    /// The kind of the binary's Cargo target, like `test` or `lib`.
    #[arg(long, env = "BAZEL_NEXTEST_BINARY_KIND")]
    binary_kind: String,

    /// The output of the `metadata` mode.
    #[arg(long, env = "BAZEL_NEXTEST_CARGO_METADATA")]
    cargo_metadata: String,

    /// `.config/nextest.toml`.
    #[arg(long, env = "BAZEL_NEXTEST_CONFIG")]
    config: String,

    /// The `Cargo.toml` at the workspace's root in the runfiles, which nextest checks for.
    #[arg(long, env = "BAZEL_NEXTEST_WORKSPACE_MANIFEST")]
    workspace_manifest: String,

    /// The directory of the binary's Cargo package, like `mirrord/layer-tests`.
    #[arg(long, env = "BAZEL_NEXTEST_PACKAGE_DIRECTORY")]
    package_directory: String,

    /// The platform that the tests run on, like `x86_64-unknown-linux-gnu`.
    #[arg(long, env = "BAZEL_NEXTEST_TRIPLE")]
    triple: String,

    /// The arguments for `cargo nextest run`, from `--test_arg`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    nextest_args: Vec<OsString>,
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();

    match cli.mode {
        Mode::Metadata {
            cargo,
            manifest,
            output,
        } => write_metadata(&cargo, &manifest, &output),
        Mode::Test(settings) => run_tests(settings),
    }
}

/// Returns `path` as the path of a `file://` URL, with forward slashes, and on Windows with a
/// leading slash before the drive.
fn url_path(path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");

    if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    }
}

/// Replaces each `from` with its `to` in every string of `value`, in order.
fn replace_strings(value: &mut Value, replacements: &[(&str, &str)]) {
    match value {
        Value::String(string) => {
            for (from, to) in replacements {
                if string.contains(from) {
                    *string = string.replace(from, to);
                }
            }
        }

        Value::Array(values) => {
            for value in values {
                replace_strings(value, replacements);
            }
        }

        Value::Object(fields) => {
            for value in fields.values_mut() {
                replace_strings(value, replacements);
            }
        }

        _ => {}
    }
}

/// The `metadata` mode.
fn write_metadata(cargo: &Path, manifest: &Path, output: &Path) -> anyhow::Result<ExitCode> {
    let root = env::current_dir().context("can't read the current directory")?;

    // `--no-deps` describes only the workspace's packages, which `Cargo.toml` files and the target
    // sources that Cargo discovers are enough for, so the action needs neither the network nor
    // Cargo's registry. Cargo writes its cache to `CARGO_HOME`, so that goes to a directory of the
    // action's own.
    let cargo_home = root.join("cargo_home");
    let out = Command::new(root.join(cargo))
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(root.join(manifest))
        .env("CARGO_HOME", &cargo_home)
        .output()
        .with_context(|| format!("can't run {}", cargo.display()))?;

    if !out.status.success() {
        bail!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let _ = fs::remove_dir_all(&cargo_home);

    let mut metadata: Value =
        serde_json::from_slice(&out.stdout).context("cargo metadata's output isn't valid JSON")?;

    let packages = metadata
        .get_mut("packages")
        .and_then(Value::as_array_mut)
        .context("cargo metadata's output has no packages")?;

    for package in packages {
        if let Some(fields) = package.as_object_mut() {
            fields.retain(|field, _| PACKAGE_FIELDS.contains(&field.as_str()));

            // nextest's parser requires a package's dependencies and features, but doesn't need
            // their contents, which change most often of all.
            fields.insert("dependencies".to_owned(), json!([]));
            fields.insert("features".to_owned(), json!({}));
        }
    }

    let root_url = format!("file://{}", url_path(&root));
    let root_url_placeholder = format!("file://{ROOT_URL_PLACEHOLDER}");
    let root = root.display().to_string();

    replace_strings(
        &mut metadata,
        &[
            (root_url.as_str(), root_url_placeholder.as_str()),
            (root.as_str(), ROOT_PLACEHOLDER),
        ],
    );

    fs::write(output, metadata.to_string())
        .with_context(|| format!("can't write {}", output.display()))?;

    Ok(ExitCode::SUCCESS)
}

/// Returns a directory of its own for nextest's files. `bazel test` gives the test a temporary
/// directory, and `bazel run`, or a copy of the runfiles on another machine, doesn't.
fn work_directory() -> anyhow::Result<PathBuf> {
    let base = match env::var_os("TEST_TMPDIR") {
        Some(directory) => PathBuf::from(directory),
        None => {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |time| time.as_nanos());
            env::temp_dir().join(format!("nextest-runner-{}-{nanos}", std::process::id()))
        }
    };

    let work = base.join("nextest");
    fs::create_dir_all(&work).with_context(|| format!("can't create {}", work.display()))?;

    Ok(work)
}

/// Returns the path of the first file under `directory` whose extension is `extension`.
fn find_file(directory: &Path, extension: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(directory).ok()?.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(found) = find_file(&path, extension) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|found| found == extension) {
            return Some(path);
        }
    }

    None
}

/// Returns `program` in a form that Windows can start when its path is long.
///
/// `CreateProcessW` only starts a program with a long path when the path is verbatim: absolute,
/// and prefixed with `\\?\`. The standard library makes long paths verbatim when it opens files,
/// but not when it starts a program, and a program in a runfiles directory easily has a long path,
/// because the directory is deep in Bazel's output tree. A short path stays as it is, because a
/// program may not expect a verbatim path as its own.
///
/// This doesn't help with the working directory of the program, which Windows doesn't take as a
/// verbatim path. The tests' working directories, their packages in the runfiles, stay well below
/// the limit under Bazel's default output root.
fn startable(program: PathBuf) -> PathBuf {
    /// The length from which Bazel treats a program's path as long: Windows' `MAX_PATH`, 260,
    /// less the 4 characters of the prefix that Windows adds to the path
    /// (<https://github.com/bazelbuild/bazel/issues/12310>).
    const LONG_PATH: usize = 256;

    if !cfg!(windows) {
        return program;
    }

    // Windows reads the `/` and `..` of a verbatim path literally, so the path has to be
    // normalized, which making it absolute also does.
    let Ok(absolute) = std::path::absolute(&program) else {
        return program;
    };

    let path = absolute.as_os_str();

    // A path that starts with `\\` is verbatim already, or names a network share, which takes
    // another prefix.
    if path.len() < LONG_PATH || path.as_encoded_bytes().starts_with(br"\\") {
        return program;
    }

    let mut verbatim = OsString::from(r"\\?\");
    verbatim.push(path);

    PathBuf::from(verbatim)
}

/// Keeps this program running when it gets a signal that asks it to stop, so that it can still
/// copy the JUnit report after nextest exits.
///
/// When a test times out, Bazel sends SIGTERM to this program and to nextest, which are in one
/// process group, and kills what's left of the group after a grace period
/// (`--local_termination_grace_seconds`, 15 seconds by default). `bazel run` does the same with
/// SIGINT on Ctrl-C. nextest reacts by stopping the tests that are running, and it then writes the
/// JUnit report with the tests that finished. The default reaction of this program would be to exit
/// before nextest has written the report, and Bazel would then write a `test.xml` with a single
/// test case for the whole target.
///
/// The handler does nothing, rather than the signal being ignored, because a process that this one
/// starts inherits the signals that it ignores, but not its handlers. nextest and the tests must
/// still get the signals. Windows has no such signals: Bazel ends a test there without one.
#[cfg(unix)]
fn outlive_termination_signals() {
    extern "C" fn do_nothing(_signal: libc::c_int) {}

    let handler = do_nothing as extern "C" fn(libc::c_int);

    for signal in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        // SAFETY: `do_nothing` has the signature of a signal handler, and does nothing, which is
        // safe at any point of the program.
        unsafe { libc::signal(signal, handler as libc::sighandler_t) };
    }
}

/// The test mode.
fn run_tests(settings: TestSettings) -> anyhow::Result<ExitCode> {
    // Bazel names the runfiles in the environment. A copy of the test and its runfiles, run
    // without Bazel, finds them next to the program.
    let runfiles = env::var_os("RUNFILES_DIR")
        .or_else(|| env::var_os("TEST_SRCDIR"))
        .map(PathBuf::from)
        .or_else(|| {
            env::args_os()
                .next()
                .map(|program| PathBuf::from(format!("{}.runfiles", program.to_string_lossy())))
        })
        .context("the runfiles aren't in the environment or next to the program")?;

    // Runfiles paths separate their components with `/`, which Windows doesn't always accept in
    // the middle of a path with `\`, so each component is joined on its own.
    let runfile = |path: &str| {
        path.split('/')
            .fold(runfiles.clone(), |runfile, component| {
                runfile.join(component)
            })
    };

    let nextest = runfile(&settings.nextest);
    let binary = runfile(&settings.binary);
    let config = runfile(&settings.config);
    let workspace_manifest = runfile(&settings.workspace_manifest);

    let workspace_root = workspace_manifest
        .parent()
        .context("the workspace's manifest has no directory")?
        .to_path_buf();

    let package_directory = &settings.package_directory;
    let binary_id = &settings.binary_id;
    let work = work_directory()?;

    // The output of `cargo metadata` for the workspace, with its root in the runfiles. nextest
    // runs the tests in the directory of their package's manifest, which is in the runfiles too.
    let metadata_path = runfile(&settings.cargo_metadata);
    let metadata = fs::read(&metadata_path)
        .with_context(|| format!("can't read {}", metadata_path.display()))?;

    let mut metadata: Value = serde_json::from_slice(&metadata)
        .with_context(|| format!("{} isn't valid JSON", metadata_path.display()))?;

    // Cargo's ID of the package that the binary belongs to, which starts with the package's
    // directory.
    let id_prefix = format!("path+file://{ROOT_URL_PLACEHOLDER}/{package_directory}#");
    let package_id = metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| package.get("id").and_then(Value::as_str))
        .find(|id| id.starts_with(&id_prefix))
        .with_context(|| format!("{package_directory} isn't a package of the Cargo workspace"))?
        .to_owned();

    // The metadata and the package's ID, with the workspace's root in the runfiles.
    let root_url = url_path(&workspace_root);
    let root = workspace_root.display().to_string();
    let replacements = [
        (ROOT_URL_PLACEHOLDER, root_url.as_str()),
        (ROOT_PLACEHOLDER, root.as_str()),
    ];

    replace_strings(&mut metadata, &replacements);
    let package_id = package_id.replace(ROOT_URL_PLACEHOLDER, &root_url);

    let cargo_metadata = work.join("cargo-metadata.json");
    fs::write(&cargo_metadata, metadata.to_string())
        .with_context(|| format!("can't write {}", cargo_metadata.display()))?;

    // The list of test binaries that `cargo nextest list --list-type binaries-only` would write
    // after a build. nextest puts the toolchain's library directory on the tests' library path,
    // for a standard library that's linked dynamically. Bazel links it statically, so the
    // directory is an empty one. nextest would warn about a missing one.
    let libdir = work.join("libdir");
    fs::create_dir_all(&libdir).with_context(|| format!("can't create {}", libdir.display()))?;

    let platform = json!({"triple": settings.triple, "target-features": "unknown"});

    let binaries_metadata = json!({
        "rust-build-meta": {
            "target-directory": work.join("target"),
            "base-output-directories": [],
            "non-test-binaries": {},
            "build-script-out-dirs": {},
            "linked-paths": [],
            "platforms": {
                "host": {
                    "platform": platform,
                    "libdir": {"status": "available", "path": libdir},
                },
                "targets": [],
            },
            "target-platforms": [platform],
        },
        "rust-binaries": {
            binary_id.clone(): {
                "binary-id": binary_id,
                "binary-name": settings.binary_name,
                "package-id": package_id,
                "kind": settings.binary_kind,
                "binary-path": binary,
                "build-platform": "target",
            },
        },
    });

    let binaries_metadata_path = work.join("binaries-metadata.json");
    fs::write(&binaries_metadata_path, binaries_metadata.to_string())
        .with_context(|| format!("can't write {}", binaries_metadata_path.display()))?;

    // Settings that take precedence over nextest's defaults, but not over `.config/nextest.toml`.
    // nextest writes its store, including the JUnit report, under the workspace's root by
    // default, which is in the runfiles, so it writes into the work directory instead. Every
    // profile writes a JUnit report, unless `.config/nextest.toml` says otherwise, because the
    // report becomes the target's `test.xml`. TOML's literal strings take Windows paths as they
    // are.
    let store = work.join("store");
    let tool_config = work.join("bazel.toml");

    fs::write(
        &tool_config,
        format!(
            "[store]\ndir = '{}'\n\n[profile.default.junit]\npath = \"junit.xml\"\n",
            store.display()
        ),
    )
    .with_context(|| format!("can't write {}", tool_config.display()))?;

    let mut command = Command::new(startable(nextest.clone()));

    command
        .args(["nextest", "run"])
        .arg("--cargo-metadata")
        .arg(&cargo_metadata)
        .arg("--binaries-metadata")
        .arg(&binaries_metadata_path)
        .arg("--config-file")
        .arg(config)
        .arg("--tool-config-file")
        .arg(format!("bazel:{}", tool_config.display()));

    // With `shard_count`, Bazel runs the target once for each shard. nextest sorts the tests and
    // deals them out to the shards in turn (`slice:`), so the shards hold the same number of
    // tests, give or take one. `hash:` would keep a test in its shard when other tests are added,
    // but that buys nothing here: every shard takes the same binary, so Bazel reruns all of them
    // when a test is added, and hashing leaves the shards of a small binary uneven. Bazel fails a
    // sharded test that doesn't touch the status file, because it would run every test in each
    // shard.
    if let (Ok(total), Ok(index)) = (env::var("TEST_TOTAL_SHARDS"), env::var("TEST_SHARD_INDEX")) {
        if let Some(status_file) = env::var_os("TEST_SHARD_STATUS_FILE") {
            fs::write(&status_file, "")
                .with_context(|| format!("can't write {}", Path::new(&status_file).display()))?;
        }

        let index: u32 = index.parse().context("TEST_SHARD_INDEX isn't a number")?;

        command
            .arg("--partition")
            .arg(format!("slice:{}/{total}", index + 1));
    }

    // `--test_runner_fail_fast` stops at the first test that fails, like nextest's `--fail-fast`.
    // Without it, nextest's own default applies, which also stops at the first failure, unlike
    // Bazel's other test runners, as `cargo nextest run` does.
    if env::var("TESTBRIDGE_TEST_RUNNER_FAIL_FAST").as_deref() == Ok("1") {
        command.arg("--fail-fast");
    }

    // `--test_filter` runs only the tests whose names contain it, as libtest's filter would. It
    // comes before the arguments from `--test_arg`, which may end with `--` and arguments for the
    // test binary.
    if let Ok(filter) = env::var("TESTBRIDGE_TEST_ONLY") {
        command.arg(filter);
    }

    command.args(&settings.nextest_args);

    // A target without tests passes, as a libtest binary without tests does. That includes a
    // shard without tests, and a target that `--test_filter` leaves no tests in.
    if env::var_os("NEXTEST_NO_TESTS").is_none() {
        command.env("NEXTEST_NO_TESTS", "pass");
    }

    // The tests find their data through the runfiles library, which needs to know where the
    // runfiles are, because nextest runs each test in its package's directory rather than in the
    // runfiles' root. The settings are this program's own, so the tests don't inherit them.
    command.env("RUNFILES_DIR", &runfiles);

    for (name, _) in env::vars_os() {
        if name.to_string_lossy().starts_with(SETTINGS_PREFIX) {
            command.env_remove(name);
        }
    }

    #[cfg(unix)]
    outlive_termination_signals();

    // nextest runs from the work directory, so that it doesn't read the workspace's
    // `.cargo/config.toml`, which configures Cargo's builds.
    let status = command
        .current_dir(&work)
        .status()
        .with_context(|| format!("can't run {}", nextest.display()))?;

    // Bazel keeps the JUnit report as the target's `test.xml`. Without it, Bazel would write a
    // report with a single test case for the whole target. The store holds no other XML file, so
    // the report is found by its extension, whatever `.config/nextest.toml` names it.
    if let (Some(xml_output), Some(report)) =
        (env::var_os("XML_OUTPUT_FILE"), find_file(&store, "xml"))
    {
        fs::copy(&report, &xml_output).with_context(|| {
            format!(
                "can't copy {} to {}",
                report.display(),
                Path::new(&xml_output).display()
            )
        })?;
    }

    Ok(match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => ExitCode::FAILURE,
    })
}
