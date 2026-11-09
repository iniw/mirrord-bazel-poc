//! Writes the JSON schema of mirrord's configuration file to the path in its first argument.

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: schema <output path>");
    std::fs::write(path, mirrord_config::schema()).expect("the schema should write");
}
