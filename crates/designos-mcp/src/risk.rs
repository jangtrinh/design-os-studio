//! Worst-case static risk for every declared MCP tool.
//!
//! The tier does not change per call. `designos_open_file` and
//! `designos_new_document` are additive because discarding unsaved work
//! requires an explicit `discard_changes: true` opt-in. Without that opt-in,
//! a dirty session refuses the call. `designos_save_file` is destructive because
//! `overwrite: true` replaces a file. `designos_apply_batch` is destructive
//! because an op can delete. `designos_export_fig_file` is destructive because
//! the session truncates an existing `.fig`.
//!
//! `External` is reserved for a tool that reads or writes outside the document
//! and the workspace, such as a network fetch. No declared tool is external.
//! `designos_import_web_capture` reads a workspace file or inline JSON.

use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiskTier {
    Read,
    Additive,
    Modify,
    Destructive,
    External,
}

impl RiskTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Additive => "additive",
            Self::Modify => "modify",
            Self::Destructive => "destructive",
            Self::External => "external",
        }
    }

    /// MCP `readOnlyHint`.
    pub fn read_only(self) -> bool {
        matches!(self, Self::Read)
    }

    pub fn destructive(self) -> bool {
        !matches!(self, Self::Read | Self::Additive)
    }

    /// MCP `openWorldHint`.
    pub fn open_world(self) -> bool {
        matches!(self, Self::External)
    }

    /// Higher rank asks more. A mixed batch uses the highest rank in the batch.
    pub fn rank(self) -> u8 {
        match self {
            Self::Read => 0,
            Self::Additive => 1,
            Self::Modify => 2,
            Self::Destructive => 3,
            Self::External => 4,
        }
    }

    pub fn strictest(self, other: Self) -> Self {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRisk {
    pub tier: RiskTier,
    pub idempotent: bool,
}

/// Canonical `designos_*` rows. Aliases resolve in [`canonical`] and share the target row.
const TOOLS: &[(&str, RiskTier, bool)] = &[
    ("designos_get_document", RiskTier::Read, true),
    ("designos_get_libraries", RiskTier::Read, true),
    ("designos_get_metadata", RiskTier::Read, true),
    ("designos_search_design_system", RiskTier::Read, true),
    ("designos_list_nodes", RiskTier::Read, true),
    ("designos_get_node", RiskTier::Read, true),
    ("designos_get_variables", RiskTier::Read, true),
    ("designos_generate_code", RiskTier::Read, true),
    ("designos_create_node", RiskTier::Additive, false),
    ("designos_update_node", RiskTier::Modify, true),
    ("designos_delete_node", RiskTier::Destructive, false),
    ("designos_list_comments", RiskTier::Read, true),
    ("designos_add_comment", RiskTier::Additive, false),
    ("designos_reply_comment", RiskTier::Modify, false),
    ("designos_resolve_comment", RiskTier::Modify, true),
    ("designos_apply_spec", RiskTier::Additive, false),
    ("designos_export_spec", RiskTier::Read, true),
    ("designos_set_dev_status", RiskTier::Modify, false),
    ("designos_export_fig_file", RiskTier::Destructive, true),
    ("designos_query_xpath", RiskTier::Read, true),
    ("designos_lint", RiskTier::Read, true),
    ("designos_get_guidelines", RiskTier::Read, true),
    ("designos_import_web_capture", RiskTier::Additive, false),
    ("designos_undo", RiskTier::Modify, false),
    ("designos_lint_fix", RiskTier::Modify, false),
    ("designos_redo", RiskTier::Modify, false),
    ("designos_get_screenshot", RiskTier::Read, true),
    ("designos_set_in_progress", RiskTier::Modify, true),
    ("designos_apply_batch", RiskTier::Destructive, false),
    ("designos_get_design_context", RiskTier::Read, true),
    ("designos_configure_session", RiskTier::Modify, true),
    ("designos_get_app_state", RiskTier::Read, true),
    ("designos_open_file", RiskTier::Additive, false),
    ("designos_save_file", RiskTier::Destructive, false),
    ("designos_render_png", RiskTier::Additive, false),
    ("designos_new_document", RiskTier::Additive, false),
    ("designos_close_document", RiskTier::Modify, false),
];

fn canonical(name: &str) -> &str {
    match name {
        "query_nodes" => "designos_query_xpath",
        "check_design" => "designos_lint",
        "export_fig_file" => "designos_export_fig_file",
        "get_guidelines" => "designos_get_guidelines",
        "get_metadata" => "designos_get_metadata",
        "get_screenshot" => "designos_get_screenshot",
        "get_design_context" => "designos_get_design_context",
        "import_web_capture" => "designos_import_web_capture",
        "set_in_progress" => "designos_set_in_progress",
        other => other,
    }
}

pub fn tool_risk(name: &str) -> Option<ToolRisk> {
    let name = canonical(name);
    TOOLS.iter().find(|(row, _, _)| *row == name).map(|&(_, tier, idempotent)| ToolRisk { tier, idempotent })
}

/// Bare name of the row `name` resolves to, or `name` when it is not in the table.
pub fn canonical_tool_name(name: &str) -> String {
    let name = canonical(name);
    TOOLS.iter().find(|(row, _, _)| *row == name).map(|(row, _, _)| (*row).to_string()).unwrap_or_else(|| name.to_string())
}

/// Declared names and aliases whose tier matches `pred`. The approval hook projects this list. It does not keep a second table.
pub fn names_where_tier(mut pred: impl FnMut(RiskTier) -> bool) -> Vec<&'static str> {
    let mut names = Vec::new();
    for &(name, tier, _) in TOOLS {
        if pred(tier) {
            names.push(name);
        }
    }
    for alias in crate::engine::ALIASES {
        if tool_risk(alias).is_some_and(|risk| pred(risk.tier)) {
            names.push(*alias);
        }
    }
    names
}

/// Writes `annotations` and `_meta.risk_tier` from [`tool_risk`].
/// Panics when `name` has no row, so a new tool cannot be listed unclassified.
pub fn stamp_tool_json(tool: &mut Value) {
    let name = tool.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let Some(risk) = tool_risk(&name) else {
        panic!("MCP tool `{name}` has no risk tier");
    };
    let obj = tool.as_object_mut().unwrap_or_else(|| panic!("MCP tool `{name}` is not a JSON object"));
    obj.insert(
        "annotations".into(),
        json!({
            "readOnlyHint": risk.tier.read_only(),
            "destructiveHint": risk.tier.destructive(),
            "idempotentHint": risk.idempotent,
            "openWorldHint": risk.tier.open_world(),
        }),
    );
    let mut meta = obj.get("_meta").and_then(Value::as_object).cloned().unwrap_or_else(Map::new);
    meta.insert("risk_tier".into(), json!(risk.tier.as_str()));
    obj.insert("_meta".into(), Value::Object(meta));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeMap;

    fn published() -> Vec<Value> {
        crate::server::tools().into_iter().map(|tool| serde_json::to_value(tool).expect("serialize tool")).collect()
    }

    fn by_name<'a>(tools: &'a [Value], name: &str) -> &'a Value {
        tools.iter().find(|tool| tool["name"] == name).unwrap_or_else(|| panic!("missing published tool {name}"))
    }

    #[test]
    fn every_declared_tool_and_alias_has_one_tier() {
        let mut rows: BTreeMap<&str, usize> = BTreeMap::new();
        for (name, _, _) in TOOLS {
            *rows.entry(*name).or_default() += 1;
        }
        let duplicates: Vec<_> = rows.iter().filter(|(_, count)| **count != 1).map(|(name, _)| *name).collect();
        assert!(duplicates.is_empty(), "duplicate risk rows: {duplicates:?}");

        let mut declared = std::collections::BTreeSet::new();
        for tool in &published() {
            declared.insert(tool["name"].as_str().unwrap().to_string());
        }
        for tool in crate::engine::tools() {
            let name = tool["name"].as_str().unwrap().to_string();
            assert!(declared.contains(&name), "engine tool {name} is not in server::tools()");
            declared.insert(name);
        }
        for name in &declared {
            assert!(tool_risk(name).is_some(), "declared tool {name} has no risk tier");
        }
        assert!(tool_risk("designos_not_a_tool").is_none(), "unknown tools must fail the tier lookup");

        for alias in crate::engine::ALIASES {
            assert_eq!(canonical(alias), crate::rules::canonical_tool_name(alias), "alias {alias} does not match rules::canonical_tool_name");
            let via_alias = tool_risk(alias).unwrap_or_else(|| panic!("alias {alias} has no tier"));
            let via_target = tool_risk(canonical(alias)).unwrap();
            assert_eq!(via_alias, via_target, "alias {alias} must share its target tier");
        }
    }

    #[test]
    fn strictest_tier_is_the_higher_rank() {
        assert_eq!(RiskTier::Read.strictest(RiskTier::Destructive), RiskTier::Destructive);
        assert_eq!(RiskTier::External.strictest(RiskTier::Destructive), RiskTier::External);
        let mixed = [RiskTier::Read, RiskTier::Additive, RiskTier::Modify, RiskTier::Destructive].into_iter().fold(RiskTier::Read, RiskTier::strictest);
        assert_eq!(tool_risk("designos_apply_batch").unwrap().tier, mixed);
        assert_eq!(tool_risk("designos_apply_batch").unwrap().tier, RiskTier::Destructive);
    }

    #[test]
    fn read_tools_publish_read_only_hints_and_tier() {
        let tools = published();
        for name in ["designos_get_document", "designos_list_nodes", "designos_query_xpath", "designos_get_node"] {
            let tool = by_name(&tools, name);
            assert_eq!(tool["annotations"]["readOnlyHint"], json!(true), "{name}");
            assert_eq!(tool["annotations"]["destructiveHint"], json!(false), "{name}");
            assert_eq!(tool["annotations"]["openWorldHint"], json!(false), "{name}");
            assert_eq!(tool["_meta"]["risk_tier"], json!("read"), "{name}");
        }
    }

    #[test]
    fn additive_tools_are_not_destructive_and_worst_case_writes_are() {
        let tools = published();
        for name in [
            "designos_create_node",
            "designos_add_comment",
            "designos_apply_spec",
            "designos_import_web_capture",
            "designos_render_png",
            "designos_open_file",
            "designos_new_document",
        ] {
            let tool = by_name(&tools, name);
            assert_eq!(tool["annotations"]["readOnlyHint"], json!(false), "{name}");
            assert_eq!(tool["annotations"]["destructiveHint"], json!(false), "{name}");
            assert_eq!(tool["_meta"]["risk_tier"], json!("additive"), "{name}");
        }
        for name in ["designos_delete_node", "designos_apply_batch", "designos_save_file", "designos_export_fig_file"] {
            let tool = by_name(&tools, name);
            assert_eq!(tool["annotations"]["destructiveHint"], json!(true), "{name}");
            assert_eq!(tool["_meta"]["risk_tier"], json!("destructive"), "{name}");
        }
        for name in [
            "designos_update_node",
            "designos_reply_comment",
            "designos_set_in_progress",
            "designos_configure_session",
            "designos_lint_fix",
            "designos_resolve_comment",
            "designos_set_dev_status",
            "designos_undo",
            "designos_redo",
        ] {
            let tool = by_name(&tools, name);
            assert_eq!(tool["annotations"]["readOnlyHint"], json!(false), "{name}");
            assert_eq!(tool["annotations"]["destructiveHint"], json!(true), "{name}");
            assert_eq!(tool["_meta"]["risk_tier"], json!("modify"), "{name}");
        }
    }

    #[test]
    fn published_hints_come_from_the_tier_table() {
        for tool in published() {
            let name = tool["name"].as_str().unwrap();
            let risk = tool_risk(name).unwrap();
            assert_eq!(tool["annotations"]["readOnlyHint"], json!(risk.tier.read_only()), "{name}");
            assert_eq!(tool["annotations"]["destructiveHint"], json!(risk.tier.destructive()), "{name}");
            assert_eq!(tool["annotations"]["idempotentHint"], json!(risk.idempotent), "{name}");
            assert_eq!(tool["annotations"]["openWorldHint"], json!(risk.tier.open_world()), "{name}");
            assert_eq!(tool["_meta"]["risk_tier"], json!(risk.tier.as_str()), "{name}");
            assert_ne!(risk.tier, RiskTier::External, "{name} is not an external tool");
        }
    }

    #[test]
    fn idempotency_key_tools_keep_their_published_hint() {
        let tools = published();
        let expected = [
            ("designos_create_node", false),
            ("designos_update_node", true),
            ("designos_delete_node", false),
            ("designos_apply_spec", false),
            ("designos_apply_batch", false),
            ("designos_add_comment", false),
            ("designos_reply_comment", false),
            ("designos_resolve_comment", true),
            ("designos_set_dev_status", false),
            ("designos_set_in_progress", true),
        ];
        for (name, idempotent) in expected {
            assert_eq!(by_name(&tools, name)["annotations"]["idempotentHint"], json!(idempotent), "{name}");
        }
    }

    #[test]
    fn external_tier_is_open_world_and_unused() {
        assert!(RiskTier::External.open_world());
        assert!(!RiskTier::External.read_only());
        assert!(RiskTier::External.destructive());
        assert_eq!(RiskTier::External.as_str(), "external");
        assert!(published().iter().all(|tool| tool["annotations"]["openWorldHint"] == json!(false)));
        assert!(TOOLS.iter().all(|(_, tier, _)| *tier != RiskTier::External));
    }
}
