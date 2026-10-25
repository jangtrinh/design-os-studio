//! Inspector "Export" section: per-layer export rows (scale + format), a
//! preview thumbnail, the export button and a "Copy as" context menu.

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::field;
use crate::widgets::menu::{dropdown, popup_menu, MenuItem, MenuResult};
use crate::widgets::{self, elide};
use designos_doc::{Document, ExportConstraint, ExportFileFormat, ExportPreset, NodeId, Rect as DRect};
use designos_export::{ExportFormat, ExportScale, ExportSetting};
use egui::{Align2, Id, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

const ROW_GAP: f32 = 8.0;
const PREVIEW_H: f32 = 120.0;
// scale | suffix | format | remove, filling the 184px content width.
const COL_GAP: f32 = 4.0;
const SCALE_W: f32 = 56.0;
const SUFFIX_W: f32 = 36.0;
const FORMAT_W: f32 = 56.0;
const THUMB_CHECK_SECONDS: f64 = 0.3;

/// What "Copy as" can put on the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyKind {
    Png,
    Svg,
    Css,
}

pub const COPY_ITEMS: [(CopyKind, &str); 3] = [(CopyKind::Png, "Copy as PNG"), (CopyKind::Svg, "Copy as SVG"), (CopyKind::Css, "Copy as CSS")];

#[derive(Clone)]
struct Thumb {
    tex: TextureHandle,
    node: NodeId,
    fingerprint: u64,
    checked: f64,
}

/// The inspector's scale parsing ("2x", "512w", "512h") as the stored constraint.
pub fn constraint_of(s: ExportScale) -> ExportConstraint {
    match s {
        ExportScale::Factor(f) => ExportConstraint::Scale(f),
        ExportScale::Width(w) => ExportConstraint::Width(w as f32),
        ExportScale::Height(h) => ExportConstraint::Height(h as f32),
    }
}

/// Inverse of [`constraint_of`].
pub fn scale_of(c: ExportConstraint) -> ExportScale {
    match c {
        ExportConstraint::Scale(f) => ExportScale::Factor(f),
        ExportConstraint::Width(w) => ExportScale::Width(w.round().max(1.0) as u32),
        ExportConstraint::Height(h) => ExportScale::Height(h.round().max(1.0) as u32),
    }
}

fn format_of(f: ExportFileFormat) -> ExportFormat {
    match f {
        ExportFileFormat::Png => ExportFormat::Png,
        ExportFileFormat::Jpg => ExportFormat::Jpg,
        ExportFileFormat::Svg => ExportFormat::Svg,
        ExportFileFormat::Pdf => ExportFormat::Pdf,
    }
}

fn file_format_of(f: ExportFormat) -> ExportFileFormat {
    match f {
        ExportFormat::Png => ExportFileFormat::Png,
        ExportFormat::Jpg => ExportFileFormat::Jpg,
        ExportFormat::Svg => ExportFileFormat::Svg,
        ExportFormat::Pdf => ExportFileFormat::Pdf,
    }
}

/// Renderable form of a stored preset.
pub fn setting_of(p: &ExportPreset) -> ExportSetting {
    ExportSetting { scale: scale_of(p.constraint), format: format_of(p.format) }
}

/// Output file name: layer name + the preset's suffix (or the `@2x` style default).
pub fn preset_file_name(p: &ExportPreset, layer: &str) -> String {
    let s = setting_of(p);
    let name = s.file_name(layer);
    if p.suffix.is_empty() {
        return name;
    }
    let ext = s.format.extension();
    format!("{}{}.{ext}", designos_export::sanitize_file_name(layer), p.suffix)
}

/// Rows every selected node has (in the first node's order). With one node: all its rows.
pub fn shared_rows(doc: &Document, sel: &[NodeId]) -> Vec<ExportPreset> {
    let Some(first) = sel.first().and_then(|id| doc.get(*id)) else { return vec![] };
    first.export_settings.iter().filter(|p| sel[1..].iter().take(128).all(|id| doc.get(*id).is_some_and(|n| n.export_settings.contains(p)))).cloned().collect()
}

/// One edit of the shared rows.
#[derive(Clone, Debug, PartialEq)]
pub enum RowEdit {
    Add(ExportPreset),
    Replace(ExportPreset, ExportPreset),
    Remove(ExportPreset),
}

impl App {
    /// Applies a row edit to every node in `sel` as one undo step.
    pub fn edit_export_rows(&mut self, sel: &[NodeId], edit: &RowEdit) {
        self.edit_nodes(sel, |n| match edit {
            RowEdit::Add(p) => n.export_settings.push(p.clone()),
            RowEdit::Replace(old, new) => {
                if let Some(slot) = n.export_settings.iter_mut().find(|p| *p == old) {
                    *slot = new.clone();
                }
            }
            RowEdit::Remove(old) => {
                if let Some(i) = n.export_settings.iter().position(|p| p == old) {
                    n.export_settings.remove(i);
                }
            }
        });
    }
}

fn fingerprint(doc: &Document, root: NodeId) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut stack = vec![root];
    let mut visited = 0usize;
    while let Some(id) = stack.pop() {
        if visited >= 64 {
            break;
        }
        let Some(n) = doc.get(id) else { continue };
        visited += 1;
        format!("{n:?}").hash(&mut h);
        for &c in n.children.iter().rev().take(16) {
            stack.push(c);
        }
    }
    for (k, v) in doc.images.iter().take(32) {
        k.hash(&mut h);
        v.len().hash(&mut h);
    }
    h.finish()
}

fn thumbnail(app: &App, ui: &Ui, node: NodeId) -> Option<Thumb> {
    // On massive 20k+ node documents, skip synchronous software CPU rasterization of multi-child containers on selection.
    if app.doc.nodes.len() > 20_000 && app.doc.get(node).is_some_and(|n| n.children.len() > 16) {
        return None;
    }
    let key = Id::new("export-thumb");
    let ctx = ui.ctx();
    let cached = ctx.data(|d| d.get_temp::<Thumb>(key));
    if let Some(t) = &cached {
        if t.node == node && app.now - t.checked < THUMB_CHECK_SECONDS {
            return cached;
        }
    }
    let fp = fingerprint(&app.doc, node);
    if let Some(mut t) = cached {
        if t.node == node && t.fingerprint == fp {
            t.checked = app.now;
            ctx.data_mut(|d| d.insert_temp(key, t.clone()));
            return Some(t);
        }
    }
    let b: DRect = designos_export::render_bounds(&app.export_doc(), node);
    if b.w <= 0.0 || b.h <= 0.0 {
        return None;
    }
    // Render at 2x the preview box so it stays sharp on hi-dpi screens.
    let scale = (theme::SECTION_CONTENT_W * 2.0 / b.w).min(PREVIEW_H * 2.0 / b.h).clamp(0.01, 2.0);
    let pm = designos_export::render_export(&app.export_doc(), node, scale)?;
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let tex = ctx.load_texture("export-preview", img, TextureOptions::LINEAR);
    let t = Thumb { tex, node, fingerprint: fp, checked: app.now };
    ctx.data_mut(|d| d.insert_temp(key, t.clone()));
    Some(t)
}

/// Draws the Export section starting at absolute `(x0, top)` and returns the
/// height it used. `x0` is the panel's outer left edge.
pub fn section(app: &mut App, ui: &mut Ui, x0: f32, top: f32, sel: &[NodeId]) -> f32 {
    let Some(&first) = sel.first() else { return 0.0 };
    let name = app.doc.get(first).map(|n| n.name.clone()).unwrap_or_default();
    let rows = shared_rows(&app.doc, sel);
    let any_rows = sel.iter().take(128).any(|id| app.doc.get(*id).is_some_and(|n| !n.export_settings.is_empty()));
    let mut edit: Option<RowEdit> = None;
    let at = |x: f32, dy: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(x0 + x, top + dy), Vec2::new(w, h));
    let mut y = 0.0;

    // Title + add button.
    let title = at(0.0, 0.0, theme::RIGHT_PANEL_WIDTH - 1.0, theme::SECTION_TITLE_HEIGHT);
    ui.painter().text(
        Pos2::new(title.left() + theme::SECTION_CONTENT_X, title.center().y),
        Align2::LEFT_CENTER,
        "Export",
        theme::font_body_strong(),
        theme::TEXT,
    );
    // Created before the button so the button sits on top of it.
    let title_resp = ui.interact(title, ui.id().with("export-title"), Sense::click());
    let plus = at(theme::SECTION_TRAILING_X, 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "title-action:Export", plus);
    widgets::record(ui, "export-add", plus);
    if widgets::icon_button(ui, plus, "export-add", Icon::Plus, false, "Add export setting").clicked() {
        edit = Some(RowEdit::Add(rows.last().cloned().unwrap_or_default()));
    }
    y += theme::SECTION_TITLE_HEIGHT;

    // Rows (the ones every selected layer has).
    for (i, old) in rows.iter().enumerate() {
        let mut row = old.clone();
        let mut changed = false;
        let scale_r = at(theme::SECTION_CONTENT_X, y, SCALE_W, theme::FIELD_HEIGHT);
        let suffix_r = at(theme::SECTION_CONTENT_X + SCALE_W + COL_GAP, y, SUFFIX_W, theme::FIELD_HEIGHT);
        let fmt_r = at(theme::SECTION_CONTENT_X + SCALE_W + SUFFIX_W + COL_GAP * 2.0, y, FORMAT_W, theme::FIELD_HEIGHT);
        let minus =
            at(theme::SECTION_CONTENT_X + theme::SECTION_CONTENT_W - theme::PANEL_ACTION_BUTTON, y, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);

        // Scale: editable text ("2x", "512w") with a presets menu behind the chevron.
        let text_r = Rect::from_min_max(scale_r.min, Pos2::new(scale_r.right() - 20.0, scale_r.bottom()));
        let chev_r = Rect::from_min_max(Pos2::new(scale_r.right() - 20.0, scale_r.top()), scale_r.max);
        let fid = ui.id().with(("export-scale", first, i));
        field::field_bg(ui, scale_r, fid);
        if let Some(s) = field::edit_text(ui, text_r, fid, &scale_of(row.constraint).to_string(), theme::font_body(), theme::TEXT, 4.0) {
            if let Some(sc) = ExportScale::parse(&s) {
                row.constraint = constraint_of(sc);
                changed = true;
            }
        }
        let chev = ui.interact(chev_r, ui.id().with(("export-scale-chev", first, i)), Sense::click());
        icons::paint(ui.painter(), Icon::ChevronDown, chev_r.center(), 12.0, if chev.hovered() { theme::ICON } else { theme::ICON_SECONDARY });
        let menu_id = ui.id().with(("export-scale-menu", first, i));
        let open = ui.data(|d| d.get_temp::<bool>(menu_id)).unwrap_or(false);
        if chev.clicked() {
            ui.data_mut(|d| d.insert_temp(menu_id, !open));
        }
        if open {
            let cur = scale_of(row.constraint);
            let items: Vec<MenuItem> = ExportScale::PRESETS.iter().map(|s| MenuItem::new(s.to_string()).checked(*s == cur)).collect();
            match popup_menu(ui.ctx(), menu_id.with("popup"), scale_r.left_bottom() + Vec2::new(0.0, 2.0), scale_r.width().max(64.0), &items, Some(scale_r)) {
                MenuResult::Picked(p) => {
                    row.constraint = constraint_of(ExportScale::PRESETS[p]);
                    changed = true;
                    ui.data_mut(|d| d.insert_temp(menu_id, false));
                }
                MenuResult::Closed => {
                    ui.data_mut(|d| d.insert_temp(menu_id, false));
                }
                MenuResult::Open => {}
            }
        }

        // Suffix: empty means the automatic "@2x" style name.
        let sid = ui.id().with(("export-suffix", first, i));
        field::field_bg(ui, suffix_r, sid);
        if let Some(s) = field::edit_text(ui, suffix_r, sid, &row.suffix, theme::font_body(), theme::TEXT, 6.0) {
            row.suffix = s.trim().to_string();
            changed = true;
        }
        if row.suffix.is_empty() && !ui.memory(|m| m.has_focus(sid)) {
            ui.painter().text(Pos2::new(suffix_r.left() + 6.0, suffix_r.center().y), Align2::LEFT_CENTER, "Suffix", theme::font_body(), theme::TEXT_TERTIARY);
        }

        let options: Vec<String> = ExportFormat::ALL.iter().map(|f| f.label().to_string()).collect();
        if let Some(p) = dropdown(ui, fmt_r, ("export-format", first, i), format_of(row.format).label(), &options) {
            row.format = file_format_of(ExportFormat::ALL[p]);
            changed = true;
        }
        if widgets::icon_button(ui, minus, ("export-remove", i), Icon::Minus, false, "Remove export setting").clicked() {
            edit = Some(RowEdit::Remove(old.clone()));
        } else if changed && row != *old {
            edit = Some(RowEdit::Replace(old.clone(), row));
        }
        y += theme::FIELD_HEIGHT + ROW_GAP;
    }
    if let Some(e) = edit.take() {
        app.edit_export_rows(sel, &e);
    }
    if rows.is_empty() && any_rows {
        // Layers disagree and share no row: say so, as Figma does.
        ui.painter().text(
            Pos2::new(x0 + theme::SECTION_CONTENT_X, top + y + theme::FIELD_HEIGHT / 2.0),
            Align2::LEFT_CENTER,
            "Mixed",
            theme::font_body(),
            theme::TEXT_TERTIARY,
        );
        y += theme::FIELD_HEIGHT + ROW_GAP;
    }
    if !any_rows {
        context_menu(app, ui, &title_resp, first);
        return y + 8.0;
    }

    // Preview.
    let prev = at(theme::SECTION_CONTENT_X, y, theme::SECTION_CONTENT_W, PREVIEW_H);
    ui.painter().rect_filled(prev, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    if let Some(t) = thumbnail(app, ui, first) {
        // The texture holds up to 2x the logical size; never show it larger than that.
        let sz = t.tex.size_vec2();
        let k = ((prev.width() - 16.0) / sz.x).min((prev.height() - 16.0) / sz.y).min(0.5);
        let draw = Rect::from_center_size(prev.center(), sz * k);
        ui.painter().image(t.tex.id(), draw, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
    }
    let prev_resp = ui.interact(prev, ui.id().with("export-preview-hit"), Sense::click());
    y += PREVIEW_H + ROW_GAP;

    // Export button.
    let btn = at(theme::SECTION_CONTENT_X, y, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    widgets::record(ui, "export-btn", btn);
    let resp = ui.interact(btn, ui.id().with("export-btn"), Sense::click());
    ui.painter().rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    icons::paint(ui.painter(), Icon::Export, Pos2::new(btn.left() + 16.0, btn.center().y), 14.0, theme::ICON_SECONDARY);
    let label = if sel.len() > 1 { format!("Export {} layers", sel.len()) } else { format!("Export {name}") };
    let label = elide(ui.painter(), &label, &theme::font_body(), btn.width() - 44.0);
    ui.painter().text(Pos2::new(btn.left() + 32.0, btn.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
    if resp.clicked() {
        run_export(app, sel);
    }
    context_menu(app, ui, &resp, first);
    context_menu(app, ui, &prev_resp, first);
    context_menu(app, ui, &title_resp, first);
    y + theme::FIELD_HEIGHT + 12.0
}

/// Opens/draws the "Copy as" menu for right clicks on `anchor`.
fn context_menu(app: &mut App, ui: &mut Ui, anchor: &egui::Response, node: NodeId) {
    let id = Id::new("export-ctx-menu");
    if anchor.secondary_clicked() {
        if let Some(p) = anchor.interact_pointer_pos() {
            ui.data_mut(|d| d.insert_temp(id, p));
        }
    }
    let Some(pos) = ui.data(|d| d.get_temp::<Pos2>(id)) else { return };
    // Only the response that opened it draws it (all three share one position).
    if !anchor.rect.contains(pos) {
        return;
    }
    let items: Vec<MenuItem> = COPY_ITEMS.iter().map(|(_, l)| MenuItem::new(*l)).collect();
    match popup_menu(ui.ctx(), id.with("popup"), pos, 150.0, &items, None) {
        MenuResult::Picked(i) => {
            ui.data_mut(|d| d.remove_temp::<Pos2>(id));
            copy_as(app, node, COPY_ITEMS[i].0);
        }
        MenuResult::Closed => {
            ui.data_mut(|d| d.remove_temp::<Pos2>(id));
        }
        MenuResult::Open => {}
    }
}

/// Puts `node` on the clipboard as PNG (2x), SVG text or CSS text.
pub fn copy_as(app: &mut App, node: NodeId, kind: CopyKind) {
    let result: Result<&str, String> = (|| {
        let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        match kind {
            CopyKind::Png => {
                let pm = designos_export::render_export(&app.export_doc(), node, 2.0).ok_or("Nothing to copy")?;
                let (w, h) = (pm.width() as usize, pm.height() as usize);
                let bytes = designos_export::rgba_bytes(&pm);
                cb.set_image(arboard::ImageData { width: w, height: h, bytes: bytes.into() }).map_err(|e| e.to_string())?;
                Ok("Copied as PNG")
            }
            CopyKind::Svg => {
                let svg = designos_export::export_svg(&app.export_doc(), node, &designos_export::SvgOptions::default());
                cb.set_text(svg).map_err(|e| e.to_string())?;
                Ok("Copied as SVG")
            }
            CopyKind::Css => {
                cb.set_text(designos_export::css_for(&app.export_doc(), node)).map_err(|e| e.to_string())?;
                Ok("Copied as CSS")
            }
        }
    })();
    match result {
        Ok(msg) => app.show_toast(msg, false),
        Err(e) => app.show_toast(format!("Copy failed: {e}"), true),
    }
}

/// One file an export writes: its name, the layers it holds and how it is encoded.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportJob {
    pub file: String,
    pub nodes: Vec<NodeId>,
    pub setting: ExportSetting,
}

/// The files exporting `ids` with their own rows writes. Image rows give one file per
/// layer and row (`name@2x.png`); every layer with a PDF row goes into ONE PDF with a
/// page per layer, in selection order, named after the layer (one) or the document (several).
pub fn plan_export(doc: &Document, ids: &[NodeId]) -> Vec<ExportJob> {
    let mut jobs: Vec<ExportJob> = Vec::new();
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut unique = |file: String, ext: &str| -> String {
        let count = seen.entry(file.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            return file;
        }
        let stem = file.rsplit_once('.').map_or(file.as_str(), |(a, _)| a);
        format!("{stem} {count}.{ext}")
    };
    let mut pdf: Option<(ExportPreset, Vec<NodeId>)> = None;
    for id in ids {
        let Some(n) = doc.get(*id) else { continue };
        let mut in_pdf = false;
        for p in &n.export_settings {
            let s = setting_of(p);
            if s.format != ExportFormat::Pdf {
                let file = unique(preset_file_name(p, &n.name), s.format.extension());
                jobs.push(ExportJob { file, nodes: vec![*id], setting: s });
            } else if !in_pdf {
                in_pdf = true;
                pdf.get_or_insert_with(|| (p.clone(), Vec::new())).1.push(*id);
            }
        }
    }
    if let Some((preset, nodes)) = pdf {
        let file = match nodes.as_slice() {
            [one] => preset_file_name(&preset, doc.get(*one).map_or("export", |n| n.name.as_str())),
            _ => format!("{}.pdf", designos_export::sanitize_file_name(&doc.name)),
        };
        jobs.push(ExportJob { file: unique(file, "pdf"), nodes, setting: setting_of(&preset) });
    }
    jobs
}

/// Where a batch of export files goes.
enum Sink {
    /// The one file the save dialog chose.
    File(PathBuf),
    /// A folder: the chosen one (files are replaced) or the fallback one (nothing is replaced).
    Dir { dir: PathBuf, keep_existing: bool },
}

impl Sink {
    fn write(&self, job: &ExportJob, bytes: &[u8]) -> Result<PathBuf, (PathBuf, std::io::Error)> {
        match self {
            Sink::File(path) => std::fs::write(path, bytes).map(|()| path.clone()).map_err(|e| (path.clone(), e)),
            Sink::Dir { dir, keep_existing: false } => {
                let path = dir.join(&job.file);
                std::fs::write(&path, bytes).map(|()| path.clone()).map_err(|e| (path, e))
            }
            Sink::Dir { dir, keep_existing: true } => crate::io::write_new_in(dir, &job.file, bytes).map_err(|e| (dir.join(&job.file), e)),
        }
    }
}

/// Writes every selected layer with its own export rows. One file asks for a
/// path; several ask for a folder and use `name@2x.png` style names. When no
/// dialog can open the files go to the fallback folder and the toast says so.
pub fn run_export(app: &mut App, ids: &[NodeId]) {
    run_export_with(app, ids, &crate::io::SystemDialogs);
}

pub fn run_export_with(app: &mut App, ids: &[NodeId], dialogs: &dyn crate::io::FileDialogs) {
    use crate::io::{DialogOutcome, FileRequest};
    let jobs = plan_export(&app.doc, ids);
    if jobs.is_empty() {
        app.show_toast("Nothing to export", true);
        return;
    }
    let single = jobs.len() == 1;
    let outcome = match jobs.as_slice() {
        [job] => dialogs.pick(&FileRequest::save(job.file.as_str()).filter(job.setting.format.label(), &[job.setting.format.extension()])),
        _ => dialogs.pick(&FileRequest::folder("Choose export folder")),
    };
    let (sink, without_dialog) = match outcome {
        DialogOutcome::Cancelled => return,
        DialogOutcome::Picked(path) if single => (Sink::File(path), false),
        DialogOutcome::Picked(dir) => (Sink::Dir { dir, keep_existing: false }, false),
        DialogOutcome::Unavailable => match dialogs.fallback_dir() {
            Some(dir) => (Sink::Dir { dir, keep_existing: true }, true),
            None => {
                app.show_toast(crate::io::NO_FALLBACK_FOLDER, true);
                return;
            }
        },
    };
    let lead = if without_dialog { "No save dialog. " } else { "" };
    // Variable bindings are applied like in the preview and the deck PDF.
    let doc = app.export_doc();
    let mut written = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    for job in &jobs {
        let bytes = match job.setting.render_many(&doc, &job.nodes) {
            Ok(bytes) => bytes,
            Err(e) if single => {
                drop(doc);
                app.show_toast(e, true);
                return;
            }
            Err(e) => {
                failed.push(format!("{}: {e}", job.file));
                continue;
            }
        };
        match sink.write(job, &bytes) {
            Ok(path) => written.push(path),
            Err((path, e)) => {
                drop(doc);
                app.show_export_write_error(without_dialog, &path, &e);
                return;
            }
        }
    }
    drop(doc);
    let skipped = if failed.is_empty() { String::new() } else { format!(" ({} skipped: {})", failed.len(), failed.join("; ")) };
    let warn = without_dialog || !failed.is_empty();
    match (written.as_slice(), &sink) {
        ([], _) => app.show_toast(format!("Nothing exported{skipped}"), true),
        ([one], _) if without_dialog => app.show_toast_with_reveal_status(format!("{lead}Saved to {}{skipped}", one.display()), warn, one.clone()),
        ([one], _) => app.show_toast_with_reveal_status(format!("Exported {}{skipped}", one.display()), warn, one.clone()),
        (many, Sink::Dir { dir, .. }) if without_dialog => {
            app.show_toast_with_reveal_status(format!("{lead}Saved {} files to {}{skipped}", many.len(), dir.display()), warn, many[0].clone())
        }
        (many, Sink::Dir { dir, .. }) => {
            app.show_toast_with_reveal_status(format!("Exported {} files to {}{skipped}", many.len(), dir.display()), warn, many[0].clone())
        }
        (many, Sink::File(path)) => app.show_toast_with_reveal_status(format!("Exported {} files{skipped}", many.len()), warn, path.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect as R};

    fn frame(ctx: &egui::Context, t: f64, events: Vec<egui::Event>, f: impl FnMut(&mut Ui)) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 800.0))), time: Some(t), events, ..Default::default() };
        let _ = ctx.run_ui(raw, f);
    }

    #[test]
    fn plus_adds_a_row_and_shows_preview_and_button() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, R::new(0.0, 0.0, 40.0, 30.0))).unwrap();
        let mut app = App::new(d);
        app.selection = vec![id];
        let sel = vec![id];
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut h0 = 0.0;
        frame(&ctx, 0.0, vec![], |ui| h0 = section(&mut app, ui, 0.0, 0.0, &sel));
        assert!(h0 < 60.0, "only the title without rows: {h0}");
        // click the "+" at the right end of the title row
        let p = Pos2::new(theme::SECTION_TRAILING_X + 12.0, 20.0);
        let ev = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        frame(&ctx, 0.1, vec![egui::Event::PointerMoved(p)], |ui| {
            section(&mut app, ui, 0.0, 0.0, &sel);
        });
        frame(&ctx, 0.2, vec![ev(true)], |ui| {
            section(&mut app, ui, 0.0, 0.0, &sel);
        });
        frame(&ctx, 0.3, vec![ev(false)], |ui| {
            section(&mut app, ui, 0.0, 0.0, &sel);
        });
        assert_eq!(app.doc.get(id).unwrap().export_settings, vec![ExportPreset::default()]);
        assert!(app.hist.can_undo(), "adding a row is an undo step");
        let mut h1 = 0.0;
        frame(&ctx, 0.4, vec![], |ui| h1 = section(&mut app, ui, 0.0, 0.0, &sel));
        // title + one row + preview + button
        assert!(h1 > h0 + PREVIEW_H, "{h1} vs {h0}");
        // the preview texture was created
        assert!(ctx.data(|d| d.get_temp::<Thumb>(Id::new("export-thumb"))).is_some());
    }

    fn two_rects() -> (App, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        d.insert(page, Node::rectangle(a, R::new(0.0, 0.0, 40.0, 30.0))).unwrap();
        let b = d.alloc_id();
        d.insert(page, Node::rectangle(b, R::new(50.0, 0.0, 40.0, 30.0))).unwrap();
        (App::new(d), a, b)
    }

    #[test]
    fn format_labels_fit_their_dropdown() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        frame(&ctx, 0.0, vec![], |ui| {
            for l in ["PNG", "JPG", "SVG", "PDF"] {
                let w = widgets::text_width(ui.painter(), l, &theme::font_menu());
                assert!(w <= FORMAT_W - 28.0, "{l} is {w}px wide");
            }
            for l in ["512w", "1.5x", "2x"] {
                let w = widgets::text_width(ui.painter(), l, &theme::font_body());
                assert!(w <= SCALE_W - 20.0 - 8.0, "{l} is {w}px wide");
            }
        });
    }

    #[test]
    fn scale_strings_round_trip_through_constraints() {
        for s in ["2x", "0.75x", "512w", "64h"] {
            let sc = ExportScale::parse(s).unwrap();
            assert_eq!(scale_of(constraint_of(sc)), sc, "{s}");
        }
        assert_eq!(constraint_of(ExportScale::parse("512w").unwrap()), ExportConstraint::Width(512.0));
        assert_eq!(constraint_of(ExportScale::parse("3x").unwrap()), ExportConstraint::Scale(3.0));
        let p = ExportPreset { format: ExportFileFormat::Jpg, constraint: ExportConstraint::Height(64.0), suffix: String::new() };
        assert_eq!(preset_file_name(&p, "Logo"), "Logo@64h.jpg");
        let p = ExportPreset { suffix: "-dark".into(), ..p };
        assert_eq!(preset_file_name(&p, "Logo"), "Logo-dark.jpg");
        let pdf = ExportPreset { format: ExportFileFormat::Pdf, ..Default::default() };
        assert_eq!(setting_of(&pdf).format, ExportFormat::Pdf);
        assert_eq!(preset_file_name(&pdf, "Hero"), "Hero.pdf");
    }

    #[test]
    fn rows_add_edit_remove_are_undoable_and_survive_save_and_load() {
        let (mut app, a, _) = two_rects();
        let one = ExportPreset::default();
        app.edit_export_rows(&[a], &RowEdit::Add(one.clone()));
        let two = ExportPreset { format: ExportFileFormat::Svg, constraint: ExportConstraint::Width(512.0), suffix: "@512".into() };
        app.edit_export_rows(&[a], &RowEdit::Add(two.clone()));
        let edited = ExportPreset { constraint: ExportConstraint::Scale(2.0), ..one.clone() };
        app.edit_export_rows(&[a], &RowEdit::Replace(one.clone(), edited.clone()));
        assert_eq!(app.doc.get(a).unwrap().export_settings, vec![edited.clone(), two.clone()]);

        let mut path = std::env::temp_dir();
        path.push(format!("designos_export_rows_{}", std::process::id()));
        let p = crate::io::save_path(&app.doc, &path).unwrap();
        let back = crate::io::load_path(&p).unwrap();
        std::fs::remove_file(p).ok();
        assert_eq!(back.get(a).unwrap().export_settings, vec![edited.clone(), two.clone()]);

        app.edit_export_rows(&[a], &RowEdit::Remove(edited));
        assert_eq!(app.doc.get(a).unwrap().export_settings, vec![two]);
        app.undo();
        assert_eq!(app.doc.get(a).unwrap().export_settings.len(), 2);
        app.undo();
        assert_eq!(app.doc.get(a).unwrap().export_settings[0], one, "the edit undoes in one step");
    }

    #[test]
    fn multi_selection_shows_and_edits_shared_rows_only() {
        let (mut app, a, b) = two_rects();
        let shared = ExportPreset::default();
        let only_a = ExportPreset { format: ExportFileFormat::Jpg, ..Default::default() };
        app.edit_export_rows(&[a, b], &RowEdit::Add(shared.clone()));
        app.edit_export_rows(&[a], &RowEdit::Add(only_a.clone()));
        assert_eq!(shared_rows(&app.doc, &[a, b]), vec![shared.clone()]);
        assert_eq!(shared_rows(&app.doc, &[a]), vec![shared.clone(), only_a.clone()]);

        let new = ExportPreset { constraint: ExportConstraint::Scale(4.0), ..shared.clone() };
        app.edit_export_rows(&[a, b], &RowEdit::Replace(shared.clone(), new.clone()));
        assert_eq!(app.doc.get(a).unwrap().export_settings, vec![new.clone(), only_a]);
        assert_eq!(app.doc.get(b).unwrap().export_settings, vec![new.clone()]);
        app.edit_export_rows(&[a, b], &RowEdit::Remove(new));
        assert_eq!(app.doc.get(b).unwrap().export_settings, vec![]);
        assert_eq!(app.doc.get(a).unwrap().export_settings.len(), 1, "the non-shared row is kept");
    }

    /// Two rectangles named Alpha and Beta, each with one PNG row.
    fn named_rects_with_png_rows() -> (App, NodeId, NodeId) {
        let (mut app, a, b) = two_rects();
        app.doc.get_mut(a).unwrap().name = "Alpha".into();
        app.doc.get_mut(b).unwrap().name = "Beta".into();
        app.edit_export_rows(&[a, b], &RowEdit::Add(ExportPreset::default()));
        (app, a, b)
    }

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("designos_export_{}_{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn files_in(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    fn toast_of(app: &App) -> Option<(String, bool)> {
        app.toast.as_ref().map(|t| (t.text.clone(), t.error))
    }

    #[test]
    fn one_file_without_a_save_dialog_lands_in_the_fallback_folder_and_says_so() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let dir = scratch_dir("one_unavailable");
        let (mut app, a, _) = named_rects_with_png_rows();
        run_export_with(&mut app, &[a], &ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
        let saved = dir.join("Alpha.png");
        assert!(saved.is_file(), "{:?}", files_in(&dir));
        assert!(std::fs::read(&saved).unwrap().starts_with(b"\x89PNG"));
        assert_eq!(toast_of(&app), Some((format!("No save dialog. Saved to {}", saved.display()), true)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn one_file_in_the_fallback_folder_never_overwrites() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let dir = scratch_dir("one_no_overwrite");
        std::fs::write(dir.join("Alpha.png"), b"keep me").unwrap();
        let (mut app, a, _) = named_rects_with_png_rows();
        run_export_with(&mut app, &[a], &ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
        assert_eq!(std::fs::read(dir.join("Alpha.png")).unwrap(), b"keep me");
        let saved = dir.join("Alpha 2.png");
        assert!(saved.is_file(), "{:?}", files_in(&dir));
        assert_eq!(toast_of(&app).unwrap().0, format!("No save dialog. Saved to {}", saved.display()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn several_files_without_a_dialog_all_land_in_the_fallback_folder() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let dir = scratch_dir("many_unavailable");
        std::fs::write(dir.join("Beta.png"), b"keep me").unwrap();
        let (mut app, a, b) = named_rects_with_png_rows();
        run_export_with(&mut app, &[a, b], &ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
        assert_eq!(files_in(&dir), ["Alpha.png", "Beta 2.png", "Beta.png"], "the existing Beta.png is kept");
        assert_eq!(std::fs::read(dir.join("Beta.png")).unwrap(), b"keep me");
        assert_eq!(toast_of(&app), Some((format!("No save dialog. Saved 2 files to {}", dir.display()), true)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_very_long_layer_name_still_saves_to_the_fallback_folder() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let dir = scratch_dir("long_name");
        let (mut app, a, b) = named_rects_with_png_rows();
        app.doc.get_mut(a).unwrap().name = "a".repeat(300);
        app.doc.get_mut(b).unwrap().name = "b".repeat(300);
        for ids in [vec![a], vec![a, b]] {
            run_export_with(&mut app, &ids, &ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
            assert!(toast_of(&app).unwrap().0.starts_with("No save dialog. Saved"), "{:?}", toast_of(&app));
        }
        let names = files_in(&dir);
        assert_eq!(names.len(), 3, "one file, then two more: {names:?}");
        assert!(names.iter().all(|n| n.len() <= 255 && n.ends_with(".png")), "{names:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_without_a_dialog_or_a_fallback_folder_still_says_so() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let (mut app, a, b) = named_rects_with_png_rows();
        for ids in [vec![a], vec![a, b]] {
            app.toast = None;
            run_export_with(&mut app, &ids, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
            assert_eq!(toast_of(&app), Some(("No save dialog. No Downloads or home folder found.".to_string(), true)));
        }
    }

    #[test]
    fn export_cancel_is_silent_and_writes_nothing() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let dir = scratch_dir("cancel");
        let (mut app, a, b) = named_rects_with_png_rows();
        for ids in [vec![a], vec![a, b]] {
            run_export_with(&mut app, &ids, &ScriptedDialogs::answering(DialogOutcome::Cancelled).with_fallback(dir.clone()));
            assert_eq!(toast_of(&app), None);
        }
        assert!(files_in(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_writes_where_the_dialog_picked() {
        use crate::io::{DialogOutcome, ScriptedDialogs};
        let dir = scratch_dir("picked");
        let (mut app, a, b) = named_rects_with_png_rows();
        let target = dir.join("chosen.png");
        run_export_with(&mut app, &[a], &ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())).with_fallback(dir.join("unused")));
        assert_eq!(toast_of(&app), Some((format!("Exported {}", target.display()), false)));
        let folder = scratch_dir("picked_folder");
        run_export_with(&mut app, &[a, b], &ScriptedDialogs::answering(DialogOutcome::Picked(folder.clone())));
        assert_eq!(files_in(&folder), ["Alpha.png", "Beta.png"]);
        assert_eq!(files_in(&dir), ["chosen.png"], "nothing lands in the fallback folder");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&folder).ok();
    }

    fn two_frames_with_pdf_rows() -> (App, NodeId, NodeId) {
        let mut d = Document::new("Screens");
        let page = d.add_page("p");
        let a = d.alloc_id();
        d.insert(page, Node::frame(a, R::new(0.0, 0.0, 40.0, 30.0))).unwrap();
        let b = d.alloc_id();
        d.insert(page, Node::frame(b, R::new(100.0, 0.0, 20.0, 50.0))).unwrap();
        d.get_mut(a).unwrap().name = "Home".into();
        d.get_mut(b).unwrap().name = "Detail".into();
        let mut app = App::new(d);
        let pdf = ExportPreset { format: ExportFileFormat::Pdf, ..Default::default() };
        app.edit_export_rows(&[a, b], &RowEdit::Add(pdf));
        (app, a, b)
    }

    fn pdf_pages(bytes: &[u8]) -> usize {
        bytes.windows(12).filter(|w| *w == b"/Type /Page ").count()
    }

    #[test]
    fn every_format_is_in_the_dropdown_and_round_trips_to_the_stored_format() {
        let labels: Vec<&str> = ExportFormat::ALL.iter().map(|f| f.label()).collect();
        assert_eq!(labels, ["PNG", "JPG", "SVG", "PDF"]);
        for f in ExportFormat::ALL {
            assert_eq!(format_of(file_format_of(f)), f);
        }
    }

    #[test]
    fn selected_frames_with_pdf_rows_export_as_one_multi_page_file() {
        let (app, a, b) = two_frames_with_pdf_rows();
        let jobs = plan_export(&app.doc, &[a, b]);
        assert_eq!(jobs.len(), 1, "one PDF, not one per frame");
        assert_eq!((jobs[0].file.as_str(), jobs[0].nodes.as_slice()), ("Screens.pdf", &[a, b][..]));
        let bytes = jobs[0].setting.render_many(&app.export_doc(), &jobs[0].nodes).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert_eq!(pdf_pages(&bytes), 2);
        // a single layer keeps its own name and gives a one-page file
        let one = plan_export(&app.doc, &[b]);
        assert_eq!((one[0].file.as_str(), one[0].nodes.as_slice()), ("Detail.pdf", &[b][..]));
        assert_eq!(pdf_pages(&one[0].setting.render_many(&app.export_doc(), &one[0].nodes).unwrap()), 1);
    }

    #[test]
    fn pdf_rows_mix_with_image_rows_and_repeat_rows_add_no_page() {
        let (mut app, a, b) = two_frames_with_pdf_rows();
        let png = ExportPreset { constraint: ExportConstraint::Scale(2.0), ..Default::default() };
        app.edit_export_rows(&[a], &RowEdit::Add(png));
        let again = ExportPreset { format: ExportFileFormat::Pdf, suffix: "-copy".into(), ..Default::default() };
        app.edit_export_rows(&[a], &RowEdit::Add(again));
        let jobs = plan_export(&app.doc, &[a, b]);
        let files: Vec<&str> = jobs.iter().map(|j| j.file.as_str()).collect();
        assert_eq!(files, ["Home@2x.png", "Screens.pdf"]);
        assert_eq!(jobs[1].nodes, vec![a, b], "Home has two PDF rows but is one page");
    }

    #[test]
    fn oversized_frame_in_a_pdf_row_reports_an_error() {
        let (mut app, a, b) = two_frames_with_pdf_rows();
        app.doc.get_mut(b).unwrap().size.x = 25_000.0;
        let jobs = plan_export(&app.doc, &[a, b]);
        assert!(jobs[0].setting.render_many(&app.export_doc(), &jobs[0].nodes).is_err());
    }

    #[test]
    fn fingerprint_changes_with_the_subtree() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, R::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let a = fingerprint(&d, id);
        assert_eq!(a, fingerprint(&d, id));
        d.get_mut(id).unwrap().opacity = 0.5;
        assert_ne!(a, fingerprint(&d, id));
    }
}
