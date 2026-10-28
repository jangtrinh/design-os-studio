//! Style changes made while a new text box is still empty.
//!
//! A text box the Text tool just made lives only in its session's transaction, and a session that
//! ends with no text discards the box. Ending the session to make a style change a step of its own
//! would therefore delete the box. The changes that can run inside the session do (picking or
//! detaching a style, so the user can pick a style and then type); the others wait for text.

use crate::app::App;
use designos_doc::*;

/// What is said when a style change needs text that is not there yet.
pub const TYPE_FIRST: &str = "Type some text first";

/// What is said when a style is applied to a new, empty text box that was selected with other layers.
const ONLY_NEW_BOX: &str = "Only the new text box was styled";

impl App {
    /// The text box being edited when it is new, still empty, and one of `ids`. Empty as
    /// `finish_text` judges it: a composition in progress is thrown away when editing ends, so it is
    /// not text yet.
    pub(crate) fn new_empty_text_id(&self, ids: &[NodeId]) -> Option<NodeId> {
        let sess = self.canvas.text.as_ref().filter(|s| s.created && ids.contains(&s.id))?;
        let len = self.doc.get(sess.id)?.kind.any_text()?.content.len();
        let composing = sess.preedit.as_ref().map_or(0, |r| r.end.min(len).saturating_sub(r.start.min(len)));
        (len == composing).then_some(sess.id)
    }

    /// Whether the text box being edited is new, still empty, and one of `ids`.
    pub(crate) fn is_new_empty_text(&self, ids: &[NodeId]) -> bool {
        self.new_empty_text_id(ids).is_some()
    }

    /// Applies style `key` picked in the inspector. A text style goes through [`App::apply_text_style`],
    /// which knows about text being edited.
    pub(crate) fn apply_picked_style(&mut self, sel: &[NodeId], key: &str) {
        if matches!(self.doc.styles.get(key), Some(Style::Text(_))) {
            self.apply_text_style(sel, key);
        } else {
            self.apply_style(sel, key);
        }
    }

    /// Applies text style `key` to `sel` from the inspector. A new, empty text box takes it inside
    /// its session (the box and the session stay, the next typing uses the style, and it all commits
    /// as the one step that makes the box). It is the only layer styled then: a step for another
    /// selected layer would re-lay out their shared parent and record the uncommitted box in it, so
    /// undo would fail on a layer that does not exist yet. Other text being edited is committed
    /// first, so the style is a step of its own.
    pub(crate) fn apply_text_style(&mut self, sel: &[NodeId], key: &str) {
        if let Some(new_box) = self.new_empty_text_id(sel) {
            self.apply_text_style_in_new_box(key);
            if sel.iter().any(|id| *id != new_box) {
                self.show_toast(ONLY_NEW_BOX, false);
            }
            return;
        }
        self.end_text_edit_of(sel);
        self.apply_style(sel, key);
    }

    fn apply_text_style_in_new_box(&mut self, key: &str) {
        let Some(Style::Text(ts)) = self.doc.styles.get(key).cloned() else { return };
        // A text style carries no colour: the box keeps its own. The link goes in the same in-session
        // undo step as the values, so undo and redo keep them together.
        crate::canvas::textsession::base_style_linked(self, Some(key), |s| *s = TextStyle { fills: s.fills.clone(), ..ts.clone() });
        // The style, not an earlier shortcut for the next character, decides how typing looks.
        if let Some(sess) = self.canvas.text.as_mut() {
            sess.typing = None;
        }
    }

    /// Unlinks the style of the new, empty text box `new_box` being edited, inside its session; the
    /// other layers of `sel` are detached as a step of their own. False when that step is refused.
    pub(crate) fn detach_text_style_in_new_box(&mut self, sel: &[NodeId], new_box: NodeId) -> bool {
        crate::canvas::textsession::base_style_linked(self, None, |_| {});
        let others = without(sel, new_box);
        if others.is_empty() {
            return true;
        }
        let op = super::detach_text_style(&self.doc, &others);
        self.run_style_op(op)
    }
}

fn without(ids: &[NodeId], id: NodeId) -> Vec<NodeId> {
    ids.iter().copied().filter(|other| *other != id).collect()
}
