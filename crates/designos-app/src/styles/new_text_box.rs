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

impl App {
    /// Whether the text box being edited is new, still empty, and one of `ids`.
    pub(crate) fn is_new_empty_text(&self, ids: &[NodeId]) -> bool {
        self.canvas
            .text
            .as_ref()
            .is_some_and(|s| s.created && ids.contains(&s.id) && self.doc.get(s.id).and_then(|n| n.kind.any_text()).is_some_and(|t| t.content.is_empty()))
    }

    /// Applies text style `key` to `sel` from the inspector. A new, empty text box takes it inside
    /// its session (the box and the session stay, the next typing uses the style, and it all commits
    /// as the one step that makes the box). Any other text being edited is committed first, so the
    /// style is a step of its own.
    pub(crate) fn apply_text_style(&mut self, sel: &[NodeId], key: &str) {
        if self.is_new_empty_text(sel) {
            self.apply_text_style_in_new_box(key);
            return;
        }
        self.end_text_edit_of(sel);
        self.apply_style(sel, key);
    }

    fn apply_text_style_in_new_box(&mut self, key: &str) {
        let Some(Style::Text(ts)) = self.doc.styles.get(key).cloned() else { return };
        // A text style carries no colour: the box keeps its own.
        crate::canvas::textsession::base_style(self, |s| *s = TextStyle { fills: s.fills.clone(), ..ts.clone() });
        self.link_new_box(Some(key));
    }

    /// Unlinks the style of the new, empty text box being edited, inside its session.
    pub(crate) fn detach_text_style_in_new_box(&mut self) {
        self.link_new_box(None);
    }

    /// Links the box being edited to text style `key`, or unlinks it, through the session's transaction.
    fn link_new_box(&mut self, key: Option<&str>) {
        let App { doc, canvas, .. } = self;
        let Some(sess) = canvas.text.as_mut() else { return };
        if key.is_some() {
            // The style, not an earlier shortcut for the next character, decides how typing looks.
            sess.typing = None;
        }
        let id = sess.id;
        if let Some(n) = sess.tx.ed.node(doc, id) {
            match key {
                Some(k) => n.style_refs.insert("text".into(), k.into()),
                None => n.style_refs.remove("text"),
            };
        }
        canvas.dirty = true;
    }
}
