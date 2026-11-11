//! Which properties of a linked text style a text layer overrides.
//!
//! This follows Figma: only four things are an override over a text style that keeps the style
//! linked, namely the font weight, the italic font style, the text decoration and a hyperlink.
//! Everything else a style owns (family, size, line height, spacing, case, features, paragraph
//! settings) is not: editing or binding it detaches the style, as it always did.
//!
//! An override lives where it was made. A whole-layer override is recorded in
//! [`Node::text_style_overrides`] (stored, not derived from "value differs from the style": a derived
//! answer is wrong once the style changes to equal an override). An override on a character range is
//! the range's own value: a run that differs from the layer's base style in one of the three style
//! properties is an override of it, which is also what Figma's `isOverrideOverTextStyle` entries mean.
//! A hyperlink is a layer property in this model and is never a style value, so a hyperlink on a
//! linked text layer is always an override.

use crate::*;
use serde::{Deserialize, Serialize};

/// A property a text style owns, plus the hyperlink a layer can carry over it. Colour and alignment are
/// the layer's own and are not listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnedField {
    Font,
    Weight,
    Italic,
    Size,
    LineHeight,
    LetterSpacing,
    ParagraphSpacing,
    Indent,
    Case,
    Decoration,
    Features,
    ListSpacing,
    LeadingTrim,
    Hyperlink,
}

impl OwnedField {
    /// Every field, in the order labels are shown.
    pub const ALL: [OwnedField; 14] = [
        OwnedField::Font,
        OwnedField::Weight,
        OwnedField::Italic,
        OwnedField::Size,
        OwnedField::LineHeight,
        OwnedField::LetterSpacing,
        OwnedField::ParagraphSpacing,
        OwnedField::Indent,
        OwnedField::Case,
        OwnedField::Decoration,
        OwnedField::Features,
        OwnedField::ListSpacing,
        OwnedField::LeadingTrim,
        OwnedField::Hyperlink,
    ];

    /// The fields that can be overridden while the style stays linked (Figma's `TextStyleOverrideType`).
    pub const OVERRIDABLE: [OwnedField; 4] = [OwnedField::Weight, OwnedField::Italic, OwnedField::Decoration, OwnedField::Hyperlink];

    /// Whether an edit of this field keeps the style linked and records an override.
    pub fn is_overridable(self) -> bool {
        Self::OVERRIDABLE.contains(&self)
    }

    /// Short name shown to people.
    pub fn label(self) -> &'static str {
        match self {
            OwnedField::Font => "Font",
            OwnedField::Weight => "Weight",
            OwnedField::Italic => "Italic",
            OwnedField::Size => "Size",
            OwnedField::LineHeight => "Line height",
            OwnedField::LetterSpacing => "Letter spacing",
            OwnedField::ParagraphSpacing => "Paragraph spacing",
            OwnedField::Indent => "Indent",
            OwnedField::Case => "Case",
            OwnedField::Decoration => "Decoration",
            OwnedField::Features => "Features",
            OwnedField::ListSpacing => "List spacing",
            OwnedField::LeadingTrim => "Leading trim",
            OwnedField::Hyperlink => "Hyperlink",
        }
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }

    /// The field a node variable binding key stands for, `None` for keys no text style owns.
    pub fn from_binding_key(key: &str) -> Option<OwnedField> {
        Some(match key {
            "fontFamily" => OwnedField::Font,
            "fontStyle" | "fontWeight" => OwnedField::Weight,
            "fontSize" => OwnedField::Size,
            "lineHeight" => OwnedField::LineHeight,
            "letterSpacing" => OwnedField::LetterSpacing,
            "paragraphSpacing" => OwnedField::ParagraphSpacing,
            "paragraphIndent" => OwnedField::Indent,
            "listSpacing" => OwnedField::ListSpacing,
            "leadingTrim" => OwnedField::LeadingTrim,
            _ => return None,
        })
    }
}

/// Variable binding keys of the properties a text style owns.
pub const TEXT_STYLE_BINDING_KEYS: [&str; 10] =
    ["fontFamily", "fontStyle", "fontSize", "fontWeight", "lineHeight", "letterSpacing", "paragraphSpacing", "paragraphIndent", "listSpacing", "leadingTrim"];

/// Whether binding a variable to `key` on a layer linked to a text style keeps the link (an override
/// of the style) and records it. A bind of any other style-owned key detaches the style.
pub fn binding_key_is_overridable(key: &str) -> bool {
    OwnedField::from_binding_key(key).is_some_and(OwnedField::is_overridable)
}

/// A set of [`OwnedField`]s, stored as one `u16` (empty by default, and then not serialized).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OwnedFieldSet(u16);

impl OwnedFieldSet {
    pub const EMPTY: OwnedFieldSet = OwnedFieldSet(0);

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn contains(&self, field: OwnedField) -> bool {
        self.0 & field.bit() != 0
    }

    pub fn insert(&mut self, field: OwnedField) {
        self.0 |= field.bit();
    }

    pub fn remove(&mut self, field: OwnedField) {
        self.0 &= !field.bit();
    }

    /// Adds every field of `other`.
    pub fn union(&mut self, other: OwnedFieldSet) {
        self.0 |= other.0;
    }

    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// Whether the set holds a field whose edit detaches the style.
    pub fn has_non_overridable(&self) -> bool {
        self.fields().any(|f| !f.is_overridable())
    }

    /// The fields in the set, in display order.
    pub fn fields(&self) -> impl Iterator<Item = OwnedField> + '_ {
        OwnedField::ALL.into_iter().filter(|f| self.contains(*f))
    }

    /// Short names of the fields, in display order.
    pub fn labels(&self) -> Vec<&'static str> {
        self.fields().map(OwnedField::label).collect()
    }
}

fn axes_without_weight(s: &TextStyle) -> Vec<&(String, f32)> {
    s.variation_axes.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("wght")).collect()
}

/// Whether `field` differs between two character styles. The paragraph fields live on the layer, not on
/// a character style, so they never differ here, and neither does the hyperlink.
fn style_field_differs(field: OwnedField, a: &TextStyle, b: &TextStyle) -> bool {
    match field {
        OwnedField::Font => a.font_family != b.font_family,
        OwnedField::Weight => a.font_weight != b.font_weight || a.get_axis("wght") != b.get_axis("wght"),
        OwnedField::Italic => a.italic != b.italic,
        OwnedField::Size => a.font_size != b.font_size,
        OwnedField::LineHeight => a.line_height != b.line_height,
        OwnedField::LetterSpacing => a.letter_spacing != b.letter_spacing,
        OwnedField::ParagraphSpacing => a.paragraph_spacing != b.paragraph_spacing,
        OwnedField::Case => a.case != b.case,
        OwnedField::Decoration => a.decoration != b.decoration,
        // The weight axis follows the weight, so it counts under Weight.
        OwnedField::Features => a.features != b.features || axes_without_weight(a) != axes_without_weight(b),
        OwnedField::Indent | OwnedField::ListSpacing | OwnedField::LeadingTrim | OwnedField::Hyperlink => false,
    }
}

fn mark_style_diff(set: &mut OwnedFieldSet, a: &TextStyle, b: &TextStyle) {
    for f in OwnedField::ALL {
        if style_field_differs(f, a, b) {
            set.insert(f);
        }
    }
}

/// The style-owned fields in which two character styles differ (colour and alignment are not owned).
pub fn text_style_diff(a: &TextStyle, b: &TextStyle) -> OwnedFieldSet {
    let mut set = OwnedFieldSet::EMPTY;
    mark_style_diff(&mut set, a, b);
    set
}

/// The three character-style fields a layer or a range can override over a text style.
pub const STYLE_OVERRIDABLE: [OwnedField; 3] = [OwnedField::Weight, OwnedField::Italic, OwnedField::Decoration];

/// Copies the value of an overridable character-style field from one style to another.
pub fn copy_overridable_field(field: OwnedField, from: &TextStyle, to: &mut TextStyle) {
    match field {
        OwnedField::Weight => {
            to.font_weight = from.font_weight;
            match from.get_axis("wght") {
                Some(v) => to.set_axis("wght", v),
                None => to.remove_axis("wght"),
            }
        }
        OwnedField::Italic => to.italic = from.italic,
        OwnedField::Decoration => to.decoration = from.decoration,
        _ => {}
    }
}

/// Whether a character style differs from `base` in `field` (weight, italic or decoration).
pub fn run_overrides_field(field: OwnedField, run: &TextStyle, base: &TextStyle) -> bool {
    style_field_differs(field, run, base)
}

fn same_runs(a: &[TextRun], b: &[TextRun]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x.start, x.end) == (y.start, y.end))
}

/// The style the character at byte `pos` has: its range's, or the base style.
fn style_at(t: &TextProps, pos: usize) -> &TextStyle {
    t.runs.iter().find(|r| r.start <= pos && pos < r.end).map(|r| &r.style).unwrap_or(&t.style)
}

/// The style-owned fields in which `after` differs from `before`: the base style, every character
/// range (compared segment by segment, so splitting or merging ranges that look the same changes
/// nothing) and the paragraph settings. Colour and alignment never count, so a fill-only or an
/// alignment-only edit gives the empty set. When the text itself changed, positions no longer match: a
/// range whose style `before` did not have counts for the fields in which it differs from the base.
pub fn style_owned_fields_changed(before: &TextProps, after: &TextProps) -> OwnedFieldSet {
    let mut set = OwnedFieldSet::EMPTY;
    mark_style_diff(&mut set, &before.style, &after.style);
    if before.content == after.content {
        let len = before.content.len();
        let mut cuts = vec![0, len];
        for r in before.runs.iter().chain(&after.runs) {
            cuts.push(r.start.min(len));
            cuts.push(r.end.min(len));
        }
        cuts.sort_unstable();
        cuts.dedup();
        for w in cuts.windows(2) {
            mark_style_diff(&mut set, style_at(before, w[0]), style_at(after, w[0]));
        }
    } else {
        for r in &after.runs {
            let known = std::iter::once(&before.style).chain(before.runs.iter().map(|b| &b.style)).any(|b| {
                let mut diff = OwnedFieldSet::EMPTY;
                mark_style_diff(&mut diff, b, &r.style);
                diff.is_empty()
            });
            if !known {
                mark_style_diff(&mut set, &before.style, &r.style);
            }
        }
    }
    if before.paragraph.indent != after.paragraph.indent {
        set.insert(OwnedField::Indent);
    }
    if before.paragraph.list_spacing != after.paragraph.list_spacing {
        set.insert(OwnedField::ListSpacing);
    }
    if before.paragraph.leading_trim != after.paragraph.leading_trim {
        set.insert(OwnedField::LeadingTrim);
    }
    set
}

fn is_linked_text(n: &Node) -> bool {
    n.style_refs.contains_key("text") && n.kind.any_text().is_some()
}

/// What an edit of a text layer did to its text style link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkOutcome {
    /// The layer has no text style link.
    Unlinked,
    /// The link stays; weight, italic or decoration changes, if any, were recorded.
    Kept,
    /// The edit changed a property that is not an override, so the style was detached.
    Detached,
}

/// The one rule for an edit of a text layer's text, where `n` is the layer after the edit and `before`
/// its text before it. On a layer linked to a text style: an edit that changes any style-owned property
/// other than weight, italic and decoration detaches the style (the values stay); an edit of only those
/// keeps the link, and a change of the layer's own base style joins [`Node::text_style_overrides`]
/// (a change of one character range is the range's own override). Colour and alignment are not owned.
pub fn apply_text_link_rule(n: &mut Node, before: &TextProps) -> LinkOutcome {
    if !is_linked_text(n) {
        return LinkOutcome::Unlinked;
    }
    let Some(after) = n.kind.any_text() else { return LinkOutcome::Unlinked };
    if style_owned_fields_changed(before, after).has_non_overridable() {
        n.style_refs.remove("text");
        n.text_style_overrides = OwnedFieldSet::EMPTY;
        return LinkOutcome::Detached;
    }
    for f in STYLE_OVERRIDABLE {
        if style_field_differs(f, &before.style, &after.style) {
            n.text_style_overrides.insert(f);
        }
    }
    LinkOutcome::Kept
}

/// Records a variable bound to `key` on `n`: on a layer linked to a text style, a weight or font style
/// variable is an override of the style (the binding wins over it). Any other style-owned key is not
/// an override: its caller detaches the style.
pub fn record_typography_bind(n: &mut Node, key: &str) {
    if is_linked_text(n) && binding_key_is_overridable(key) {
        n.text_style_overrides.insert(OwnedField::Weight);
    }
}

/// The overrides `node` stores or carries on its character ranges, without checking them against the
/// style: the stored set, the weight, italic and decoration in which a range differs from the base
/// style, and a hyperlink. Empty when the layer is not a text layer linked to a text style.
pub fn raw_text_style_overrides(node: &Node) -> OwnedFieldSet {
    let Some(t) = node.kind.any_text().filter(|_| node.style_refs.contains_key("text")) else { return OwnedFieldSet::EMPTY };
    let mut set = node.text_style_overrides;
    for r in &t.runs {
        for f in STYLE_OVERRIDABLE {
            if style_field_differs(f, &r.style, &t.style) {
                set.insert(f);
            }
        }
    }
    if node.hyperlink.as_deref().is_some_and(|h| !h.is_empty()) {
        set.insert(OwnedField::Hyperlink);
    }
    set
}

/// The properties `node` really overrides in its text style: [`raw_text_style_overrides`] without the
/// stored layer-wide ones that now equal the style's own value (a bold turned off again overrides nothing).
pub fn text_style_overrides(doc: &Document, node: &Node) -> OwnedFieldSet {
    let raw = raw_text_style_overrides(node);
    let (Some(t), Some(Style::Text(style))) = (node.kind.any_text(), node.style_refs.get("text").and_then(|k| doc.styles.get(k))) else { return raw };
    let mut set = OwnedFieldSet::EMPTY;
    for f in raw.fields() {
        let bound = f == OwnedField::Weight && ["fontWeight", "fontStyle"].iter().any(|k| node.variables.bindings.contains_key(*k));
        let stored_only = node.text_style_overrides.contains(f) && !bound && !t.runs.iter().any(|r| style_field_differs(f, &r.style, &t.style));
        if !stored_only || style_field_differs(f, &t.style, style) {
            set.insert(f);
        }
    }
    set
}

/// Drops the stored overrides of `n` whose value equals the one in `style`, the text style before an edit
/// of it: such a layer follows its style (a bold turned off again), so the edit must reach it. A weight
/// bound to a variable stays an override.
pub fn drop_overrides_equal_to_style(n: &mut Node, style: &TextStyle) {
    let Some(t) = n.kind.any_text() else { return };
    let bound = ["fontWeight", "fontStyle"].iter().any(|k| n.variables.bindings.contains_key(*k));
    for f in STYLE_OVERRIDABLE {
        if n.text_style_overrides.contains(f) && !(f == OwnedField::Weight && bound) && !style_field_differs(f, &t.style, style) {
            n.text_style_overrides.remove(f);
        }
    }
}

/// Puts the overridden values of `old` back into `new`, which just took a style's values: an edit of
/// the style reaches only what a layer does not override. The base style keeps the properties in its
/// stored set. A character range keeps the weight, italic and decoration in which it differed from the
/// old base style, and follows the new base in the rest. Ranges are matched by position, and left alone
/// when `new` has a different range list.
pub fn keep_overridden_fields(new: &mut Node, old: &Node) {
    let (Some(nt), Some(ot)) = (new.kind.any_text_mut(), old.kind.any_text()) else { return };
    for f in STYLE_OVERRIDABLE {
        if old.text_style_overrides.contains(f) {
            copy_overridable_field(f, &ot.style, &mut nt.style);
        }
    }
    if same_runs(&nt.runs, &ot.runs) {
        let base = nt.style.clone();
        for (n, o) in nt.runs.iter_mut().zip(&ot.runs) {
            for f in STYLE_OVERRIDABLE {
                copy_overridable_field(f, if style_field_differs(f, &o.style, &ot.style) { &o.style } else { &base }, &mut n.style);
            }
        }
    }
    new.text_style_overrides = old.text_style_overrides;
}

/// Runs `apply` to write a text style's values onto `n`, while keeping every property `n`
/// overrides (layer-wide [`Node::text_style_overrides`] and character-range overrides). A
/// stored override whose value equals `old_style` (a bold turned off again) is dropped first
/// so the style write reaches it.
pub fn with_kept_text_style_overrides(n: &mut Node, old_style: Option<&Style>, apply: impl FnOnce(&mut Node)) -> bool {
    let before = n.clone();
    if raw_text_style_overrides(n).is_empty() {
        apply(n);
        return *n != before;
    }
    let mut kept = n.clone();
    if let Some(Style::Text(old)) = old_style {
        drop_overrides_equal_to_style(&mut kept, old);
    }
    apply(n);
    keep_overridden_fields(n, &kept);
    *n != before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(size: f32) -> TextProps {
        match Node::text(NodeId(1), Rect::new(0.0, 0.0, 100.0, 20.0), "Hello", TextStyle { font_size: size, ..Default::default() }).kind {
            NodeKind::Text(t) => t,
            _ => unreachable!(),
        }
    }

    fn linked(size: f32) -> Node {
        let mut n = Node::text(NodeId(1), Rect::new(0.0, 0.0, 100.0, 20.0), "Hello", TextStyle { font_size: size, ..Default::default() });
        n.style_refs.insert("text".into(), "Body".into());
        n
    }

    #[test]
    fn the_set_round_trips_and_lists_labels_in_display_order() {
        let mut s = OwnedFieldSet::default();
        assert!(s.is_empty());
        s.insert(OwnedField::Decoration);
        s.insert(OwnedField::Weight);
        s.insert(OwnedField::Weight);
        assert_eq!(s.labels(), vec!["Weight", "Decoration"]);
        assert_eq!(s.len(), 2);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<OwnedFieldSet>(&json).unwrap(), s);
    }

    #[test]
    fn only_four_fields_are_overridable() {
        let over: Vec<_> = OwnedField::ALL.into_iter().filter(|f| f.is_overridable()).map(OwnedField::label).collect();
        assert_eq!(over, vec!["Weight", "Italic", "Decoration", "Hyperlink"]);
        assert!(binding_key_is_overridable("fontWeight") && binding_key_is_overridable("fontStyle"));
        assert!(!binding_key_is_overridable("fontSize") && !binding_key_is_overridable("fontFamily") && !binding_key_is_overridable("lineHeight"));
    }

    #[test]
    fn colour_and_alignment_are_not_owned() {
        let before = props(16.0);
        let mut after = before.clone();
        after.style.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        after.style.align_h = TextAlignH::Center;
        assert!(style_owned_fields_changed(&before, &after).is_empty());
        after.style.font_size = 20.0;
        after.paragraph.indent = 4.0;
        assert_eq!(style_owned_fields_changed(&before, &after).labels(), vec!["Size", "Indent"]);
    }

    #[test]
    fn the_changed_fields_name_exactly_what_changed() {
        let before = props(16.0);
        let mut after = before.clone();
        after.style.letter_spacing = 1.5;
        assert_eq!(style_owned_fields_changed(&before, &after).labels(), vec!["Letter spacing"]);
        assert!(style_owned_fields_changed(&before, &after).has_non_overridable());
        let mut bold = before.clone();
        bold.style.set_font_weight(700);
        assert!(!style_owned_fields_changed(&before, &bold).has_non_overridable());
    }

    #[test]
    fn a_fill_only_range_split_or_merge_changes_nothing() {
        let mut before = props(16.0);
        before.runs.push(TextRun { start: 0, end: 2, style: TextStyle { fills: vec![Paint::solid(Color::rgb(0, 0, 255))], ..before.style.clone() } });
        let mut after = before.clone();
        after.runs = vec![TextRun { start: 0, end: 1, style: after.runs[0].style.clone() }, TextRun { start: 1, end: 2, style: after.runs[0].style.clone() }];
        assert!(style_owned_fields_changed(&before, &after).is_empty(), "same look, different range list");
        after.runs.clear();
        assert!(style_owned_fields_changed(&before, &after).is_empty(), "a colour-only range dropped");
    }

    #[test]
    fn a_change_on_one_range_names_its_field() {
        let before = props(16.0);
        let mut after = before.clone();
        after.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_weight: 700, ..after.style.clone() } });
        assert_eq!(style_owned_fields_changed(&before, &after).labels(), vec!["Weight"]);
        let mut later = after.clone();
        later.runs[0].style.font_size = 30.0;
        assert_eq!(style_owned_fields_changed(&after, &later).labels(), vec!["Size"]);
    }

    #[test]
    fn the_weight_axis_counts_under_weight_not_features() {
        let before = props(16.0);
        let mut after = before.clone();
        after.style.set_axis("wght", 550.0);
        after.style.set_font_weight(550);
        assert_eq!(style_owned_fields_changed(&before, &after).labels(), vec!["Weight"]);
    }

    #[test]
    fn the_rule_keeps_the_link_for_weight_italic_and_decoration_and_records_them() {
        let mut n = linked(16.0);
        let before = n.kind.any_text().unwrap().clone();
        if let Some(t) = n.kind.any_text_mut() {
            t.style.font_weight = 700;
            t.style.italic = true;
            t.style.decoration = TextDecoration::Underline;
        }
        assert_eq!(apply_text_link_rule(&mut n, &before), LinkOutcome::Kept);
        assert!(n.style_refs.contains_key("text"));
        assert_eq!(n.text_style_overrides.labels(), vec!["Weight", "Italic", "Decoration"]);
    }

    #[test]
    fn the_rule_detaches_for_every_other_owned_field_and_for_a_mixed_edit() {
        for edit in [
            (|s: &mut TextStyle| s.font_size = 24.0) as fn(&mut TextStyle),
            |s| s.line_height = LineHeight::Pixels(30.0),
            |s| s.letter_spacing = 2.0,
            |s| s.paragraph_spacing = 8.0,
            |s| s.font_family = "Serif".into(),
            |s| s.case = TextCase::Upper,
            |s| {
                s.font_weight = 700;
                s.font_size = 24.0;
            },
        ] {
            let mut n = linked(16.0);
            let before = n.kind.any_text().unwrap().clone();
            edit(&mut n.kind.any_text_mut().unwrap().style);
            assert_eq!(apply_text_link_rule(&mut n, &before), LinkOutcome::Detached);
            assert!(n.style_refs.is_empty() && n.text_style_overrides.is_empty());
        }
    }

    #[test]
    fn a_hyperlink_on_a_linked_text_is_an_override_and_other_texts_have_none() {
        let mut n = linked(16.0);
        assert!(raw_text_style_overrides(&n).is_empty());
        n.hyperlink = Some("https://example.com".into());
        assert_eq!(raw_text_style_overrides(&n).labels(), vec!["Hyperlink"]);
        n.style_refs.clear();
        assert!(raw_text_style_overrides(&n).is_empty(), "no style, nothing to override");
    }

    #[test]
    fn a_range_that_differs_from_the_base_is_an_override_without_a_stored_bit() {
        let mut n = linked(16.0);
        n.kind.any_text_mut().unwrap().runs.push(TextRun {
            start: 0,
            end: 2,
            style: TextStyle { font_weight: 700, ..TextStyle { font_size: 16.0, ..Default::default() } },
        });
        assert!(n.text_style_overrides.is_empty());
        assert_eq!(raw_text_style_overrides(&n).labels(), vec!["Weight"]);
    }

    #[test]
    fn a_stored_override_that_equals_the_style_again_is_not_shown() {
        let mut d = Document::new("t");
        d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 16.0, ..Default::default() }));
        let mut n = linked(16.0);
        n.text_style_overrides.insert(OwnedField::Weight);
        assert!(text_style_overrides(&d, &n).is_empty(), "the weight equals the style's");
        n.kind.any_text_mut().unwrap().style.font_weight = 700;
        assert_eq!(text_style_overrides(&d, &n).labels(), vec!["Weight"]);
    }
}
