//! The docs and skills that mirrord's MCP server answers from, and the YAML parser it reads
//! configs with.

use std::io::Read;

use flate2::read::GzDecoder;

/// The `docs` and `skills` directories of `corpus/`, as a gzipped tar with paths relative to
/// `corpus/`.
///
/// Bazel packs the archive and passes its path to rustc in `MIRRORD_MCP_CORPUS`. The corpus changes
/// rarely, so the crate always embeds it, and builds again when it changes. A Cargo build has no
/// step that packs it, so `.cargo/config.toml` points the variable at an empty placeholder.
static ARCHIVE: &[u8] = include_bytes!(env!("MIRRORD_MCP_CORPUS"));

/// Returns the path and contents of every file in the corpus.
pub fn corpus() -> Vec<(String, String)> {
    let mut archive = tar::Archive::new(GzDecoder::new(ARCHIVE));
    archive
        .entries()
        .expect("the corpus is a tar")
        .map(|entry| {
            let mut entry = entry.expect("the corpus's entries are readable");
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            let mut contents = String::new();
            entry.read_to_string(&mut contents).unwrap();
            (path, contents)
        })
        .collect()
}

/// Returns the kind of the root node of `yaml`'s syntax tree.
pub fn yaml_root_kind(yaml: &str) -> String {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_yaml::LANGUAGE.into())
        .expect("the YAML grammar matches tree-sitter's version");
    let tree = parser.parse(yaml, None).expect("the parser has a language");
    tree.root_node().kind().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_corpus() {
        let paths: Vec<_> = corpus().into_iter().map(|(path, _)| path).collect();
        assert_eq!(paths, ["docs/getting-started.md", "skills/debug-a-pod.md"]);
    }

    #[test]
    fn parses_yaml() {
        assert_eq!(yaml_root_kind("target: pod/app\n"), "stream");
    }
}
