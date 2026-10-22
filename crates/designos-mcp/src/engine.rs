//! Typed, local document operations shared by the MCP transport and future bridges.
use designos_doc::live_patch::{live_ops_for_command, live_ops_for_independent, LiveOp};
use designos_doc::{Annotation, Command, Comment, CommentAnchor, CommentThread, DevStatus, Document, History, Node, NodeId, NodeKind, NodeSpec, Vec2};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

pub type LintRulesMap = std::collections::HashMap<NodeId, std::collections::BTreeSet<&'static str>>;

pub struct Engine {
    pub doc: Document,
    pub history: History,
    pub active_page_id: Option<NodeId>,
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
    /// Active RD-3 agent `run_id` (`DESIGNOS_AGENT_RUN_ID`) when running as a Studio agent.
    pub run_id: Option<String>,
    /// Active RD-3 frame lease table (`leases.json`).
    pub leases: designos_doc::live_patch::LeaseTable,
}

impl Default for Engine {
    fn default() -> Self {
        let mut doc = Document::new("Untitled");
        let page = doc.add_page("Page 1");
        let run_id = std::env::var("DESIGNOS_AGENT_RUN_ID").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Self {
            doc,
            history: History::default(),
            active_page_id: Some(page),
            live_ops: None,
            live_patch_label: None,
            owned_frames: None,
            lint_cache: None,
            lint_rules_cache: None,
            run_id,
            leases: designos_doc::live_patch::LeaseTable::default(),
        }
    }
}

impl Engine {
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

    /// Returns the active agent `run_id` (`self.run_id` or `DESIGNOS_AGENT_RUN_ID`).
    pub fn effective_run_id(&self) -> Option<String> {
        self.run_id.clone().or_else(|| std::env::var("DESIGNOS_AGENT_RUN_ID").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    }
    /// Resolves the target page when a tool call omits `parent_id` / `page_id`:
    /// prefers `self.active_page_id` (the user's currently selected page in `design:os studio`)
    /// if valid in `self.doc.pages`, falling back to `self.doc.pages.first()`.
    pub fn resolve_default_page(&self) -> Result<NodeId, String> {
        self.active_page_id
            .filter(|p| self.doc.pages.contains(p))
            .or_else(|| self.doc.pages.first().copied())
            .ok_or_else(|| "Document has no pages".to_string())
    }

    pub fn call(&mut self, name: &str, args: Value) -> Result<Value, String> {
        // Nothing here drains the lazy layout queue between calls, so no tool may start from, read,
        // or serialize a document with deferred layout still in it.
        self.settle_layout();
        if matches!(
            name,
            "designos_create_node"
                | "designos_update_node"
                | "designos_delete_node"
                | "designos_apply_spec"
                | "designos_add_comment"
                | "designos_reply_comment"
                | "designos_resolve_comment"
                | "designos_set_dev_status"
                | "designos_undo"
                | "designos_redo"
        ) {
            let caller = self.effective_run_id();
            let eff_table = crate::tools::lease::effective_lease_table(&self.leases, self.owned_frames.as_deref(), caller.as_deref());
            crate::tools::lease::check_tool_write_guard(
                &self.doc,
                &self.history,
                self.resolve_default_page().ok(),
                &eff_table,
                caller.as_deref(),
                name,
                &args,
            )?;
        }
        match name {
            "designos_get_document" => {
                let _: Empty = parse(args)?;
                Ok(json!({"name":self.doc.name,"kind":self.doc.kind,
                    "active_page_id":self.resolve_default_page().ok().map(|p| p.0),
                    "pages":self.doc.pages.iter().filter_map(|id|self.doc.get(*id)).map(summary).collect::<Vec<_>>(),
                    "node_count":self.doc.nodes.len(),"variable_count":self.doc.variables.len(),
                    "comment_count":self.doc.comments.len(),
                    "style_count":self.doc.styles.len(),
                    "styles":self.doc.styles.iter().map(|(key,values)|{
                        let meta=self.doc.style_meta.get(key);
                        json!({"key":key,"name":self.doc.style_name(key),"property":designos_doc::style_property(&self.doc,key),
                            "description":meta.map(|m|m.description.as_str()).unwrap_or(""),"library_key":meta.and_then(|m|m.key.as_deref()),"values":values})
                    }).collect::<Vec<_>>(),
                    "can_undo":self.history.can_undo(),"can_redo":self.history.can_redo()}))
            }
            "designos_get_libraries" => {
                let _: Empty = parse(args)?;
                Ok(crate::tools::libraries::get_libraries(&self.doc))
            }
            "designos_list_nodes" => {
                let a: ListArgs = parse(args)?;
                if a.limit == 0 || a.limit > 500 {
                    return Err("limit must be between 1 and 500".into());
                }
                if let Some(id) = a.parent_id {
                    self.node(id)?;
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
                let nodes: Vec<_> = matching.iter().skip(a.offset).take(a.limit).map(|n| summary(n)).collect();
                let next = a.offset.saturating_add(nodes.len());
                Ok(json!({"nodes":nodes,"total":matching.len(),"offset":a.offset,
                    "next_offset":(next < matching.len()).then_some(next)}))
            }
            "designos_get_node" => {
                let a: NodeArgs = parse(args)?;
                Ok(json!({"node":self.node(a.node_id)?}))
            }
            "designos_get_screenshot" => crate::tools::screenshot::execute_get_screenshot(&self.doc, args),
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
                    let allowed: Vec<&str> = designos_export::CodeLanguage::ALL.iter().map(|l| l.id()).chain(std::iter::once("svg")).collect();
                    return Err(format!("language must be one of: {}", allowed.join(", ")));
                };
                Ok(json!({"node_id":id,"language":a.language,"code":code}))
            }
            "designos_create_node" => self.create(parse(args)?),
            "designos_update_node" => self.update(parse(args)?),
            "designos_delete_node" => {
                let a: DeleteArgs = parse(args)?;
                let n = self.node(a.node_id)?;
                if n.parent.is_none() {
                    return Err("page roots cannot be deleted by this tool".into());
                }
                let id = n.id;
                let before_doc = a.diff_image.then(|| self.doc.clone());
                self.exec(Command::Remove { id })?;
                let mut out = json!({"node_id":id,"deleted":true});
                if let Some(before) = &before_doc {
                    crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[id], &mut out);
                }
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
                if a.text.trim().is_empty() {
                    return Err("text must not be empty".into());
                }
                bounded_text("text", &a.text, 10_000)?;
                if let Some(author) = &a.author {
                    bounded_text("author", author, 256)?;
                }
                if !a.x.is_finite() || !a.y.is_finite() {
                    return Err("x and y must be finite".into());
                }
                let page = if let Some(nid) = a.node_id {
                    self.node(nid)?;
                    self.doc.page_of(NodeId(nid)).ok_or_else(|| format!("node {nid} does not belong to a page"))?
                } else if let Some(pid) = a.page_id {
                    let p = NodeId(pid);
                    if !self.doc.pages.contains(&p) {
                        return Err(format!("page {pid} not found"));
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
                let resolved = self.doc.resolve_comment_targets().into_iter().find(|t| t.id == thread_id).ok_or("Failed to resolve comment thread")?;
                let mut out = serde_json::to_value(&resolved).map_err(|e| e.to_string())?;
                if let Some(obj) = out.as_object_mut() {
                    obj.insert("thread".into(), serde_json::to_value(&resolved).map_err(|e| e.to_string())?);
                    obj.insert("changed".into(), json!(true));
                }
                Ok(out)
            }
            "designos_reply_comment" => {
                let a: ReplyCommentArgs = parse(args)?;
                if a.text.trim().is_empty() {
                    return Err("text must not be empty".into());
                }
                bounded_text("text", &a.text, 10_000)?;
                if let Some(author) = &a.author {
                    bounded_text("author", author, 256)?;
                }
                let mut comments = self.doc.comments.clone();
                let t = comments.iter_mut().find(|t| t.id == a.thread_id).ok_or_else(|| format!("comment thread '{}' not found", a.thread_id))?;
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
                let resolved_thread = self.doc.resolve_comment_targets().into_iter().find(|t| t.id == a.thread_id).ok_or("Failed to resolve comment thread")?;
                Ok(json!({"thread": resolved_thread, "changed": true}))
            }
            "designos_resolve_comment" => {
                let a: ResolveCommentArgs = parse(args)?;
                let mut comments = self.doc.comments.clone();
                let t = comments.iter_mut().find(|t| t.id == a.thread_id).ok_or_else(|| format!("comment thread '{}' not found", a.thread_id))?;
                let changed = t.resolved != a.resolved;
                t.resolved = a.resolved;
                if changed {
                    self.exec(Command::SetComments { comments })?;
                }
                Ok(json!({"thread_id": a.thread_id, "resolved": a.resolved, "changed": changed}))
            }
            "designos_apply_spec" => {
                if let Some(raw_spec) = args.get("spec") {
                    designos_doc::validate_raw_spec_slots(raw_spec).map_err(|e| e.to_string())?;
                }
                let a: ApplySpecArgs = parse(args)?;
                let parent = a
                    .parent_id
                    .filter(|&id| id != 0)
                    .map(NodeId)
                    .or_else(|| self.resolve_default_page().ok())
                    .ok_or_else(|| "Document has no pages".to_string())?;
                if !self.node(parent.0)?.kind.is_container() {
                    return Err("parent_id must identify a container".into());
                }
                let before_doc = a.diff_image.then(|| self.doc.clone());
                let snapshots = crate::tools::library_source::load_enabled_snapshots(&self.doc);
                let (cmd, report) = designos_doc::apply_spec_command(&mut self.doc, parent, &a.spec, &snapshots).map_err(|e| e.to_string())?;
                self.exec(cmd)?;
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
                    "issues": report.issues,
                });
                if !report.stale_library_assets.is_empty() {
                    resp["stale_library_assets"] = json!(report.stale_library_assets);
                }
                if let Some(before) = &before_doc {
                    crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[root_id], &mut resp);
                }
                Ok(resp)
            }
            "designos_export_spec" => {
                let a: NodeArgs = parse(args)?;
                let spec = self.doc.export_spec(NodeId(a.node_id))?;
                Ok(json!({"node_id": a.node_id, "spec": spec}))
            }
            "designos_set_dev_status" => {
                let a: SetDevStatusArgs = parse(args)?;
                let mut node = self.node(a.node_id)?.clone();
                if let Some(status) = &a.status {
                    node.dev.status = match status.trim().to_ascii_lowercase().as_str() {
                        "ready" | "ready_for_dev" | "readyfordev" => Some(DevStatus::ReadyForDev),
                        "completed" | "done" => Some(DevStatus::Completed),
                        "clear" | "none" | "" => None,
                        other => return Err(format!("status must be 'ready', 'completed', or 'clear' (got '{other}')")),
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
            "designos_export_fig_file" | "export_fig_file" => {
                let a: ExportFigArgs = parse(args)?;
                let path = std::path::Path::new(&a.output_path);
                if path.extension().and_then(|x| x.to_str()) != Some("fig") {
                    return Err("output_path must end in .fig".into());
                }
                let bytes = designos_fig::export_fig(&self.doc).map_err(|e| format!("Failed to export .fig: {e}"))?;
                std::fs::write(path, &bytes).map_err(|e| format!("Cannot write .fig file: {e}"))?;
                Ok(json!({"output_path": a.output_path, "path": a.output_path, "bytes": bytes.len(), "saved": true}))
            }
            "designos_query_xpath" | "query_nodes" => {
                let a: QueryXpathArgs = parse(args)?;
                let root = a.page_id.map(NodeId).or_else(|| self.resolve_default_page().ok());
                let selector = a.selector.trim();
                let mut matches = Vec::new();
                let mut visit = |n: &Node| {
                    let kind_upper = n.kind.default_name().to_ascii_uppercase();
                    let matched = if selector.is_empty() || selector == "//*" || selector == "*" {
                        true
                    } else if let Some(tag) = selector.strip_prefix("//") {
                        if let Some((tag_part, pred)) = tag.split_once('[') {
                            let tag_ok = tag_part.is_empty() || tag_part == "*" || kind_upper == tag_part.to_ascii_uppercase();
                            let pred_clean = pred.trim_end_matches(']');
                            let name_ok = if let Some(val) = pred_clean.strip_prefix("@name=") {
                                let unquoted = val.trim().trim_matches('\'').trim_matches('"');
                                n.name == unquoted
                            } else if pred_clean.contains("contains(@name") {
                                let parts: Vec<&str> = pred_clean.split(['\'', '"']).collect();
                                parts.get(1).is_some_and(|needle| n.name.to_lowercase().contains(&needle.to_lowercase()))
                            } else {
                                true
                            };
                            tag_ok && name_ok
                        } else {
                            kind_upper == tag.to_ascii_uppercase() || n.name.to_lowercase().contains(&tag.to_lowercase())
                        }
                    } else {
                        n.name.to_lowercase().contains(&selector.to_lowercase()) || kind_upper == selector.to_ascii_uppercase()
                    };
                    if matched {
                        matches.push(summary(n));
                    }
                };
                if let Some(r) = root {
                    self.doc.walk(r, |n, _| visit(n));
                } else {
                    for n in self.doc.nodes.values() {
                        visit(n);
                    }
                }
                let total = matches.len();
                Ok(json!({"selector": a.selector, "page_id": root.map(|r| r.0), "nodes": matches, "total": total}))
            }
            "designos_get_metadata" | "get_metadata" => crate::tools::metadata::call_get_metadata(self, args),
            "designos_lint" | "check_design" => {
                let a: LintArgs = parse(args)?;
                let root = a.page_id.map(NodeId).or_else(|| self.resolve_default_page().ok());
                let mut findings = Vec::new();
                let mut checked_nodes = Vec::new();
                let mut fresh_hits: std::collections::HashMap<NodeId, std::collections::BTreeSet<&'static str>> = std::collections::HashMap::new();
                let mut check_node = |n: &Node| {
                    if matches!(n.kind, NodeKind::Page { .. }) {
                        return;
                    }
                    checked_nodes.push(n.id);
                    if (n.size.x <= 0.0 || n.size.y <= 0.0) && !designos_export::is_lint_rule_ignored(n, "zero-bounds") {
                        findings.push(json!({"rule": "zero-bounds", "severity": "error", "node_id": n.id.0, "name": n.name, "message": "Layer has zero or negative width/height"}));
                        fresh_hits.entry(n.id).or_default().insert("zero-bounds");
                    }
                    let trimmed = n.name.trim();
                    if matches!(trimmed, "Frame" | "Rectangle" | "Ellipse" | "Text" | "Group")
                        && !designos_export::is_lint_rule_ignored(n, "default-layer-name")
                    {
                        findings.push(json!({"rule": "default-layer-name", "severity": "warning", "node_id": n.id.0, "name": n.name, "message": "Give layer a semantic name"}));
                        fresh_hits.entry(n.id).or_default().insert("default-layer-name");
                    }
                };
                if let Some(r) = root {
                    self.doc.walk(r, |n, _| check_node(n));
                } else {
                    for n in self.doc.nodes.values() {
                        check_node(n);
                    }
                }
                let (structural, structural_checked) =
                    designos_export::structural_lint_rules_for_nodes(&self.doc, &checked_nodes, std::time::Duration::from_millis(50));
                let mut structural_hits: std::collections::HashMap<NodeId, std::collections::BTreeSet<&'static str>> = std::collections::HashMap::new();
                for (id, rules) in structural {
                    if rules.contains("clipped-text") {
                        if let Some(n) = self.doc.get(id) {
                            findings.push(json!({"rule": "clipped-text", "severity": "warning", "node_id": n.id.0, "name": n.name, "message": "Fixed-size text layer clips its content"}));
                        }
                        structural_hits.entry(id).or_default().insert("clipped-text");
                    }
                }
                let rev = (self.doc.runtime_id(), self.doc.mutation_epoch());
                self.update_lint_cache(rev, &checked_nodes, &["zero-bounds", "default-layer-name"], fresh_hits);
                self.update_lint_cache(rev, &structural_checked, &["clipped-text"], structural_hits);
                let count = findings.len();
                Ok(json!({"page_id": root.map(|r| r.0), "findings": findings, "total": count, "passed": count == 0}))
            }
            "designos_get_guidelines" | "get_guidelines" => crate::tools::guidelines::call_get_guidelines(&self.doc, args),
            "designos_apply_batch" => crate::tools::batch::apply(self, args),
            "designos_undo" | "designos_redo" => {
                let _: Empty = parse(args)?;
                let undo = name == "designos_undo";
                let next = if undo { self.history.peek_undo() } else { self.history.peek_redo() }.cloned();
                let ops = match &next {
                    Some(command) => self.live_ops_for(command)?,
                    None => Vec::new(),
                };
                let changed = if undo { self.history.undo(&mut self.doc) } else { self.history.redo(&mut self.doc) }.map_err(|e| e.to_string())?;
                // Undo and redo defer a deep layout tail to later frames; this engine has no frames.
                self.settle_layout();
                self.record(ops);
                Ok(json!({"changed":changed,"can_undo":self.history.can_undo(),"can_redo":self.history.can_redo()}))
            }
            _ => Err(format!("unknown tool: {name}")),
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

    pub(crate) fn node(&self, id: u64) -> Result<&Node, String> {
        if id == 0 {
            return Err("node IDs must be positive integers".into());
        }
        self.doc.get(NodeId(id)).ok_or_else(|| format!("node {id} not found"))
    }

    pub fn exec(&mut self, mut command: Command) -> Result<(), String> {
        let rid = self.effective_run_id();
        if let Some(r) = rid.as_deref() {
            designos_doc::live_patch::stamp_command_authorship(&mut command, r);
        }
        let w = designos_doc::writer_command(&self.doc, command);
        let ops = self.live_ops_for_parts(&w.parts)?;
        self.history.exec_for_run(&mut self.doc, w.cmd, rid, self.live_patch_label.clone()).map_err(|e| e.to_string())?;
        self.record(ops);
        Ok(())
    }

    /// Reverts all undo steps belonging to `run_id` in reverse order and records the live ops.
    pub fn undo_run_and_record(&mut self, run_id: &str) -> Result<usize, String> {
        let mut cmds_in_rev_order = Vec::new();
        let steps = self.history.undo_steps();
        let metas = self.history.undo_metas();
        for idx in (0..steps.len()).rev() {
            if metas.get(idx).and_then(|m| m.run_id.as_deref()) == Some(run_id) {
                cmds_in_rev_order.push(steps[idx].clone());
            }
        }
        if cmds_in_rev_order.is_empty() {
            return Ok(0);
        }
        let batch_cmd = Command::Batch(cmds_in_rev_order);
        let ops = self.live_ops_for(&batch_cmd)?;
        let count = self.history.undo_run(&mut self.doc, run_id).map_err(|e| e.to_string())?;
        self.settle_layout();
        self.record(ops);
        Ok(count)
    }

    /// The ops `command` will perform, when recording; refuses edits the editor cannot replay.
    pub(crate) fn live_ops_for(&self, command: &Command) -> Result<Vec<LiveOp>, String> {
        if self.live_ops.is_none() {
            return Ok(Vec::new());
        }
        if let Command::Batch(cmds) = command {
            if designos_doc::live_patch::is_independent_batch(&self.doc, cmds) {
                return live_ops_for_independent(&self.doc, cmds);
            }
        }
        live_ops_for_command(&self.doc, command)
    }

    /// Records independent command parts (from [`designos_doc::writer_command`]) without cloning
    /// the document when `parts` came from a boundary-expanded batch.
    fn live_ops_for_parts(&self, parts: &[Command]) -> Result<Vec<LiveOp>, String> {
        if self.live_ops.is_none() {
            return Ok(Vec::new());
        }
        live_ops_for_independent(&self.doc, parts)
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

    fn create(&mut self, a: CreateArgs) -> Result<Value, String> {
        let parent_id = match a.parent_id {
            Some(id) => {
                self.node(id)?;
                NodeId(id)
            }
            None => self.resolve_default_page()?,
        };
        if let Some(fill_str) = &a.fill {
            designos_doc::resolve_easeui_color(fill_str).ok_or_else(|| format!("invalid fill color '{fill_str}'"))?;
        }
        let before_doc = a.diff_image.then(|| self.doc.clone());
        let (id, index, mut node) = crate::tools::build_created_node(&mut self.doc, parent_id, None, &a.kind, a.name, a.text, a.x, a.y, a.width, a.height)?;
        if let Some(fills) = a.fills {
            node.fills = fills;
        } else if let Some(fill_str) = &a.fill {
            if let Some(color) = designos_doc::resolve_easeui_color(fill_str) {
                node.fills = vec![designos_doc::Paint::solid(color)];
            }
        }
        self.exec(Command::Insert { parent: parent_id, index, nodes: vec![node] })?;
        if let Some(page_id) = self.doc.page_of(id) {
            self.relayout(page_id);
        }
        let mut out = json!({"node":self.doc.get(id),"parent_id":parent_id.0,"changed":true});
        if let Some(before) = &before_doc {
            crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[id], &mut out);
        }
        Ok(out)
    }

    fn update(&mut self, a: UpdateArgs) -> Result<Value, String> {
        let (node, changed) = crate::tools::build_updated_node(
            &self.doc,
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
        if changed {
            self.exec(Command::Update { node: Box::new(node) })?;
            if let Some(page_id) = self.doc.page_of(id) {
                self.relayout(page_id);
            }
        }
        let mut out = json!({"node":self.doc.get(NodeId(a.node_id)),"changed":changed});
        if let Some(before) = &before_doc {
            crate::tools::screenshot::attach_diff_to_reply(before, &self.doc, &[id], &mut out);
        }
        Ok(out)
    }
}

fn summary(n: &Node) -> Value {
    json!({"node_id":n.id,"name":n.name,"kind":n.kind.default_name(),"parent_id":n.parent,
        "child_count":n.children.len(),"visible":n.visible,"x":n.transform.tx,"y":n.transform.ty,
        "width":n.size.x,"height":n.size.y})
}

pub(crate) fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}
use crate::tools::bounded_text;
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
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyCommentArgs {
    thread_id: String,
    text: String,
    author: Option<String>,
    resolved: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveCommentArgs {
    thread_id: String,
    #[serde(default = "default_true")]
    resolved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplySpecArgs {
    parent_id: Option<u64>,
    spec: NodeSpec,
    #[serde(default)]
    diff_image: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetDevStatusArgs {
    node_id: u64,
    status: Option<String>,
    annotation: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportFigArgs {
    #[serde(alias = "path")]
    pub output_path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryXpathArgs {
    #[serde(default, alias = "query")]
    selector: String,
    #[serde(default)]
    page_id: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LintArgs {
    #[serde(default)]
    page_id: Option<u64>,
}

/// JSON tool descriptors kept beside the argument validators.
pub fn tools() -> Vec<Value> {
    let id = json!({"type":"integer","minimum":1,"description":"DesignOS numeric node ID; obtain from list_nodes/get_document."});
    let coordinate = json!({"type":"number","minimum":-1000000,"maximum":1000000});
    let size = json!({"type":"number","minimum":0,"maximum":1000000});
    let text = json!({"type":"string","maxLength":100000,"description":"Plain text; limited to 100000 UTF-8 bytes."});
    let name = json!({"type":"string","maxLength":1024,"description":"Layer name; limited to 1024 UTF-8 bytes."});
    let mut out = Vec::new();
    let mut add = |tool: &str, description: &str, properties: Value, required: Vec<&str>, read_only: bool, idempotent: bool| {
        out.push(json!({"name":format!("designos_{tool}"),"description":description,
            "inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
            "annotations":{"readOnlyHint":read_only,"destructiveHint":!read_only,"idempotentHint":idempotent,"openWorldHint":false}}));
    };
    add(
        "get_document",
        "Summarize the current in-memory document: pages, counts, shared styles (fill, text, effect, grid) with their values, and undo state.",
        json!({}),
        vec![],
        true,
        true,
    );
    add(
        "get_libraries",
        "List this file's local publishable assets, enabled libraries (with pinned and latest versions and update_available), and disabled libraries in the active library root.",
        json!({}),
        vec![],
        true,
        true,
    );
    add(
        "get_metadata",
        "Return a sparse XML node hierarchy (IDs, types, names, page-space bounds, layout, component/variant props, text previews, and lint issue counts) for a page or subtree.",
        crate::tools::metadata::properties_schema(),
        vec![],
        true,
        true,
    );
    add(
        "list_nodes",
        "List node summaries in numeric ID order. Query matches names case-insensitively; parent_id filters direct children. Use next_offset to paginate.",
        json!({"parent_id":id,"query":{"type":"string","maxLength":1024},"offset":{"type":"integer","minimum":0,"default":0},"limit":{"type":"integer","minimum":1,"maximum":500,"default":100}}),
        vec![],
        true,
        true,
    );
    add("get_node", "Read the full native node including geometry, paints, text, bindings, and child IDs.", json!({"node_id":id}), vec!["node_id"], true, true);
    add(
        "get_variables",
        "Read variable definitions and collections. With node_id, also resolve that node's direct bindings using inherited collection modes.",
        json!({"node_id":id}),
        vec![],
        true,
        true,
    );
    let code_langs: Vec<&str> = designos_export::CodeLanguage::ALL.iter().map(|l| l.id()).chain(std::iter::once("svg")).collect();
    add(
        "generate_code",
        "Generate developer handoff code or SVG for a layer using resolved variable values. Returns source text; writes no files.",
        json!({"node_id":id,"language":{"type":"string","enum":code_langs},"fragment":{"type":"boolean","default":false,"description":"When language is html, emit only <style> and the root element without <!doctype html>, <html>, <head>, or <body>."}}),
        vec!["node_id", "language"],
        true,
        true,
    );
    add("create_node","Create one layer in a container; defaults to x/y=0, width/height=100. Supports undo. Text is only valid for text layers. Changes are in memory until saved.",
        json!({"parent_id":id,"kind":{"type":"string","enum":["frame","rectangle","ellipse","text"]},"name":name,"text":text,"x":coordinate,"y":coordinate,"width":size,"height":size,"fill":{"type":"string","maxLength":1024},"fills":{"type":"array","items":{"type":"object"}},"diff_image":{"type":"boolean","default":false}}),vec!["parent_id","kind"],false,false);
    add("update_node","Update specified fields of one non-page layer. Coordinates are parent-relative. Replacing text clears text runs and list ranges. Supports undo; changes remain in memory until saved.",
        json!({"node_id":id,"name":name,"text":text,"x":coordinate,"y":coordinate,"width":size,"height":size,"visible":{"type":"boolean"},"opacity":{"type":"number","minimum":0,"maximum":1},"fill":{"type":"string","maxLength":1024},"fills":{"type":"array","items":{"type":"object"}},"diff_image":{"type":"boolean","default":false}}),vec!["node_id"],false,true);
    add(
        "delete_node",
        "Remove a non-page layer and its entire subtree. Supports undo; changes remain in memory until saved.",
        json!({"node_id":id,"diff_image":{"type":"boolean","default":false}}),
        vec!["node_id"],
        false,
        false,
    );
    add(
        "list_comments",
        "List canvas comment threads enriched with resolved world coordinates, target node ID/name/kind, and top-level frame ID/name. Filter by resolved state, page_id, or node_id.",
        json!({"resolved":{"type":"boolean"},"page_id":id,"node_id":id}),
        vec![],
        true,
        true,
    );
    add(
        "add_comment",
        "Pin a new comment thread to a node (with local offset x/y) or page coordinates. Supports undo.",
        json!({"text":{"type":"string","minLength":1,"maxLength":10000},"author":{"type":"string","maxLength":256},"node_id":id,"page_id":id,"x":coordinate,"y":coordinate}),
        vec!["text"],
        false,
        false,
    );
    add(
        "reply_comment",
        "Append a reply message to an existing canvas comment thread and optionally mark it resolved/unresolved. Supports undo.",
        json!({"thread_id":{"type":"string","minLength":1},"text":{"type":"string","minLength":1,"maxLength":10000},"author":{"type":"string","maxLength":256},"resolved":{"type":"boolean"}}),
        vec!["thread_id", "text"],
        false,
        false,
    );
    add(
        "resolve_comment",
        "Mark a canvas comment thread resolved (default true) or unresolved (false). Supports undo.",
        json!({"thread_id":{"type":"string","minLength":1},"resolved":{"type":"boolean","default":true}}),
        vec!["thread_id"],
        false,
        true,
    );
    add(
        "apply_spec",
        "Build a complete UI subtree from a declarative NodeSpec (supports EaseUI BrainWave 2.0 presets like easeui-screen/easeui-card/easeui-button-primary, $easeui.* tokens, and Auto-Layout 5.0 row/column/wrap/hug/fill) in a single atomic undoable step.",
        json!({"parent_id":id,"spec":{"type":"object","description":"Declarative NodeSpec tree (kind, name, preset, width, height, width_sizing, height_sizing, fill, stroke, corner_radius, text, font_size, layout, dev_status, annotations, children)."},"diff_image":{"type":"boolean","default":false}}),
        vec!["spec"],
        false,
        false,
    );
    add(
        "export_spec",
        "Export a node and its descendants as a declarative NodeSpec JSON tree, enriched with Dev Mode status/annotations and any canvas comment threads pinned within the subtree.",
        json!({"node_id":id}),
        vec!["node_id"],
        true,
        true,
    );
    add(
        "set_dev_status",
        "Set a layer's Dev Mode handoff status ('ready', 'completed', or 'clear') and/or append a developer annotation. Supports undo.",
        json!({"node_id":id,"status":{"type":"string","enum":["ready","completed","clear","none"]},"annotation":{"type":"string","maxLength":10000}}),
        vec!["node_id"],
        false,
        false,
    );
    add(
        "export_fig_file",
        "Export the current session document to a Figma binary .fig container (with fig-kiwi header and deflate chunks) inside the workspace.",
        json!({"output_path":{"type":"string","description":"Destination .fig path inside the workspace."}}),
        vec!["output_path"],
        false,
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
        true,
        true,
    );
    add("undo", "Undo the latest successful edit in this session; returns changed=false if the stack is empty.", json!({}), vec![], false, false);
    add("redo", "Redo the most recently undone edit in this session; returns changed=false if the stack is empty.", json!({}), vec![], false, false);
    out.push(crate::tools::screenshot::tool_descriptor());
    out.push(crate::tools::batch::tool_descriptor());
    out.extend(crate::tools::agents::tool_descriptors());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::Rect;

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
            e.doc.style_meta.insert(key.into(), StyleMeta { name: name.into(), key: None, description: String::new(), property });
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
        assert!(err.contains("uikit") && err.contains(android_id) && err.contains("html"), "{err}");
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
        for tool in ["designos_generate_code", "designos_export_fig_file", "designos_export_spec"] {
            let mut e = Engine::default();
            let (frame, last) = rows_frame(&mut e);
            queue_the_last_row(&mut e, frame, last);
            let out = dir.join("out.fig");
            let args = match tool {
                "designos_generate_code" => json!({"node_id":frame.0,"language":"css"}),
                "designos_export_fig_file" => json!({"output_path":out.to_str().unwrap()}),
                _ => json!({"node_id":frame.0}),
            };
            e.call(tool, args).unwrap();
            assert_settled(&e, last, tool);
        }
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
        let mut e = Engine { doc: designos_doc::fixtures::button_library(), active_page_id: None, ..Engine::default() };
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
        let mut e =
            Engine { doc: designos_doc::fixtures::card_with_slots(), history: History::default(), active_page_id: None, live_ops: None, ..Engine::default() };
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
        let parsed_err: designos_doc::SpecError = serde_json::from_str(&err_str).unwrap();
        assert_eq!(parsed_err.code, "INVALID_SLOT_VALUE");
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
}
