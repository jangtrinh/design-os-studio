//! The layers panel name filter: which rows stay visible and where the matched
//! characters are, plus the filter field itself.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::panels::layers::Row;
use crate::theme;
use crate::widgets::{self, field};
use designos_doc::find::{FindOptions, Matcher};
use designos_doc::{Document, NodeId};
use egui::{Align, Key, Margin, Pos2, Rect, Sense, TextEdit, Ui, Vec2};
use std::ops::Range;

/// Compiled filter, or `None` when the filter is empty.
pub fn matcher(filter: &str) -> Option<Matcher> {
    Matcher::new(filter.trim(), FindOptions::default()).ok()
}

/// Byte ranges of `name` that match the filter, for highlighting.
pub fn highlights(m: &Matcher, name: &str) -> Vec<Range<usize>> {
    m.find_all(name).into_iter().map(|(r, _)| r).collect()
}

/// Rows of the layers tree that match the filter or contain a match, top-most
/// first. Ancestors of matches are always shown; their other children are not.
pub fn rows(doc: &Document, page: NodeId, m: &Matcher) -> Vec<Row> {
    fn rec(doc: &Document, id: NodeId, depth: usize, m: &Matcher, out: &mut Vec<Row>) -> bool {
        let Some(n) = doc.get(id) else { return false };
        let at = out.len();
        let mut below = false;
        for c in n.children.iter().rev() {
            below |= rec(doc, *c, depth + 1, m, out);
        }
        let own = !m.find_all(&n.name).is_empty();
        if own || below {
            if depth > 0 {
                // The row goes before its descendants.
                out.insert(at, Row { id, depth: depth - 1, has_children: false });
            }
            return true;
        }
        false
    }
    let mut out = Vec::new();
    rec(doc, page, 0, m, &mut out);
    out
}

/// Paints `ranges` of `name` highlighted, behind the text that starts at `x`.
pub fn paint_highlights(p: &egui::Painter, name: &str, ranges: &[Range<usize>], x: f32, row: Rect, font: &egui::FontId, max_x: f32) {
    for r in ranges {
        let a = x + widgets::text_width(p, &name[..r.start], font);
        let b = x + widgets::text_width(p, &name[..r.end], font);
        if a >= max_x {
            break;
        }
        let hl = Rect::from_min_max(Pos2::new(a, row.center().y - 9.0), Pos2::new(b.min(max_x), row.center().y + 9.0));
        p.rect_filled(hl, theme::cr(theme::RADIUS_MARK), theme::FIND_HIGHLIGHT);
    }
}

/// The filter field under the "Layers" header: search icon, text, clear button.
/// Escape (or the clear button on an empty field) closes it.
pub fn show_field(app: &mut App, ui: &mut Ui, row: Rect) {
    let p = ui.painter().clone();
    let r = Rect::from_min_max(Pos2::new(row.left() + 8.0, row.top() + 2.0), Pos2::new(row.right() - 8.0, row.bottom() - 2.0));
    let id = ui.id().with("layer-filter");
    field::field_bg(ui, r, id);
    icons::paint(&p, Icon::Search, Pos2::new(r.left() + 14.0, r.center().y), 14.0, theme::ICON_SECONDARY);
    let clear = Rect::from_center_size(Pos2::new(r.right() - 14.0, r.center().y), Vec2::splat(20.0));
    let text_rect = Rect::from_min_max(Pos2::new(r.left() + 26.0, r.top()), Pos2::new(clear.left() - 2.0, r.bottom()));
    if app.layers.filter_focus {
        // Keep asking until the field has it: the click on the header icon that opened the
        // filter can still take the focus away on the frame it happens.
        if ui.memory(|m| m.has_focus(id)) {
            app.layers.filter_focus = false;
        } else {
            ui.memory_mut(|m| m.request_focus(id));
        }
    }
    let esc = ui.memory(|m| m.has_focus(id)) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
    let font = theme::font_body();
    let te = TextEdit::singleline(&mut app.layers.filter)
        .id(id)
        .font(font.clone())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .vertical_align(Align::Center)
        .hint_text(egui::RichText::new("Filter layers by name").font(font).color(theme::TEXT_TERTIARY))
        .desired_width(text_rect.width());
    ui.put(text_rect, te);
    let has_text = !app.layers.filter.is_empty();
    if has_text {
        let cr = ui.interact(clear, ui.id().with("layer-filter-clear"), Sense::click());
        icons::paint(&p, Icon::Close, clear.center(), 12.0, if cr.hovered() { theme::ICON } else { theme::ICON_SECONDARY });
        if cr.clicked() {
            app.layers.filter.clear();
            ui.memory_mut(|m| m.request_focus(id));
        }
    }
    if esc {
        app.close_layer_filter();
    }
}

impl App {
    /// Shows the layers name filter and focuses its field.
    pub fn open_layer_filter(&mut self) {
        self.layers.filter_open = true;
        self.layers.filter_focus = true;
    }

    /// Hides the filter and shows the whole tree again.
    pub fn close_layer_filter(&mut self) {
        self.layers.filter_open = false;
        self.layers.filter_focus = false;
        self.layers.filter.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect as DRect};

    fn sample() -> (Document, NodeId, [NodeId; 5]) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let add = |d: &mut Document, parent: NodeId, name: &str, frame: bool| {
            let id = d.alloc_id();
            let r = DRect::new(0.0, 0.0, 10.0, 10.0);
            let mut n = if frame { Node::frame(id, r) } else { Node::rectangle(id, r) };
            n.name = name.into();
            d.insert(parent, n).unwrap()
        };
        let header = add(&mut d, page, "Header", true);
        let logo = add(&mut d, header, "Logo mark", false);
        let title = add(&mut d, header, "Title", false);
        let footer = add(&mut d, page, "Footer", true);
        let logo2 = add(&mut d, footer, "LOGO small", false);
        (d, page, [header, logo, title, footer, logo2])
    }

    #[test]
    fn filter_keeps_matches_and_their_ancestors_only() {
        let (d, page, [header, logo, _title, footer, logo2]) = sample();
        let m = matcher("logo").unwrap();
        let rows = rows(&d, page, &m);
        let ids: Vec<NodeId> = rows.iter().map(|r| r.id).collect();
        // Top-most first: Footer (and its match) above Header (and its match); "Title" is dropped.
        assert_eq!(ids, vec![footer, logo2, header, logo]);
        assert_eq!(rows.iter().map(|r| r.depth).collect::<Vec<_>>(), vec![0, 1, 0, 1]);
        assert!(rows.iter().all(|r| !r.has_children));
    }

    #[test]
    fn filter_is_case_insensitive_plain_text_and_empty_means_none() {
        let (d, page, _) = sample();
        assert!(matcher("").is_none() && matcher("   ").is_none());
        assert!(rows(&d, page, &matcher("zzz").unwrap()).is_empty());
        // Regex characters are literal in the name filter.
        assert!(rows(&d, page, &matcher("l.go").unwrap()).is_empty());
        assert_eq!(rows(&d, page, &matcher("TITLE").unwrap()).len(), 2);
    }

    #[test]
    fn highlight_ranges_are_char_aligned() {
        let m = matcher("é").unwrap();
        assert_eq!(highlights(&m, "Café É"), vec![3..5, 6..8]);
    }
}
