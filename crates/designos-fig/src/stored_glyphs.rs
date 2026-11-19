//! Which texts of a large `.fig` file keep the glyph outlines Figma stored with them.
//!
//! `derivedTextData` is where Figma keeps the outlines it drew. A small file keeps it for every text. A large file
//! (more than 5,000 entries, or a message payload over 2 MB) cannot afford that: the kit's 12,884 texts carry
//! 55 MB of it. The importer cannot ask the font database which families a machine has, and a document must not
//! change with the machine that opened it, so it decides from the one set that is the same everywhere: the
//! bundled families. A text set in a bundled family always shapes with the bundled face, so its stored outlines
//! would never be drawn and are not kept. Every other family keeps them, up to [`BUDGET_BYTES`] per import.

use crate::kiwi::KiwiValue;
use std::sync::Arc;

/// How many bytes of `derivedTextData` (as JSON) one large-file import keeps. The kit needs 156 KB of it. A file
/// whose texts all use a non-bundled font stops keeping at this size and counts the texts it dropped
/// ([`crate::rest_import::ImportReport::dropped_stored_glyphs`]).
///
/// What is kept costs about 5.3 times its JSON size in memory once the outlines are built (JSON string, glyph
/// paths and the shared blobs): the kit's 109 KB of JSON retain 581 KB. So the allowance bounds an import's extra
/// memory at roughly 340 MiB.
pub(crate) const BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// Whether `family` is one of the families the app embeds. Exact name, ignoring case: a family that only has a
/// substitute (Helvetica to Arimo) is not bundled, which is how the render path treats it too.
pub(crate) fn is_bundled_family(family: &str) -> bool {
    designos_fonts::bundled().iter().any(|asset| asset.family.eq_ignore_ascii_case(family))
}

/// Whether a node change with these decoded `fields` is a text in a font the importer cannot prove installed,
/// so its `derivedTextData` is worth keeping in a large file. `false` for any other node, and for a text whose
/// `type` or `fontName` is not among `fields` yet (the kiwi decoder asks while it is still reading the message).
pub(crate) fn keeps_stored_glyphs(fields: &[(Arc<str>, KiwiValue)]) -> bool {
    let field = |name: &str| fields.iter().find(|(k, _)| &**k == name).map(|(_, v)| v);
    if !matches!(field("type"), Some(KiwiValue::Enum(t)) if &**t == "TEXT") {
        return false;
    }
    match field("fontName").and_then(|f| f.get("family")) {
        Some(KiwiValue::String(family)) => !is_bundled_family(family),
        _ => false,
    }
}

/// JSON-side twin of [`keeps_stored_glyphs`] for a text that is already JSON: whether its `fontName.family` is
/// anything but a bundled family. A text without a family keeps nothing (nothing says what font it is set in).
pub(crate) fn json_keeps_stored_glyphs(node: &serde_json::Value) -> bool {
    node.get("fontName").and_then(|f| f.get("family")).and_then(serde_json::Value::as_str).is_some_and(|family| !is_bundled_family(family))
}

/// Running total of the `derivedTextData` one large-file import keeps.
pub(crate) struct Budget {
    limit: usize,
    kept_bytes: usize,
    pub dropped_texts: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self::with_limit(BUDGET_BYTES)
    }
}

impl Budget {
    pub fn with_limit(limit: usize) -> Self {
        Self { limit, kept_bytes: 0, dropped_texts: 0 }
    }

    /// Takes `len` bytes if they fit; otherwise counts a dropped text and refuses.
    pub fn take(&mut self, len: usize) -> bool {
        match self.kept_bytes.checked_add(len).filter(|&total| total <= self.limit) {
            Some(total) => {
                self.kept_bytes = total;
                true
            }
            None => {
                self.dropped_texts += 1;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fig::take_stored_glyphs;
    use crate::kiwi::KiwiValue as V;

    fn text_node(family: &str) -> V {
        let font = V::Struct(vec![("family".into(), V::String(family.into()))]);
        let derived = V::Message(vec![("layoutSize".into(), V::Struct(vec![("x".into(), V::Float(20.0))]))]);
        V::Message(vec![("type".into(), V::Enum("TEXT".into())), ("fontName".into(), font), ("derivedTextData".into(), derived)])
    }

    #[test]
    fn only_a_non_bundled_family_keeps_its_stored_glyphs() {
        let fields = |v: &V| match v {
            V::Message(f) => f.clone(),
            _ => unreachable!(),
        };
        assert!(!keeps_stored_glyphs(&fields(&text_node("Inter"))), "a bundled family is installed everywhere");
        assert!(!keeps_stored_glyphs(&fields(&text_node("inter"))), "the family name is matched ignoring case");
        assert!(!keeps_stored_glyphs(&fields(&text_node("Roboto Mono"))));
        assert!(keeps_stored_glyphs(&fields(&text_node("Helvetica"))), "a family that only has a substitute is not installed");
        assert!(keeps_stored_glyphs(&fields(&text_node("Some Icon Font"))));
        let mut frame = fields(&text_node("Some Icon Font"));
        frame[0].1 = V::Enum("FRAME".into());
        assert!(!keeps_stored_glyphs(&frame), "only a text node has glyphs worth keeping");
        assert!(!keeps_stored_glyphs(&frame[..1]), "a message that has not shown its font yet keeps nothing");
    }

    #[test]
    fn the_budget_refuses_a_text_that_does_not_fit_and_counts_it() {
        let mut budget = Budget::with_limit(100);
        assert!(budget.take(60));
        assert!(!budget.take(41), "60 + 41 passes 100");
        assert_eq!(budget.dropped_texts, 1);
        assert!(budget.take(40), "a smaller text still fits after a refusal");
        assert!(!budget.take(usize::MAX), "a size that overflows is refused, not wrapped");
        assert_eq!(budget.dropped_texts, 2);
    }

    #[test]
    fn taking_a_text_moves_its_derived_data_out_and_obeys_the_budget() {
        let mut roomy = Budget::default();
        let mut kept = text_node("Some Icon Font");
        assert!(take_stored_glyphs(&mut kept, &mut roomy).is_some_and(|json| json.get("layoutSize").is_some()));
        assert!(kept.get("derivedTextData").is_none(), "the node no longer holds it");

        let mut bundled = text_node("Inter");
        assert!(take_stored_glyphs(&mut bundled, &mut roomy).is_none());
        assert!(bundled.get("derivedTextData").is_none(), "a bundled family's is dropped");
        assert_eq!(roomy.dropped_texts, 0, "a bundled family is not a text over budget");

        let mut tight = Budget::with_limit(5);
        assert!(take_stored_glyphs(&mut text_node("Some Icon Font"), &mut tight).is_none());
        assert_eq!(tight.dropped_texts, 1);
    }
}
