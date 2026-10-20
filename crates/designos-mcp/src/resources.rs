//! MCP resource `designos://rules` (`text/markdown`) exposing the generated agent rules pack (E15-15).
use rmcp::model::{ListResourcesResult, ReadResourceResult, Resource, ResourceContents};
use std::collections::BTreeSet;

pub const DESIGNOS_RULES_URI: &str = "designos://rules";
pub const DESIGNOS_RULES_NAME: &str = "designos_rules";
pub const DESIGNOS_RULES_MIME: &str = "text/markdown";

/// Returns the catalog of MCP resources exposed by `designos-mcp`.
pub fn resources() -> Vec<Resource> {
    vec![Resource::new(DESIGNOS_RULES_URI, DESIGNOS_RULES_NAME)
        .with_title("DESIGN:OS Studio Agent Rules")
        .with_description("Generated design-agent rules pack, tool reference, NodeSpec schema, and build-verify-fix workflow.")
        .with_mime_type(DESIGNOS_RULES_MIME)]
}

/// Lists all MCP resources exposed by `designos-mcp`.
pub fn list_resources() -> ListResourcesResult {
    ListResourcesResult { resources: resources(), ..Default::default() }
}

/// Reads an MCP resource by URI, filtering the rules pack to `enabled_tools` when configured.
pub fn read_resource(uri: &str, enabled_tools: Option<&BTreeSet<String>>) -> Result<ReadResourceResult, String> {
    if uri != DESIGNOS_RULES_URI {
        return Err(format!("UNKNOWN_RESOURCE: unknown resource URI '{uri}'; valid_options: [\"{DESIGNOS_RULES_URI}\"]"));
    }
    let text = crate::rules::generate_rules_pack(enabled_tools)?;
    Ok(ReadResourceResult::new(vec![ResourceContents::text(text, DESIGNOS_RULES_URI).with_mime_type(DESIGNOS_RULES_MIME)]))
}
