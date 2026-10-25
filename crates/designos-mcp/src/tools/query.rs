//! MCP `designos_query_xpath` (`query_nodes`) streaming query handler and schema (E15-17 / #602).

use crate::engine::parse;
use crate::tools::errors::{ErrorCode, ToolError};
use designos_doc::lint::LintEnv;
use designos_doc::query::{
    run_with_env, QueryError, QueryFormat, QueryOptions, DEFAULT_LIMIT, DEFAULT_MAX_SCANNED, MAX_LIMIT, MAX_MAX_SCANNED, MIN_MAX_SCANNED,
};
use designos_doc::{Document, NodeId};
use serde::Deserialize;
use serde_json::{json, Value};

const SELECTOR_EXAMPLE: &str = "//FRAME[@name='Card']";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryXpathToolArgs {
    #[serde(alias = "query")]
    pub selector: String,
    #[serde(default)]
    pub page_id: Option<u64>,
    #[serde(default, alias = "within")]
    pub node_id: Option<u64>,
    #[serde(default)]
    pub node_ids: Option<Vec<u64>>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default, alias = "budget", alias = "node_budget")]
    pub max_scanned: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default, alias = "verbosity", alias = "projection")]
    pub format: Option<String>,
    #[serde(default)]
    pub with_problems: bool,
    #[serde(default)]
    pub hardcoded: Option<bool>,
}

/// Returns the JSON schema properties and required fields for `designos_query_xpath` (`E15-17` §5).
pub fn tool_schema(id_schema: &Value) -> (Value, Vec<&'static str>) {
    let props = json!({
        "selector": {
            "type": "string",
            "minLength": 1,
            "maxLength": 2048,
            "description": "XPath-subset selector; a bare word such as Card matches node names. Examples: //TEXT[has-problem('text-truncated')], //FRAME[@name='Card']//*[has-hardcoded('fills/0')], //*[@inProgress]."
        },
        "page_id": id_schema,
        "node_id": id_schema,
        "limit": {
            "type": "integer",
            "minimum": 1,
            "maximum": MAX_LIMIT,
            "default": DEFAULT_LIMIT
        },
        "max_scanned": {
            "type": "integer",
            "minimum": MIN_MAX_SCANNED,
            "maximum": MAX_MAX_SCANNED,
            "default": DEFAULT_MAX_SCANNED
        },
        "cursor": {
            "type": "string",
            "description": "Opaque continuation cursor returned by the previous page."
        },
        "format": {
            "type": "string",
            "enum": ["rows", "objects"],
            "default": "rows"
        },
        "with_problems": {
            "type": "boolean",
            "default": false
        },
        "hardcoded": {
            "type": "boolean",
            "description": "Optional filter: true keeps only nodes with a hard-coded value that matches a variable or style (a hardcoded-value finding); false drops them."
        }
    });
    (props, vec!["selector"])
}

/// Maps a [`QueryError`] onto the named MCP catalogue. `scope_path` is the argument that named the
/// scope (`/page_id` or `/node_id`). A stale cursor is `CURSOR_STALE` (restart without a cursor); a
/// rule the session cannot evaluate is `RULE_UNAVAILABLE`, whose message names the reason.
fn query_error(err: QueryError, scope_path: &str) -> ToolError {
    match err {
        QueryError::InvalidSelector { message, corrected_example } => {
            ToolError::new(ErrorCode::InvalidSelector, "/selector", message).with_corrected_example(json!({ "selector": corrected_example }))
        }
        QueryError::InvalidCursor { message } => ToolError::new(ErrorCode::InvalidCursor, "/cursor", message),
        QueryError::CursorStale { cursor_epoch, document_epoch } => {
            ToolError::new(ErrorCode::CursorStale, "/cursor", format!("cursor revision {cursor_epoch} does not match document revision {document_epoch}"))
        }
        QueryError::UnknownRule { rule } => ToolError::new(ErrorCode::UnknownRule, "/selector", format!("unknown lint rule '{rule}' in has-problem()"))
            .with_valid_options(designos_doc::lint::all_rule_ids()),
        QueryError::RuleUnavailable { rule, reason } => {
            ToolError::new(ErrorCode::RuleUnavailable, "/selector", format!("lint rule '{rule}' cannot be evaluated ({reason})"))
        }
        QueryError::NodeNotFound { id } => ToolError::new(ErrorCode::NodeNotFound, scope_path, format!("node {} not found", id.0)),
        QueryError::ExactlyOneScope => ToolError::new(ErrorCode::ExactlyOneScope, "", "provide at most one of page_id or node_id"),
    }
}

/// Executes `designos_query_xpath` against `doc` and `active_page_id`. `env` is the session's lint
/// environment: without a text engine, a `has-problem` on a text rule is `RULE_UNAVAILABLE`.
pub fn execute_query_xpath(doc: &Document, active_page_id: Option<NodeId>, env: &dyn LintEnv, raw_args: Value) -> Result<Value, ToolError> {
    let args: QueryXpathToolArgs = parse(raw_args)?;

    if args.selector.trim().is_empty() || args.selector.len() > 2048 {
        return Err(ToolError::new(ErrorCode::InvalidSelector, "/selector", "selector must be 1..=2048 characters")
            .with_corrected_example(json!({ "selector": SELECTOR_EXAMPLE })));
    }

    let explicit_within = args.node_id.or_else(|| args.node_ids.as_ref().and_then(|v| v.first().copied()));
    let scope_path = if explicit_within.is_some() { "/node_id" } else { "/page_id" };

    if args.page_id.is_some() && explicit_within.is_some() {
        return Err(query_error(QueryError::ExactlyOneScope, scope_path));
    }

    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/limit", format!("limit must be between 1 and {MAX_LIMIT}"))
            .with_received(json!(limit))
            .with_expected(format!("1..={MAX_LIMIT}")));
    }

    let max_scanned = args.max_scanned.unwrap_or(DEFAULT_MAX_SCANNED);
    if !(MIN_MAX_SCANNED..=MAX_MAX_SCANNED).contains(&max_scanned) {
        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/max_scanned", format!("max_scanned must be between {MIN_MAX_SCANNED} and {MAX_MAX_SCANNED}"))
            .with_received(json!(max_scanned))
            .with_expected(format!("{MIN_MAX_SCANNED}..={MAX_MAX_SCANNED}")));
    }

    let format = QueryFormat::parse(args.format.as_deref()).map_err(|_| {
        ToolError::new(
            ErrorCode::InvalidArguments,
            "/format",
            format!("unknown format '{}'; expected 'rows' or 'objects'", args.format.as_deref().unwrap_or_default()),
        )
        .with_valid_options(["rows", "objects"])
    })?;

    let resolved_page = if explicit_within.is_some() {
        None
    } else if let Some(pid) = args.page_id {
        if pid == 0 {
            return Err(ToolError::new(ErrorCode::NodeNotFound, "/page_id", "page_id must be >= 1"));
        }
        Some(NodeId(pid))
    } else {
        active_page_id.or_else(|| doc.pages.first().copied())
    };

    let within = match explicit_within {
        Some(0) => return Err(ToolError::new(ErrorCode::NodeNotFound, "/node_id", "node_id must be >= 1")),
        Some(id) => Some(NodeId(id)),
        None => None,
    };

    let page = run_with_env(
        doc,
        &QueryOptions {
            selector: args.selector,
            page: resolved_page,
            within,
            limit: Some(limit),
            max_scanned: Some(max_scanned),
            cursor: args.cursor,
            format,
            with_problems: args.with_problems,
            hardcoded: args.hardcoded,
            ..Default::default()
        },
        Some(env),
    )
    .map_err(|e| query_error(e, scope_path))?;

    Ok(json!({
        "columns": page.columns,
        "rows": page.rows,
        "count": page.count,
        "scanned": page.scanned,
        "scanned_total": page.scanned_total,
        "scope_size": page.scope_size,
        "remaining": page.remaining,
        "done": page.done,
        "complete": page.complete,
        "cursor": page.cursor,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use designos_doc::{Node, NodeKind, Rect, TextAutoResize, TextStyle};

    #[test]
    fn mcp_query_xpath_uses_render_lint_env_matching_designos_lint_on_wide_glyphs() {
        let mut engine = Engine::default();
        let page = engine.doc.pages[0];
        let fid = engine.doc.alloc_id();
        engine.doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 200.0))).unwrap();

        // 10 uppercase 'W's at 16px in a 100x24 fixed box:
        // ApproxLintEnv estimates width = 10 * 16 * 0.55 = 88px <= 100px (1 line, NOT truncated).
        // RenderLintEnv (cosmic-text) measures real Inter 'W' glyphs (>140px > 100px -> wraps to 2 lines -> truncated).
        let wide_id = engine.doc.alloc_id();
        let mut wide_text = Node::text(wide_id, Rect::new(0.0, 0.0, 100.0, 24.0), "WWWWWWWWWW", TextStyle { font_size: 16.0, ..Default::default() });
        if let NodeKind::Text(ref mut tp) = wide_text.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        engine.doc.insert(fid, wide_text).unwrap();

        // Narrow text that fits in both ApproxLintEnv and RenderLintEnv
        let ok_id = engine.doc.alloc_id();
        let mut ok_text = Node::text(ok_id, Rect::new(0.0, 32.0, 100.0, 24.0), "iiii", TextStyle { font_size: 16.0, ..Default::default() });
        if let NodeKind::Text(ref mut tp) = ok_text.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        engine.doc.insert(fid, ok_text).unwrap();

        let lint_val = engine.call("designos_lint", json!({"page_id": page.0, "rules": ["text-truncated"]})).unwrap();
        let lint_ids: Vec<u64> = lint_val["findings"].as_array().unwrap().iter().map(|f| f["node_id"].as_u64().unwrap()).collect();
        assert_eq!(lint_ids, vec![wide_id.0], "RenderLintEnv must flag wide_id as text-truncated");

        let query_val = engine.call("designos_query_xpath", json!({"selector": "//TEXT[has-problem('text-truncated')]", "page_id": page.0})).unwrap();
        let query_ids: Vec<u64> = query_val["rows"].as_array().unwrap().iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(query_ids, lint_ids, "designos_query_xpath must use RenderLintEnv and return identical IDs to designos_lint");
    }

    fn engine_with_text_frame() -> (Engine, NodeId) {
        let mut engine = Engine::default();
        let page = engine.doc.pages[0];
        let fid = engine.doc.alloc_id();
        engine.doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
        (engine, page)
    }

    #[test]
    fn invalid_selector_names_the_selector_path_with_a_corrected_example() {
        let (mut engine, page) = engine_with_text_frame();
        let err = engine.call("designos_query_xpath", json!({"selector": "//FRAME[@name='unclosed", "page_id": page.0})).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidSelector);
        assert_eq!(err.path, "/selector");
        assert!(err.corrected_example.is_some(), "an invalid selector carries a corrected example: {err:?}");
        assert!(err.safe_to_retry);
    }

    #[test]
    fn bare_name_queries_of_the_previous_tool_still_find_nodes_by_name() {
        let (mut engine, page) = engine_with_text_frame();
        let mut named = |name: &str| {
            engine.call("designos_create_node", json!({"parent_id": page.0, "kind": "frame", "name": name})).unwrap()["node"]["id"].as_u64().unwrap()
        };
        let hero = named("Hero Banner");
        let card = named("Card");
        let ids = |engine: &mut Engine, args: Value| -> Vec<u64> {
            let reply = engine.call("designos_query_xpath", args).unwrap();
            reply["rows"].as_array().unwrap().iter().map(|r| r[0].as_u64().unwrap()).collect()
        };
        assert_eq!(ids(&mut engine, json!({"selector": "//Card", "page_id": page.0})), vec![card]);
        assert_eq!(ids(&mut engine, json!({"query": "Hero", "page_id": page.0})), vec![hero], "the `query` alias of the previous tool");
        assert_eq!(ids(&mut engine, json!({"selector": "Hero Banner", "page_id": page.0})), vec![hero]);
        assert_eq!(ids(&mut engine, json!({"selector": "//*[contains(@name,'card')]", "page_id": page.0})), vec![card], "a lower-case needle finds \"Card\"");
        assert_eq!(ids(&mut engine, json!({"selector": "//Page", "page_id": page.0})), vec![page.0], "a bare page word finds the page");
        let err = engine.call("designos_query_xpath", json!({"selector": "//Hero Banner//TEXT", "page_id": page.0})).unwrap_err();
        assert_eq!((err.code, err.path.as_str()), (ErrorCode::InvalidSelector, "/selector"));
        assert_eq!(err.corrected_example, Some(json!({"selector": "//*[contains(@name,'Hero Banner')]//TEXT"})));
    }

    #[test]
    fn unknown_page_and_conflicting_scope_are_named_errors() {
        let (mut engine, page) = engine_with_text_frame();
        let missing = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "page_id": 999_999})).unwrap_err();
        assert_eq!(missing.code, ErrorCode::NodeNotFound);
        assert_eq!(missing.path, "/page_id");
        let missing_node = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "node_id": 999_999})).unwrap_err();
        assert_eq!(missing_node.code, ErrorCode::NodeNotFound);
        assert_eq!(missing_node.path, "/node_id");
        let both = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "page_id": page.0, "node_id": page.0})).unwrap_err();
        assert_eq!(both.code, ErrorCode::ExactlyOneScope);
    }

    #[test]
    fn argument_errors_keep_their_catalogue_codes_and_paths() {
        let (mut engine, page) = engine_with_text_frame();
        let limit = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "page_id": page.0, "limit": 501})).unwrap_err();
        assert_eq!((limit.code, limit.path.as_str()), (ErrorCode::ValueOutOfRange, "/limit"));
        let budget = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "page_id": page.0, "max_scanned": 10})).unwrap_err();
        assert_eq!((budget.code, budget.path.as_str()), (ErrorCode::ValueOutOfRange, "/max_scanned"));
        let format = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "page_id": page.0, "format": "csv"})).unwrap_err();
        assert_eq!((format.code, format.path.as_str()), (ErrorCode::InvalidArguments, "/format"));
        assert_eq!(format.valid_options, vec!["rows".to_string(), "objects".to_string()]);
        let rule = engine.call("designos_query_xpath", json!({"selector": "//*[has-problem('nope')]", "page_id": page.0})).unwrap_err();
        assert_eq!(rule.code, ErrorCode::UnknownRule);
        assert!(rule.valid_options.iter().any(|r| r == "text-truncated"), "{rule:?}");
        let unknown = engine.call("designos_query_xpath", json!({"selector": "//FRAME", "pagee": 1})).unwrap_err();
        assert_eq!(unknown.code, ErrorCode::UnknownField);
    }

    #[test]
    fn a_cursor_from_before_an_edit_is_refused_with_a_restart_step() {
        let (mut engine, page) = engine_with_text_frame();
        let second = engine.doc.alloc_id();
        engine.doc.insert(page, Node::frame(second, Rect::new(0.0, 120.0, 200.0, 100.0))).unwrap();
        let page_one = engine.call("designos_query_xpath", json!({"selector": "//*", "page_id": page.0, "limit": 1})).unwrap();
        let cursor = page_one["cursor"].as_str().expect("a one-row page leaves a cursor").to_string();
        engine.call("designos_create_node", json!({"parent_id": page.0, "kind": "frame", "name": "Late"})).unwrap();
        let err = engine.call("designos_query_xpath", json!({"selector": "//*", "page_id": page.0, "limit": 1, "cursor": cursor})).unwrap_err();
        assert_eq!(err.code, ErrorCode::CursorStale);
        assert_eq!(err.code.as_str(), "CURSOR_STALE");
        assert_eq!(err.path, "/cursor");
        assert!(err.safe_to_retry, "a stale cursor is safe to retry from the start: {err:?}");
        assert!(err.next_step.contains("without a cursor"), "{err:?}");
        let malformed = engine.call("designos_query_xpath", json!({"selector": "//*", "page_id": page.0, "cursor": "not a cursor"})).unwrap_err();
        assert_eq!(malformed.code, ErrorCode::InvalidCursor, "a cursor that cannot be read stays INVALID_CURSOR: {malformed:?}");
    }

    #[test]
    fn a_text_rule_without_a_text_engine_is_rule_unavailable_naming_the_reason() {
        let (mut engine, page) = engine_with_text_frame();
        engine.disable_text_lint_engine = true;
        let err = engine.call("designos_query_xpath", json!({"selector": "//TEXT[has-problem('text-truncated')]", "page_id": page.0})).unwrap_err();
        assert_eq!(err.code, ErrorCode::RuleUnavailable, "{err:?}");
        assert_eq!(err.code.as_str(), "RULE_UNAVAILABLE");
        assert_eq!(err.path, "/selector");
        assert!(err.message.contains("text-truncated") && err.message.contains("no_text_engine"), "the reason is named: {err:?}");
        assert!(err.next_step.contains("Remove the rule"), "{err:?}");
        // A rule that needs no text engine still answers.
        let ok = engine.call("designos_query_xpath", json!({"selector": "//FRAME[has-problem('zero-bounds')]", "page_id": page.0}));
        assert!(ok.is_ok(), "{ok:?}");
        engine.disable_text_lint_engine = false;
        let with_engine = engine.call("designos_query_xpath", json!({"selector": "//TEXT[has-problem('text-truncated')]", "page_id": page.0}));
        assert!(with_engine.is_ok(), "{with_engine:?}");
    }
}
