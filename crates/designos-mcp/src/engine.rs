//! Typed, local document operations shared by the MCP transport and future bridges.
use designos_doc::live_patch::{live_ops_for_command, LiveOp};
use designos_doc::{
    Annotation, Command, Comment, CommentAnchor, CommentThread, DevStatus, Document, History, Node, NodeId, NodeKind, NodeSpec, Rect, TextStyle, Vec2,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

pub struct Engine {
    pub doc: Document,
    pub history: History,
    pub active_page_id: Option<NodeId>,
    /// Edits executed since the last [`Engine::take_live_ops`], recorded while an open editor
    /// mirrors this session (`None` = not recording).
    pub live_ops: Option<Vec<LiveOp>>,
}

impl Default for Engine {
    fn default() -> Self {
        let mut doc = Document::new("Untitled");
        let page = doc.add_page("Page 1");
        Self { doc, history: History::default(), active_page_id: Some(page), live_ops: None }
    }
}

impl Engine {
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
                let code = match a.language.as_str() {
                    "css" => designos_export::css_for(&doc, id),
                    "tailwind" => designos_export::tailwind_for(&doc, id),
                    "react" => designos_export::react_for(&doc, id),
                    "swiftui" => designos_export::swiftui_for(&doc, id),
                    "uikit" => designos_export::uikit_for(&doc, id),
                    "compose" => designos_export::compose_for(&doc, id),
                    "android-xml" => designos_export::android_xml_for(&doc, id),
                    "svg" => designos_export::export_svg(&doc, id, &Default::default()),
                    _ => return Err("language must be css, tailwind, react, swiftui, uikit, compose, android-xml, or svg".into()),
                };
                Ok(json!({"node_id":id,"language":a.language,"code":code}))
            }
            "designos_create_node" => self.create(parse(args)?),
            "designos_update_node" => self.update(parse(args)?),
            "designos_delete_node" => {
                let a: NodeArgs = parse(args)?;
                let n = self.node(a.node_id)?;
                if n.parent.is_none() {
                    return Err("page roots cannot be deleted by this tool".into());
                }
                let id = n.id;
                self.exec(Command::Remove { id })?;
                Ok(json!({"node_id":id,"deleted":true}))
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
                let mut trial = self.doc.clone();
                let root_id = trial.apply_spec(parent, &a.spec)?;
                let mut nodes = Vec::new();
                trial.walk(root_id, |n, _| nodes.push(n.clone()));
                let index = self.doc.get(parent).ok_or("parent not found")?.children.len();
                self.exec(Command::Insert { parent, index, nodes })?;
                self.doc.relayout_subtree(parent);
                Ok(json!({"root_node_id": root_id, "parent_id": parent.0, "node": self.doc.get(root_id), "changed": true}))
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
            "designos_lint" | "check_design" => {
                let a: LintArgs = parse(args)?;
                let root = a.page_id.map(NodeId).or_else(|| self.resolve_default_page().ok());
                let mut findings = Vec::new();
                let mut check_node = |n: &Node| {
                    if matches!(n.kind, NodeKind::Page { .. }) {
                        return;
                    }
                    if n.size.x <= 0.0 || n.size.y <= 0.0 {
                        findings.push(json!({"rule": "zero-bounds", "severity": "error", "node_id": n.id.0, "name": n.name, "message": "Layer has zero or negative width/height"}));
                    }
                    let trimmed = n.name.trim();
                    if matches!(trimmed, "Frame" | "Rectangle" | "Ellipse" | "Text" | "Group") {
                        findings.push(json!({"rule": "default-layer-name", "severity": "warning", "node_id": n.id.0, "name": n.name, "message": "Give layer a semantic name"}));
                    }
                };
                if let Some(r) = root {
                    self.doc.walk(r, |n, _| check_node(n));
                } else {
                    for n in self.doc.nodes.values() {
                        check_node(n);
                    }
                }
                let count = findings.len();
                Ok(json!({"page_id": root.map(|r| r.0), "findings": findings, "total": count, "passed": count == 0}))
            }
            "designos_undo" | "designos_redo" => {
                let _: Empty = parse(args)?;
                let undo = name == "designos_undo";
                let next = if undo { self.history.peek_undo() } else { self.history.peek_redo() }.cloned();
                let ops = match &next {
                    Some(command) => self.live_ops_for(command)?,
                    None => Vec::new(),
                };
                let changed = if undo { self.history.undo(&mut self.doc) } else { self.history.redo(&mut self.doc) }.map_err(|e| e.to_string())?;
                self.record(ops);
                Ok(json!({"changed":changed,"can_undo":self.history.can_undo(),"can_redo":self.history.can_redo()}))
            }
            _ => Err(format!("unknown tool: {name}")),
        }
    }

    fn node(&self, id: u64) -> Result<&Node, String> {
        if id == 0 {
            return Err("node IDs must be positive integers".into());
        }
        self.doc.get(NodeId(id)).ok_or_else(|| format!("node {id} not found"))
    }

    fn exec(&mut self, command: Command) -> Result<(), String> {
        let ops = self.live_ops_for(&command)?;
        self.history.exec(&mut self.doc, command).map_err(|e| e.to_string())?;
        self.record(ops);
        Ok(())
    }

    /// The ops `command` will perform, when recording; refuses edits the editor cannot replay.
    fn live_ops_for(&self, command: &Command) -> Result<Vec<LiveOp>, String> {
        if self.live_ops.is_none() {
            return Ok(Vec::new());
        }
        live_ops_for_command(&self.doc, command)
    }

    fn record(&mut self, ops: Vec<LiveOp>) {
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
        let parent = self.node(parent_id.0)?;
        if !parent.kind.is_container() {
            return Err("parent_id must identify a container".into());
        }
        let index = parent.children.len();
        geometry(a.x, a.y, a.width, a.height)?;
        if let Some(name) = &a.name {
            bounded_text("name", name, 1024)?;
        }
        if let Some(text) = &a.text {
            bounded_text("text", text, 100_000)?;
        }
        if a.kind != "text" && a.text.is_some() {
            return Err("text can only be set on text nodes".into());
        }
        let rect = Rect::new(a.x, a.y, a.width, a.height);
        // Allocate only after every argument has passed validation.
        let constructor: fn(NodeId, Rect) -> Node = match a.kind.as_str() {
            "frame" => Node::frame,
            "rectangle" => Node::rectangle,
            "ellipse" => Node::ellipse,
            "text" => |id, r| Node::text(id, r, "", TextStyle::default()),
            _ => return Err("kind must be frame, rectangle, ellipse, or text".into()),
        };
        // Imported native files may carry a stale allocation counter. Derive a
        // fresh ID from the actual nodes instead of risking an overwrite.
        let id = self.doc.try_alloc_id().ok_or("Document has exhausted node IDs")?;
        let mut node = constructor(id, rect);
        if let Some(name) = a.name {
            node.name = name;
        }
        if let (Some(text), NodeKind::Text(props)) = (a.text, &mut node.kind) {
            props.content = text;
        }
        self.exec(Command::Insert { parent: parent_id, index, nodes: vec![node] })?;
        if let Some(page_id) = self.doc.page_of(id) {
            self.doc.relayout_subtree(page_id);
        }
        Ok(json!({"node":self.doc.get(id),"parent_id":parent_id.0,"changed":true}))
    }

    fn update(&mut self, a: UpdateArgs) -> Result<Value, String> {
        let mut node = self.node(a.node_id)?.clone();
        if node.parent.is_none() {
            return Err("page roots cannot be updated by this tool".into());
        }
        if let Some(name) = a.name {
            bounded_text("name", &name, 1024)?;
            node.name = name;
        }
        if let Some(x) = a.x {
            node.transform.tx = x;
        }
        if let Some(y) = a.y {
            node.transform.ty = y;
        }
        if let Some(width) = a.width {
            node.size.x = width;
        }
        if let Some(height) = a.height {
            node.size.y = height;
        }
        geometry(node.transform.tx, node.transform.ty, node.size.x, node.size.y)?;
        if let Some(visible) = a.visible {
            node.visible = visible;
        }
        if let Some(opacity) = a.opacity {
            if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                return Err("opacity must be between 0 and 1".into());
            }
            node.opacity = opacity;
        }
        if let Some(text) = a.text {
            bounded_text("text", &text, 100_000)?;
            let NodeKind::Text(props) = &mut node.kind else {
                return Err("text can only be set on text nodes".into());
            };
            if props.content != text {
                props.content = text;
                props.runs.clear();
                props.lists.clear();
            }
        }
        let id = node.id;
        let changed = self.doc.get(id) != Some(&node);
        if changed {
            self.exec(Command::Update { node: Box::new(node) })?;
            if let Some(page_id) = self.doc.page_of(id) {
                self.doc.relayout_subtree(page_id);
            }
        }
        Ok(json!({"node":self.doc.get(NodeId(a.node_id)),"changed":changed}))
    }
}

fn summary(n: &Node) -> Value {
    json!({"node_id":n.id,"name":n.name,"kind":n.kind.default_name(),"parent_id":n.parent,
        "child_count":n.children.len(),"visible":n.visible,"x":n.transform.tx,"y":n.transform.ty,
        "width":n.size.x,"height":n.size.y})
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}
fn bounded_text(field: &str, value: &str, max: usize) -> Result<(), String> {
    if value.len() > max {
        Err(format!("{field} exceeds {max} UTF-8 bytes"))
    } else {
        Ok(())
    }
}
fn geometry(x: f32, y: f32, width: f32, height: f32) -> Result<(), String> {
    if [x, y, width, height].iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.0) {
        return Err("geometry must be finite and within +/-1000000".into());
    }
    if width < 0.0 || height < 0.0 {
        return Err("width and height must be non-negative".into());
    }
    Ok(())
}
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
    add(
        "generate_code",
        "Generate developer handoff code or SVG for a layer using resolved variable values. Returns source text; writes no files.",
        json!({"node_id":id,"language":{"type":"string","enum":["css","tailwind","react","swiftui","uikit","compose","android-xml","svg"]}}),
        vec!["node_id", "language"],
        true,
        true,
    );
    add("create_node","Create one layer in a container; defaults to x/y=0, width/height=100. Supports undo. Text is only valid for text layers. Changes are in memory until saved.",
        json!({"parent_id":id,"kind":{"type":"string","enum":["frame","rectangle","ellipse","text"]},"name":name,"text":text,"x":coordinate,"y":coordinate,"width":size,"height":size}),vec!["parent_id","kind"],false,false);
    add("update_node","Update specified fields of one non-page layer. Coordinates are parent-relative. Replacing text clears text runs and list ranges. Supports undo; changes remain in memory until saved.",
        json!({"node_id":id,"name":name,"text":text,"x":coordinate,"y":coordinate,"width":size,"height":size,"visible":{"type":"boolean"},"opacity":{"type":"number","minimum":0,"maximum":1}}),vec!["node_id"],false,true);
    add(
        "delete_node",
        "Remove a non-page layer and its entire subtree. Supports undo; changes remain in memory until saved.",
        json!({"node_id":id}),
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
        json!({"parent_id":id,"spec":{"type":"object","description":"Declarative NodeSpec tree (kind, name, preset, width, height, width_sizing, height_sizing, fill, stroke, corner_radius, text, font_size, layout, dev_status, annotations, children)."}}),
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
    add("undo", "Undo the latest successful edit in this session; returns changed=false if the stack is empty.", json!({}), vec![], false, false);
    add("redo", "Redo the most recently undone edit in this session; returns changed=false if the stack is empty.", json!({}), vec![], false, false);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
        e.doc
            .styles
            .insert("Layout/Cols".into(), Style::Grid(vec![LayoutGrid::Columns { count: 12, gutter: 20.0, offset: 0.0, color: Color::rgba(255, 0, 0, 26) }]));
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
        for language in ["css", "tailwind", "react", "swiftui", "uikit", "compose", "android-xml", "svg"] {
            let output = e.call("designos_generate_code", json!({"node_id":id,"language":language})).unwrap();
            assert!(!output["code"].as_str().unwrap().is_empty());
        }
        let uikit = e.call("designos_generate_code", json!({"node_id":id,"language":"uikit"})).unwrap();
        assert!(uikit["code"].as_str().unwrap().starts_with("let card = "), "{uikit}");
        let xml = e.call("designos_generate_code", json!({"node_id":id,"language":"android-xml"})).unwrap();
        assert!(xml["code"].as_str().unwrap().starts_with("<!-- res/layout/card.xml -->"), "{xml}");
        let err = e.call("designos_generate_code", json!({"node_id":id,"language":"vue"})).unwrap_err();
        assert!(err.contains("uikit") && err.contains("android-xml"), "{err}");
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
}
