//! mirrord's configuration file, and its JSON schema, which editors use to complete and check it.
//!
//! `mirrord-schema.json` at the root of the repository holds the schema. `bazel test //...` fails
//! when it's out of date, and `bazel run //:mirrord_schema` updates it.

use schemars::JsonSchema;
use serde::Deserialize;

/// The configuration file of mirrord.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerFileConfig {
    /// The pod, deployment or other resource to run in the context of, like `deployment/app`.
    pub target: Option<String>,

    /// Whether to run without a target, in the context of the cluster alone.
    #[serde(default)]
    pub targetless: bool,
}

/// Returns the JSON schema of [`LayerFileConfig`], as `mirrord-schema.json` holds it.
pub fn schema() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(LayerFileConfig))
        .expect("the schema serializes to JSON")
        + "\n"
}
