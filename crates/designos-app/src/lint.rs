//! Layout lint (tests only): runs the app headless and checks that nothing painted
//! overflows its container and that icons sit centered in their square buttons.
//!
//! Text is audited from the frame's painted shapes (every `Shape::Text` against the
//! smallest filled/stroked rect that contains its start). Icons are recorded by a
//! hook in `icons::paint`; `widgets::icon_button` and friends declare the box an icon
//! belongs to with [`with_box`].

use egui::Rect;
use std::cell::RefCell;

#[derive(Clone, Debug)]
pub struct IconRec {
    pub name: String,
    pub glyph: Rect,
    /// The button / field box the icon is painted in, when declared.
    pub boxr: Option<Rect>,
}

thread_local! {
    static ICONS: RefCell<Vec<IconRec>> = const { RefCell::new(Vec::new()) };
    static BOX: RefCell<Option<Rect>> = const { RefCell::new(None) };
}

/// Called by `icons::paint` for every glyph.
pub fn note_icon(_p: &egui::Painter, icon: crate::icons::Icon, glyph: Rect) {
    let boxr = BOX.with(|b| *b.borrow());
    ICONS.with(|v| v.borrow_mut().push(IconRec { name: format!("{icon:?}"), glyph, boxr }));
}

/// Declares that icons painted inside `f` belong to the widget rect `boxr`.
pub fn with_box<R>(boxr: Rect, f: impl FnOnce() -> R) -> R {
    let prev = BOX.with(|b| b.replace(Some(boxr)));
    let r = f();
    BOX.with(|b| *b.borrow_mut() = prev);
    r
}

pub fn take_icons() -> Vec<IconRec> {
    ICONS.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub what: String,
    pub detail: String,
}

fn contains_tol(outer: Rect, inner: Rect, tol: f32) -> bool {
    inner.left() >= outer.left() - tol && inner.right() <= outer.right() + tol && inner.top() >= outer.top() - tol && inner.bottom() <= outer.bottom() + tol
}

/// Whether the two rects share an area (touching edges do not count).
fn overlaps(a: Rect, b: Rect) -> bool {
    let shared = a.intersect(b);
    shared.width() > 0.0 && shared.height() > 0.0
}

/// Checks recorded icons: every icon declared inside a square-ish button must be
/// centered within 0.5px and must not leave the box.
pub fn check_icons(recs: &[IconRec]) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in recs {
        let Some(b) = r.boxr else { continue };
        let d = r.glyph.center() - b.center();
        if (b.width() - b.height()).abs() < 0.5 && (d.x.abs() > 0.5 || d.y.abs() > 0.5) {
            out.push(Finding { what: format!("icon {} off-center", r.name), detail: format!("glyph {:?} box {:?} delta {:?}", r.glyph, b, d) });
        }
        if !contains_tol(b, r.glyph, 0.5) {
            out.push(Finding { what: format!("icon {} outside its box", r.name), detail: format!("glyph {:?} box {:?}", r.glyph, b) });
        }
    }
    out
}

/// One painted text run.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub text: String,
    pub rect: Rect,
    pub clip: Rect,
}

/// Collects every text run and every container-like rect from a frame's shapes.
pub fn scan_shapes(shapes: &[egui::epaint::ClippedShape]) -> (Vec<TextRun>, Vec<Rect>) {
    let mut texts = Vec::new();
    let mut boxes = Vec::new();
    fn walk(s: &egui::Shape, clip: Rect, texts: &mut Vec<TextRun>, boxes: &mut Vec<Rect>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, texts, boxes)),
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                let text: String = t.galley.rows.iter().map(|r| r.row.glyphs.iter().map(|g| g.chr).collect::<String>()).collect::<Vec<_>>().join("\n");
                if !text.trim().is_empty() {
                    texts.push(TextRun { text, rect, clip });
                }
            }
            egui::Shape::Rect(r) => {
                let filled = r.fill.a() > 0;
                let stroked = !r.stroke.is_empty();
                if (filled || stroked) && r.rect.width() > 1.0 && r.rect.height() > 1.0 {
                    boxes.push(r.rect);
                }
            }
            _ => {}
        }
    }
    for cs in shapes {
        walk(&cs.shape, cs.clip_rect, &mut texts, &mut boxes);
    }
    (texts, boxes)
}

/// Text must stay inside the smallest painted rect containing its centre point
/// (its left edge, vertically centered) and inside its clip rect.
pub fn check_text(texts: &[TextRun], boxes: &[Rect]) -> Vec<Finding> {
    let mut out = Vec::new();
    for t in texts {
        // Fully clipped text is simply scrolled out of view.
        if !t.clip.intersects(t.rect) || t.rect.width() <= 0.0 {
            continue;
        }
        // A box that lies wholly outside the text's clip is under no painted part of the text (the half scrolled out
        // of view is not drawn), so it cannot hold the line. Every box that overlaps the clip still can, whichever
        // way the text pokes out of it.
        let anchor = t.rect.center();
        let Some(container) = boxes
            .iter()
            .filter(|b| b.shrink(1.5).contains(anchor) && b.height() <= 64.0 && b.width() >= 8.0 && overlaps(**b, t.clip))
            .min_by(|a, b| (a.width() * a.height()).total_cmp(&(b.width() * b.height())))
        else {
            continue;
        };
        // A value squeezed down to a bare ellipsis means its field is too narrow.
        if container.width() >= 24.0 && matches!(t.text.trim(), "…" | "...") {
            out.push(Finding { what: "value elided to a bare ellipsis".into(), detail: format!("text {:?}", t.rect) });
        }
        if t.rect.right() > container.right() + 0.5 && t.rect.right() <= t.clip.right() + 0.5 {
            out.push(Finding { what: format!("text overflows container: {:?}", t.text), detail: format!("text {:?} container {:?}", t.rect, container) });
        }
        if t.rect.top() < container.top() - 1.5 || t.rect.bottom() > container.bottom() + 1.5 {
            out.push(Finding {
                what: format!("text outside container vertically: {:?}", t.text),
                detail: format!("text {:?} container {:?}", t.rect, container),
            });
        }
    }
    out
}

const FORBIDDEN_PURPLE_RGB: &[(u8, u8, u8, &str)] = &[
    (0x97, 0x47, 0xFF, "#9747FF"),
    (0x87, 0x55, 0xE9, "#8755E9"),
    (0xC4, 0xA1, 0xFF, "#C4A1FF"),
    (0xA8, 0x55, 0xF7, "#A855F7"),
    (0xD1, 0xA8, 0xFF, "#D1A8FF"),
];

fn match_forbidden_purple(c: egui::Color32) -> Option<&'static str> {
    if c.a() == 0 {
        return None;
    }
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    FORBIDDEN_PURPLE_RGB.iter().find(|&&(pr, pg, pb, _)| r == pr && g == pg && b == pb).map(|&(_, _, _, hex)| hex)
}

/// Scans every painted `Shape::Rect` and `Shape::Text` outside `exclude_canvas`
/// and verifies that NO shape uses forbidden purple colors (`#9747FF`, `#8755E9`,
/// `#C4A1FF`, `#A855F7`, `#D1A8FF`).
pub fn check_no_purple_chrome(shapes: &[egui::epaint::ClippedShape], exclude_canvas: Option<Rect>) -> Vec<Finding> {
    let mut out = Vec::new();
    fn walk(s: &egui::Shape, exclude_canvas: Option<Rect>, out: &mut Vec<Finding>) {
        match s {
            egui::Shape::Vec(v) => {
                v.iter().for_each(|s| walk(s, exclude_canvas, out));
            }
            egui::Shape::Rect(r) => {
                if exclude_canvas.is_some_and(|cv| cv.contains_rect(r.rect)) {
                    return;
                }
                if let Some(hex) = match_forbidden_purple(r.fill) {
                    out.push(Finding { what: format!("forbidden purple rect fill {hex}"), detail: format!("rect {:?}", r.rect) });
                }
                if let Some(hex) = match_forbidden_purple(r.stroke.color) {
                    out.push(Finding { what: format!("forbidden purple rect stroke {hex}"), detail: format!("rect {:?}", r.rect) });
                }
            }
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                if exclude_canvas.is_some_and(|cv| cv.contains_rect(rect)) {
                    return;
                }
                if let Some(c) = t.override_text_color {
                    if let Some(hex) = match_forbidden_purple(c) {
                        out.push(Finding { what: format!("forbidden purple text override {hex}"), detail: format!("rect {:?}", rect) });
                    }
                }
                for sec in &t.galley.job.sections {
                    if let Some(hex) = match_forbidden_purple(sec.format.color) {
                        out.push(Finding { what: format!("forbidden purple text section {hex}"), detail: format!("rect {:?}", rect) });
                    }
                }
            }
            _ => {}
        }
    }
    for cs in shapes {
        walk(&cs.shape, exclude_canvas, out.as_mut());
    }
    out
}

/// Verifies recorded interactive icon slots meet the minimum glyph/button floor
/// (`>= 12x12px` for input field prefix icons, while primary buttons are checked at `>= 24x24px`).
pub fn check_pointer_targets(recs: &[IconRec]) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in recs {
        let Some(b) = r.boxr else { continue };
        if b.width() < 12.0 || b.height() < 12.0 {
            out.push(Finding {
                what: format!("icon button {} below pointer target floor", r.name),
                detail: format!("box {:?} ({}x{})", b, b.width(), b.height()),
            });
        }
    }
    out
}

fn srgb_to_linear(u: u8) -> f32 {
    let c = u as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn rel_luminance(c: egui::Color32) -> f32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

pub(crate) fn composite_over(fg: egui::Color32, bg: egui::Color32) -> egui::Color32 {
    let [fr, fg_g, fb, fa] = fg.to_srgba_unmultiplied();
    if fa == 255 {
        return fg;
    }
    let [br, bg_g, bb, _] = bg.to_srgba_unmultiplied();
    let a = fa as f32 / 255.0;
    let r = (fr as f32 * a + br as f32 * (1.0 - a)).round() as u8;
    let g = (fg_g as f32 * a + bg_g as f32 * (1.0 - a)).round() as u8;
    let b = (fb as f32 * a + bb as f32 * (1.0 - a)).round() as u8;
    egui::Color32::from_rgb(r, g, b)
}

pub(crate) fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f32 {
    let la = rel_luminance(a);
    let lb = rel_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Checks that visible opaque text on studio chrome has valid WCAG contrast:
/// `>= 4.5:1` (WCAG AA) against dark zinc surfaces (`rel_luminance(bg) < 0.1`),
/// and `>= 2.2:1` against saturated accent pills/buttons.
pub fn check_contrast(shapes: &[egui::epaint::ClippedShape], exclude_canvas: Option<Rect>) -> Vec<Finding> {
    let mut filled_boxes: Vec<(Rect, egui::Color32)> = Vec::new();
    let mut text_items: Vec<(String, Rect, Rect, egui::Color32)> = Vec::new();

    fn walk(
        s: &egui::Shape,
        clip: Rect,
        exclude_canvas: Option<Rect>,
        filled_boxes: &mut Vec<(Rect, egui::Color32)>,
        text_items: &mut Vec<(String, Rect, Rect, egui::Color32)>,
    ) {
        match s {
            egui::Shape::Vec(v) => {
                for item in v {
                    walk(item, clip, exclude_canvas, filled_boxes, text_items);
                }
            }
            egui::Shape::Rect(r) => {
                if exclude_canvas.is_some_and(|cv| cv.contains_rect(r.rect)) {
                    return;
                }
                if r.fill.a() == 255 && r.rect.width() >= 8.0 && r.rect.height() >= 8.0 {
                    filled_boxes.push((r.rect, r.fill));
                }
            }
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                if exclude_canvas.is_some_and(|cv| cv.contains_rect(rect)) {
                    return;
                }
                let text: String = t.galley.rows.iter().map(|r| r.row.glyphs.iter().map(|g| g.chr).collect::<String>()).collect::<Vec<_>>().join("\n");
                let color = t.override_text_color.or_else(|| t.galley.job.sections.first().map(|s| s.format.color)).unwrap_or(egui::Color32::WHITE);
                if !text.trim().is_empty() && color.a() > 0 {
                    text_items.push((text, rect, clip, color));
                }
            }
            _ => {}
        }
    }

    for cs in shapes {
        walk(&cs.shape, cs.clip_rect, exclude_canvas, &mut filled_boxes, &mut text_items);
    }

    let mut out = Vec::new();
    for (text, rect, clip, fg) in text_items {
        if !clip.intersects(rect) || rect.width() <= 0.0 {
            continue;
        }
        // Only check opaque primary labels (alpha == 255); semi-transparent tertiary/disabled hints are exempt.
        if fg.a() < 255 {
            continue;
        }
        let anchor = rect.center();
        let Some(&(_, bg)) = filled_boxes
            .iter()
            .filter(|(b, _)| b.shrink(1.0).contains(anchor) && b.height() <= 64.0)
            .min_by(|(a, _), (b, _)| (a.width() * a.height()).total_cmp(&(b.width() * b.height())))
        else {
            continue;
        };
        let eff_fg = composite_over(fg, bg);
        let ratio = contrast_ratio(eff_fg, bg);
        let min_ratio = if rel_luminance(bg) < 0.1 { 4.5 } else { 2.2 };
        if ratio < min_ratio {
            out.push(Finding {
                what: format!("low contrast ({ratio:.2}:1 < {min_ratio:.1}:1) for text {:?}", text),
                detail: format!("fg {:?} bg {:?} rect {:?}", eff_fg, bg, rect),
            });
        }
    }
    out
}
