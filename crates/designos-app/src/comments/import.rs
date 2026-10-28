//! Figma REST comments (`GET /v1/files/:key/comments`) mapped onto designos threads.
//!
//! ```json
//! { "comments": [{
//!     "id": "1", "parent_id": "", "message": "text",
//!     "user": { "handle": "Jane" },
//!     "created_at": "2024-03-01T10:00:00Z", "resolved_at": null,
//!     "client_meta": { "node_id": "12:34", "node_offset": { "x": 10, "y": 20 } }
//! }] }
//! ```
//!
//! `client_meta` is either a canvas point (`x`, `y`) or a node plus an offset into it
//! (`node_id`, `node_offset`). Node ids are matched against `Node::source_id`, which
//! the Figma importers fill. Replies (`parent_id`) join the thread of their root.

use super::time;
use designos_doc::{Comment, CommentAnchor, CommentThread, Document, NodeId, Vec2};
use serde_json::Value;

/// What an import changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Merged {
    pub threads: usize,
    pub replies: usize,
    pub skipped: usize,
}

fn comment_list(v: &Value) -> Option<&Vec<Value>> {
    match v {
        Value::Array(a) => Some(a),
        Value::Object(o) => o.get("comments")?.as_array(),
        _ => None,
    }
}

/// True when `text` is a Figma comments payload (and not a design or variables file).
pub fn is_comments_payload(text: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return false };
    if v.get("document").is_some() || v.get("nodes").is_some() || v.get("pages").is_some() {
        return false;
    }
    comment_list(&v).is_some_and(|l| !l.is_empty() && l.iter().all(|c| c.get("message").is_some()))
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

fn point(v: &Value) -> Option<Vec2> {
    Some(Vec2::new(v.get("x")?.as_f64()? as f32, v.get("y")?.as_f64()? as f32))
}

fn anchor_of(doc: &Document, meta: &Value) -> CommentAnchor {
    let first = doc.pages.first().copied().unwrap_or(NodeId(0));
    let node_id = str_of(meta, "node_id");
    if !node_id.is_empty() {
        let offset = meta.get("node_offset").and_then(point).unwrap_or(Vec2::ZERO);
        if let Some(n) = doc.nodes.values().find(|n| n.source_id.as_deref() == Some(node_id)) {
            let page = doc.page_of(n.id).unwrap_or(first);
            return CommentAnchor { page, node: Some(n.id), offset };
        }
        // The layer is not in this file: keep the pin at the offset on the first page.
        return CommentAnchor { page: first, node: None, offset };
    }
    CommentAnchor { page: first, node: None, offset: point(meta).unwrap_or(Vec2::ZERO) }
}

/// Builds the threads described by `text` (not yet merged into the document).
pub fn parse_threads(doc: &Document, text: &str) -> Result<(Vec<CommentThread>, usize), String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("Not valid JSON: {e}"))?;
    let list = comment_list(&v).ok_or("No \"comments\" array found")?;
    let mut items: Vec<&Value> = list.iter().filter(|c| c.get("message").is_some()).collect();
    items.sort_by_key(|c| time::parse_rfc3339(str_of(c, "created_at")).unwrap_or(0));
    let mk_comment = |c: &Value| {
        let user = c.get("user");
        let author = user.map(|u| str_of(u, "handle")).filter(|h| !h.is_empty()).or_else(|| user.map(|u| str_of(u, "name")).filter(|h| !h.is_empty()));
        let created = str_of(c, "created_at");
        Comment {
            id: format!("figma-{}", str_of(c, "id")),
            author: author.unwrap_or("Unknown").to_string(),
            author_id: None,
            author_color: None,
            text: str_of(c, "message").to_string(),
            created: if time::parse_rfc3339(created).is_some() { created.to_string() } else { time::now_rfc3339() },
            edited: None,
        }
    };
    let mut threads: Vec<CommentThread> = Vec::new();
    let mut skipped = 0;
    for c in &items {
        let parent = str_of(c, "parent_id");
        if parent.is_empty() {
            threads.push(CommentThread {
                id: format!("figma-{}", str_of(c, "id")),
                anchor: anchor_of(doc, c.get("client_meta").unwrap_or(&Value::Null)),
                messages: vec![mk_comment(c)],
                resolved: c.get("resolved_at").is_some_and(|r| !r.is_null()),
            });
        }
    }
    for c in &items {
        let parent = str_of(c, "parent_id");
        if parent.is_empty() {
            continue;
        }
        match threads.iter_mut().find(|t| t.id == format!("figma-{parent}")) {
            Some(t) => t.messages.push(mk_comment(c)),
            None => skipped += 1,
        }
    }
    Ok((threads, skipped))
}

/// Adds `imported` to `existing`: unknown threads are appended, known ones only gain
/// messages they did not have (so importing the same file twice changes nothing).
pub fn merge(existing: &[CommentThread], imported: Vec<CommentThread>) -> (Vec<CommentThread>, Merged) {
    let mut out = existing.to_vec();
    let mut m = Merged::default();
    for t in imported {
        match out.iter_mut().find(|e| e.id == t.id) {
            Some(e) => {
                for msg in t.messages {
                    if !e.messages.iter().any(|x| x.id == msg.id) {
                        e.messages.push(msg);
                        m.replies += 1;
                    }
                }
            }
            None => {
                m.replies += t.messages.len().saturating_sub(1);
                m.threads += 1;
                out.push(t);
            }
        }
    }
    (out, m)
}

/// A small Figma comments export used by the tests and the `--fixture comments` screenshot.
#[cfg(any(test, feature = "fixtures"))]
pub const FIXTURE: &str = include_str!("fixtures/figma_comments.json");

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect};

    fn doc() -> Document {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(100.0, 100.0, 300.0, 200.0));
        f.source_id = Some("10:1".into());
        d.insert(page, f).unwrap();
        d
    }

    #[test]
    fn detects_payloads() {
        assert!(is_comments_payload(FIXTURE));
        assert!(is_comments_payload(r#"[{"message":"x"}]"#));
        assert!(!is_comments_payload(r#"{"comments":[]}"#));
        assert!(!is_comments_payload(r#"{"document":{},"comments":[{"message":"x"}]}"#));
        assert!(!is_comments_payload("nope"));
    }

    #[test]
    fn maps_the_figma_shape() {
        let d = doc();
        let (threads, skipped) = parse_threads(&d, FIXTURE).unwrap();
        assert_eq!(skipped, 1, "the reply to an unknown comment is skipped");
        assert_eq!(threads.len(), 3);
        let t = &threads[0];
        assert_eq!(t.id, "figma-101");
        assert_eq!(
            t.messages.iter().map(|m| (m.author.as_str(), m.text.as_str())).collect::<Vec<_>>(),
            [("Jane Cooper", "Can we make the title bigger?"), ("kir", "Done, check the new size.")]
        );
        assert!(!t.resolved);
        let frame = d.nodes.values().find(|n| n.source_id.as_deref() == Some("10:1")).unwrap().id;
        assert_eq!(t.anchor.node, Some(frame));
        assert_eq!((t.anchor.offset.x, t.anchor.offset.y), (24.0, 16.5));
        assert_eq!(t.anchor.page, d.pages[0]);
        let free = &threads[1];
        assert!(free.resolved, "resolved_at marks the thread resolved");
        assert_eq!((free.anchor.node, free.anchor.offset.x, free.anchor.offset.y), (None, 640.0, 120.0));
        let gone = &threads[2];
        assert_eq!(gone.anchor.node, None, "an unknown layer falls back to a page point");
        assert_eq!(gone.messages[0].author, "Unknown");
        assert!(time::parse_rfc3339(&t.messages[0].created).is_some());
    }

    #[test]
    fn merging_twice_changes_nothing() {
        let d = doc();
        let (threads, _) = parse_threads(&d, FIXTURE).unwrap();
        let (first, m) = merge(&d.comments, threads.clone());
        assert_eq!(m, Merged { threads: 3, replies: 1, skipped: 0 });
        let (second, m2) = merge(&first, threads);
        assert_eq!(second, first);
        assert_eq!(m2, Merged::default());
    }

    #[test]
    fn errors_are_reported() {
        let d = doc();
        assert!(parse_threads(&d, "{").is_err());
        assert!(parse_threads(&d, "{\"a\":1}").is_err());
    }
}
