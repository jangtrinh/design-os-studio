//! Typed, local document operations shared by the MCP transport and future bridges.
use crate::tools::errors::{tool_output_schema, ErrorCode, ToolError};
use designos_doc::live_patch::{live_ops_for_command, live_ops_for_independent, LiveOp};
use designos_doc::{Annotation, Command, Comment, CommentAnchor, CommentThread, DevStatus, Document, History, Node, NodeId, NodeKind, NodeSpec, Vec2};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

pub type LintRulesMap = std::collections::HashMap<NodeId, std::collections::BTreeSet<&'static str>>;

pub struct CachedLintReport {
    runtime_id: u64,
    mutation_epoch: u64,
    node_count: usize,
    roots: Vec<NodeId>,
    preset: Option<String>,
    rules: Option<Vec<String>>,
    include_ignored: bool,
    stack_cursor: Option<String>,
    report: designos_doc::lint::LintReport,
    /// Document-level findings (no owning node) appended after `report.findings`; only the
    /// first chunk of a scan carries them.
    extra_findings: Vec<Value>,
}

/// One page over `report.findings` followed by `extras` (JSON findings with no owning node).
/// `total`, `by_rule` and `by_severity` include the extras; the page never exceeds `limit`.
fn page_with_extras(
    report: &designos_doc::lint::LintReport,
    extras: &[Value],
    offset: usize,
    limit: usize,
    cached_hit: bool,
    stack_prefix: Option<&str>,
) -> Value {
    let mut value = report.paginate_with_prefix(offset, limit, cached_hit, stack_prefix).to_value();
    if extras.is_empty() {
        return value;
    }
    let own = report.findings.len();
    let all = own + extras.len();
    let start = offset.min(all);
    let end = (start + limit).min(all);
    let mut page: Vec<Value> = report
        .findings
        .iter()
        .skip(start)
        .take(end.saturating_sub(start).min(own.saturating_sub(start)))
        .map(|f| serde_json::to_value(f).unwrap_or(Value::Null))
        .collect();
    page.extend(extras.iter().skip(start.saturating_sub(own)).take(end.saturating_sub(start.max(own))).cloned());
    let page_len = page.len();
    value["findings"] = Value::Array(page);
    value["total"] = json!(report.total + extras.len());
    value["truncated"] = json!(!report.complete || page_len < report.total + extras.len());
    value["cursor"] = if end < all {
        match stack_prefix {
            Some(prefix) => json!(format!("{prefix}+offset:{end}")),
            None => json!(format!("offset:{end}")),
        }
    } else if !report.complete {
        json!(report.cursor)
    } else {
        Value::Null
    };
    for (map, key) in [("by_rule", "broken-style-binding"), ("by_severity", "warning")] {
        let n = value[map].get(key).and_then(Value::as_u64).unwrap_or(0) + extras.len() as u64;
        value[map][key] = json!(n);
    }
    value
}

pub struct Engine {
    pub doc: Document,
    pub history: History,
    pub active_page_id: Option<NodeId>,
    /// Default agent run ID resolved from the Studio session (`context.json` / `mcp-config.json`).
    pub default_run_id: Option<String>,
    /// Default agent runtime label resolved from the Studio session (`context.json` / `mcp-config.json`).
    pub default_agent: Option<String>,
    /// Edits executed since the last [`Engine::take_live_ops`], recorded while an open editor
    /// mirrors this session (`None` = not recording).
    pub live_ops: Option<Vec<LiveOp>>,
    /// Undo-group label attached to the next [`designos_doc::live_patch::LivePatch`] line when set.
    pub live_patch_label: Option<String>,
    /// Optional RD-3 frame ownership lock: when `Some`, writes outside these root frames are refused.
    pub owned_frames: Option<Vec<NodeId>>,
    /// Per-node lint issue counts cached for `(runtime_id, mutation_epoch)`.
    pub lint_cache: Option<((u64, u64), std::collections::HashMap<NodeId, u16>)>,
    /// Per-node active lint rule IDs cached for `(runtime_id, mutation_epoch)`, allowing
    /// partial passes (`lint: true` vs `designos_lint`) to merge without clobbering disjoint rules.
    pub lint_rules_cache: Option<((u64, u64), LintRulesMap)>,
    /// Per-call node budget for `designos_lint` worker slices (`100_000` by default).
    pub lint_call_max_nodes: usize,
    #[doc(hidden)]
    pub lint_report_cache: Option<CachedLintReport>,
    /// Per-session options (`designos_configure_session`, E15-12).
    pub session_options: crate::tools::session_options::SessionOptions,
    /// When `true` (used by headless tests), disables the text-shaping engine so text rules are
    /// reported in `issues_skipped_rules` (`E15-12` §8.5).
    pub disable_text_lint_engine: bool,
    /// Monotonic in-memory mutation counter used by [`crate::server::Session`] to compute `safe_to_retry`.
    pub revision: u64,
    /// Optional set of enabled canonical tool names from `designos.enabled_tools` (`E15-15` §8.5).
    pub enabled_tools: Option<std::collections::BTreeSet<String>>,
    /// Open file path synced from Studio (`context.json`) or `designos_open_file` / `designos_save_file`.
    pub file_path: Option<String>,
    /// Canvas selection node IDs synced from Studio (`context.json`).
    /// Never exposed unless `designos_get_app_state` is called with `include_selection: true` (`#698`).
    pub selected_node_ids: Vec<NodeId>,
    /// True when attached to a live Studio session (`scoped` flag in `designos_get_app_state`).
    pub scoped: bool,
    /// Base document revision synced from Studio (`context.json` / `bootstrap.json`).
    pub base_doc_revision: Option<u64>,
    /// Timestamp synced from Studio's `bootstrap.json` when the session document has not mutated since sync.
    pub bootstrap_generated_at: Option<String>,
    /// Size of the session's idempotency ledger (`E15-6`), refreshed by `Session` before each call so
    /// `designos_get_app_state` can report it. An engine used without a `Session` has no ledger: `0`.
    pub ledger_entries: usize,
}

impl Default for Engine {
    fn default() -> Self {
        let mut doc = Document::new("Untitled");
        let page = doc.add_page("Page 1");
        Self {
            doc,
            history: History::default(),
            active_page_id: Some(page),
            default_run_id: None,
            default_agent: None,
            live_ops: None,
            live_patch_label: None,
            owned_frames: None,
            lint_call_max_nodes: 100_000,
            lint_cache: None,
            lint_rules_cache: None,
            lint_report_cache: None,
            session_options: crate::tools::session_options::SessionOptions::default(),
            disable_text_lint_engine: false,
            revision: 0,
            enabled_tools: None,
            file_path: None,
            selected_node_ids: Vec::new(),
            scoped: false,
            base_doc_revision: None,
            bootstrap_generated_at: None,
            ledger_entries: 0,
        }
    }
}

/// What the `fill` input of `create_node` and `update_node` takes.
const FILL_DESCRIPTION: &str = "A #RRGGBB or #RRGGBBAA hex color, a $easeui.* token, or var:<collection>/<name>.";

/// Every tool name and legacy alias accepted by [`Engine::call`].
pub const DISPATCHED: &[&str] = &[
    "designos_get_app_state",
    "designos_get_document",
    "designos_get_libraries",
    "designos_search_design_system",
    "designos_get_metadata",
    "get_metadata",
    "designos_list_nodes",
    "designos_get_node",
    "designos_get_screenshot",
    "get_screenshot",
    "designos_get_design_context",
    "get_design_context",
    "designos_import_web_capture",
    "import_web_capture",
    "designos_get_variables",
    "designos_generate_code",
    "designos_create_node",
    "designos_update_node",
    "designos_delete_node",
    "designos_list_comments",
    "designos_add_comment",
    "designos_reply_comment",
    "designos_resolve_comment",
    "designos_apply_spec",
    "designos_export_spec",
    "designos_set_dev_status",
    "designos_set_in_progress",
    "set_in_progress",
    "designos_export_fig_file",
    "export_fig_file",
    "designos_get_guidelines",
    "get_guidelines",
    "designos_query_xpath",
    "query_nodes",
    "designos_lint",
    "designos_lint_fix",
    "check_design",
    "designos_undo",
    "designos_redo",
    "designos_apply_batch",
    "designos_configure_session",
];

/// Non-prefixed compatibility aliases accepted by [`Engine::call`].
pub const ALIASES: &[&str] = &[
    "export_fig_file",
    "get_guidelines",
    "get_metadata",
    "get_screenshot",
    "get_design_context",
    "import_web_capture",
    "query_nodes",
    "check_design",
    "set_in_progress",
];

fn non_empty_trimmed(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn read_session_json_field(env_file_var: &str, session_rel: &str, pointer: &str) -> Option<String> {
    let path = std::env::var_os(env_file_var).map(std::path::PathBuf::from).filter(|p| !p.as_os_str().is_empty()).or_else(|| {
        std::env::var_os("DESIGNOS_SESSION_DIR").map(std::path::PathBuf::from).filter(|p| !p.as_os_str().is_empty()).map(|d| d.join(session_rel))
    })?;
    let raw = std::fs::read_to_string(path).ok()?;
    let val: Value = serde_json::from_str(&raw).ok()?;
    non_empty_trimmed(val.pointer(pointer).and_then(Value::as_str))
}

impl Engine {
    pub const DISPATCHED: &[&str] = DISPATCHED;
    pub const ALIASES: &[&str] = ALIASES;

    /// Updates the per-node lint cache for `rev` by replacing `recomputed_rules` on `checked_nodes`
    /// with `fresh_hits` while preserving any other rule hits or unchecked nodes from the same revision.
    pub fn update_lint_cache(
        &mut self,
        rev: (u64, u64),
        checked_nodes: &[NodeId],
        recomputed_rules: &[&'static str],
        fresh_hits: std::collections::HashMap<NodeId, std::collections::BTreeSet<&'static str>>,
    ) {
        let mut rules_map = match self.lint_rules_cache.take() {
            Some((cached_rev, existing)) if cached_rev == rev => existing,
            _ => std::collections::HashMap::new(),
        };
        for &id in checked_nodes {
            if let Some(set) = rules_map.get_mut(&id) {
                for &rule in recomputed_rules {
                    set.remove(rule);
                }
                if set.is_empty() {
                    rules_map.remove(&id);
                }
            }
        }
        for (id, hits) in fresh_hits {
            if !hits.is_empty() {
                rules_map.entry(id).or_default().extend(hits);
            }
        }
        let counts: std::collections::HashMap<NodeId, u16> =
            rules_map.iter().filter_map(|(&id, set)| (!set.is_empty()).then_some((id, set.len().min(u16::MAX as usize) as u16))).collect();
        self.lint_rules_cache = Some((rev, rules_map));
        self.lint_cache = Some((rev, counts));
    }

    /// Refreshes the per-node issue counts that `designos_get_metadata` badges read after a complete
    /// `designos_lint` run: `zero-bounds` and `default-layer-name` are recomputed for every node
    /// under `roots`, independent of the preset, leaving other cached rules untouched.
    fn refresh_node_lint_cache(&mut self, roots: &[NodeId]) {
        let mut checked = Vec::new();
        let mut hits: LintRulesMap = LintRulesMap::new();
        for &root in roots {
            self.doc.walk(root, |n, _| {
                if matches!(n.kind, NodeKind::Page { .. }) {
                    return;
                }
                checked.push(n.id);
                if (n.size.x <= 0.0 || n.size.y <= 0.0) && !designos_export::is_lint_rule_ignored(n, "zero-bounds") {
                    hits.entry(n.id).or_default().insert("zero-bounds");
                }
                if matches!(n.name.trim(), "Frame" | "Rectangle" | "Ellipse" | "Text" | "Group")
                    && !designos_export::is_lint_rule_ignored(n, "default-layer-name")
                {
                    hits.entry(n.id).or_default().insert("default-layer-name");
                }
            });
        }
        let rev = (self.doc.runtime_id(), self.doc.mutation_epoch());
        self.update_lint_cache(rev, &checked, &["zero-bounds", "default-layer-name"], hits);
    }

    /// Clears the cached lint report (e.g. when the session swaps `self.doc`).
    pub fn clear_lint_cache(&mut self) {
        self.lint_cache = None;
        self.lint_rules_cache = None;
        self.lint_report_cache = None;
    }

    /// Returns the active [`designos_doc::lint::LintEnv`] for this engine.
    pub fn lint_env(&self) -> &dyn designos_doc::lint::LintEnv {
        if self.disable_text_lint_engine {
            &designos_doc::lint::NoLintEnv
        } else {
            &designos_render::RenderLintEnv
        }
    }

    /// Resolves the effective `run_id` for an agent operation from explicit args, `DESIGNOS_AGENT_RUN_ID`,
    /// the bound Studio [`Session`] defaults, or `context.json` / `mcp-config.json`.
    pub fn resolve_agent_run_id(&self, explicit: Option<&str>) -> Option<String> {
        non_empty_trimmed(explicit)
            .or_else(|| std::env::var("DESIGNOS_AGENT_RUN_ID").ok().and_then(|s| non_empty_trimmed(Some(&s))))
            .or_else(|| self.default_run_id.as_deref().and_then(|s| non_empty_trimmed(Some(s))))
            .or_else(|| read_session_json_field("DESIGNOS_CONTEXT_JSON", "context.json", "/run_id"))
            .or_else(|| read_session_json_field("DESIGNOS_MCP_CONFIG", "mcp-config.json", "/mcpServers/designos/env/DESIGNOS_AGENT_RUN_ID"))
    }

    /// Resolves the effective `agent` runtime label from explicit args, `DESIGNOS_AGENT_RUNTIME` /
    /// `DESIGNOS_AGENT_NAME`, the bound Studio [`Session`] defaults, or `context.json` / `mcp-config.json`.
    pub fn resolve_agent_name(&self, explicit: Option<&str>) -> Option<String> {
        non_empty_trimmed(explicit)
            .or_else(|| std::env::var("DESIGNOS_AGENT_RUNTIME").ok().and_then(|s| non_empty_trimmed(Some(&s))))
            .or_else(|| std::env::var("DESIGNOS_AGENT_NAME").ok().and_then(|s| non_empty_trimmed(Some(&s))))
            .or_else(|| self.default_agent.as_deref().and_then(|s| non_empty_trimmed(Some(s))))
            .or_else(|| read_session_json_field("DESIGNOS_CONTEXT_JSON", "context.json", "/agent_runtime"))
            .or_else(|| read_session_json_field("DESIGNOS_MCP_CONFIG", "mcp-config.json", "/mcpServers/designos/env/DESIGNOS_AGENT_RUNTIME"))
    }

    /// Resolves the target page when a tool call omits `parent_id` / `page_id`:
    /// prefers `self.active_page_id` (the user's currently selected page in `design:os studio`)
    /// if valid in `self.doc.pages`, falling back to `self.doc.pages.first()`.
    pub fn resolve_default_page(&self) -> Result<NodeId, ToolError> {
        self.active_page_id
            .filter(|p| self.doc.pages.contains(p))
            .or_else(|| self.doc.pages.first().copied())
            .ok_or_else(|| ToolError::new(ErrorCode::PageNotFound, "/page_id", "Document has no pages"))
    }

    pub(crate) fn collect_library_snapshots(&self, _spec: &NodeSpec) -> Vec<designos_doc::LibrarySnapshot> {
        crate::tools::library_source::load_enabled_snapshots(&self.doc)
    }

    pub(crate) fn check_owned_frames(&self, target: NodeId, path: &str) -> Result<(), ToolError> {
        let env_owned: Option<Vec<NodeId>> = std::env::var("DESIGNOS_OWNED_FRAMES")
            .ok()
            .map(|s| s.split(',').filter_map(|p| p.trim().parse::<u64>().ok()).filter(|&id| id > 0).map(NodeId).collect())
            .filter(|v: &Vec<NodeId>| !v.is_empty());
        let Some(frames) = self.owned_frames.as_deref().or(env_owned.as_deref()) else {
            return Ok(());
        };
        let allowed = frames.iter().any(|&f| f == target || self.doc.is_ancestor(f, target));
        if !allowed {
            let valid: Vec<String> = frames.iter().map(|f| f.0.to_string()).collect();
            return Err(ToolError::new(ErrorCode::OutsideOwnedFrames, path, format!("node {} is outside this agent run's owned frames", target.0))
                .with_valid_options(valid));
        }
        Ok(())
    }

    pub fn call(&mut self, name: &str, args: Value) -> Result<Value, ToolError> {
        // Nothing here drains the lazy layout queue between calls, so no tool may start from, read,
        // or serialize a document with deferred layout still in it.
        self.settle_layout();
        match name {
            "designos_get_app_state" => crate::tools::bootstrap::call_get_app_state(self, args).map_err(ToolError::from),
            "designos_get_document" => {
                let _: Empty = parse(args)?;
                let in_progress_nodes: Vec<Value> = self
                    .doc
                    .in_progress
                    .keys()
                    .copied()
                    .filter_map(|id| {
                        let ip = self.doc.get_in_progress(id)?;
                        Some(json!({
                            "node_id": id.0,
                            "name": self.doc.get(id).map(|n| n.name.as_str()).unwrap_or(""),
                            "run_id": ip.run_id,
                            "agent": ip.agent,
                            "since_ms": ip.since_ms,
                            "stale_after_s": ip.stale_after_s,
                            "status": ip.status,
                        }))
                    })
                    .collect();
                let in_progress_map: serde_json::Map<String, Value> = self
                    .doc
                    .in_progress
                    .keys()
                    .copied()
                    .filter_map(|id| {
                        let ip = self.doc.get_in_progress(id)?;
                        Some((id.0.to_string(), serde_json::to_value(ip).ok()?))
                    })
                    .collect();
                Ok(json!({"name":self.doc.name,"kind":self.doc.kind,
                    "active_page_id":self.resolve_default_page().ok().map(|p| p.0),
                    "pages":self.doc.pages.iter().filter_map(|id|self.doc.get(*id)).map(|n| summary(&self.doc, n)).collect::<Vec<_>>(),
                    "node_count":self.doc.nodes.len(),"variable_count":self.doc.variables.len(),
                    "comment_count":self.doc.comments.len(),
                    "style_count":self.doc.styles.len(),
                    "in_progress_count":in_progress_nodes.len(),
                    "in_progress_nodes":in_progress_nodes,
                    "in_progress":in_progress_map,
                    "styles":self.doc.styles.iter().map(|(key,values)|{
                        let meta=self.doc.style_meta.get(key);
                        let empty = std::collections::BTreeMap::new();
                        json!({"key":key,"name":self.doc.style_name(key),"property":designos_doc::style_property(&self.doc,key),
                            "description":meta.map(|m|m.description.as_str()).unwrap_or(""),"library_key":meta.and_then(|m|m.key.as_deref()),
                            "bindings":meta.map(|m|&m.bindings).unwrap_or(&empty),"values":values})
                    }).collect::<Vec<_>>(),
                    "can_undo":self.history.can_undo(),"can_redo":self.history.can_redo()}))
            }
            "designos_get_libraries" => {
                let _: Empty = parse(args)?;
                Ok(crate::tools::libraries::get_libraries(&self.doc))
            }
            "designos_search_design_system" => crate::tools::search_design_system::search_design_system(&self.doc, args).map_err(|e| {
                let err = ToolError::from(e);
                let path = if err.code == ErrorCode::UnknownPropertyKey { "/property" } else { "" };
                err.with_path(path)
            }),
            "designos_list_nodes" => {
                let a: ListArgs = parse(args)?;
                if a.limit == 0 || a.limit > 500 {
                    return Err(
                        ToolError::new(ErrorCode::ValueOutOfRange, "/limit", "limit must be between 1 and 500").with_corrected_example(json!({ "limit": 100 }))
                    );
                }
                if let Some(id) = a.parent_id {
                    self.node_at(id, "/parent_id")?;
                }
                if let Some(q) = &a.query {
                    bounded_text("query", q, 1024)?;
                }
                let query = a.query.unwrap_or_default().to_lowercase();
                let matching: Vec<_> = self
                    .doc
                    .nodes
                    .values()
                    .filter(|n| a.parent_id.is_none_or(|id| n.parent == Some(NodeId(id))) && n.name.to_lowercase().contains(&query))
                    .collect();
                let nodes: Vec<_> = matching.iter().skip(a.offset).take(a.limit).map(|n| summary(&self.doc, n)).collect();
                let next = a.offset.saturating_add(nodes.len());
                Ok(json!({"nodes":nodes,"total":matching.len(),"offset":a.offset,
                    "next_offset":(next < matching.len()).then_some(next)}))
            }
            "designos_get_node" => {
                let a: NodeArgs = parse(args)?;
                let n = self.node(a.node_id)?;
                let mut node_val = serde_json::to_value(n).map_err(|e| e.to_string())?;
                let ip = self.doc.get_in_progress(n.id).cloned();
                if let (Some(obj), Some(ip_ref)) = (node_val.as_object_mut(), &ip) {
                    obj.insert("in_progress".into(), serde_json::to_value(ip_ref).map_err(|e| e.to_string())?);
                }
                let mut out = json!({"node": node_val});
                if let (Some(obj), Some(ip_val)) = (out.as_object_mut(), ip) {
                    obj.insert("in_progress".into(), serde_json::to_value(ip_val).map_err(|e| e.to_string())?);
                }
                Ok(out)
            }
            "designos_get_screenshot" | "get_screenshot" => crate::tools::screenshot::execute_get_screenshot(&self.doc, args).map_err(ToolError::from),
            "designos_get_design_context" | "get_design_context" => {
                crate::tools::design_context::execute_get_design_context(&self.doc, args).map_err(ToolError::from)
            }
            "designos_get_variables" => {
                let a: VariablesArgs = parse(args)?;
                let effective = if let Some(id) = a.node_id {
                    let n = self.node(id)?;
                    Some(
                        n.variables
                            .bindings
                            .iter()
                            .map(|(property, variable)| {
                                json!({
                                    "property":property,"variable_id":variable,
                                    "mode_id":self.doc.variables.get(variable).and_then(|v|self.doc.variable_mode(n.id,&v.collection)),
                                    "value":self.doc.resolve_variable(n.id,variable),
                                    "code_syntax":self.doc.variables.get(variable).map(|v|&v.code_syntax)
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                };
                // `code_syntax` is skipped in the native format when empty; tools get a stable shape.
                let mut variables = json!(self.doc.variables);
                for v in variables.as_object_mut().into_iter().flat_map(|m| m.values_mut()).filter_map(|v| v.as_object_mut()) {
                    v.entry("code_syntax").or_insert_with(|| json!({}));
                }
                Ok(json!({"variables":variables,"collections":self.doc.variable_collections,"effective_bindings":effective}))
            }
            "designos_generate_code" => {
                let a: CodeArgs = parse(args)?;
                let id = self.node(a.node_id)?.id;
                let doc = designos_doc::apply_bound_variables(&self.doc);
                let code = if a.language == "svg" {
                    designos_export::export_svg(&doc, id, &Default::default())
                } else if let Some(lang) = designos_export::CodeLanguage::from_id(&a.language) {
                    if lang == designos_export::CodeLanguage::Html {
                        designos_export::html_with(&doc, id, &designos_export::HtmlOptions { fragment: a.fragment })
                    } else {
                        lang.generate(&doc, id, false)
                    }
                } else {
                    let ids: Vec<&str> = designos_export::language::CodeLanguage::ALL.iter().map(|l| l.id()).collect();
                    let mut opts = ids.clone();
                    opts.push("svg");
                    return Err(ToolError::new(ErrorCode::UnknownLanguage, "/language", format!("language must be {}, or svg", ids.join(", ")))
                        .with_valid_options(opts)
                        .with_corrected_example(json!({ "node_id": a.node_id, "language": "react" })));
                };
                Ok(json!({"node_id":id,"language":a.language,"code":code}))
            }
            "designos_create_node" => self.create(parse(args)?),
            "designos_update_node" => self.update(parse(args)?),
            "designos_delete_node" => {
                let a: DeleteArgs = parse(args)?;
                crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
                let n = self.node(a.node_id)?;
                let Some(former_parent) = n.parent else {
                    let non_page_id = self.doc.nodes.values().find(|c| c.parent.is_some()).map(|c| c.id.0).unwrap_or(2);
                    return Err(ToolError::new(ErrorCode::PageRootProtected, "/node_id", "page roots cannot be deleted by this tool")
                        .with_corrected_example(json!({ "node_id": non_page_id })));
                };
                let id = n.id;
                self.check_owned_frames(id, "/node_id")?;
                let before_doc = a.diff_image.then(|| self.doc.clone());
                let baseline =
                    crate::tools::issues::PreCallBaseline::capture(&self.doc, &[former_parent], &self.session_options.issues, a.lint, self.lint_env());
                self.exec(Command::Remove { id })?;
                self.relayout(former_parent);
                let mut out = json!({"node_id":id,"deleted":true});
                if let Some(before) = &before_doc {
                    crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[id], &mut out);
                }
                crate::tools::issues::attach_issues_block(
                    &self.doc,
                    &[former_parent],
                    &self.session_options.issues,
                    a.lint,
                    &baseline,
                    self.lint_env(),
                    &[],
                    &mut out,
                );
                Ok(out)
            }
            "designos_list_comments" => {
                let a: ListCommentsArgs = parse(args)?;
                let list: Vec<_> = self
                    .doc
                    .resolve_comment_targets()
                    .into_iter()
                    .filter(|c| a.resolved.is_none_or(|r| c.resolved == r))
                    .filter(|c| a.page_id.is_none_or(|pid| c.page_id == pid))
                    .filter(|c| a.node_id.is_none_or(|nid| c.target_node_id == Some(nid) || c.target_frame_id == Some(nid)))
                    .collect();
                let total = list.len();
                Ok(json!({"comments": list, "total": total}))
            }
            "designos_add_comment" => {
                let a: AddCommentArgs = parse(args)?;
                crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
                if a.text.trim().is_empty() {
                    return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/text", "text must not be empty")
                        .with_corrected_example(json!({ "text": "Review padding" })));
                }
                bounded_text("text", &a.text, 10_000)?;
                if let Some(author) = &a.author {
                    bounded_text("author", author, 256)?;
                }
                if !a.x.is_finite() || !a.y.is_finite() {
                    return Err(
                        ToolError::new(ErrorCode::ValueOutOfRange, "/x", "x and y must be finite").with_corrected_example(json!({ "x": 0.0, "y": 0.0 }))
                    );
                }
                let page = if let Some(nid) = a.node_id {
                    self.node(nid)?;
                    self.doc
                        .page_of(NodeId(nid))
                        .ok_or_else(|| ToolError::new(ErrorCode::PageNotFound, "/node_id", format!("node {nid} does not belong to a page")))?
                } else if let Some(pid) = a.page_id {
                    let p = NodeId(pid);
                    if !self.doc.pages.contains(&p) {
                        return Err(ToolError::new(ErrorCode::PageNotFound, "/page_id", format!("page {pid} not found")));
                    }
                    p
                } else {
                    self.resolve_default_page()?
                };
                let anchor = CommentAnchor { page, node: a.node_id.map(NodeId), offset: Vec2::new(a.x, a.y) };
                let mut seq = self.doc.comments.len() + 1;
                let thread_id = loop {
                    let candidate = format!("t{seq}");
                    if !self.doc.comments.iter().any(|t| t.id == candidate) {
                        break candidate;
                    }
                    seq += 1;
                };
                let msg_id = format!("{thread_id}-1");
                let mut comments = self.doc.comments.clone();
                comments.push(CommentThread {
                    id: thread_id.clone(),
                    anchor,
                    messages: vec![Comment {
                        id: msg_id,
                        author: a.author.unwrap_or_else(|| "Agent".into()),
                        author_id: None,
                        author_color: None,
                        text: a.text.trim().into(),
                        created: "2026-10-14T23:30:00Z".into(),
                        edited: None,
                    }],
                    resolved: false,
                });
                self.exec(Command::SetComments { comments })?;
                let resolved = self
                    .doc
                    .resolve_comment_targets()
                    .into_iter()
                    .find(|t| t.id == thread_id)
                    .ok_or_else(|| ToolError::new(ErrorCode::InternalError, "", "Failed to resolve comment thread"))?;
                let mut out = serde_json::to_value(&resolved).map_err(|e| ToolError::new(ErrorCode::InternalError, "", e.to_string()))?;
                if let Some(obj) = out.as_object_mut() {
                    obj.insert("thread".into(), serde_json::to_value(&resolved).map_err(|e| ToolError::new(ErrorCode::InternalError, "", e.to_string()))?);
                    obj.insert("changed".into(), json!(true));
                }
                Ok(out)
            }
            "designos_reply_comment" => {
                let a: ReplyCommentArgs = parse(args)?;
                crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
                if a.text.trim().is_empty() {
                    return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/text", "text must not be empty")
                        .with_corrected_example(json!({ "thread_id": a.thread_id, "text": "Addressed" })));
                }
                bounded_text("text", &a.text, 10_000)?;
                if let Some(author) = &a.author {
                    bounded_text("author", author, 256)?;
                }
                let mut comments = self.doc.comments.clone();
                let valid_threads: Vec<String> = comments.iter().map(|t| t.id.clone()).collect();
                let t = comments.iter_mut().find(|t| t.id == a.thread_id).ok_or_else(|| {
                    ToolError::new(ErrorCode::CommentNotFound, "/thread_id", format!("comment thread '{}' not found", a.thread_id))
                        .with_valid_options(valid_threads)
                })?;
                let msg_id = format!("{}-{}", t.id, t.messages.len() + 1);
                t.messages.push(Comment {
                    id: msg_id,
                    author: a.author.unwrap_or_else(|| "Agent".into()),
                    author_id: None,
                    author_color: None,
                    text: a.text.trim().into(),
                    created: "2026-10-14T23:30:00Z".into(),
                    edited: None,
                });
                if let Some(resolved) = a.resolved {
                    t.resolved = resolved;
                }
                self.exec(Command::SetComments { comments })?;
                let resolved_thread = self
                    .doc
                    .resolve_comment_targets()
                    .into_iter()
                    .find(|t| t.id == a.thread_id)
                    .ok_or_else(|| ToolError::new(ErrorCode::InternalError, "", "Failed to resolve comment thread"))?;
                Ok(json!({"thread": resolved_thread, "changed": true}))
            }
            "designos_resolve_comment" => {
                let a: ResolveCommentArgs = parse(args)?;
                crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
                let mut comments = self.doc.comments.clone();
                let valid_threads: Vec<String> = comments.iter().map(|t| t.id.clone()).collect();
                let t = comments.iter_mut().find(|t| t.id == a.thread_id).ok_or_else(|| {
                    ToolError::new(ErrorCode::CommentNotFound, "/thread_id", format!("comment thread '{}' not found", a.thread_id))
                        .with_valid_options(valid_threads)
                })?;
                let changed = t.resolved != a.resolved;
                t.resolved = a.resolved;
                if changed {
                    self.exec(Command::SetComments { comments })?;
                }
                Ok(json!({"thread_id": a.thread_id, "resolved": a.resolved, "changed": changed}))
            }
            "designos_apply_spec" => {
                if let Some(raw_spec) = args.get("spec") {
                    designos_doc::validate_raw_spec_slots(raw_spec).map_err(|e| crate::tools::errors::from_spec_error(e, "/spec"))?;
                }
                let a: ApplySpecArgs = parse_spec(args)?;
                crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
                let parent = match a.parent_id.filter(|&id| id != 0) {
                    Some(id) => {
                        self.node_at(id, "/parent_id")?;
                        NodeId(id)
                    }
                    None => self.resolve_default_page()?,
                };
                self.check_owned_frames(parent, "/parent_id")?;
                if !self.node_at(parent.0, "/parent_id")?.kind.is_container() {
                    return Err(ToolError::new(ErrorCode::NotAContainer, "/parent_id", "parent_id must identify a container")
                        .with_corrected_example(json!({ "parent_id": 1 })));
                }
                let before_doc = a.diff_image.then(|| self.doc.clone());
                let baseline =
                    crate::tools::issues::PreCallBaseline::capture_container(&self.doc, parent, &self.session_options.issues, a.lint, self.lint_env());
                let snapshots = self.collect_library_snapshots(&a.spec);
                let (cmd, report) = designos_doc::apply_spec_command(&mut self.doc, parent, &a.spec, &snapshots)
                    .map_err(|e| crate::tools::errors::from_spec_error(e, "/spec"))?;
                self.exec(cmd)?;
                for (id, ip) in &report.in_progress {
                    if self.doc.in_progress.get(id) != Some(ip) {
                        let mut ip = ip.clone();
                        if ip.run_id.is_none() {
                            ip.run_id = self.resolve_agent_run_id(None);
                        }
                        if ip.agent.is_none() {
                            ip.agent = self.resolve_agent_name(None);
                        }
                        if ip.stale_after_s.is_none() {
                            ip.stale_after_s = Some(designos_doc::default_in_progress_stale_s());
                        }
                        self.doc.set_in_progress(*id, Some(ip.clone()));
                        self.record(vec![LiveOp::SetInProgress { node: *id, state: Some(ip) }]);
                    }
                }
                self.relayout(parent);
                let root_id = report.root_node_id;
                let mut resp = json!({
                    "root_node_id": root_id,
                    "parent_id": parent.0,
                    "node": self.doc.get(root_id),
                    "changed": true,
                    "name_to_id": report.name_to_id,
                    "instances": report.instances,
                    "imported_assets": report.imported_assets,
                    "bindings_created": report.bindings_created,
                    "styles_linked": report.styles_linked,
                    "warnings": report.warnings,
                });
                if !report.stale_library_assets.is_empty() {
                    resp["stale_library_assets"] = json!(report.stale_library_assets);
                }
                if let Some(before) = &before_doc {
                    crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[root_id], &mut resp);
                }
                crate::tools::issues::attach_issues_block(
                    &self.doc,
                    &[root_id],
                    &self.session_options.issues,
                    a.lint,
                    &baseline,
                    self.lint_env(),
                    &report.issues,
                    &mut resp,
                );
                Ok(resp)
            }
            "designos_export_spec" => {
                let a: NodeArgs = parse(args)?;
                let spec = self.doc.export_spec(NodeId(a.node_id)).map_err(|e| ToolError::new(ErrorCode::NodeNotFound, "/node_id", e))?;
                Ok(json!({"node_id": a.node_id, "spec": spec}))
            }
            "designos_set_dev_status" => {
                let a: SetDevStatusArgs = parse(args)?;
                crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
                let mut node = self.node(a.node_id)?.clone();
                self.check_owned_frames(node.id, "/node_id")?;
                if let Some(status) = &a.status {
                    node.dev.status = match status.trim().to_ascii_lowercase().as_str() {
                        "ready" | "ready_for_dev" | "readyfordev" => Some(DevStatus::ReadyForDev),
                        "completed" | "done" => Some(DevStatus::Completed),
                        "clear" | "none" | "" => None,
                        other => {
                            return Err(ToolError::new(
                                ErrorCode::InvalidArguments,
                                "/status",
                                format!("status must be 'ready', 'completed', or 'clear' (got '{other}')"),
                            )
                            .with_valid_options(vec!["ready", "completed", "clear", "none"])
                            .with_corrected_example(json!({ "node_id": a.node_id, "status": "ready" })));
                        }
                    };
                }
                if let Some(annotation) = &a.annotation {
                    let trimmed = annotation.trim();
                    if !trimmed.is_empty() {
                        bounded_text("annotation", trimmed, 10_000)?;
                        node.dev.annotations.push(Annotation { text: trimmed.to_string(), properties: Vec::new(), category: None });
                    }
                }
                self.exec(Command::Update { node: Box::new(node) })?;
                Ok(json!({"node_id": a.node_id, "dev": self.doc.get(NodeId(a.node_id)).map(|n| &n.dev), "changed": true}))
            }
            "designos_set_in_progress" | "set_in_progress" => {
                let a: SetInProgressArgs = parse(args)?;
                if let Some(key) = &a.idempotency_key {
                    bounded_text("idempotency_key", key, 128)?;
                }
                if let Some(agent) = &a.agent {
                    bounded_text("agent", agent, 256)?;
                }
                if let Some(status) = &a.status {
                    bounded_text("status", status, 512)?;
                }
                if let Some(run_id) = &a.run_id {
                    bounded_text("run_id", run_id, 256)?;
                }
                if let Some(stale) = a.stale_after_s {
                    if !(60..=86_400).contains(&stale) {
                        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/stale_after_s", "stale_after_s must be between 60 and 86400")
                            .with_received(json!(stale))
                            .with_expected("integer seconds from 60 to 86400"));
                    }
                }
                let flag = a
                    .value
                    .or(a.in_progress)
                    .ok_or_else(|| ToolError::new(ErrorCode::MissingField, "/value", "missing required boolean field 'value'").with_expected("boolean"))?;
                let mut ids = a.node_ids.unwrap_or_default();
                if let Some(single) = a.node_id {
                    if !ids.contains(&single) {
                        ids.push(single);
                    }
                }
                if ids.is_empty() {
                    return Err(ToolError::new(ErrorCode::InvalidArguments, "/node_ids", "node_ids must contain at least 1 node ID")
                        .with_corrected_example(json!({ "node_ids": [1], "value": true })));
                }
                if ids.len() > 200 {
                    return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/node_ids", "node_ids must contain at most 200 node IDs")
                        .with_received(json!(ids.len()))
                        .with_expected("1 to 200 node IDs"));
                }
                let effective_run_id = self.resolve_agent_run_id(a.run_id.as_deref());
                let effective_agent = self.resolve_agent_name(a.agent.as_deref());
                let effective_status = a.status.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                let effective_stale_s = a.stale_after_s.unwrap_or_else(designos_doc::default_in_progress_stale_s);

                // Pre-validate every target node before mutating any state.
                for &raw_id in &ids {
                    if raw_id == 0 || self.doc.get(NodeId(raw_id)).is_none() {
                        return Err(ToolError::new(ErrorCode::NodeNotFound, "/node_ids", format!("node {raw_id} not found")).with_received(json!(raw_id)));
                    }
                    let id = NodeId(raw_id);
                    if flag {
                        if let Err(root_opt) = self.doc.is_in_progress_root(id) {
                            let message = match root_opt {
                                Some(r) => format!("node {raw_id} is not a root frame; target root frame {} instead", r.0),
                                None => format!("node {raw_id} is not a root frame"),
                            };
                            return Err(ToolError::new(ErrorCode::InProgressTargetNotRoot, "/node_ids", message)
                                .with_received(json!({"node_id": raw_id, "root_frame_id": root_opt.map(|r| r.0)}))
                                .with_valid_options(root_opt.into_iter().map(|r| r.0.to_string())));
                        }
                        if !a.force {
                            if let Some(existing) = self.doc.get_in_progress(id) {
                                if let Some(ex_run) = &existing.run_id {
                                    if effective_run_id.as_deref() != Some(ex_run.as_str()) {
                                        return Err(ToolError::new(
                                            ErrorCode::NodeInProgressByOtherRun,
                                            "/node_ids",
                                            format!("node {raw_id} is already flagged in_progress by run '{ex_run}'"),
                                        )
                                        .with_received(json!({"node_id": raw_id, "existing_run_id": ex_run, "existing_agent": existing.agent})));
                                    }
                                }
                            }
                        }
                    }
                }

                let mut updated = Vec::with_capacity(ids.len());
                let mut any_changed = false;
                let now = designos_doc::now_ms();
                for &raw_id in &ids {
                    let id = NodeId(raw_id);
                    let state = flag.then(|| designos_doc::InProgress {
                        run_id: effective_run_id.clone(),
                        agent: effective_agent.clone(),
                        since_ms: now,
                        stale_after_s: Some(effective_stale_s),
                        status: effective_status.clone(),
                    });
                    let prev = self.doc.set_in_progress(id, state.clone());
                    if prev != state {
                        any_changed = true;
                        self.record(vec![LiveOp::SetInProgress { node: id, state }]);
                    }
                    updated.push(json!({"node_id": raw_id, "in_progress": flag}));
                }
                Ok(json!({
                    "updated": updated,
                    "changed": any_changed
                }))
            }
            "designos_export_fig_file" | "export_fig_file" => {
                let a: ExportFigArgs = parse(args)?;
                let path = std::path::Path::new(&a.output_path);
                if path.extension().and_then(|x| x.to_str()) != Some("fig") {
                    return Err(ToolError::new(ErrorCode::UnsupportedExtension, "/output_path", "output_path must end in .fig")
                        .with_valid_options(vec![".fig"])
                        .with_corrected_example(json!({ "output_path": "export.fig" })));
                }
                let (bytes, omitted) = designos_fig::export_fig_with_report(&self.doc)
                    .map_err(|e| ToolError::new(ErrorCode::IoError, "/output_path", format!("Failed to export .fig: {e}")))?;
                std::fs::write(path, &bytes).map_err(|e| ToolError::new(ErrorCode::IoError, "/output_path", format!("Cannot write .fig file: {e}")))?;
                // A `.fig` has no place for comments: the result says how many threads this file left out.
                Ok(json!({
                    "output_path": a.output_path,
                    "path": a.output_path,
                    "bytes": bytes.len(),
                    "saved": true,
                    "omitted_extensions": omitted,
                    "omitted_comment_threads": self.doc.comments.len(),
                }))
            }
            "designos_query_xpath" | "query_nodes" => {
                self.settle_layout();
                crate::tools::query::execute_query_xpath(&self.doc, self.resolve_default_page().ok(), self.lint_env(), args)
            }
            "designos_get_metadata" | "get_metadata" => crate::tools::metadata::call_get_metadata(self, args).map_err(ToolError::from),
            "designos_lint" | "check_design" | "designos_lint_fix" => {
                let a: LintArgs = parse(args)?;
                if (a.page_id.is_some() && (a.node_ids.is_some() || a.node_id.is_some())) || (a.node_id.is_some() && a.node_ids.is_some()) {
                    return Err(ToolError::new(ErrorCode::ExactlyOneScope, "", "provide at most one of page_id, node_id or node_ids"));
                }
                let limit = a.limit.unwrap_or(100);
                if !(1..=500).contains(&limit) {
                    return Err("limit must be between 1 and 500 (safe_to_retry: true)".into());
                }
                let roots: Vec<NodeId> = if let Some(pid) = a.page_id {
                    if pid == 0 || self.doc.get(NodeId(pid)).is_none() {
                        return Err(format!("NODE_NOT_FOUND: page {pid} not found (safe_to_retry: true)").into());
                    }
                    vec![NodeId(pid)]
                } else if let Some(nid) = a.node_id {
                    if nid == 0 || self.doc.get(NodeId(nid)).is_none() {
                        return Err(format!("NODE_NOT_FOUND: node {nid} not found (safe_to_retry: true)").into());
                    }
                    vec![NodeId(nid)]
                } else if let Some(ids) = &a.node_ids {
                    if ids.is_empty() || ids.len() > 500 {
                        return Err("node_ids must contain 1..=500 IDs (safe_to_retry: true)".into());
                    }
                    let mut out = Vec::with_capacity(ids.len());
                    for &nid in ids {
                        if nid == 0 || self.doc.get(NodeId(nid)).is_none() {
                            return Err(format!("NODE_NOT_FOUND: node {nid} not found (safe_to_retry: true)").into());
                        }
                        out.push(NodeId(nid));
                    }
                    out
                } else {
                    match self.resolve_default_page() {
                        Ok(p) => vec![p],
                        Err(_) => self.doc.pages.clone(),
                    }
                };
                let default_preset = if name == "designos_lint_fix" { designos_doc::lint::LintPreset::Strict } else { designos_doc::lint::LintPreset::Agent };
                let mut cfg = match a.preset.as_deref() {
                    Some(p) => designos_doc::lint::LintConfig::from_preset_str(p).map_err(|e| ToolError::from(e.to_string()).with_path("/preset"))?,
                    None => designos_doc::lint::LintConfig::from_preset(default_preset),
                };
                if let Some(rules) = &a.rules {
                    cfg = cfg.with_rules(rules).map_err(|e| ToolError::from(e.to_string()).with_path("/rules"))?;
                }
                cfg.include_ignored = a.include_ignored.unwrap_or(false);
                cfg.limit = Some(limit);
                cfg.cursor = a.cursor.clone();

                self.settle_layout();
                if name == "designos_lint_fix" {
                    let (cmds, fix_payload) = designos_doc::lint::lint_fix_commands(&self.doc, &roots, &cfg);
                    let fixed_count = fix_payload.get("fixed_count").and_then(Value::as_u64).unwrap_or(0);
                    let changes = fix_payload.get("changes").cloned().unwrap_or_else(|| json!([]));
                    if !cmds.is_empty() {
                        self.exec(Command::Batch(cmds))?;
                        for &r in &roots {
                            if let Some(p) = self.doc.page_of(r) {
                                self.relayout(p);
                            }
                        }
                        self.settle_layout();
                    }
                    return Ok(json!({ "changed": fixed_count > 0, "fixed_count": fixed_count, "changes": changes }));
                }

                let (stack_cursor, offset_cursor) = if let Some(cur) = a.cursor.as_deref() {
                    let bad_cursor = |what: &str| {
                        ToolError::new(ErrorCode::InvalidCursor, "/cursor", format!("{what} '{cur}'"))
                            .with_expected("offset:<n>, stack:<id> or stack:<id>+offset:<n>, as returned in the previous reply cursor")
                    };
                    if let Some(off_str) = cur.strip_prefix("offset:") {
                        let off = off_str.parse::<usize>().map_err(|_| bad_cursor("malformed offset cursor"))?;
                        (None, Some(off))
                    } else if let Some(rest) = cur.strip_prefix("stack:") {
                        if let Some((id_str, off_str)) = rest.split_once("+offset:") {
                            let id = id_str.parse::<u64>().map_err(|_| bad_cursor("malformed stack cursor"))?;
                            if id == 0 {
                                return Err(bad_cursor("malformed stack cursor"));
                            }
                            let off = off_str.parse::<usize>().map_err(|_| bad_cursor("malformed offset cursor"))?;
                            (Some(format!("stack:{id}")), Some(off))
                        } else {
                            let id = rest.parse::<u64>().map_err(|_| bad_cursor("malformed stack cursor"))?;
                            if id == 0 {
                                return Err(bad_cursor("malformed stack cursor"));
                            }
                            (Some(format!("stack:{id}")), None)
                        }
                    } else {
                        return Err(bad_cursor("cursor must start with 'offset:' or 'stack:'"));
                    }
                } else {
                    (None, None)
                };

                if let Some(offset) = offset_cursor {
                    if let Some(cached) = self.lint_report_cache.as_ref().filter(|c| {
                        c.runtime_id == self.doc.runtime_id()
                            && c.mutation_epoch == self.doc.mutation_epoch()
                            && c.node_count == self.doc.nodes.len()
                            && c.roots == roots
                            && c.preset == a.preset
                            && c.rules == a.rules
                            && c.include_ignored == cfg.include_ignored
                            && c.stack_cursor == stack_cursor
                    }) {
                        return Ok(page_with_extras(&cached.report, &cached.extra_findings, offset, limit, true, stack_cursor.as_deref()));
                    }
                }

                let mut unpaged_cfg = cfg.clone();
                unpaged_cfg.limit = None;
                unpaged_cfg.cursor = None;

                let mut call_budget = designos_doc::lint::Budget::with_limits(self.lint_call_max_nodes.max(1), Some(std::time::Duration::from_millis(2_000)));
                call_budget.cursor = stack_cursor.clone();

                let report = designos_render::lint_env::lint_scoped_on_worker(
                    &self.doc,
                    &roots,
                    &unpaged_cfg,
                    512,
                    std::time::Duration::from_millis(4),
                    &mut call_budget,
                )
                .report;

                if stack_cursor.is_none() && report.complete {
                    self.refresh_node_lint_cache(&roots);
                }
                // Style findings belong to the whole document, not to a node chunk: fold them into the
                // first chunk only (a resumed `stack:` chunk never repeats them), and into the cached
                // report so every page of that chunk shares one total and honours `limit`.
                let extra_findings = if stack_cursor.is_none() { designos_doc::lint::broken_style_binding_findings_for(&self.doc, &cfg) } else { Vec::new() };
                let value = page_with_extras(&report, &extra_findings, offset_cursor.unwrap_or(0), limit, false, stack_cursor.as_deref());
                self.lint_report_cache = Some(CachedLintReport {
                    runtime_id: self.doc.runtime_id(),
                    mutation_epoch: self.doc.mutation_epoch(),
                    node_count: self.doc.nodes.len(),
                    roots,
                    preset: a.preset,
                    rules: a.rules,
                    include_ignored: cfg.include_ignored,
                    stack_cursor,
                    report,
                    extra_findings,
                });
                Ok(value)
            }
            "designos_apply_batch" => crate::tools::batch::apply(self, args),
            "designos_configure_session" => self.session_options.apply_patch(args).map_err(ToolError::from),
            "designos_import_web_capture" | "import_web_capture" => {
                let a: ImportWebCaptureArgs = parse(args)?;
                let page = a
                    .page_id
                    .filter(|&id| id != 0)
                    .map(NodeId)
                    .or_else(|| self.resolve_default_page().ok())
                    .ok_or_else(|| "Document has no pages".to_string())?;
                if !matches!(self.node(page.0)?.kind, NodeKind::Page { .. }) {
                    return Err("page_id must identify a page".into());
                }
                if let Some(x) = a.x {
                    if !x.is_finite() {
                        return Err("x must be finite".into());
                    }
                }
                if let Some(y) = a.y {
                    if !y.is_finite() {
                        return Err("y must be finite".into());
                    }
                }
                let raw_json = match (&a.capture, &a.path) {
                    (Some(Value::String(s)), _) => s.clone(),
                    (Some(v), _) => serde_json::to_string(v).map_err(|e| format!("Invalid capture payload: {e}"))?,
                    (None, Some(path)) => std::fs::read_to_string(path).map_err(|e| format!("Cannot read web capture '{path}': {e}"))?,
                    (None, None) => return Err("Provide either 'path' or 'capture'".into()),
                };
                let opts =
                    designos_webcapture::ImportOptions { origin: a.x.zip(a.y), infer_block_auto_layout: a.infer_block_auto_layout, ..Default::default() };
                let (cmd, report) =
                    designos_webcapture::import_capture_json(&self.doc, page, &raw_json, &opts).map_err(|e| format!("Failed to import web capture: {e}"))?;
                self.exec(cmd)?;
                self.relayout(page);
                Ok(json!({
                    "root_node_id": report.root_node_id.0,
                    "page_id": page.0,
                    "root_name": report.root_name,
                    "summary_toast": report.summary_toast(),
                    "report": report,
                    "node": self.doc.get(report.root_node_id),
                    "changed": true
                }))
            }
            "designos_get_guidelines" | "get_guidelines" => crate::tools::guidelines::call_get_guidelines_with_enabled_tools(
                &self.doc,
                args,
                &crate::tools::guidelines::guidelines_dir(),
                self.enabled_tools.as_ref(),
            )
            .map_err(|e| {
                let err = ToolError::from(e);
                let path = if matches!(err.code, ErrorCode::UnknownTopic | ErrorCode::TopicNotAvailable) { "/topic" } else { "" };
                err.with_path(path)
            }),
            "designos_undo" | "designos_redo" => {
                let _: Empty = parse(args)?;
                let undo = name == "designos_undo";
                let next = if undo { self.history.peek_undo() } else { self.history.peek_redo() }.cloned();
                let ops = match &next {
                    Some(command) => self.live_ops_for(command)?,
                    None => Vec::new(),
                };
                let changed = if undo { self.history.undo(&mut self.doc) } else { self.history.redo(&mut self.doc) }
                    .map_err(|e| ToolError::new(ErrorCode::InternalError, "", e.to_string()))?;
                if changed {
                    self.revision = self.revision.wrapping_add(1);
                }
                self.doc.prune_in_progress();
                // Undo and redo defer a deep layout tail to later frames; this engine has no frames.
                self.settle_layout();
                self.record(ops);
                Ok(json!({"changed":changed,"can_undo":self.history.can_undo(),"can_redo":self.history.can_redo()}))
            }
            _ => Err(ToolError::new(ErrorCode::UnknownTool, "", format!("unknown tool: {name}"))),
        }
    }

    /// Re-solves auto layout under `root` and finishes what the pass had to defer. The editor
    /// drains deferred layout a step per frame; this engine has no window, and its document is
    /// read, saved and exported as is.
    pub(crate) fn relayout(&mut self, root: NodeId) {
        self.doc.relayout_subtree(root);
        self.doc.flush_pending_relayout();
    }

    /// Finishes any auto-layout the document still has deferred. Callers that serialize or
    /// export the document directly call this first.
    pub fn settle_layout(&mut self) {
        self.doc.flush_pending_relayout();
    }

    pub(crate) fn node(&self, id: u64) -> Result<&Node, ToolError> {
        self.node_at(id, "/node_id")
    }

    pub(crate) fn node_at(&self, id: u64, path: &str) -> Result<&Node, ToolError> {
        if id == 0 {
            let field = path.trim_start_matches('/');
            let valid_id = self.doc.pages.first().map(|p| p.0).unwrap_or(1);
            let mut m = serde_json::Map::new();
            m.insert(field.to_string(), json!(valid_id));
            return Err(ToolError::new(ErrorCode::ValueOutOfRange, path, "node IDs must be positive integers").with_corrected_example(Value::Object(m)));
        }
        self.doc.get(NodeId(id)).ok_or_else(|| ToolError::new(ErrorCode::NodeNotFound, path, format!("node {id} not found")))
    }

    pub fn exec(&mut self, command: Command) -> Result<(), ToolError> {
        let w = designos_doc::writer_command(&self.doc, command);
        let ops = self.live_ops_for_parts(&w.parts)?;
        self.history.exec(&mut self.doc, w.cmd).map_err(|e| ToolError::new(ErrorCode::InternalError, "", e.to_string()))?;
        self.revision = self.revision.wrapping_add(1);
        self.record(ops);
        Ok(())
    }

    /// The ops `command` will perform, when recording; refuses edits the editor cannot replay.
    pub(crate) fn live_ops_for(&self, command: &Command) -> Result<Vec<LiveOp>, ToolError> {
        if self.live_ops.is_none() {
            return Ok(Vec::new());
        }
        if let Command::Batch(cmds) = command {
            if designos_doc::live_patch::is_independent_batch(&self.doc, cmds) {
                return live_ops_for_independent(&self.doc, cmds).map_err(|e| ToolError::new(ErrorCode::InternalError, "", e));
            }
        }
        live_ops_for_command(&self.doc, command).map_err(|e| ToolError::new(ErrorCode::InternalError, "", e))
    }

    /// Records independent command parts (from [`designos_doc::writer_command`]) without cloning
    /// the document when `parts` came from a boundary-expanded batch.
    fn live_ops_for_parts(&self, parts: &[Command]) -> Result<Vec<LiveOp>, ToolError> {
        if self.live_ops.is_none() {
            return Ok(Vec::new());
        }
        live_ops_for_independent(&self.doc, parts).map_err(|e| ToolError::new(ErrorCode::InternalError, "", e))
    }

    pub(crate) fn record(&mut self, ops: Vec<LiveOp>) {
        if let Some(log) = &mut self.live_ops {
            log.extend(ops);
        }
    }

    /// Drains the edits recorded since the last call.
    pub fn take_live_ops(&mut self) -> Vec<LiveOp> {
        self.live_ops.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn create(&mut self, a: CreateArgs) -> Result<Value, ToolError> {
        crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
        let parent_id = match a.parent_id {
            Some(id) => {
                self.node_at(id, "/parent_id")?;
                NodeId(id)
            }
            None => self.resolve_default_page()?,
        };
        self.check_owned_frames(parent_id, "/parent_id")?;
        crate::tools::fill_write::validate_fill_color(a.fill.as_deref())?;
        let before_doc = a.diff_image.then(|| self.doc.clone());
        let baseline = crate::tools::issues::PreCallBaseline::capture_container(&self.doc, parent_id, &self.session_options.issues, a.lint, self.lint_env());
        let (id, index, mut node) = crate::tools::build_created_node(&mut self.doc, parent_id, None, &a.kind, a.name, a.text, a.x, a.y, a.width, a.height)?;
        let mut warnings = Vec::new();
        crate::tools::fill_write::write_fill(&mut self.doc, &mut node, parent_id, FillTarget::NewLayer, a.fill.as_deref(), a.fills, &mut warnings)?;
        self.exec(Command::Insert { parent: parent_id, index, nodes: vec![node] })?;
        if let Some(page_id) = self.doc.page_of(id) {
            self.relayout(page_id);
        }
        let mut out = json!({"node":self.doc.get(id),"parent_id":parent_id.0,"changed":true});
        attach_warnings(&mut out, &warnings);
        crate::tools::issues::attach_issues_block(&self.doc, &[id], &self.session_options.issues, a.lint, &baseline, self.lint_env(), &[], &mut out);
        if let Some(before) = &before_doc {
            crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[id], &mut out);
        }
        Ok(out)
    }

    fn update(&mut self, a: UpdateArgs) -> Result<Value, ToolError> {
        crate::tools::ledger::validate_idempotency_key_opt(&a.idempotency_key)?;
        self.check_owned_frames(NodeId(a.node_id), "/node_id")?;
        let (node, changed, warnings) = crate::tools::build_updated_node(
            &mut self.doc,
            NodeId(a.node_id),
            crate::tools::UpdateFields {
                name: a.name,
                text: a.text,
                x: a.x,
                y: a.y,
                width: a.width,
                height: a.height,
                visible: a.visible,
                opacity: a.opacity,
                fill: a.fill,
                fills: a.fills,
            },
        )?;
        let id = node.id;
        let before_doc = a.diff_image.then(|| self.doc.clone());
        let baseline = crate::tools::issues::PreCallBaseline::capture(&self.doc, &[id], &self.session_options.issues, a.lint, self.lint_env());
        if changed {
            self.exec(Command::Update { node: Box::new(node) })?;
            if let Some(page_id) = self.doc.page_of(id) {
                self.relayout(page_id);
            }
        }
        let mut out = json!({"node":self.doc.get(NodeId(a.node_id)),"changed":changed});
        attach_warnings(&mut out, &warnings);
        crate::tools::issues::attach_issues_block(&self.doc, &[id], &self.session_options.issues, a.lint, &baseline, self.lint_env(), &[], &mut out);
        if let Some(before) = &before_doc {
            crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[id], &mut out);
        }
        Ok(out)
    }
}

/// Adds the bind warnings (such as a variable that has no value in the node's mode) to a write reply, as `apply_spec` does.
fn attach_warnings(out: &mut Value, warnings: &[designos_doc::SpecWarning]) {
    if warnings.is_empty() {
        return;
    }
    if let (Some(obj), Ok(v)) = (out.as_object_mut(), serde_json::to_value(warnings)) {
        obj.insert("warnings".into(), v);
    }
}

fn summary(doc: &Document, n: &Node) -> Value {
    let mut out = json!({"node_id":n.id,"name":n.name,"kind":n.kind.default_name(),"parent_id":n.parent,
        "child_count":n.children.len(),"visible":n.visible,"x":n.transform.tx,"y":n.transform.ty,
        "width":n.size.x,"height":n.size.y});
    if let (Some(obj), Some(ip)) = (out.as_object_mut(), doc.get_in_progress(n.id)) {
        if let Ok(v) = serde_json::to_value(ip) {
            obj.insert("in_progress".into(), v);
        }
    }
    out
}

pub(crate) fn parse<T: DeserializeOwned>(args: Value) -> Result<T, ToolError> {
    let raw = args.clone();
    serde_json::from_value(args).map_err(|e| crate::tools::errors::from_serde_error_with_args(&e, "", false, Some(&raw)))
}

pub(crate) fn parse_spec<T: DeserializeOwned>(args: Value) -> Result<T, ToolError> {
    let raw = args.clone();
    serde_json::from_value(args).map_err(|e| crate::tools::errors::from_serde_error_with_args(&e, "", true, Some(&raw)))
}

use crate::tools::bounded_text;
use crate::tools::fill_write::FillTarget;
fn default_limit() -> usize {
    100
}
fn default_size() -> f32 {
    100.0
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeArgs {
    node_id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    node_id: u64,
    #[serde(default)]
    diff_image: bool,
    #[serde(default)]
    lint: Option<crate::tools::session_options::PerCallLintOverride>,
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    parent_id: Option<u64>,
    query: Option<String>,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariablesArgs {
    node_id: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeArgs {
    node_id: u64,
    language: String,
    #[serde(default)]
    fragment: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    #[serde(default)]
    parent_id: Option<u64>,
    kind: String,
    name: Option<String>,
    text: Option<String>,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default = "default_size")]
    width: f32,
    #[serde(default = "default_size")]
    height: f32,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    fills: Option<Vec<designos_doc::Paint>>,
    #[serde(default)]
    diff_image: bool,
    #[serde(default)]
    lint: Option<crate::tools::session_options::PerCallLintOverride>,
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateArgs {
    node_id: u64,
    name: Option<String>,
    text: Option<String>,
    x: Option<f32>,
    y: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    visible: Option<bool>,
    opacity: Option<f32>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    fills: Option<Vec<designos_doc::Paint>>,
    #[serde(default)]
    diff_image: bool,
    #[serde(default)]
    lint: Option<crate::tools::session_options::PerCallLintOverride>,
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListCommentsArgs {
    resolved: Option<bool>,
    page_id: Option<u64>,
    node_id: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddCommentArgs {
    text: String,
    author: Option<String>,
    node_id: Option<u64>,
    page_id: Option<u64>,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyCommentArgs {
    thread_id: String,
    text: String,
    author: Option<String>,
    resolved: Option<bool>,
    #[serde(default)]
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveCommentArgs {
    thread_id: String,
    #[serde(default = "default_true")]
    resolved: bool,
    #[serde(default)]
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplySpecArgs {
    parent_id: Option<u64>,
    spec: NodeSpec,
    #[serde(default)]
    diff_image: bool,
    #[serde(default)]
    lint: Option<crate::tools::session_options::PerCallLintOverride>,
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetDevStatusArgs {
    node_id: u64,
    status: Option<String>,
    annotation: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetInProgressArgs {
    #[serde(default)]
    node_ids: Option<Vec<u64>>,
    #[serde(default)]
    node_id: Option<u64>,
    #[serde(default)]
    value: Option<bool>,
    #[serde(default)]
    in_progress: Option<bool>,
    #[serde(default, alias = "agent_name")]
    agent: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    stale_after_s: Option<u32>,
    #[serde(default)]
    force: bool,
    #[serde(default)]
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportFigArgs {
    #[serde(alias = "path")]
    pub output_path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LintArgs {
    #[serde(default)]
    page_id: Option<u64>,
    #[serde(default)]
    node_id: Option<u64>,
    #[serde(default)]
    node_ids: Option<Vec<u64>>,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    rules: Option<Vec<String>>,
    #[serde(default)]
    include_ignored: Option<bool>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportWebCaptureArgs {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub capture: Option<Value>,
    #[serde(default)]
    pub page_id: Option<u64>,
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    #[serde(default = "default_true")]
    pub infer_block_auto_layout: bool,
}

/// JSON tool descriptors kept beside the argument validators.
pub fn tools() -> Vec<Value> {
    let id = json!({"type":"integer","minimum":1,"description":"DesignOS numeric node ID; obtain from list_nodes/get_document."});
    let coordinate = json!({"type":"number","minimum":-1000000,"maximum":1000000});
    let size = json!({"type":"number","minimum":0,"maximum":1000000});
    let text = json!({"type":"string","maxLength":100000,"description":"Plain text; limited to 100000 UTF-8 bytes."});
    let name = json!({"type":"string","maxLength":1024,"description":"Layer name; limited to 1024 UTF-8 bytes."});
    let lint_opt = json!({"anyOf":[{"type":"boolean"},{"type":"string","enum":["preset","errors_only","off"]}],"description":"Optional per-call lint override (false or 'off' skips the per-call lint pass)."});
    let idempotency_key = json!({
        "type": "string",
        "minLength": 1,
        "maxLength": 128,
        "pattern": "^[A-Za-z0-9._:-]{1,128}$",
        "description": "Optional agent-supplied idempotency key for deduplicating replayed writes within the active session."
    });
    let mut out = Vec::new();
    let mut add =
        |tool: &str, description: &str, properties: Value, required: Vec<&str>, out_props: Value, out_req: &[&str], read_only: bool, idempotent: bool| {
            out.push(json!({"name":format!("designos_{tool}"),"description":description,
            "inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
            "outputSchema":tool_output_schema(out_props, out_req),
            "annotations":{"readOnlyHint":read_only,"destructiveHint":!read_only,"idempotentHint":idempotent,"openWorldHint":false}}));
        };
    add(
        "get_document",
        "Summarize the current in-memory document: pages, counts, shared styles (fill, text, effect, grid) with their values, and undo state.",
        json!({}),
        vec![],
        json!({
            "name": {"type": "string"},
            "kind": {"type": "string"},
            "active_page_id": {"type": ["integer", "null"]},
            "pages": {"type": "array"},
            "node_count": {"type": "integer"},
            "variable_count": {"type": "integer"},
            "comment_count": {"type": "integer"},
            "style_count": {"type": "integer"},
            "styles": {"type": "array"},
            "can_undo": {"type": "boolean"},
            "can_redo": {"type": "boolean"}
        }),
        &["name", "pages", "node_count", "can_undo", "can_redo"],
        true,
        true,
    );
    add(
        "get_libraries",
        "List this file's local publishable assets, enabled libraries (with pinned and latest versions and update_available), and disabled libraries in the active library root.",
        json!({}),
        vec![],
        json!({ "libraries": {"type": "array"} }),
        &["libraries"],
        true,
        true,
    );
    add(
        "get_metadata",
        "Return a sparse XML node hierarchy (IDs, types, names, page-space bounds, layout, component/variant props, text previews, and lint issue counts) for a page or subtree.",
        crate::tools::metadata::properties_schema(),
        vec![],
        json!({
            "root_id": {"type": "integer"},
            "xml": {"type": "string"},
            "node_count": {"type": "integer"},
            "truncated": {"type": "boolean"},
            "remaining_estimate": {"type": "integer"},
            "hidden_omitted": {"type": "integer"},
            "lint_ran": {"type": "boolean"},
            "scoped": {"type": "boolean"}
        }),
        &["root_id", "xml", "node_count", "truncated", "remaining_estimate", "hidden_omitted", "lint_ran"],
        true,
        true,
    );
    add(
        "search_design_system",
        "Search components, component sets, shared styles, and variables across this file and its enabled libraries, returning ranked instance-ready references and NodeSpec examples.",
        json!({
            "query": {"type": "string", "minLength": 1, "maxLength": 256},
            "kinds": {"type": "array", "items": {"enum": ["component", "style", "variable"]}},
            "property": {"type": "string", "description": "Bindable property key to filter variables by scope, e.g. fills/0, itemSpacing."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 50, "default": 20}
        }),
        vec!["query"],
        json!({
            "results": {"type": "array"},
            "total": {"type": "integer"},
            "libraries_searched": {"type": "array"}
        }),
        &["results", "total", "libraries_searched"],
        true,
        true,
    );
    add(
        "list_nodes",
        "List node summaries in numeric ID order. Query matches names case-insensitively; parent_id filters direct children. Use next_offset to paginate.",
        json!({"parent_id":id,"query":{"type":"string","maxLength":1024},"offset":{"type":"integer","minimum":0,"default":0},"limit":{"type":"integer","minimum":1,"maximum":500,"default":100}}),
        vec![],
        json!({
            "nodes": {"type": "array"},
            "total": {"type": "integer"},
            "offset": {"type": "integer"},
            "next_offset": {"type": ["integer", "null"]}
        }),
        &["nodes", "total", "offset"],
        true,
        true,
    );
    add(
        "get_node",
        "Read the full native node including geometry, paints, text, bindings, and child IDs.",
        json!({"node_id":id}),
        vec!["node_id"],
        json!({ "node": {"type": "object"} }),
        &["node"],
        true,
        true,
    );
    add(
        "get_variables",
        "Read variable definitions and collections. With node_id, also resolve that node's direct bindings using inherited collection modes.",
        json!({"node_id":id}),
        vec![],
        json!({
            "variables": {"type": "object"},
            "collections": {"type": "object"},
            "effective_bindings": {"type": ["array", "null"]}
        }),
        &["variables", "collections"],
        true,
        true,
    );
    let code_langs: Vec<&str> = designos_export::CodeLanguage::ALL.iter().map(|l| l.id()).chain(std::iter::once("svg")).collect();
    add(
        "generate_code",
        "Generate developer handoff code or SVG for a layer using resolved variable values. Returns source text; writes no files.",
        json!({"node_id":id,"language":{"type":"string","enum":code_langs},"fragment":{"type":"boolean","default":false,"description":"When language is html, emit only <style> and the root element without <!doctype html>, <html>, <head>, or <body>."}}),
        vec!["node_id", "language"],
        json!({
            "node_id": {"type": "integer"},
            "language": {"type": "string"},
            "code": {"type": "string"}
        }),
        &["node_id", "language", "code"],
        true,
        true,
    );
    add(
        "create_node",
        "Create one layer in a container; defaults to x/y=0, width/height=100. Supports undo. Text is only valid for text layers. Changes are in memory until saved. Content added to, removed from or moved in an instance slot is kept when the main component changes.",
        json!({"parent_id":id,"kind":{"type":"string","enum":["frame","rectangle","ellipse","text"]},"name":name,"text":text,"x":coordinate,"y":coordinate,"width":size,"height":size,"fill":{"type":"string","maxLength":1024,"description":FILL_DESCRIPTION},"fills":{"type":"array","items":{"type":"object"}},"diff_image":{"type":"boolean","default":false},"lint":lint_opt,"idempotency_key":idempotency_key}),
        vec!["parent_id", "kind"],
        json!({
            "node": {"type": "object"},
            "parent_id": {"type": "integer"},
            "changed": {"type": "boolean"}
        }),
        &["node", "parent_id", "changed"],
        false,
        false,
    );
    add(
        "update_node",
        "Update specified fields of one non-page layer. Coordinates are parent-relative. Replacing text clears text runs and list ranges. Supports undo; changes remain in memory until saved.",
        json!({"node_id":id,"name":name,"text":text,"x":coordinate,"y":coordinate,"width":size,"height":size,"visible":{"type":"boolean"},"opacity":{"type":"number","minimum":0,"maximum":1},"fill":{"type":"string","maxLength":1024,"description":FILL_DESCRIPTION},"fills":{"type":"array","items":{"type":"object"}},"diff_image":{"type":"boolean","default":false},"lint":lint_opt,"idempotency_key":idempotency_key}),
        vec!["node_id"],
        json!({
            "node": {"type": "object"},
            "changed": {"type": "boolean"}
        }),
        &["node", "changed"],
        false,
        true,
    );
    add(
        "delete_node",
        "Remove a non-page layer and its entire subtree. Supports undo; changes remain in memory until saved. Content added to, removed from or moved in an instance slot is kept when the main component changes.",
        json!({"node_id":id,"diff_image":{"type":"boolean","default":false},"lint":lint_opt,"idempotency_key":idempotency_key}),
        vec!["node_id"],
        json!({
            "node_id": {"type": "integer"},
            "deleted": {"type": "boolean"}
        }),
        &["node_id", "deleted"],
        false,
        false,
    );
    add(
        "list_comments",
        "List canvas comment threads enriched with resolved world coordinates, target node ID/name/kind, and top-level frame ID/name. Filter by resolved state, page_id, or node_id.",
        json!({"resolved":{"type":"boolean"},"page_id":id,"node_id":id}),
        vec![],
        json!({
            "comments": {"type": "array"},
            "total": {"type": "integer"}
        }),
        &["comments", "total"],
        true,
        true,
    );
    add(
        "add_comment",
        "Pin a new comment thread to a node (with local offset x/y) or page coordinates. Supports undo.",
        json!({"text":{"type":"string","minLength":1,"maxLength":10000},"author":{"type":"string","maxLength":256},"node_id":id,"page_id":id,"x":coordinate,"y":coordinate,"idempotency_key":idempotency_key}),
        vec!["text"],
        json!({
            "id": {"type": "string"},
            "thread": {"type": "object"},
            "changed": {"type": "boolean"}
        }),
        &["id", "thread", "changed"],
        false,
        false,
    );
    add(
        "reply_comment",
        "Append a reply message to an existing canvas comment thread and optionally mark it resolved/unresolved. Supports undo.",
        json!({"thread_id":{"type":"string","minLength":1},"text":{"type":"string","minLength":1,"maxLength":10000},"author":{"type":"string","maxLength":256},"resolved":{"type":"boolean"},"idempotency_key":idempotency_key}),
        vec!["thread_id", "text"],
        json!({
            "thread": {"type": "object"},
            "changed": {"type": "boolean"}
        }),
        &["thread", "changed"],
        false,
        false,
    );
    add(
        "resolve_comment",
        "Mark a canvas comment thread resolved (default true) or unresolved (false). Supports undo.",
        json!({"thread_id":{"type":"string","minLength":1},"resolved":{"type":"boolean","default":true},"idempotency_key":idempotency_key}),
        vec!["thread_id"],
        json!({
            "thread_id": {"type": "string"},
            "resolved": {"type": "boolean"},
            "changed": {"type": "boolean"}
        }),
        &["thread_id", "resolved", "changed"],
        false,
        true,
    );
    add(
        "apply_spec",
        concat!(
            "Build a complete UI subtree from a declarative NodeSpec (supports EaseUI BrainWave 2.0 presets like easeui-screen/easeui-card/easeui-button-primary, $easeui.* tokens, and Auto-Layout 5.0 row/column/wrap/hug/fill) in a single atomic undoable step together with the variables it creates. Replies with bindings_created, styles_linked and warnings (for example DEPRECATED_EASEUI_TOKEN). Content added to, removed from or moved in an instance slot is kept when the main component changes.\n\n",
            "Examples:\n",
            "1) Token-bound frame with auto-layout:\n",
            "{\"parent_id\":1,\"spec\":{\"kind\":\"frame\",\"name\":\"Hero\",\"layout\":{\"direction\":\"column\",\"gap\":16,\"padding\":[24,24,24,24]},\"fill\":\"var:Color/Surface\",\"children\":[{\"kind\":\"text\",\"name\":\"Title\",\"text\":\"Welcome\"}]}}\n",
            "2) Component instance with variant and overrides:\n",
            "{\"parent_id\":1,\"spec\":{\"kind\":\"instance\",\"name\":\"PrimaryCTA\",\"component\":\"Button\",\"variant\":{\"Size\":\"lg\",\"State\":\"default\"},\"overrides\":[{\"path\":\"Label\",\"text\":\"Continue\"}]}}\n",
            "3) Component instance with filled slots:\n",
            "{\"parent_id\":1,\"spec\":{\"kind\":\"instance\",\"name\":\"SettingsCard\",\"component\":\"Card\",\"slots\":{\"header\":[{\"kind\":\"instance\",\"component\":\"CardHeader\"}],\"body\":[{\"kind\":\"instance\",\"component\":\"FormRow\"}],\"footer\":false}}}"
        ),
        json!({"parent_id":id,"spec":{"type":"object","description":"Declarative NodeSpec tree (kind, name, preset, width, height, width_sizing, height_sizing, fill, stroke, corner_radius, text, font_size, layout, dev_status, annotations, children)."},"diff_image":{"type":"boolean","default":false},"lint":lint_opt,"idempotency_key":idempotency_key}),
        vec!["spec"],
        json!({
            "root_node_id": {"type": "integer"},
            "parent_id": {"type": "integer"},
            "node": {"type": "object"},
            "changed": {"type": "boolean"},
            "name_to_id": {"type": "object"},
            "instances": {"type": "array"},
            "imported_assets": {"type": "array"},
            "issues": {"type": "array"},
            "bindings_created": {"type": "integer"},
            "styles_linked": {"type": "integer"},
            "warnings": {"type": "array"}
        }),
        &["root_node_id", "parent_id", "node", "changed", "name_to_id", "instances", "issues"],
        false,
        false,
    );
    add(
        "export_spec",
        "Export a node and its descendants as a declarative NodeSpec JSON tree, enriched with Dev Mode status/annotations and any canvas comment threads pinned within the subtree.",
        json!({"node_id":id}),
        vec!["node_id"],
        json!({
            "node_id": {"type": "integer"},
            "spec": {"type": "object"}
        }),
        &["node_id", "spec"],
        true,
        true,
    );
    add(
        "set_dev_status",
        "Set a layer's Dev Mode handoff status ('ready', 'completed', or 'clear') and/or append a developer annotation. Supports undo.",
        json!({"node_id":id,"status":{"type":"string","enum":["ready","completed","clear","none"]},"annotation":{"type":"string","maxLength":10000},"idempotency_key":idempotency_key}),
        vec!["node_id"],
        json!({
            "node_id": {"type": "integer"},
            "dev": {"type": "object"},
            "changed": {"type": "boolean"}
        }),
        &["node_id", "dev", "changed"],
        false,
        false,
    );
    add(
        "export_fig_file",
        "Export the current session document to a Figma binary .fig container (with fig-kiwi header and deflate chunks) inside the workspace.",
        json!({"output_path":{"type":"string","description":"Destination .fig path inside the workspace."}}),
        vec!["output_path"],
        json!({
            "output_path": {"type": "string"},
            "path": {"type": "string"},
            "bytes": {"type": "integer"},
            "saved": {"type": "boolean"}
        }),
        &["output_path", "bytes", "saved"],
        false,
        true,
    );
    let (q_props, q_req) = crate::tools::query::tool_schema(&id);
    add(
        "query_xpath",
        concat!(
            "Query layers in a page or subtree with a streaming single-pass XPath evaluator, node budget, cursor pagination, compact rows, and solver-backed linter predicates (has-problem, is-bound, has-hardcoded, @problems, @inProgress).\n\n",
            "Examples:\n",
            "1) Truncated text:\n",
            "{\"selector\":\"//TEXT[has-problem('text-truncated')]\"}\n",
            "2) Hard-coded fills inside a frame:\n",
            "{\"selector\":\"//FRAME[@name='Card']//*[has-hardcoded('fills/0')]\"}\n",
            "3) Layers an agent is working on:\n",
            "{\"selector\":\"//*[@inProgress]\"}"
        ),
        q_props,
        q_req,
        json!({
            "columns": {"type": "array"},
            "rows": {"type": "array"},
            "count": {"type": "integer"},
            "scanned": {"type": "integer"},
            "scanned_total": {"type": "integer"},
            "scope_size": {"type": "integer"},
            "remaining": {"type": "integer"},
            "done": {"type": "boolean"},
            "complete": {"type": "boolean"},
            "cursor": {"type": ["string", "null"]}
        }),
        &["columns", "rows", "count", "scanned", "done", "complete"],
        true,
        true,
    );
    add(
        "lint",
        "Run the unified solver-backed geometric, text, token, and structural design linter on a page, specific node subtrees, or the active page.",
        json!({
            "page_id": id,
            "node_ids": {"type": "array", "items": id, "maxItems": 500},
            "preset": {"type": "string", "default": "agent"},
            "rules": {"type": "array", "items": {"type": "string"}},
            "include_ignored": {"type": "boolean", "default": false},
            "limit": {"type": "integer", "minimum": 1, "maximum": 500, "default": 100},
            "cursor": {"type": "string"}
        }),
        vec![],
        json!({
            "page_id": {"type": ["integer", "null"]},
            "findings": {"type": "array"},
            "total": {"type": "integer"},
            "passed": {"type": "boolean"}
        }),
        &["findings", "total"],
        true,
        true,
    );
    add(
        "get_guidelines",
        "List available design guideline topics or return the guideline document for a topic ('design-system', surface guides, 'archetypes', 'archetype:<id>', or 'rules').",
        json!({
            "topic":{"type":"string"},
            "platform":{"type":"string","enum":["web","ios","android"],"default":"web"},
            "max_tokens":{"type":"integer","minimum":500,"maximum":25000,"default":8000}
        }),
        vec![],
        json!({
            "topics": {"type": "array"},
            "topic": {"type": "string"},
            "title": {"type": "string"},
            "summary": {"type": "string"},
            "applies_to": {"type": "array"},
            "archetype": {"type": "object"},
            "content": {"type": "string"},
            "truncated": {"type": "boolean"},
            "sources": {"type": "array"}
        }),
        &[],
        true,
        true,
    );
    add(
        "import_web_capture",
        "Import a .webcapture.json / .capture.json bundle (or inline capture JSON) into a page as an editable auto-layout frame tree in one undoable step, returning the ImportReport.",
        json!({
            "path":{"type":"string","description":"Path to a .webcapture.json or .capture.json file inside the workspace."},
            "capture":{"description":"Inline web capture JSON object or JSON string (alternative to path)."},
            "page_id":id,
            "x":coordinate,
            "y":coordinate,
            "infer_block_auto_layout":{"type":"boolean","default":true}
        }),
        vec![],
        json!({
            "root_node_id": {"type": "integer"},
            "page_id": {"type": "integer"},
            "root_name": {"type": "string"},
            "summary_toast": {"type": "string"},
            "report": {"type": "object"},
            "node": {"type": ["object", "null"]},
            "changed": {"type": "boolean"}
        }),
        &["root_node_id", "page_id", "report", "changed"],
        false,
        false,
    );
    add(
        "undo",
        "Undo the latest successful edit in this session; returns changed=false if the stack is empty.",
        json!({}),
        vec![],
        json!({
            "changed": {"type": "boolean"},
            "can_undo": {"type": "boolean"},
            "can_redo": {"type": "boolean"}
        }),
        &["changed", "can_undo", "can_redo"],
        false,
        false,
    );
    add(
        "lint_fix",
        "Apply deterministic fixes for auto-fixable lint findings on a page, specific node subtrees, or the active page in a single undo step.",
        json!({
            "page_id": id,
            "node_id": id,
            "node_ids": {"type": "array", "items": id, "maxItems": 500},
            "preset": {"type": "string", "default": "strict"},
            "rules": {"type": "array", "items": {"type": "string"}}
        }),
        vec![],
        json!({
            "changed": {"type": "boolean"},
            "fixed_count": {"type": "integer"},
            "changes": {"type": "array"}
        }),
        &["changed", "fixed_count", "changes"],
        false,
        false,
    );
    add(
        "redo",
        "Redo the most recently undone edit in this session; returns changed=false if the stack is empty.",
        json!({}),
        vec![],
        json!({
            "changed": {"type": "boolean"},
            "can_undo": {"type": "boolean"},
            "can_redo": {"type": "boolean"}
        }),
        &["changed", "can_undo", "can_redo"],
        false,
        false,
    );
    out.push(crate::tools::screenshot::tool_descriptor());
    out.push(json!({
        "name": "designos_set_in_progress",
        "description": "Flag or clear root frames as in progress while an agent works on them (work state, not an undo step).",
        "inputSchema": {
            "type": "object",
            "required": ["node_ids", "value"],
            "additionalProperties": false,
            "properties": {
                "node_ids": {
                    "type": "array",
                    "items": { "type": "integer", "minimum": 1 },
                    "minItems": 1,
                    "maxItems": 200
                },
                "value": { "type": "boolean" },
                "stale_after_s": { "type": "integer", "minimum": 60, "maximum": 86400 },
                "force": { "type": "boolean", "default": false },
                "idempotency_key": { "type": "string", "maxLength": 128 }
            }
        },
        "outputSchema": tool_output_schema(
            json!({
                "updated": {"type": "array"},
                "changed": {"type": "boolean"}
            }),
            &["updated", "changed"],
        ),
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    }));
    out.push(crate::tools::batch::tool_descriptor());
    out.push(crate::tools::design_context::tool_descriptor());
    out.push(crate::tools::session_options::tool_descriptor());
    out.push(crate::tools::bootstrap::tool_definition());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{NodeKind, Rect};

    #[test]
    fn variable_inspection_and_codegen_resolve_inherited_cross_collection_aliases() {
        use designos_doc::{Variable, VariableCollection, VariableKind, VariableMode, VariableValue};
        let mut e = Engine::default();
        let id = create(&mut e, "rectangle", "Box");
        for collection in ["alias", "size"] {
            e.doc.variable_collections.insert(
                collection.into(),
                VariableCollection {
                    id: collection.into(),
                    name: collection.into(),
                    modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                    default_mode: "light".into(),
                    variables: vec![],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
        }
        for (id, collection, light, dark) in [
            ("width", "size", VariableValue::Number(100.0), VariableValue::Number(240.0)),
            ("bound", "alias", VariableValue::Alias("width".into()), VariableValue::Alias("width".into())),
        ] {
            e.doc.variables.insert(
                id.into(),
                Variable {
                    id: id.into(),
                    name: id.into(),
                    collection: collection.into(),
                    kind: VariableKind::Number,
                    values: [("light".into(), light), ("dark".into(), dark)].into_iter().collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,

                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
        }
        e.doc.get_mut(NodeId(1)).unwrap().variables.modes.insert("size".into(), "dark".into());
        e.doc.get_mut(NodeId(id)).unwrap().variables.bindings.insert("width".into(), "bound".into());
        e.doc.variables.get_mut("bound").unwrap().code_syntax.insert(designos_doc::CodeSyntaxPlatform::Web, "var(--size-bound, 240px)".into());
        e.doc.variables.get_mut("bound").unwrap().code_syntax.insert(designos_doc::CodeSyntaxPlatform::Ios, "Size.bound".into());
        let before = e.doc.to_json().unwrap();
        let values = e.call("designos_get_variables", json!({"node_id":id})).unwrap();
        assert_eq!(values["effective_bindings"][0]["value"], json!({"Number":240.0}));
        let syntax = json!({"WEB": "var(--size-bound, 240px)", "iOS": "Size.bound"});
        assert_eq!(values["variables"]["bound"]["code_syntax"], syntax);
        assert_eq!(values["effective_bindings"][0]["code_syntax"], syntax);
        assert_eq!(values["variables"]["width"]["code_syntax"], json!({}));
        let css = e.call("designos_generate_code", json!({"node_id":id,"language":"css"})).unwrap();
        assert!(css["code"].as_str().unwrap().contains("width: var(--size-bound, 240px);"), "{css}");
        let svg = e.call("designos_generate_code", json!({"node_id":id,"language":"svg"})).unwrap();
        assert!(svg["code"].as_str().unwrap().contains("width=\"240\""));
        assert_eq!(e.doc.to_json().unwrap(), before);
    }
    #[test]
    fn get_document_lists_shared_effect_and_grid_styles_and_nodes_carry_their_links() {
        use designos_doc::{Color, Effect, LayoutGrid, Style, StyleMeta, StyleProperty, Vec2};
        let mut e = Engine::default();
        let id = create(&mut e, "frame", "Card");
        assert_eq!(e.call("designos_get_document", json!({})).unwrap()["styles"], json!([]), "no styles yet");
        let shadow =
            Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false };
        e.doc.styles.insert("Shadow/Card".into(), Style::Effect(vec![shadow]));
        e.doc.styles.insert("Layout/Cols".into(), Style::Grid(vec![LayoutGrid::columns(12, 20.0, 0.0, Color::rgba(255, 0, 0, 26))]));
        for (key, name, property) in [("Shadow/Card", "Shadow/Card", StyleProperty::Effect), ("Layout/Cols", "Layout / 12 col", StyleProperty::Grid)] {
            e.doc.style_meta.insert(key.into(), StyleMeta { name: name.into(), key: None, description: String::new(), property, bindings: Default::default() });
        }
        e.doc.get_mut(NodeId(id)).unwrap().style_refs.insert("effect".into(), "Shadow/Card".into());
        let doc = e.call("designos_get_document", json!({})).unwrap();
        assert_eq!(doc["style_count"], 2);
        let styles = doc["styles"].as_array().unwrap();
        let by_key = |k: &str| styles.iter().find(|s| s["key"] == k).unwrap_or_else(|| panic!("style {k} listed"));
        assert_eq!((by_key("Shadow/Card")["property"].as_str(), by_key("Shadow/Card")["name"].as_str()), (Some("Effect"), Some("Shadow/Card")));
        assert_eq!(by_key("Shadow/Card")["values"]["Effect"][0]["DropShadow"]["radius"], 8.0);
        assert_eq!((by_key("Layout/Cols")["property"].as_str(), by_key("Layout/Cols")["name"].as_str()), (Some("Grid"), Some("Layout / 12 col")));
        assert_eq!(by_key("Layout/Cols")["values"]["Grid"][0]["Columns"]["count"], 12);
        let node = e.call("designos_get_node", json!({"node_id":id})).unwrap();
        assert_eq!(node["node"]["style_refs"]["effect"], "Shadow/Card");
    }
    fn create(e: &mut Engine, kind: &str, name: &str) -> u64 {
        e.call("designos_create_node", json!({"parent_id":1,"kind":kind,"name":name})).unwrap()["node"]["id"].as_u64().unwrap()
    }
    #[test]
    fn crud_round_trips_through_undo_and_redo() {
        let mut e = Engine::default();
        let id = create(&mut e, "text", "Title");
        let before = e.doc.to_json().unwrap();
        e.call("designos_update_node", json!({"node_id":id,"text":"Hello","x":17,"visible":false,"opacity":0.5})).unwrap();
        assert_eq!(e.doc.get(NodeId(id)).unwrap().transform.tx, 17.0);
        e.call("designos_undo", json!({})).unwrap();
        assert_eq!(e.doc.to_json().unwrap(), before);
        e.call("designos_redo", json!({})).unwrap();
        assert_eq!(e.doc.get(NodeId(id)).unwrap().kind.any_text().unwrap().content, "Hello");
        e.call("designos_delete_node", json!({"node_id":id})).unwrap();
        assert!(e.doc.get(NodeId(id)).is_none());
        e.call("designos_undo", json!({})).unwrap();
        assert!(e.doc.get(NodeId(id)).is_some());
    }
    #[test]
    fn invalid_mutations_leave_document_and_history_unchanged() {
        let mut e = Engine::default();
        let id = create(&mut e, "rectangle", "Box");
        let before = e.doc.to_json().unwrap();
        for (name, args) in [
            ("designos_create_node", json!({"parent_id":1,"kind":"bad"})),
            ("designos_create_node", json!({"parent_id":id,"kind":"frame"})),
            ("designos_create_node", json!({"parent_id":1,"kind":"text","width":-1})),
            ("designos_update_node", json!({"node_id":id,"name":"changed","text":"invalid"})),
            ("designos_update_node", json!({"node_id":id,"x":1e100})),
            ("designos_update_node", json!({"node_id":id,"opacity":2})),
            ("designos_update_node", json!({"node_id":id,"unknown":true})),
            ("designos_delete_node", json!({"node_id":1})),
            ("designos_delete_node", json!({"node_id":0})),
        ] {
            assert!(e.call(name, args).is_err(), "{name}");
            assert_eq!(e.doc.to_json().unwrap(), before);
        }
        e.call("designos_undo", json!({})).unwrap();
        assert!(!e.history.can_undo());
        assert!(e.doc.get(NodeId(id)).is_none());
    }
    #[test]
    fn name_filter_pagination_and_code_generation() {
        let mut e = Engine::default();
        let id = create(&mut e, "frame", "Card");
        create(&mut e, "rectangle", "card background");
        create(&mut e, "ellipse", "Icon");
        let result = e.call("designos_list_nodes", json!({"parent_id":1,"query":"CARD","limit":1})).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["nodes"][0]["node_id"], id);
        assert_eq!(result["next_offset"], 1);
        for language in designos_export::language::CodeLanguage::ALL.iter().map(|l| l.id()).chain(["svg"]) {
            let output = e.call("designos_generate_code", json!({"node_id":id,"language":language})).unwrap();
            assert!(!output["code"].as_str().unwrap().is_empty());
        }
        let html_doc = e.call("designos_generate_code", json!({"node_id":id,"language":"html"})).unwrap();
        assert!(html_doc["code"].as_str().unwrap().starts_with("<!doctype html>"), "{html_doc}");
        let html_frag = e.call("designos_generate_code", json!({"node_id":id,"language":"html","fragment":true})).unwrap();
        assert!(html_frag["code"].as_str().unwrap().starts_with("<style>"), "{html_frag}");
        let uikit = e.call("designos_generate_code", json!({"node_id":id,"language":"uikit"})).unwrap();
        assert!(uikit["code"].as_str().unwrap().starts_with("let card = "), "{uikit}");
        let android_id = designos_export::language::CodeLanguage::AndroidXml.id();
        let xml = e.call("designos_generate_code", json!({"node_id":id,"language":android_id})).unwrap();
        assert!(xml["code"].as_str().unwrap().starts_with("<!-- res/layout/card.xml -->"), "{xml}");
        let err = e.call("designos_generate_code", json!({"node_id":id,"language":"vue"})).unwrap_err();
        assert!(err.message.contains("uikit") && err.message.contains(android_id) && err.message.contains("html"), "{err}");
        assert!(e.call("designos_list_nodes", json!({"limit":501})).is_err());
        assert!(e.call("designos_get_node", json!({"node_id":"2"})).is_err());
    }

    #[test]
    fn spec_and_comments_workflow_round_trips() {
        let mut e = Engine::default();
        let applied = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": 1,
                    "spec": {
                        "name": "IAM · Login Screen",
                        "preset": "easeui-screen",
                        "dev_status": "ready",
                        "annotations": ["Use BrainWave 2.0 dark tokens"],
                        "children": [
                            {
                                "name": "Login Card",
                                "preset": "easeui-card",
                                "width": 400.0,
                                "height_sizing": "hug",
                                "children": [
                                    {
                                        "kind": "text",
                                        "text": "Sign in to DESIGN:OS",
                                        "font_size": 20.0,
                                        "font_weight": 600,
                                        "text_color": "$easeui.text.primary"
                                    },
                                    {
                                        "name": "Submit Button",
                                        "preset": "easeui-button-primary",
                                        "width_sizing": "fill",
                                        "text": "Continue"
                                    }
                                ]
                            }
                        ]
                    }
                }),
            )
            .unwrap();
        let screen_id = applied["root_node_id"].as_u64().unwrap();
        assert_eq!(applied["changed"], true);

        let btn_list = e.call("designos_list_nodes", json!({"query":"Submit Button"})).unwrap();
        let btn_id = btn_list["nodes"][0]["node_id"].as_u64().unwrap();
        assert_eq!(btn_list["nodes"][0]["width"], 360.0);

        let added = e
            .call(
                "designos_add_comment",
                json!({
                    "node_id": btn_id,
                    "author": "Jang",
                    "text": "Make this CTA use $easeui.accent.orange and full width",
                    "x": 12.0,
                    "y": 8.0
                }),
            )
            .unwrap();
        assert_eq!(added["id"], "t1");
        assert_eq!(added["target_node_id"], btn_id);
        assert_eq!(added["target_frame_id"], screen_id);

        let listed = e.call("designos_list_comments", json!({"resolved": false, "node_id": screen_id})).unwrap();
        assert_eq!(listed["total"], 1);

        let replied = e
            .call(
                "designos_reply_comment",
                json!({
                    "thread_id": "t1",
                    "author": "Agent",
                    "text": "Verified: CTA uses easeui-button-primary with Fill width (360px)."
                }),
            )
            .unwrap();
        assert_eq!(replied["thread"]["messages"].as_array().unwrap().len(), 2);

        let resolved = e.call("designos_resolve_comment", json!({"thread_id": "t1", "resolved": true})).unwrap();
        assert_eq!(resolved["resolved"], true);
        assert_eq!(resolved["changed"], true);

        let unresolved_list = e.call("designos_list_comments", json!({"resolved": false})).unwrap();
        assert_eq!(unresolved_list["total"], 0);

        let dev_updated = e
            .call(
                "designos_set_dev_status",
                json!({
                    "node_id": screen_id,
                    "status": "completed",
                    "annotation": "Implemented in EaseUI React"
                }),
            )
            .unwrap();
        assert_eq!(dev_updated["dev"]["status"], "Completed");
        assert_eq!(dev_updated["dev"]["annotations"].as_array().unwrap().len(), 2);

        let exported = e.call("designos_export_spec", json!({"node_id": screen_id})).unwrap();
        assert_eq!(exported["spec"]["name"], "IAM · Login Screen");
        assert_eq!(exported["spec"]["dev_status"], "Completed");
        assert_eq!(exported["spec"]["children"][0]["children"][1]["comments"][0]["id"], "t1");
        assert_eq!(exported["spec"]["children"][0]["children"][1]["comments"][0]["resolved"], true);

        // Undo set_dev_status, resolve_comment, reply_comment, add_comment, and apply_spec
        for _ in 0..5 {
            e.call("designos_undo", json!({})).unwrap();
        }
        assert!(e.doc.get(NodeId(screen_id)).is_none());
        assert!(e.doc.comments.is_empty());
    }

    /// A vertical frame of 4,097 centred rows on the first page, each holding one 20 px rectangle
    /// at x = 40. Widening the frame to 500 moves every rectangle to x = 240, but one relayout
    /// spends its 4,096 re-solves on the first rows and queues the last one for the lazy pass.
    fn rows_frame(e: &mut Engine) -> (NodeId, NodeId) {
        use designos_doc::{AutoLayout, LayoutAlign, LayoutChild, LayoutDirection, Sizing};
        let page = e.resolve_default_page().unwrap();
        let frame = e.doc.alloc_id();
        let mut f = Node::frame(frame, Rect::new(0.0, 0.0, 100.0, 2000.0));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..AutoLayout::default()
            });
        }
        e.doc.insert(page, f).unwrap();
        let mut last = NodeId(0);
        for _ in 0..4_097 {
            let row = e.doc.alloc_id();
            let mut r = Node::frame(row, Rect::new(0.0, 0.0, 100.0, 20.0));
            if let NodeKind::Frame(fp) = &mut r.kind {
                fp.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    primary_align: LayoutAlign::Center,
                    ..AutoLayout::default()
                });
            }
            r.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fixed, ..LayoutChild::default() };
            e.doc.insert(frame, r).unwrap();
            last = e.doc.alloc_id();
            e.doc.insert(row, Node::rectangle(last, Rect::new(40.0, 0.0, 20.0, 20.0))).unwrap();
        }
        (frame, last)
    }

    fn assert_settled(e: &Engine, last: NodeId, what: &str) {
        assert_eq!(e.doc.relayout_skipped_count(), 0, "{what}: the lazy layout queue must be empty when the call returns");
        assert_eq!(e.doc.get(last).unwrap().transform.tx, 240.0, "{what}: the last row is centred in the widened frame");
    }

    /// Widens the frame behind the engine's back and leaves the last row queued, as an earlier
    /// relayout would have.
    fn queue_the_last_row(e: &mut Engine, frame: NodeId, last: NodeId) {
        e.doc.get_mut(frame).unwrap().size.x = 500.0;
        e.doc.relayout_subtree(frame);
        assert_eq!(e.doc.relayout_skipped_count(), 1, "precondition: the last row is queued");
        assert_eq!(e.doc.get(last).unwrap().transform.tx, 40.0, "precondition: its rectangle is not centred yet");
    }

    #[test]
    fn update_node_finishes_the_layout_it_started() {
        let mut e = Engine::default();
        let (frame, last) = rows_frame(&mut e);
        e.call("designos_update_node", json!({"node_id":frame.0,"width":500})).unwrap();
        assert_settled(&e, last, "update_node");
        let node = e.call("designos_get_node", json!({"node_id":last.0})).unwrap();
        assert_eq!(node["node"]["transform"]["tx"], 240.0, "a reader sees the finished position");
    }

    #[test]
    fn create_node_finishes_the_layout_it_started() {
        let mut e = Engine::default();
        let (frame, last) = rows_frame(&mut e);
        e.doc.get_mut(frame).unwrap().size.x = 500.0;
        e.call("designos_create_node", json!({"parent_id":frame.0,"kind":"rectangle","name":"Extra"})).unwrap();
        assert_settled(&e, last, "create_node");
    }

    #[test]
    fn apply_spec_commits_variables_with_the_nodes_and_reports_them() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let applied = e
            .call(
                "designos_apply_spec",
                json!({"parent_id": page.0, "spec": {"name": "Panel", "kind": "frame", "fill": "$easeui.bg.panel", "stroke": "$easeui.border.subtle"}}),
            )
            .unwrap();
        let root = applied["root_node_id"].as_u64().unwrap();

        // The collection the bindings point at is part of the document, not only of a discarded copy.
        let collection = e.doc.variable_collections.get("easeui-deprecated").expect("EaseUI (deprecated) collection");
        assert_eq!(collection.name, "EaseUI (deprecated)");
        assert_eq!(e.doc.get(NodeId(root)).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("easeui-deprecated/bg/panel"));
        assert!(e.doc.variables.contains_key("easeui-deprecated/bg/panel"));

        // The reply carries the report.
        assert_eq!(applied["bindings_created"], 2, "{applied}");
        assert_eq!(applied["styles_linked"], 0);
        let warnings = applied["warnings"].as_array().unwrap();
        let deprecated: Vec<_> = warnings.iter().filter(|w| w["code"] == "DEPRECATED_EASEUI_TOKEN").collect();
        assert_eq!(deprecated.len(), 2, "{warnings:?}");
        assert_eq!(deprecated[0]["token"], "$easeui.bg.panel");
        assert_eq!(deprecated[0]["replacement"], "var:EaseUI (deprecated)/bg/panel");
        assert_eq!(deprecated[0]["path"], "spec.fill");

        // export_spec keeps the binding, so export then apply keeps the colour linked.
        let exported = e.call("designos_export_spec", json!({"node_id": root})).unwrap();
        assert_eq!(exported["spec"]["fill"], "var:EaseUI (deprecated)/bg/panel", "{exported}");
        let again = e.call("designos_apply_spec", json!({"parent_id": page.0, "spec": exported["spec"]})).unwrap();
        let copy = again["root_node_id"].as_u64().unwrap();
        assert_eq!(e.doc.get(NodeId(copy)).unwrap().variables.bindings.get("fills/0"), e.doc.get(NodeId(root)).unwrap().variables.bindings.get("fills/0"));
        assert_eq!(again["warnings"].as_array().unwrap().iter().filter(|w| w["code"] == "DEPRECATED_EASEUI_TOKEN").count(), 0);

        // The second apply added no data, so one undo removes only its nodes; one more undo removes the
        // first apply's nodes and the collection together, and redo restores both.
        e.call("designos_undo", json!({})).unwrap();
        assert!(e.doc.get(NodeId(copy)).is_none());
        assert!(e.doc.variable_collections.contains_key("easeui-deprecated"));
        e.call("designos_undo", json!({})).unwrap();
        assert!(e.doc.get(NodeId(root)).is_none());
        assert!(!e.doc.variable_collections.contains_key("easeui-deprecated"));
        assert!(e.doc.variables.is_empty());
        e.call("designos_redo", json!({})).unwrap();
        assert!(e.doc.get(NodeId(root)).is_some());
        assert!(e.doc.variable_collections.contains_key("easeui-deprecated"));
        assert!(e.doc.variables.contains_key("easeui-deprecated/bg/panel"));
    }

    #[test]
    fn a_failed_apply_spec_leaves_variables_and_undo_depth_untouched() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let depth = e.history.undo_depth();
        let err = e
            .call(
                "designos_apply_spec",
                json!({"parent_id": page.0, "spec": {"kind": "frame", "children": [
                    {"kind": "rectangle", "fill": "$easeui.bg.panel"},
                    {"kind": "rectangle", "fill": "var:Color/Nope"}
                ]}}),
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::UnknownVariable, "{err}");
        assert!(e.doc.variable_collections.is_empty());
        assert!(e.doc.variables.is_empty());
        assert_eq!(e.history.undo_depth(), depth);
    }

    #[test]
    fn apply_spec_finishes_the_layout_it_started() {
        let mut e = Engine::default();
        let (frame, last) = rows_frame(&mut e);
        e.doc.get_mut(frame).unwrap().size.x = 500.0;
        e.call("designos_apply_spec", json!({"parent_id":frame.0,"spec":{"kind":"rectangle","name":"Extra","width":10,"height":10}})).unwrap();
        assert_settled(&e, last, "apply_spec");
    }

    #[test]
    fn readers_that_serialize_or_generate_finish_a_queue_they_find() {
        let dir = std::env::temp_dir().join(format!("designos_engine_settle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for tool in ["designos_generate_code", "designos_export_fig_file", "designos_export_spec", "designos_lint"] {
            let mut e = Engine::default();
            let (frame, last) = rows_frame(&mut e);
            queue_the_last_row(&mut e, frame, last);
            let out = dir.join("out.fig");
            let args = match tool {
                "designos_generate_code" => json!({"node_id":frame.0,"language":"css"}),
                "designos_export_fig_file" => json!({"output_path":out.to_str().unwrap()}),
                "designos_lint" => json!({"node_ids":[frame.0],"limit":10}),
                _ => json!({"node_id":frame.0}),
            };
            e.call(tool, args).unwrap();
            assert_settled(&e, last, tool);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_fig_file_reports_the_comment_threads_a_fig_leaves_out() {
        let dir = std::env::temp_dir().join(format!("designos_engine_fig_comments_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = Engine::default();
        let none = e.call("designos_export_fig_file", json!({"output_path": dir.join("none.fig").to_str().unwrap()})).unwrap();
        assert_eq!(none["omitted_comment_threads"], 0);
        e.call("designos_add_comment", json!({"text": "Make this bigger"})).unwrap();
        e.call("designos_add_comment", json!({"text": "And this one"})).unwrap();
        let two = e.call("designos_export_fig_file", json!({"output_path": dir.join("two.fig").to_str().unwrap()})).unwrap();
        assert_eq!(two["omitted_comment_threads"], 2);
        assert_eq!(e.doc.comments.len(), 2, "the session keeps its comments");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_duplicate_language_list() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let needle = format!("\"{}\"", designos_export::language::CodeLanguage::AndroidXml.id());
        fn scan_dir(dir: &std::path::Path, needle: &str) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    scan_dir(&path, needle);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path).unwrap();
                    for (idx, line) in content.lines().enumerate() {
                        if line.contains(needle) && !line.contains("CodeLanguage") {
                            panic!("{}:{} contains literal {needle} outside a CodeLanguage call", path.display(), idx + 1);
                        }
                    }
                }
            }
        }
        scan_dir(&src_dir, &needle);
    }

    #[test]
    fn mcp_generate_code_ids_match_dropdown() {
        let list = tools();
        let gen_tool = list.iter().find(|t| t["name"] == "designos_generate_code").unwrap();
        let enum_vals: Vec<String> = gen_tool["inputSchema"]["properties"]["language"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .filter(|v| v != "svg")
            .collect();
        let expected: Vec<String> = designos_export::language::CodeLanguage::ALL.iter().map(|l| l.id().to_string()).collect();
        assert_eq!(enum_vals, expected);
    }

    #[test]
    fn apply_spec_instance_round_trips_designos_and_fig_and_exports_spec() {
        let mut e = Engine { doc: designos_doc::fixtures::button_library(), ..Engine::default() };
        let page = e.doc.pages[0];
        e.active_page_id = Some(page);

        let res = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": page.0,
                    "spec": {
                        "kind": "instance",
                        "name": "Hero CTA",
                        "component": "Button",
                        "variant": { "Size": "lg", "State": "hover" },
                        "overrides": [
                            { "path": "Label", "text": "Launch" },
                            { "path": "Icon", "swap": "Icon/Arrow" }
                        ]
                    }
                }),
            )
            .unwrap();

        let root_id = NodeId(res["root_node_id"].as_u64().unwrap());
        assert_eq!(res["instances"].as_array().unwrap().len(), 1);
        assert_eq!(res["name_to_id"]["Hero CTA"], root_id.0);

        // .designos JSON round-trip
        let json_str = e.doc.to_json().unwrap();
        let from_json = Document::from_json(&json_str).unwrap();
        let inst_json = from_json.get(root_id).unwrap();
        let comp_json = designos_doc::instance_component(inst_json).unwrap();
        assert_eq!(from_json.get(comp_json).unwrap().name, "Size=lg, State=hover");

        // .fig binary round-trip
        let fig_bytes = designos_fig::export_fig(&e.doc).unwrap();
        let from_fig = designos_fig::import_fig(&fig_bytes).unwrap();
        let fig_inst = from_fig
            .nodes
            .values()
            .find(|n| matches!(n.kind, NodeKind::Instance { .. }) && n.name == "Hero CTA")
            .expect("Hero CTA instance must survive .fig round-trip");
        let fig_comp = designos_doc::instance_component(fig_inst).expect("Hero CTA must remain linked to its variant component after .fig import");
        assert_eq!(from_fig.get(fig_comp).unwrap().name, "Size=lg, State=hover");
        let fig_label_id = designos_doc::resolve_override_path(&from_fig, fig_inst.id, "Label").unwrap();
        assert_eq!(from_fig.get(fig_label_id).unwrap().kind.any_text().unwrap().content, "Launch");

        // designos_export_spec -> designos_apply_spec round-trip
        let exported = e.call("designos_export_spec", json!({ "node_id": root_id.0 })).unwrap();
        assert_eq!(exported["spec"]["kind"], "instance");
        let reapplied = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": page.0,
                    "spec": exported["spec"]
                }),
            )
            .unwrap();
        let root2 = NodeId(reapplied["root_node_id"].as_u64().unwrap());
        assert_eq!(designos_doc::instance_component(e.doc.get(root_id).unwrap()), designos_doc::instance_component(e.doc.get(root2).unwrap()));

        // Undo removes the second instance in one step
        e.call("designos_undo", json!({})).unwrap();
        assert!(e.doc.get(root2).is_none());
        assert!(e.doc.get(root_id).is_some());
    }

    #[test]
    fn engine_exec_applies_the_boundary() {
        use designos_doc::live_patch::{replay_live_patch, LivePatch, LivePatchState};
        use designos_doc::{AlignSelf, AutoLayout, FrameProps, LayoutDirection, Rect, Sizing};

        let mut e = Engine::default();
        let page = e.active_page_id.unwrap();
        let fid = e.doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 120.0));
        let mut al = AutoLayout::v1();
        al.direction = LayoutDirection::Horizontal;
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Fixed;
        frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), grids: Vec::new() });
        e.doc.insert(page, frame).unwrap();

        let cid = e.doc.alloc_id();
        let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 50.0, 30.0));
        child.layout_child.horizontal = Sizing::Fixed;
        child.layout_child.vertical = Sizing::Fill;
        child.layout_child.align_self = AlignSelf::Stretch;
        e.doc.insert(fid, child).unwrap();

        let mut mirror_doc = e.doc.clone();
        e.live_ops = Some(Vec::new());

        let mut flipped = e.doc.get(fid).unwrap().clone();
        flipped.kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().direction = LayoutDirection::Vertical;
        e.exec(Command::Update { node: Box::new(flipped) }).unwrap();
        e.relayout(fid);

        let ops = e.take_live_ops();
        assert_eq!(ops.len(), 2, "recorded live ops must include parent update and child update");
        assert!(matches!(&ops[0], LiveOp::Update { id, .. } if *id == fid));
        match &ops[1] {
            LiveOp::Update { id, set, .. } => {
                assert_eq!(*id, cid);
                assert_eq!(
                    set.get("layout_child"),
                    Some(&json!({"align_self": "Auto"})),
                    "child LiveOp::Update must carry only the changed layout_child subfields"
                );
            }
            other => panic!("expected child LiveOp::Update, got {other:?}"),
        }

        // Simulate a concurrent editor edit to the child's margin before the agent patch arrives.
        mirror_doc.get_mut(cid).unwrap().layout_child.margin = [4.0, 8.0, 4.0, 8.0];

        let report = replay_live_patch(&mut mirror_doc, &LivePatch::new(ops), &mut LivePatchState::default(), &mut |d, c| c.apply(d).map(|_| ())).unwrap();
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(
            mirror_doc.get(cid).unwrap().layout_child.margin,
            [4.0, 8.0, 4.0, 8.0],
            "replaying the child update must preserve concurrent editor layout_child subfields"
        );
        mirror_doc.get_mut(cid).unwrap().layout_child.margin = [0.0; 4];
        mirror_doc.relayout_subtree(fid);
        mirror_doc.flush_pending_relayout();

        assert_eq!(mirror_doc.get(fid).unwrap(), e.doc.get(fid).unwrap());
        assert_eq!(mirror_doc.get(cid).unwrap(), e.doc.get(cid).unwrap());
        assert_eq!(e.doc.get(cid).unwrap().layout_child.align_self, AlignSelf::Auto);
        assert_eq!(e.doc.get(cid).unwrap().layout_child.vertical, Sizing::Fill);
    }

    #[test]
    fn engine_exec_does_not_clone_the_document_for_a_flip() {
        use designos_doc::live_patch::batch_doc_clones;
        use designos_doc::{AlignSelf, AutoLayout, FrameProps, LayoutDirection, Rect, Sizing};
        use std::time::Instant;

        let mut e = Engine::default();
        let page = e.active_page_id.unwrap();

        let fid = e.doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 300.0));
        let mut al = AutoLayout::v1();
        al.direction = LayoutDirection::Horizontal;
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Fixed;
        frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), grids: Vec::new() });
        e.doc.insert(page, frame).unwrap();

        // Populate 10,000 direct children under `fid` with the duplicate stretch pair.
        let mut child_ids = Vec::with_capacity(10_000);
        for i in 0..10_000usize {
            let cid = e.doc.alloc_id();
            let mut c = Node::rectangle(cid, Rect::new((i % 100) as f32 * 4.0, (i / 100) as f32 * 4.0, 20.0, 10.0));
            c.parent = Some(fid);
            c.layout_child.horizontal = Sizing::Fixed;
            c.layout_child.vertical = Sizing::Fill;
            c.layout_child.align_self = AlignSelf::Stretch;
            e.doc.nodes.insert(cid, c);
            child_ids.push(cid);
        }
        e.doc.get_mut(fid).unwrap().children = child_ids;

        // Populate the remaining nodes on `page` so total document size is 100,000 nodes.
        let existing = e.doc.nodes.len();
        let remaining = 100_000usize.saturating_sub(existing);
        let mut page_extra = Vec::with_capacity(remaining);
        for i in 0..remaining {
            let id = e.doc.alloc_id();
            let mut n = Node::rectangle(id, Rect::new((i % 200) as f32 * 6.0, (i / 200) as f32 * 6.0, 4.0, 4.0));
            n.parent = Some(page);
            e.doc.nodes.insert(id, n);
            page_extra.push(id);
        }
        e.doc.get_mut(page).unwrap().children.extend(page_extra);
        assert_eq!(e.doc.nodes.len(), 100_000);

        let mut flipped = e.doc.get(fid).unwrap().clone();
        flipped.kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().direction = LayoutDirection::Vertical;
        let update_cmd = Command::Update { node: Box::new(flipped) };

        // Compute the reference batch ops via `live_ops_for_command` on the batch BEFORE mutating `e.doc`.
        let w_ref = designos_doc::writer_command(&e.doc, update_cmd.clone());
        let clones_before_batch = batch_doc_clones();
        let batch_ops = live_ops_for_command(&e.doc, &w_ref.cmd).unwrap();
        assert_eq!(batch_doc_clones(), clones_before_batch + 1, "batch path clones the document once");

        // Now run through `Engine::exec` and verify zero document clones and identical live ops.
        e.live_ops = Some(Vec::new());
        let clones_before_exec = batch_doc_clones();
        let t0 = Instant::now();
        e.exec(update_cmd).unwrap();
        let elapsed = t0.elapsed();
        assert_eq!(batch_doc_clones(), clones_before_exec, "Engine::exec must not clone the document");

        let exec_ops = e.take_live_ops();
        assert_eq!(exec_ops.len(), 10_001);
        assert_eq!(exec_ops, batch_ops);

        // Undo and redo must also record live ops without cloning the 100k-node document.
        e.live_ops = Some(Vec::new());
        let clones_before_undo = batch_doc_clones();
        e.call("designos_undo", json!({})).unwrap();
        assert_eq!(batch_doc_clones(), clones_before_undo, "designos_undo must not clone the document");
        assert_eq!(e.take_live_ops().len(), 10_001);

        e.live_ops = Some(Vec::new());
        let clones_before_redo = batch_doc_clones();
        e.call("designos_redo", json!({})).unwrap();
        assert_eq!(batch_doc_clones(), clones_before_redo, "designos_redo must not clone the document");
        assert_eq!(e.take_live_ops().len(), 10_001);

        let budget_ms: u128 = if cfg!(debug_assertions) { 2_500 } else { 100 };
        eprintln!("[#583 bench] 10k-children on 100k-node doc Engine::exec={}ms (budget={}ms)", elapsed.as_secs_f64() * 1000.0, budget_ms);
        assert!(elapsed.as_millis() <= budget_ms, "Engine::exec took {}ms > {}ms budget", elapsed.as_millis(), budget_ms);
    }

    #[test]
    fn apply_spec_slots_issues_and_fig_round_trip() {
        let mut e = Engine { doc: designos_doc::fixtures::card_with_slots(), ..Engine::default() };
        let page = e.doc.pages[0];
        e.active_page_id = Some(page);

        // Passing `"footer": true` fails with structured INVALID_SLOT_VALUE and does not mutate doc/history
        let err_str = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": page.0,
                    "spec": {
                        "kind": "instance",
                        "component": "Card",
                        "slots": { "footer": true }
                    }
                }),
            )
            .unwrap_err();
        assert_eq!(err_str.code, ErrorCode::InvalidSlotValue);
        assert!(!e.history.can_undo());

        // Filling `body` with a recommended FormRow + a non-recommended raw frame succeeds and returns `issues`
        let res = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": page.0,
                    "spec": {
                        "kind": "instance",
                        "name": "SettingsCard",
                        "component": "Card",
                        "slots": {
                            "header": [
                                { "kind": "instance", "name": "Hdr", "component": "CardHeader" }
                            ],
                            "body": [
                                { "kind": "instance", "name": "RowA", "component": "FormRow" },
                                { "kind": "frame", "name": "CustomBanner", "height": 32 }
                            ],
                            "footer": false
                        }
                    }
                }),
            )
            .unwrap();

        let root_id = NodeId(res["root_node_id"].as_u64().unwrap());
        let issues = res["issues"].as_array().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0]["rule"], "slot-non-recommended");
        assert_eq!(issues[0]["severity"], "warning");
        assert_eq!(issues[0]["node_name"], "CustomBanner");

        // .fig binary round-trip preserves filled slot children
        let fig_bytes = designos_fig::export_fig(&e.doc).unwrap();
        let from_fig = designos_fig::import_fig(&fig_bytes).unwrap();
        let fig_card = from_fig
            .nodes
            .values()
            .find(|n| matches!(n.kind, NodeKind::Instance { .. }) && n.name == "SettingsCard")
            .expect("SettingsCard must survive .fig round-trip");
        let fig_body = designos_doc::resolve_override_path(&from_fig, fig_card.id, "body").unwrap();
        assert_eq!(from_fig.get(fig_body).unwrap().children.len(), 2);

        // Undo removes the placed Card and all its slot children in 1 step
        e.call("designos_undo", json!({})).unwrap();
        assert!(e.doc.get(root_id).is_none());
    }

    #[test]
    fn mcp_lint_returns_full_rule_set() {
        use designos_doc::{
            AutoLayout, Color, FrameProps, LayoutChild, LayoutDirection, Paint, Sizing, TextAutoResize, TextStyle, Variable, VariableCollection, VariableKind,
            VariableMode, VariableValue,
        };
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();

        e.doc.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                id: "c1".into(),
                name: "Tokens".into(),
                modes: vec![VariableMode { id: "default".into(), name: "Default".into() }],
                default_mode: "default".into(),
                variables: vec!["v1".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        e.doc.variables.insert(
            "v1".into(),
            Variable {
                id: "v1".into(),
                name: "Color/Primary".into(),
                collection: "c1".into(),
                kind: VariableKind::Color,
                values: [("default".to_string(), VariableValue::Color(Color::rgb(12, 34, 56)))].into_iter().collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        // 1. overflow-parent: non-clipping frame with overflowing child
        let ov_parent = e.doc.alloc_id();
        let mut ov_f = Node::frame(ov_parent, Rect::new(0.0, 0.0, 100.0, 100.0));
        ov_f.name = "OverflowParent".into();
        ov_f.kind = NodeKind::Frame(FrameProps { clips_content: false, ..Default::default() });
        e.doc.insert(page, ov_f).unwrap();
        let ov_child = e.doc.alloc_id();
        let mut ov_c = Node::rectangle(ov_child, Rect::new(60.0, 0.0, 80.0, 40.0));
        ov_c.name = "OverflowChild".into();
        ov_c.fills = vec![Paint::solid(Color::rgb(12, 34, 56))]; // also hardcoded-color + hardcoded-value
        e.doc.insert(ov_parent, ov_c).unwrap();

        // 2 & 3. clipped-partial and clipped-full inside clipping frame
        let clip_parent = e.doc.alloc_id();
        let mut clip_f = Node::frame(clip_parent, Rect::new(200.0, 0.0, 100.0, 100.0));
        clip_f.name = "ClipParent".into();
        clip_f.kind = NodeKind::Frame(FrameProps { clips_content: true, ..Default::default() });
        e.doc.insert(page, clip_f).unwrap();
        let cp_id = e.doc.alloc_id();
        let mut cp_n = Node::rectangle(cp_id, Rect::new(80.0, 10.0, 40.0, 40.0));
        cp_n.name = "PartialClip".into();
        e.doc.insert(clip_parent, cp_n).unwrap();
        let cf_id = e.doc.alloc_id();
        let mut cf_n = Node::rectangle(cf_id, Rect::new(140.0, 10.0, 20.0, 20.0));
        cf_n.name = "FullClip".into();
        e.doc.insert(clip_parent, cf_n).unwrap();

        // 4, 8, 19, 26, 27. Plain frame with sibling-overlap, missing-auto-layout, misaligned-siblings, sizing-conflict, default-layer-name
        let plain_f = e.doc.alloc_id();
        let mut pf = Node::frame(plain_f, Rect::new(400.0, 0.0, 200.0, 200.0));
        pf.name = "Frame 1".into(); // default-layer-name
        e.doc.insert(page, pf).unwrap();
        let s1 = e.doc.alloc_id();
        let mut s1_n = Node::rectangle(s1, Rect::new(10.0, 10.0, 60.0, 40.0));
        s1_n.name = "SibA".into();
        s1_n.fills = vec![Paint::solid(Color::BLACK)];
        s1_n.layout_child = LayoutChild { horizontal: Sizing::Fill, ..Default::default() }; // sizing-conflict
        e.doc.insert(plain_f, s1_n).unwrap();
        let s2 = e.doc.alloc_id();
        let mut s2_n = Node::rectangle(s2, Rect::new(40.0, 12.0, 60.0, 40.0)); // overlaps s1 + 2px y misalignment
        s2_n.name = "SibB".into();
        s2_n.fills = vec![Paint::solid(Color::BLACK)];
        e.doc.insert(plain_f, s2_n).unwrap();

        // 5, 6, 9, 11, 15, 20, 22, 17. Text rules + low-contrast-text + detached-text-style
        let bg_f = e.doc.alloc_id();
        let mut bg = Node::frame(bg_f, Rect::new(0.0, 240.0, 400.0, 200.0));
        bg.name = "TextHost".into();
        bg.fills = vec![Paint::solid(Color::WHITE)];
        e.doc.insert(page, bg).unwrap();

        // text-truncated + non-scale-font-size (15px) + detached-text-style + low-contrast-text
        let t_trunc = e.doc.alloc_id();
        let mut tt = Node::text(
            t_trunc,
            Rect::new(8.0, 8.0, 80.0, 20.0),
            "Enterprise annual plan subscription",
            TextStyle { font_size: 15.0, fills: vec![Paint::solid(Color::rgb(210, 210, 210))], ..Default::default() },
        );
        tt.name = "PlanTitle".into();
        if let NodeKind::Text(tp) = &mut tt.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        e.doc.insert(bg_f, tt).unwrap();

        // text-unexpected-wrap
        let t_wrap = e.doc.alloc_id();
        let mut tw = Node::text(t_wrap, Rect::new(8.0, 40.0, 60.0, 60.0), "Subscribe now", TextStyle { font_size: 16.0, ..Default::default() });
        tw.name = "CTA".into();
        if let NodeKind::Text(tp) = &mut tw.kind {
            tp.auto_resize = TextAutoResize::Height;
        }
        e.doc.insert(bg_f, tw).unwrap();

        // text-missing-fill + placeholder-text + tiny-text
        let t_ghost = e.doc.alloc_id();
        let mut tg =
            Node::text(t_ghost, Rect::new(8.0, 110.0, 120.0, 20.0), "Lorem ipsum dolor", TextStyle { font_size: 9.0, fills: vec![], ..Default::default() });
        tg.name = "GhostLorem".into();
        tg.fills.clear();
        e.doc.insert(bg_f, tg).unwrap();

        // 7 & 16. collapsed-size + non-4px-grid + inconsistent-radius
        let col_id = e.doc.alloc_id();
        let mut col_n = Node::rectangle(col_id, Rect::new(0.0, 480.0, 0.0, 19.0));
        col_n.name = "CollapsedRect".into();
        col_n.corner_radii = [7.0, 7.0, 7.0, 7.0]; // off-scale radius
        e.doc.insert(page, col_n).unwrap();

        // 10, 14, 18. Auto-layout with >50% absolute children, inconsistent-spacing (11px), unbalanced-padding
        let al_id = e.doc.alloc_id();
        let mut al_n = Node::frame(al_id, Rect::new(100.0, 480.0, 200.0, 120.0));
        al_n.name = "AutoFrame".into();
        al_n.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                item_spacing: 11.0,
                padding: [8.0, 24.0, 8.0, 8.0],
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            }),
            ..Default::default()
        });
        e.doc.insert(page, al_n).unwrap();
        for i in 0..2 {
            let ac = e.doc.alloc_id();
            let mut ac_n = Node::rectangle(ac, Rect::new(0.0, 0.0, 20.0, 20.0));
            ac_n.name = format!("AbsChild{i}");
            ac_n.layout_child = LayoutChild { absolute: true, ..Default::default() };
            e.doc.insert(al_id, ac_n).unwrap();
        }

        // 12. detached-instance
        let det_id = e.doc.alloc_id();
        let mut det_n = Node::frame(det_id, Rect::new(340.0, 480.0, 80.0, 40.0));
        det_n.name = "DetachedCard".into();
        det_n.detached_symbol_id = Some("guid:99:1".into());
        e.doc.insert(page, det_n).unwrap();

        // 14 & 24. empty-container + deeply-nested (depth > 6)
        let mut parent_id = page;
        for d in 1..=7 {
            let nid = e.doc.alloc_id();
            let mut nf = Node::frame(nid, Rect::new(0.0, 0.0, 80.0, 80.0));
            nf.name = format!("Nest{d}");
            if d == 7 {
                nf.fills.clear();
            }
            e.doc.insert(parent_id, nf).unwrap();
            parent_id = nid;
        }

        // 21. small-touch-target (button < 44x44)
        let btn_id = e.doc.alloc_id();
        let mut btn = Node::frame(btn_id, Rect::new(500.0, 480.0, 32.0, 28.0));
        btn.name = "Save Button".into();
        btn.fills = vec![Paint::solid(Color::BLACK)];
        e.doc.insert(page, btn).unwrap();

        bind_style_to_missing_variable(&mut e);
        let res = e.call("designos_lint", json!({"preset": "strict", "limit": 500})).unwrap();
        assert!(res.get("passed").is_none(), "old two-rule `passed` field must be gone");
        let by_rule = res["by_rule"].as_object().expect("by_rule map");
        for rule_id in designos_doc::lint::all_rule_ids() {
            assert!(
                by_rule.get(rule_id).and_then(Value::as_u64).unwrap_or(0) >= 1,
                "strict preset must report finding for rule '{rule_id}', got by_rule={by_rule:?}"
            );
        }
    }

    #[test]
    fn same_findings_three_callers() {
        use designos_doc::lint::{lint, LintConfig, LintPreset};
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();

        let comp_id = e.doc.alloc_id();
        let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 120.0, 40.0));
        comp.name = "PrimaryButton".into();
        comp.source_id = Some("10:1".into());
        comp.kind = NodeKind::Component(Default::default());
        e.doc.insert(page, comp).unwrap();

        let det_id = e.doc.alloc_id();
        let mut det = Node::frame(det_id, Rect::new(0.0, 80.0, 120.0, 40.0));
        det.name = "DetachedCopy".into();
        det.detached_symbol_id = Some("guid:10:1".into());
        e.doc.insert(page, det).unwrap();

        // Caller 1: MCP designos_lint with preset "check-designs"
        let mcp_res = e.call("designos_lint", json!({"preset": "check-designs"})).unwrap();
        let mcp_findings = mcp_res["findings"].as_array().unwrap();

        // Caller 2: Direct library call with RenderLintEnv and CheckDesigns preset
        let lib_cfg = LintConfig::from_preset(LintPreset::CheckDesigns);
        let lib_report = lint(&e.doc, &lib_cfg, &designos_render::RenderLintEnv);
        let lib_val = lib_report.to_value();
        let lib_findings = lib_val["findings"].as_array().unwrap();

        // Caller 3: Check designs shared detached rule engine
        let comps = designos_doc::lint::rules::detached::Components::new(&e.doc);
        let det_node = e.doc.get(det_id).unwrap();
        let cd_plan = designos_doc::lint::rules::detached::plan(&e.doc, &comps, det_node);
        let cd_equal = match cd_plan {
            designos_doc::lint::rules::detached::DetachedPlan::Compare { component } => {
                assert_eq!(component, comp_id);
                designos_doc::lint::rules::detached::matches_component(&e.doc, det_id, component)
            }
            other => panic!("expected Compare plan, got {other:?}"),
        };
        let (cd_msg, cd_reconnect, _) = designos_doc::lint::rules::detached::compared_outcome(&e.doc, comp_id, cd_equal);
        // Touch comps again so clippy knows it's used.
        assert_eq!(comps.resolve("guid:10:1"), Some(comp_id));

        assert_eq!(mcp_findings, lib_findings, "MCP and direct library call must match identically");
        assert_eq!(mcp_findings.len(), 1);
        assert_eq!(mcp_findings[0]["rule"], "detached-instance");
        assert_eq!(mcp_findings[0]["node_id"], det_id.0);
        assert_eq!(mcp_findings[0]["message"], cd_msg);
        assert_eq!(mcp_findings[0]["suggested_fix"]["to"].as_u64(), cd_reconnect.map(|id| id.0));
    }

    #[test]
    fn mcp_lint_error_contracts_and_history_fix() {
        let mut e = Engine::default();
        let id = create(&mut e, "frame", "Frame 1");
        let id2 = create(&mut e, "frame", "Frame 2");

        let err_scope = e.call("designos_lint", json!({"page_id": 1, "node_ids": [id]})).unwrap_err();
        assert!(err_scope.contains("EXACTLY_ONE_SCOPE") && err_scope.contains("safe_to_retry"), "{err_scope}");

        let err_preset = e.call("designos_lint", json!({"preset": "nonexistent-preset"})).unwrap_err();
        assert!(err_preset.contains("UNKNOWN_PRESET") && err_preset.contains("valid_options"), "{err_preset}");

        let err_rule = e.call("designos_lint", json!({"rules": ["nonexistent-rule"]})).unwrap_err();
        assert!(err_rule.contains("UNKNOWN_RULE") && err_rule.contains("valid_options"), "{err_rule}");

        let err_node = e.call("designos_lint", json!({"node_ids": [999_999]})).unwrap_err();
        assert!(err_node.contains("NODE_NOT_FOUND"), "{err_node}");

        // designos_lint is readOnlyHint: true and must reject `fix: true` without mutating.
        let err_fix_on_readonly = e.call("designos_lint", json!({"rules": ["default-layer-name"], "fix": true})).unwrap_err();
        assert_eq!(err_fix_on_readonly.code, ErrorCode::UnknownField, "{err_fix_on_readonly}");
        assert_eq!(err_fix_on_readonly.path, "/fix", "{err_fix_on_readonly}");
        assert_eq!(e.doc.get(NodeId(id)).unwrap().name, "Frame 1");

        // designos_lint_fix with preset "agent" (where auto-fixable rules are off) must not mutate.
        let no_fix_agent = e.call("designos_lint_fix", json!({"preset": "agent"})).unwrap();
        assert_eq!(no_fix_agent["fixed_count"], 0);
        assert_eq!(e.doc.get(NodeId(id)).unwrap().name, "Frame 1");
        assert_eq!(e.doc.get(NodeId(id2)).unwrap().name, "Frame 2");

        // designos_lint_fix scoped by node_ids fixes only the targeted subtree and routes through History.
        let fixed = e.call("designos_lint_fix", json!({"node_ids": [id], "rules": ["default-layer-name"]})).unwrap();
        assert_eq!(fixed["fixed_count"], 1);
        assert_ne!(e.doc.get(NodeId(id)).unwrap().name, "Frame 1");
        assert_eq!(e.doc.get(NodeId(id2)).unwrap().name, "Frame 2", "unscoped sibling must remain untouched");
        e.call("designos_undo", json!({})).unwrap();
        assert_eq!(e.doc.get(NodeId(id)).unwrap().name, "Frame 1");

        // Offset pagination serves from Engine::lint_cache without rescanning (`scanned == 0`).
        let page1 = e.call("designos_lint", json!({"rules": ["default-layer-name"], "limit": 1})).unwrap();
        assert_eq!(page1["findings"].as_array().unwrap().len(), 1);
        assert_eq!(page1["total"], 2);
        assert_eq!(page1["complete"], true);
        assert_eq!(page1["cursor"], "offset:1");
        assert!(page1["scanned"].as_u64().unwrap() >= 2);

        let page2 = e.call("designos_lint", json!({"rules": ["default-layer-name"], "limit": 1, "cursor": "offset:1"})).unwrap();
        assert_eq!(page2["findings"].as_array().unwrap().len(), 1);
        assert_eq!(page2["total"], 2);
        assert_eq!(page2["complete"], true);
        assert!(page2["cursor"].is_null());
        assert_eq!(page2["scanned"], 0, "offset cursor pagination must serve from cached report without rescanning");
        assert_ne!(page1["findings"][0]["node_id"], page2["findings"][0]["node_id"]);

        let catalog = tools();
        assert!(catalog.iter().any(|t| t["name"] == "designos_lint_fix" && t["annotations"]["readOnlyHint"] == false));
    }

    fn bind_style_to_missing_variable(e: &mut Engine) {
        let mut bindings = std::collections::BTreeMap::new();
        bindings.insert("fill".to_string(), "var-deleted".to_string());
        e.doc.style_meta.insert(
            "Brand/Primary".into(),
            designos_doc::StyleMeta {
                name: "Brand/Primary".into(),
                key: None,
                description: String::new(),
                property: designos_doc::StyleProperty::Fill,
                bindings,
            },
        );
    }

    #[test]
    fn mcp_lint_broken_style_binding_appears_once_across_budgeted_chunks_with_consistent_totals() {
        let mut e = Engine::default();
        for i in 1..=20 {
            create(&mut e, "frame", &format!("Frame {i}"));
        }
        bind_style_to_missing_variable(&mut e);
        e.lint_call_max_nodes = 8;
        e.clear_lint_cache();

        let mut cursor: Option<String> = None;
        let mut style_hits = 0usize;
        let mut chunk_totals: Vec<(bool, u64)> = Vec::new();
        let mut saw_incomplete_chunk = false;
        for _ in 0..40 {
            let mut req = json!({"preset": "strict", "limit": 3});
            if let Some(ref c) = cursor {
                req["cursor"] = json!(c);
            }
            let res = e.call("designos_lint", req).unwrap();
            let batch = res["findings"].as_array().unwrap();
            assert!(batch.len() <= 3, "page must not exceed limit: {}", batch.len());
            style_hits += batch.iter().filter(|f| f["rule"] == "broken-style-binding").count();
            let resumed = cursor.as_deref().is_some_and(|c| c.starts_with("stack:"));
            let by_rule_hits = res["by_rule"]["broken-style-binding"].as_u64().unwrap_or(0);
            assert_eq!(by_rule_hits, u64::from(!resumed), "by_rule counts the style finding only in the first chunk");
            chunk_totals.push((resumed, res["total"].as_u64().unwrap()));
            saw_incomplete_chunk |= res["complete"] == false;
            match res["cursor"].as_str() {
                Some(next) => cursor = Some(next.to_string()),
                None => break,
            }
        }
        assert!(saw_incomplete_chunk, "test must exercise budgeted chunks");
        assert_eq!(style_hits, 1, "the style finding must be returned exactly once across all chunks");
        let first: Vec<u64> = chunk_totals.iter().take_while(|(resumed, _)| !resumed).map(|(_, t)| *t).collect();
        assert!(first.len() > 1, "first chunk must span several cached pages");
        assert!(first.windows(2).all(|w| w[0] == w[1]), "cached pages must report the first page total: {first:?}");
    }

    fn style_hits(res: &Value) -> usize {
        res["findings"].as_array().unwrap().iter().filter(|f| f["rule"] == "broken-style-binding").count()
    }

    #[test]
    fn mcp_lint_broken_style_binding_follows_the_active_preset() {
        let mut e = Engine::default();
        create(&mut e, "frame", "Frame 1");
        bind_style_to_missing_variable(&mut e);
        for preset in ["agent", "recommended", "strict", "tokens"] {
            let res = e.call("designos_lint", json!({"preset": preset, "limit": 500})).unwrap();
            assert_eq!(style_hits(&res), 1, "{preset}");
            assert_eq!(res["by_rule"]["broken-style-binding"], 1, "{preset}");
        }
        let default = e.call("designos_lint", json!({"limit": 500})).unwrap();
        assert_eq!(style_hits(&default), 1, "default preset is agent");
        let layout = e.call("designos_lint", json!({"preset": "layout", "limit": 500})).unwrap();
        assert_eq!(style_hits(&layout), 0);
        assert!(layout["by_rule"].get("broken-style-binding").is_none());
    }

    #[test]
    fn mcp_lint_rules_list_accepts_broken_style_binding() {
        let mut e = Engine::default();
        create(&mut e, "frame", "Frame 1");
        bind_style_to_missing_variable(&mut e);
        let only = e.call("designos_lint", json!({"rules": ["broken-style-binding"], "limit": 500})).unwrap();
        assert_eq!(style_hits(&only), 1);
        assert_eq!(only["findings"].as_array().unwrap().len(), 1, "only the requested rule is returned");
        assert_eq!(only["total"], 1);
        let mixed = e.call("designos_lint", json!({"rules": ["default-layer-name", "broken-style-binding"], "limit": 500})).unwrap();
        assert_eq!(style_hits(&mixed), 1);
        let other = e.call("designos_lint", json!({"rules": ["default-layer-name"], "limit": 500})).unwrap();
        assert_eq!(style_hits(&other), 0);
    }

    #[test]
    fn mcp_lint_page_with_extras_marks_truncated_from_node_and_style_findings() {
        let mut e = Engine::default();
        create(&mut e, "frame", "Frame 1");
        create(&mut e, "frame", "Frame 2");
        bind_style_to_missing_variable(&mut e);
        let mut meta = e.doc.style_meta["Brand/Primary"].clone();
        meta.name = "Brand/Secondary".into();
        e.doc.style_meta.insert("Brand/Secondary".into(), meta);
        e.clear_lint_cache();
        let req = json!({"rules": ["default-layer-name", "broken-style-binding"], "limit": 3});
        let first = e.call("designos_lint", req.clone()).unwrap();
        assert_eq!(first["total"], 4, "2 node + 2 style findings");
        assert_eq!(first["findings"].as_array().unwrap().len(), 3);
        assert_eq!(first["truncated"], true);
        let mut next = req;
        next["cursor"] = first["cursor"].clone();
        let last = e.call("designos_lint", next).unwrap();
        assert_eq!(last["findings"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn mcp_lint_budgeted_chunk_pagination_preserves_all_findings_and_invalidates_on_doc_swap() {
        let mut e = Engine::default();
        for i in 1..=20 {
            create(&mut e, "frame", &format!("Frame {i}"));
        }

        // Full unpaged reference scan.
        let full = e.call("designos_lint", json!({"rules": ["default-layer-name"], "limit": 500})).unwrap();
        let full_findings = full["findings"].as_array().unwrap().clone();
        assert_eq!(full_findings.len(), 20);
        assert_eq!(full["complete"], true);
        assert!(full["cursor"].is_null());

        // Force incomplete (`stack:`) chunks (8 nodes per worker call) while paging with `limit = 3`
        // so each incomplete and final chunk produces more findings than `limit`.
        e.lint_call_max_nodes = 8;
        e.clear_lint_cache();

        let mut paged_findings = Vec::new();
        let mut total_scanned = 0u64;
        let mut cursor: Option<String> = None;
        let mut saw_incomplete_chunk = false;
        let mut saw_resumed_offset_cursor = false;
        let mut pages = 0usize;

        loop {
            let mut req = json!({"rules": ["default-layer-name"], "limit": 3});
            if let Some(ref c) = cursor {
                req["cursor"] = json!(c);
            }
            let res = e.call("designos_lint", req).unwrap();
            pages += 1;
            let batch = res["findings"].as_array().unwrap();
            assert!(!batch.is_empty() && batch.len() <= 3);
            paged_findings.extend(batch.iter().cloned());

            let scanned = res["scanned"].as_u64().unwrap();
            if cursor.as_deref().is_some_and(|c| c.contains("offset:")) {
                assert_eq!(scanned, 0, "offset page inside a chunk must serve from cache without rescanning");
            } else {
                assert!(scanned > 0, "initial or stack-resumed chunk page must scan nodes");
            }
            total_scanned += scanned;

            if res["complete"] == false {
                saw_incomplete_chunk = true;
            }
            match res["cursor"].as_str() {
                Some(next) => {
                    if next.contains("+offset:") {
                        saw_resumed_offset_cursor = true;
                    }
                    cursor = Some(next.to_string());
                }
                None => {
                    assert_eq!(res["complete"], true);
                    break;
                }
            }
            assert!(pages <= 20, "pagination loop did not terminate");
        }

        assert!(saw_incomplete_chunk, "test must exercise incomplete (`complete: false`) chunks");
        assert!(saw_resumed_offset_cursor, "test must exercise `stack:<id>+offset:<n>` pagination on a resumed chunk");
        assert_eq!(total_scanned, full["scanned"].as_u64().unwrap(), "all 20 scene nodes must be scanned once across chunks");
        assert_eq!(paged_findings, full_findings, "all pages concatenated across budgeted chunks must equal the full scan");

        // Replacing `e.doc` with a new Document that has the same node count and epoch must not hit stale cache.
        e.lint_call_max_nodes = 100_000;
        let _ = e.call("designos_lint", json!({"rules": ["default-layer-name"], "limit": 1})).unwrap();
        let mut replacement = Document::new("Replacement");
        let rep_page = replacement.add_page("Page 1");
        for i in 1..=20 {
            let fid = replacement.alloc_id();
            let mut n = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
            n.name = format!("SemanticCard{i}");
            replacement.insert(rep_page, n).unwrap();
        }
        assert_eq!(replacement.nodes.len(), e.doc.nodes.len());
        e.doc = replacement;
        let after_swap = e.call("designos_lint", json!({"rules": ["default-layer-name"], "limit": 1, "cursor": "offset:0"})).unwrap();
        assert_eq!(after_swap["findings"].as_array().unwrap().len(), 0, "new document has no default layer names; stale cache must not be served");
        assert!(after_swap["scanned"].as_u64().unwrap() > 0, "runtime_id mismatch must invalidate lint_cache and rescan");
    }

    fn setup_slot_component_and_instance(e: &mut Engine, with_default_mirror: bool) -> (NodeId, NodeId, NodeId) {
        let page = e.active_page_id.unwrap();
        let comp_id = e.doc.alloc_id();
        let mut comp = Node::new(comp_id, NodeKind::Component(designos_doc::FrameProps::default()));
        comp.name = "Card".into();
        comp.size = Vec2::new(200.0, 120.0);
        e.doc.insert(page, comp).unwrap();

        let slot_id = e.doc.alloc_id();
        let mut slot = Node::frame(slot_id, Rect::new(8.0, 8.0, 184.0, 104.0));
        slot.name = "Body".into();
        e.doc.insert(comp_id, slot).unwrap();

        if with_default_mirror {
            let def_id = e.doc.alloc_id();
            let mut def = Node::rectangle(def_id, Rect::new(0.0, 0.0, 40.0, 20.0));
            def.name = "DefaultItem".into();
            e.doc.insert(slot_id, def).unwrap();
        }

        designos_doc::convert_to_slot(&mut e.doc, slot_id).apply(&mut e.doc).unwrap();
        designos_doc::create_instance(&mut e.doc, comp_id, page, Vec2::new(240.0, 0.0)).apply(&mut e.doc).unwrap();
        let inst_id = designos_doc::instances_of(&e.doc, comp_id)[0];
        let inst_slot = e.doc.get(inst_id).unwrap().children[0];
        assert!(designos_doc::is_instance_slot(&e.doc, inst_slot));
        assert!(!e.doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent));
        (comp_id, inst_id, inst_slot)
    }

    #[test]
    fn apply_spec_refuses_an_instance_that_would_make_a_main_hold_itself() {
        let mut e = Engine::default();
        let page = e.active_page_id.unwrap();
        let main = |e: &mut Engine, name: &str, x: f32| {
            let id = e.doc.alloc_id();
            let mut frame = Node::frame(id, Rect::new(x, 0.0, 100.0, 100.0));
            frame.name = name.into();
            e.doc.insert(page, frame).unwrap();
            designos_doc::create_component(&mut e.doc, id).apply(&mut e.doc).unwrap();
            id
        };
        let (b, a) = (main(&mut e, "B", 0.0), main(&mut e, "A", 300.0));
        // A holds an instance of B, so an instance of A inside B would make B hold itself
        designos_doc::create_instance(&mut e.doc, b, a, Vec2::ZERO).apply(&mut e.doc).unwrap();
        let nodes = e.doc.nodes.len();
        let res = e.call("designos_apply_spec", json!({"parent_id": b.0, "spec": {"kind": "instance", "component": "A"}}));
        assert!(res.is_err(), "an error, not an instance of A in B: {res:?}");
        assert_eq!(e.doc.nodes.len(), nodes, "nothing was inserted");
        // an instance of A on the page is fine
        e.call("designos_apply_spec", json!({"parent_id": page.0, "spec": {"kind": "instance", "component": "A"}})).unwrap();
    }

    #[test]
    fn mcp_insert_into_instance_slot_survives_main_edit() {
        let mut e = Engine::default();
        let (comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, false);

        let res = e
            .call(
                "designos_create_node",
                json!({
                    "parent_id": inst_slot.0,
                    "kind": "rectangle",
                    "name": "CustomBadge",
                    "width": 60.0,
                    "height": 24.0
                }),
            )
            .unwrap();
        let created_id = NodeId(res["node"]["id"].as_u64().unwrap());

        assert!(
            e.doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent),
            "designos_create_node into an instance slot must mark OverrideField::SlotContent"
        );

        // Edit the main component and sync instances: the custom child inside the instance slot must survive.
        e.doc.get_mut(comp_id).unwrap().size.x = 240.0;
        designos_doc::sync_instances(&mut e.doc, comp_id).apply(&mut e.doc).unwrap();

        assert_eq!(e.doc.get(inst_slot).unwrap().children, vec![created_id]);
        assert!(e.doc.get(created_id).is_some());
    }

    #[test]
    fn mcp_apply_spec_into_instance_slot_survives() {
        let mut e = Engine::default();
        let (comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, false);

        e.call(
            "designos_apply_spec",
            json!({
                "parent_id": inst_slot.0,
                "spec": {
                    "name": "SpecCard",
                    "width": 120.0,
                    "height": 48.0,
                    "children": [
                        { "kind": "text", "name": "SpecTitle", "text": "Hello Slot" }
                    ]
                }
            }),
        )
        .unwrap();

        assert!(
            e.doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent),
            "designos_apply_spec into an instance slot must mark OverrideField::SlotContent"
        );
        let slot_children = e.doc.get(inst_slot).unwrap().children.clone();
        assert_eq!(slot_children.len(), 1);

        e.doc.get_mut(comp_id).unwrap().size.y = 160.0;
        designos_doc::sync_instances(&mut e.doc, comp_id).apply(&mut e.doc).unwrap();

        assert_eq!(e.doc.get(inst_slot).unwrap().children, slot_children);
        assert_eq!(e.doc.get(slot_children[0]).unwrap().name, "SpecCard");
    }

    #[test]
    fn mcp_delete_default_mirror_is_kept_deleted() {
        let mut e = Engine::default();
        let (comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, true);
        let mirror_id = e.doc.get(inst_slot).unwrap().children[0];

        e.call(
            "designos_delete_node",
            json!({
                "node_id": mirror_id.0
            }),
        )
        .unwrap();

        assert!(e.doc.get(inst_slot).unwrap().children.is_empty());
        assert!(
            e.doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent),
            "deleting a default mirror child from an instance slot must mark OverrideField::SlotContent"
        );

        // Edit the main component and sync instances: the deleted default mirror must NOT be re-created.
        e.doc.get_mut(comp_id).unwrap().size.x = 260.0;
        designos_doc::sync_instances(&mut e.doc, comp_id).apply(&mut e.doc).unwrap();

        assert!(e.doc.get(inst_slot).unwrap().children.is_empty(), "synced instance slot must stay empty after default mirror was deleted");
    }

    #[test]
    fn recording_a_marked_insert_does_not_clone_the_document() {
        use designos_doc::live_patch::batch_doc_clones;

        let mut e = Engine::default();
        let (_comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, false);

        let mut editor_doc = e.doc.clone();
        e.live_ops = Some(Vec::new());
        let before = batch_doc_clones();
        let child_id = e.doc.alloc_id();
        let child = Node::rectangle(child_id, Rect::new(0.0, 0.0, 32.0, 16.0));
        e.exec(Command::Insert { parent: inst_slot, index: 0, nodes: vec![child] }).unwrap();
        assert_eq!(batch_doc_clones(), before, "recording a slot-marked insert must not clone the document");

        let ops = e.take_live_ops();
        assert_eq!(ops.len(), 2, "must emit Update for the marked slot and Insert for the child");
        assert!(e.doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent));

        // Simulate the user recoloring the slot in the editor (`OverrideField::Fills`) before replaying the agent patch.
        editor_doc.get_mut(inst_slot).unwrap().component.overrides.insert(designos_doc::OverrideField::Fills);
        editor_doc.get_mut(inst_slot).unwrap().component.description = "User slot note".into();

        let report = designos_doc::live_patch::replay_live_patch(
            &mut editor_doc,
            &designos_doc::live_patch::LivePatch::new(ops),
            &mut designos_doc::live_patch::LivePatchState::default(),
            &mut |d, c| c.apply(d).map(|_| ()),
        )
        .unwrap();
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        let editor_slot = editor_doc.get(inst_slot).unwrap();
        assert!(editor_slot.component.overrides.contains(&designos_doc::OverrideField::SlotContent), "replayed slot must carry OverrideField::SlotContent");
        assert!(
            editor_slot.component.overrides.contains(&designos_doc::OverrideField::Fills),
            "replaying the synthetic slot-mark Update must not wipe out OverrideField::Fills set in the editor"
        );
        assert_eq!(editor_slot.component.description, "User slot note");
    }

    #[test]
    fn undoing_a_marked_slot_insert_restores_slot_overrides_without_wiping_editor_overrides() {
        // Case A: Slot already has `Size` on both sides. Agent inserts via `designos_apply_spec`, then `designos_undo`.
        // Replaying undo must remove `SlotContent` while keeping `Size`, so main-component sync resumes.
        {
            let mut e = Engine::default();
            let (comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, true);
            e.doc.get_mut(inst_slot).unwrap().component.overrides.insert(designos_doc::OverrideField::Size);

            let mut editor_doc = e.doc.clone();
            let mut state = designos_doc::live_patch::LivePatchState::default();
            e.live_ops = Some(Vec::new());

            e.call(
                "designos_apply_spec",
                json!({
                    "parent_id": inst_slot.0,
                    "spec": { "kind": "rectangle", "name": "Inserted", "width": 20.0, "height": 20.0 }
                }),
            )
            .unwrap();
            let insert_ops = e.take_live_ops();
            designos_doc::live_patch::replay_live_patch(&mut editor_doc, &designos_doc::live_patch::LivePatch::new(insert_ops), &mut state, &mut |d, c| {
                c.apply(d).map(|_| ())
            })
            .unwrap();
            assert!(editor_doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent));
            assert!(editor_doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::Size));

            e.call("designos_undo", json!({})).unwrap();
            let undo_ops = e.take_live_ops();
            designos_doc::live_patch::replay_live_patch(&mut editor_doc, &designos_doc::live_patch::LivePatch::new(undo_ops), &mut state, &mut |d, c| {
                c.apply(d).map(|_| ())
            })
            .unwrap();

            let editor_ovs = &editor_doc.get(inst_slot).unwrap().component.overrides;
            assert!(
                !editor_ovs.contains(&designos_doc::OverrideField::SlotContent),
                "Case A: undoing marked insert must remove SlotContent so sync resumes, got {editor_ovs:?}"
            );
            assert!(editor_ovs.contains(&designos_doc::OverrideField::Size), "Case A: existing Size override must remain");
            let _ = comp_id;
        }

        // Case B: Agent starts with empty overrides on the slot, while the editor has `Fills` (an editor-only override).
        // Agent inserts into the slot, then calls `designos_undo`. Replaying undo must remove `SlotContent` and keep `Fills`.
        {
            let mut e = Engine::default();
            let (_comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, false);

            let mut editor_doc = e.doc.clone();
            editor_doc.get_mut(inst_slot).unwrap().component.overrides.insert(designos_doc::OverrideField::Fills);
            let mut state = designos_doc::live_patch::LivePatchState::default();
            e.live_ops = Some(Vec::new());

            e.call(
                "designos_create_node",
                json!({
                    "parent_id": inst_slot.0,
                    "kind": "rectangle",
                    "name": "TempChild",
                    "width": 24.0,
                    "height": 24.0
                }),
            )
            .unwrap();
            let insert_ops = e.take_live_ops();
            designos_doc::live_patch::replay_live_patch(&mut editor_doc, &designos_doc::live_patch::LivePatch::new(insert_ops), &mut state, &mut |d, c| {
                c.apply(d).map(|_| ())
            })
            .unwrap();
            assert!(editor_doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent));
            assert!(editor_doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::Fills));

            e.call("designos_undo", json!({})).unwrap();
            let undo_ops = e.take_live_ops();
            designos_doc::live_patch::replay_live_patch(&mut editor_doc, &designos_doc::live_patch::LivePatch::new(undo_ops), &mut state, &mut |d, c| {
                c.apply(d).map(|_| ())
            })
            .unwrap();

            let editor_ovs = &editor_doc.get(inst_slot).unwrap().component.overrides;
            assert!(
                !editor_ovs.contains(&designos_doc::OverrideField::SlotContent),
                "Case B: undoing marked insert must remove SlotContent, got {editor_ovs:?}"
            );
            assert!(
                editor_ovs.contains(&designos_doc::OverrideField::Fills),
                "Case B: undoing marked insert must not wipe editor-only Fills override, got {editor_ovs:?}"
            );
        }
    }

    #[test]
    fn apply_spec_library_instance_into_slot_replays_imported_main_component_in_live_patch() {
        let dir = std::env::temp_dir().join(format!("designos_pr721_lib_live_patch_{}", designos_library::new_id()));
        let lib = designos_library::Library::open(&dir).unwrap();
        let lib_doc = designos_doc::fixtures::button_library();
        lib.publish_library("lib-buttons", &lib_doc, "v1").unwrap();
        crate::tools::library_source::set_test_library_root(Some(dir.clone()));

        let mut e = Engine::default();
        let (_comp_id, _inst_id, inst_slot) = setup_slot_component_and_instance(&mut e, false);
        e.doc.library.enabled.push(designos_doc::LibraryRef { id: "lib-buttons".into(), name: "Button Kit".into(), version: 1 });

        let mut editor_doc = e.doc.clone();
        e.live_ops = Some(Vec::new());

        let res = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": inst_slot.0,
                    "spec": {
                        "kind": "instance",
                        "name": "Slot Arrow Icon",
                        "component": "Icon/Arrow"
                    }
                }),
            )
            .unwrap();
        crate::tools::library_source::set_test_library_root(None);
        let _ = std::fs::remove_dir_all(&dir);

        let root_id = NodeId(res["root_node_id"].as_u64().unwrap());
        let ops = e.take_live_ops();
        let report = designos_doc::live_patch::replay_live_patch(
            &mut editor_doc,
            &designos_doc::live_patch::LivePatch::new(ops),
            &mut designos_doc::live_patch::LivePatchState::default(),
            &mut |d, c| c.apply(d).map(|_| ()),
        )
        .unwrap();
        assert!(report.skipped.is_empty(), "live patch replay must not skip any op: {:?}", report.skipped);
        assert!(
            editor_doc.get(inst_slot).unwrap().component.overrides.contains(&designos_doc::OverrideField::SlotContent),
            "replayed instance slot must be marked with OverrideField::SlotContent"
        );
        let replayed_inst = editor_doc.get(root_id).expect("placed library instance must exist in replayed document");
        let main_comp_id = designos_doc::instance_component(replayed_inst).expect("placed library instance must reference its main component");
        let replayed_main = editor_doc.get(main_comp_id).expect("imported library main component must exist in replayed document");
        assert_eq!(replayed_main.name, "Icon/Arrow");
        assert_eq!(replayed_main.parent, editor_doc.library_page);
    }
}
