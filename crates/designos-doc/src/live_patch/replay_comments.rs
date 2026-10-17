//! Editor side: replay comment edits without overwriting threads or replies the agent never saw.

use super::{LivePatchReport, ThreadChange};
use crate::{Command, Comment, CommentThread, Document, NodeId};
use std::collections::BTreeMap;

/// Agent comment ids that collided with ones the editor already used, and their new ids.
#[derive(Clone, Debug, Default)]
pub(super) struct CommentRemap {
    threads: BTreeMap<String, String>,
    /// Keyed by (editor thread id, agent message id).
    messages: BTreeMap<(String, String), String>,
}

impl CommentRemap {
    fn thread(&self, id: &str) -> String {
        self.threads.get(id).cloned().unwrap_or_else(|| id.to_string())
    }
    fn message(&self, thread: &str, id: &str) -> String {
        self.messages.get(&(thread.to_string(), id.to_string())).cloned().unwrap_or_else(|| id.to_string())
    }
}

pub(super) fn resolve_comments(
    doc: &Document,
    added: &[CommentThread],
    changed: &[ThreadChange],
    removed: &[String],
    remap: &mut CommentRemap,
    node: &dyn Fn(NodeId) -> NodeId,
    report: &mut LivePatchReport,
) -> Option<Command> {
    let mut comments = doc.comments.clone();
    for thread in added {
        let mut t = thread.clone();
        t.anchor.page = node(t.anchor.page);
        t.anchor.node = t.anchor.node.map(node);
        t.id = remap.thread(&thread.id);
        if comments.contains(&t) {
            continue;
        }
        if comments.iter().any(|c| c.id == t.id) {
            t.id = fresh_id(&t.id, |id| comments.iter().any(|c| c.id == id));
            remap.threads.insert(thread.id.clone(), t.id.clone());
        }
        comments.push(t);
    }
    for ch in changed {
        let id = remap.thread(&ch.id);
        let Some(t) = comments.iter_mut().find(|t| t.id == id) else {
            report.skipped.push(format!("comment thread '{id}' no longer exists"));
            continue;
        };
        apply_change(t, ch, remap, node);
    }
    let removed: Vec<String> = removed.iter().map(|id| remap.thread(id)).collect();
    comments.retain(|t| !removed.contains(&t.id));
    if comments == doc.comments {
        report.duplicates += 1;
        return None;
    }
    Some(Command::SetComments { comments })
}

fn apply_change(t: &mut CommentThread, ch: &ThreadChange, remap: &mut CommentRemap, node: &dyn Fn(NodeId) -> NodeId) {
    if let Some(resolved) = ch.resolved {
        t.resolved = resolved;
    }
    if let Some(mut anchor) = ch.anchor {
        anchor.page = node(anchor.page);
        anchor.node = anchor.node.map(node);
        t.anchor = anchor;
    }
    for msg in &ch.added {
        let mut m: Comment = msg.clone();
        m.id = remap.message(&t.id, &msg.id);
        if t.messages.contains(&m) {
            continue;
        }
        if t.messages.iter().any(|x| x.id == m.id) {
            m.id = fresh_id(&m.id, |id| t.messages.iter().any(|x| x.id == id));
            remap.messages.insert((t.id.clone(), msg.id.clone()), m.id.clone());
        }
        t.messages.push(m);
    }
    for msg in &ch.edited {
        let id = remap.message(&t.id, &msg.id);
        if let Some(slot) = t.messages.iter_mut().find(|x| x.id == id) {
            *slot = Comment { id, ..msg.clone() };
        }
    }
    let gone: Vec<String> = ch.removed.iter().map(|id| remap.message(&t.id, id)).collect();
    t.messages.retain(|m| !gone.contains(&m.id));
}

/// `base.2`, `base.3`, ... : the first one `taken` rejects.
fn fresh_id(base: &str, taken: impl Fn(&str) -> bool) -> String {
    (2u32..).map(|n| format!("{base}.{n}")).find(|c| !taken(c)).expect("an unused id")
}
