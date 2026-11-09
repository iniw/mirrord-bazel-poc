//! The agent, reduced to what shows how it builds: it uses the containerd client, whose gRPC code a
//! build script generates with `protoc`, and reports its version, which the CLI's image tag for it
//! comes from.

#[cfg(target_os = "linux")]
fn main() {
    println!("agent ready - version {}", env!("CARGO_PKG_VERSION"));
    println!(
        "{:?}",
        containerd_client::services::v1::VersionResponse::default()
    );
}

/// The agent only runs in Linux pods. Bazel never builds it for other platforms, but Cargo builds
/// the whole workspace for the host, so the agent needs a `main` there too.
#[cfg(not(target_os = "linux"))]
fn main() {
    println!("mirrord-agent is only supported on Linux");
    std::process::exit(1);
}
