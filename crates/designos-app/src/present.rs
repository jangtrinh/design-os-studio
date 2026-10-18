//! Present view: runs the prototype full-window. The [`Player`] decides what
//! is on screen; this module renders frames to textures (cached per frame and
//! content revision), draws transitions and overlays, and feeds it pointer,
//! wheel and key input.

use crate::app::App;
use crate::icons::Icon;
use crate::proto::geom;
use crate::proto::player::{content_extent, hit_layer, Effect, Player};
use crate::proto::smart::{subtree_doc, SmartPlan};
use crate::theme;
use crate::widgets::{self, menu::dropdown, record};
use designos_doc::*;
use egui::{Align2, Color32, Context, Event, Pos2, Rect as ERect, Sense, TextureHandle, TextureOptions, Ui, Vec2 as EV};
use std::collections::HashMap;

/// Largest texture side we upload (scrolling content can be very tall).
const MAX_TEX_SIDE: f32 = 8192.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Key {
    /// 0 for the live document, else the id of a snapshot.
    doc: u64,
    frame: NodeId,
    rev: u64,
    scale_bits: u32,
    layer: u8,
}

struct Entry {
    tex: TextureHandle,
    /// Frame units covered by the texture.
    units: Vec2,
    used: u64,
}

struct Drag {
    start: Pos2,
    last: Pos2,
    target: Option<NodeId>,
    moved: bool,
    scrolling: bool,
}

struct SmartRun {
    key: (NodeId, NodeId, u64, u64),
    plan: Option<SmartPlan>,
    tex: Option<TextureHandle>,
}

pub struct PresentState {
    pub player: Player,
    pub start: NodeId,
    cache: HashMap<Key, Entry>,
    frame_no: u64,
    smart: Option<SmartRun>,
    drag: Option<Drag>,
    /// Screen rect of the current frame in the last painted frame (for tests and hit testing).
    pub stage: ERect,
    pub scale: f32,
    /// Device screen (rect, corner radius in px) the stage is letterboxed into, if a device is set.
    pub device_screen: Option<(ERect, f32)>,
    pub pending_url: Option<(String, bool)>,
    pub close: bool,
    pub fullscreen: bool,
    pub start_fullscreen: bool,
    initial_fullscreen: bool,
    /// The live variables panel (a debugging aid; Figma has none) is open.
    pub show_vars: bool,
    /// Screen rect of the variables panel in the last painted frame (clicks on it do not reach the stage).
    pub vars_rect: ERect,
    /// Decoded video layers (see `crate::video`).
    pub videos: crate::video::VideoRuntime,
}

impl PresentState {
    pub fn new(doc: &Document, start: NodeId) -> Self {
        let player = Player::new(doc, start);
        PresentState {
            start: player.current,
            player,
            cache: HashMap::new(),
            frame_no: 0,
            smart: None,
            drag: None,
            stage: ERect::NOTHING,
            scale: 1.0,
            device_screen: None,
            pending_url: None,
            close: false,
            fullscreen: false,
            start_fullscreen: false,
            initial_fullscreen: false,
            show_vars: false,
            vars_rect: ERect::NOTHING,
            videos: Default::default(),
        }
    }

    pub fn restart(&mut self, doc: &Document, start: NodeId) {
        self.player.restart(doc, start);
        self.start = self.player.current;
        self.drag = None;
        self.smart = None;
        self.pending_url = None;
        self.videos = Default::default();
    }

    fn set_fullscreen(&mut self, ctx: &Context, fullscreen: bool) {
        self.fullscreen = fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        ctx.request_repaint();
    }

    /// Screen position of the center of `node` in the current frame (for tests).
    #[cfg(test)]
    pub fn screen_center_of(&self, node: NodeId) -> Option<Pos2> {
        let frame = self.player.top_frame_of(node)?;
        let r = crate::proto::player::rect_in_frame(&self.player.doc, node, frame);
        let scroll =
            if self.player.doc.get(frame).is_some_and(|f| f.prototype.fixed_children.iter().any(|c| *c == node || self.player.doc.is_ancestor(*c, node))) {
                Vec2::ZERO
            } else {
                self.player.scroll_of(frame)
            };
        Some(self.stage.min + EV::new((r.x + r.w / 2.0 - scroll.x) * self.scale, (r.y + r.h / 2.0 - scroll.y) * self.scale))
    }
}

/// Largest rect of the aspect of `frame` that fits `avail` with `pad` around it.
pub fn stage_layout(avail: ERect, frame: Vec2, pad: f32) -> (ERect, f32) {
    let sw = (avail.width() - 2.0 * pad).max(10.0) / frame.x.max(1.0);
    let sh = (avail.height() - 2.0 * pad).max(10.0) / frame.y.max(1.0);
    let s = sw.min(sh).clamp(0.05, theme::PRESENT_MAX_SCALE);
    (ERect::from_center_size(avail.center(), EV::new(frame.x * s, frame.y * s)), s)
}

/// Letterboxes `frame` into a device screen of `device` units that fits `avail` (with a bezel and
/// `pad` around it). Returns the device screen rect, its corner radius in px, the frame's rect
/// and the frame's scale.
pub fn device_layout(avail: ERect, frame: Vec2, device: Vec2, radius: f32, pad: f32) -> (ERect, f32, ERect, f32) {
    let (screen, dev_scale) = stage_layout(avail, device, pad + theme::PRESENT_BEZEL);
    let k = (screen.width() / frame.x.max(1.0)).min(screen.height() / frame.y.max(1.0));
    let stage = ERect::from_center_size(screen.center(), EV::new(frame.x * k, frame.y * k));
    (screen, radius * dev_scale, stage, k)
}

// ----------------------------------------------------------------- rendering

struct Layers {
    content: (egui::TextureId, Vec2),
    fixed: Option<egui::TextureId>,
    frame_units: Vec2,
}

fn upload(ctx: &Context, pm: &designos_render::tiny_skia::Pixmap, name: String) -> TextureHandle {
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    ctx.load_texture(name, img, TextureOptions::LINEAR)
}

/// Renders the scrolling content (and the fixed layer, if any) of a frame.
fn render_layers(
    doc: &Document,
    frame: NodeId,
    scale: f32,
    max_side: f32,
) -> Option<(designos_render::tiny_skia::Pixmap, Option<designos_render::tiny_skia::Pixmap>, Vec2)> {
    let n = doc.get(frame)?;
    let ext = content_extent(doc, frame);
    let (sx, sy) = match n.prototype.overflow {
        Overflow::None => (false, false),
        Overflow::Horizontal => (true, false),
        Overflow::Vertical => (false, true),
        Overflow::Both => (true, true),
    };
    let content_units = Vec2::new(if sx { ext.x.max(n.size.x) } else { n.size.x }, if sy { ext.y.max(n.size.y) } else { n.size.y });
    let scrolls = content_units.x > n.size.x + 0.5 || content_units.y > n.size.y + 0.5;
    // Textures are stretched onto the stage, so a smaller one is only a little softer.
    let fit = |units: Vec2| (max_side / (units.x.max(units.y) * scale).max(1.0)).min(1.0);
    if !scrolls {
        return Some((designos_render::render_node(doc, frame, scale * fit(n.size))?, None, n.size));
    }
    let k = fit(content_units);
    let mut s = subtree_doc(doc, frame);
    {
        let fixed = n.prototype.fixed_children.clone();
        let f = s.get_mut(frame)?;
        f.size = content_units;
        if let Some(p) = f.kind.frame_props_mut() {
            p.clips_content = false;
        }
        for c in fixed {
            if let Some(cn) = s.get_mut(c) {
                cn.visible = false;
            }
        }
    }
    let content = designos_render::render_node(&s, frame, scale * k)?;
    let fixed = if n.prototype.fixed_children.is_empty() {
        None
    } else {
        let mut s2 = subtree_doc(doc, frame);
        let keep = n.prototype.fixed_children.clone();
        let kids = s2.get(frame)?.children.clone();
        for c in kids {
            if !keep.contains(&c) {
                if let Some(cn) = s2.get_mut(c) {
                    cn.visible = false;
                }
            }
        }
        if let Some(f) = s2.get_mut(frame) {
            f.fills.clear();
            f.stroke.paints.clear();
            f.effects.clear();
        }
        designos_render::render_node(&s2, frame, scale * fit(n.size))
    };
    Some((content, fixed, content_units))
}

impl PresentState {
    fn layers(&mut self, ctx: &Context, doc: Option<&Document>, doc_id: u64, frame: NodeId, px_scale: f32) -> Option<Layers> {
        let rev = if doc_id == 0 { self.player.frame_rev(frame) } else { 0 };
        let bits = px_scale.to_bits();
        let ck = Key { doc: doc_id, frame, rev, scale_bits: bits, layer: 0 };
        let fk = Key { layer: 1, ..ck };
        let frame_no = self.frame_no;
        if !self.cache.contains_key(&ck) {
            let d = doc.unwrap_or(&self.player.doc);
            let max_side = (ctx.input(|i| i.max_texture_side) as f32).min(MAX_TEX_SIDE);
            let (c, f, units) = render_layers(d, frame, px_scale, max_side)?;
            self.cache.insert(ck, Entry { tex: upload(ctx, &c, format!("present {frame:?} {rev}")), units, used: frame_no });
            if let Some(f) = f {
                self.cache.insert(fk, Entry { tex: upload(ctx, &f, format!("present fixed {frame:?} {rev}")), units: Vec2::ZERO, used: frame_no });
            }
        }
        let e = self.cache.get_mut(&ck)?;
        e.used = frame_no;
        let content = (e.tex.id(), e.units);
        let fixed = self.cache.get_mut(&fk).map(|e| {
            e.used = frame_no;
            e.tex.id()
        });
        let fu = doc.unwrap_or(&self.player.doc).get(frame).map(|n| n.size).unwrap_or(Vec2::ZERO);
        Some(Layers { content, fixed, frame_units: fu })
    }

    fn evict(&mut self) {
        let now = self.frame_no;
        self.cache.retain(|_, e| now.saturating_sub(e.used) < 240);
        if self.cache.len() > 48 {
            let mut v: Vec<(Key, u64)> = self.cache.iter().map(|(k, e)| (*k, e.used)).collect();
            v.sort_by_key(|(_, u)| *u);
            for (k, _) in v.into_iter().take(self.cache.len() - 48) {
                self.cache.remove(&k);
            }
        }
    }
}

fn draw_layers(p: &egui::Painter, l: &Layers, rect: ERect, scroll: Vec2, alpha: f32) {
    let tint = Color32::WHITE.gamma_multiply(alpha.clamp(0.0, 1.0));
    let (cu, fu) = (l.content.1, l.frame_units);
    let uv = ERect::from_min_max(
        Pos2::new(scroll.x / cu.x.max(1.0), scroll.y / cu.y.max(1.0)),
        Pos2::new((scroll.x + fu.x) / cu.x.max(1.0), (scroll.y + fu.y) / cu.y.max(1.0)),
    );
    p.image(l.content.0, rect, uv, tint);
    if let Some(f) = l.fixed {
        p.image(f, rect, ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
    }
}

// --------------------------------------------------------------------- input

fn key_name(key: egui::Key, m: egui::Modifiers) -> String {
    let base = format!("{key:?}");
    let base = base.strip_prefix("Num").map(String::from).unwrap_or(base);
    let mut parts = Vec::new();
    if m.ctrl || m.command {
        parts.push("Ctrl".to_string());
    }
    if m.alt {
        parts.push("Alt".into());
    }
    if m.shift {
        parts.push("Shift".into());
    }
    parts.push(base);
    parts.join("+")
}

/// Keyboard stand-in for a game controller (there is no pure-Rust controller backend in the
/// build): Alt+Shift with Z X C V = A B X Y, Q E = LB RB, arrows = D-pad, Enter = Start,
/// Backspace = Back. Keeps gamepad triggers testable without hardware and clear of key triggers.
pub fn gamepad_button_for_key(key: egui::Key, m: egui::Modifiers) -> Option<&'static str> {
    if !(m.alt && m.shift) {
        return None;
    }
    Some(match key {
        egui::Key::Z => "A",
        egui::Key::X => "B",
        egui::Key::C => "X",
        egui::Key::V => "Y",
        egui::Key::Q => "LB",
        egui::Key::E => "RB",
        egui::Key::ArrowUp => "DPadUp",
        egui::Key::ArrowDown => "DPadDown",
        egui::Key::ArrowLeft => "DPadLeft",
        egui::Key::ArrowRight => "DPadRight",
        egui::Key::Enter => "Start",
        egui::Key::Backspace => "Back",
        _ => return None,
    })
}

/// Whether a link may be opened (web and mail links only).
pub fn url_allowed(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:")
}

fn open_url(ctx: &Context, url: &str) -> Result<(), String> {
    if !url_allowed(url) {
        return Err("Blocked: only http(s) and mailto links open".into());
    }
    #[cfg(feature = "open-url")]
    {
        let _ = ctx;
        open::that(url.trim()).map_err(|e| format!("Could not open link: {e}"))
    }
    #[cfg(not(feature = "open-url"))]
    {
        ctx.copy_text(url.trim().to_string());
        Err("Link copied to the clipboard (built without link opening)".into())
    }
}

fn icon_close(p: &egui::Painter, c: Pos2, col: Color32) {
    let k = 5.0;
    let st = egui::Stroke::new(1.5, col);
    p.line_segment([c + EV::new(-k, -k), c + EV::new(k, k)], st);
    p.line_segment([c + EV::new(-k, k), c + EV::new(k, -k)], st);
}

// ---------------------------------------------------------------------- show

#[derive(Clone, Copy)]
enum SlideMove {
    Previous,
    Next,
    First,
    Last,
}

fn slide_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<SlideMove> {
    if modifiers != egui::Modifiers::NONE {
        return None;
    }
    match key {
        egui::Key::ArrowRight | egui::Key::Space | egui::Key::PageDown => Some(SlideMove::Next),
        egui::Key::ArrowLeft | egui::Key::PageUp => Some(SlideMove::Previous),
        egui::Key::Home => Some(SlideMove::First),
        egui::Key::End => Some(SlideMove::Last),
        _ => None,
    }
}

fn slide_target(slides: &[NodeId], current: NodeId, movement: SlideMove) -> Option<NodeId> {
    match movement {
        SlideMove::First => slides.first().copied(),
        SlideMove::Last => slides.last().copied(),
        SlideMove::Previous => slides.iter().position(|id| *id == current).and_then(|i| i.checked_sub(1)).and_then(|i| slides.get(i)).copied(),
        SlideMove::Next => slides.iter().position(|id| *id == current).and_then(|i| slides.get(i + 1)).copied(),
    }
}

fn move_slide(st: &mut PresentState, slides: &[NodeId], movement: SlideMove, now: f64, ctx: &Context) {
    if let Some(target) = slide_target(slides, st.player.current, movement) {
        if st.player.navigate_slide(target, now) {
            st.drag = None;
            st.smart = None;
            st.videos = Default::default();
            ctx.request_repaint();
        }
    }
}

fn slide_button(ui: &mut Ui, rect: ERect, key: &'static str, label: &str, enabled: bool) -> bool {
    record(ui, key, rect);
    let response = ui.interact(rect, ui.id().with(key), if enabled { Sense::click() } else { Sense::hover() });
    widgets::button_bg(ui.painter(), rect, &response, false, theme::cr(theme::RADIUS_CONTROL));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::font_body(), if enabled { theme::TEXT } else { theme::TEXT_TERTIARY });
    enabled && response.clicked()
}

/// Draws the Present view over the whole window and handles its input.
pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let Some(mut st) = app.proto.present.take() else { return };
    let full = ui.max_rect();
    let now = ctx.input(|i| i.time);
    st.frame_no += 1;
    if st.frame_no == 1 {
        st.player.rebase_clock(now);
        st.initial_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        st.fullscreen = st.initial_fullscreen;
        if st.start_fullscreen {
            st.set_fullscreen(&ctx, true);
        }
    }
    let toggle = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11));
    if toggle {
        let fullscreen = !st.fullscreen;
        st.set_fullscreen(&ctx, fullscreen);
    }
    st.player.tick(now);
    let is_deck = app.is_deck();
    let slides = if is_deck { app.deck_slides() } else { Vec::new() };
    // Process deck keys before computing stage geometry so differently sized slides
    // are fitted on the same frame in which they are selected.
    if is_deck && st.pending_url.is_none() {
        for event in ctx.input(|i| i.events.clone()) {
            if let Event::Key { key, pressed: true, repeat: false, modifiers, .. } = event {
                if let Some(movement) = slide_key(key, modifiers) {
                    move_slide(&mut st, &slides, movement, now, &ctx);
                }
            }
        }
    }
    let p = ui.painter().clone();
    p.rect_filled(full, 0.0, page_bg(app));

    // ---- top bar
    let bar = ERect::from_min_size(full.min, EV::new(full.width(), theme::PRESENT_BAR_HEIGHT));
    if !st.fullscreen {
        let flows: Vec<Flow> = if is_deck { Vec::new() } else { app.doc.flows.clone() };
        let cur_flow = flows.iter().position(|f| f.start == st.start);
        let names: Vec<String> = flows.iter().map(|f| f.name.clone()).collect();
        let label = if is_deck {
            app.doc.name.clone()
        } else {
            cur_flow.map(|i| names[i].clone()).unwrap_or_else(|| app.doc.get(st.start).map(|n| n.name.clone()).unwrap_or_default())
        };
        let dd = ERect::from_min_size(Pos2::new(bar.left() + 16.0, bar.top() + 12.0), EV::new(200.0, 24.0));
        record(ui, "present-flow", dd);
        if names.is_empty() {
            p.rect_filled(dd, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            p.text(Pos2::new(dd.left() + 8.0, dd.center().y), Align2::LEFT_CENTER, &label, theme::font_body(), theme::TEXT);
        } else if let Some(i) = dropdown(ui, dd, "present-flow", &label, &names) {
            let start = flows[i].start;
            st.restart(&app.doc, start);
        }
        let rb = ERect::from_min_size(Pos2::new(dd.right() + 8.0, dd.top()), EV::new(76.0, 24.0));
        record(ui, "present-restart", rb);
        let rr = ui.interact(rb, ui.id().with("present-restart"), Sense::click());
        widgets::button_bg(&p, rb, &rr, false, theme::cr(theme::RADIUS_CONTROL));
        p.text(rb.center(), Align2::CENTER_CENTER, "Restart", theme::font_body(), theme::TEXT);
        if rr.clicked() {
            let s = if is_deck { slides.first().copied().unwrap_or(st.start) } else { st.start };
            st.restart(&app.doc, s);
        }
        // Variables: a small toggle for the live values panel (hidden when the file has no variables).
        if !app.doc.variables.is_empty() {
            let vb = ERect::from_min_size(Pos2::new(rb.right() + 8.0, rb.top()), EV::new(76.0, 24.0));
            record(ui, "present-vars-toggle", vb);
            let vr = ui.interact(vb, ui.id().with("present-vars"), Sense::click());
            widgets::button_bg(&p, vb, &vr, st.show_vars, theme::cr(theme::RADIUS_CONTROL));
            p.text(vb.center(), Align2::CENTER_CENTER, "Variables", theme::font_body(), if st.show_vars { theme::TEXT } else { theme::TEXT_SECONDARY });
            if vr.on_hover_text("Show live variable values").clicked() {
                st.show_vars = !st.show_vars;
            }
        }
        let cb = ERect::from_min_size(Pos2::new(bar.right() - 40.0, bar.top() + 8.0), EV::splat(32.0));
        let fb = ERect::from_min_size(Pos2::new(cb.left() - 104.0, bar.top() + 12.0), EV::new(96.0, 24.0));
        record(ui, "present-fullscreen", fb);
        let fr = ui.interact(fb, ui.id().with("present-fullscreen"), Sense::click());
        widgets::button_bg(&p, fb, &fr, false, theme::cr(theme::RADIUS_CONTROL));
        p.text(fb.center(), Align2::CENTER_CENTER, "Fullscreen", theme::font_body(), theme::TEXT);
        if fr.on_hover_text("Fullscreen (F11). Esc returns to preview.").clicked() {
            st.set_fullscreen(&ctx, true);
        }
        record(ui, "present-close", cb);
        let cr = ui.interact(cb, ui.id().with("present-close"), Sense::click());
        widgets::button_bg(&p, cb, &cr, false, theme::cr(theme::RADIUS_CONTROL));
        icon_close(&p, cb.center(), theme::ICON);
        if cr.on_hover_text("Exit (Esc)").clicked() {
            st.close = true;
        }
        let title = app.doc.get(st.player.current).map(|n| n.name.clone()).unwrap_or_default();
        p.text(Pos2::new(bar.center().x, bar.center().y), Align2::CENTER_CENTER, title, theme::font_body(), theme::TEXT_SECONDARY);
        widgets::hline(&p, bar.left(), bar.right(), bar.bottom() - 1.0, Color32::from_white_alpha(18));
    }

    // Deck navigation occupies its own footer, outside the prototype hit area.
    let footer_height = if is_deck && !st.fullscreen { 44.0 } else { 0.0 };
    if is_deck && !st.fullscreen {
        let footer = ERect::from_min_max(Pos2::new(full.left(), full.bottom() - footer_height), full.max);
        widgets::hline(&p, footer.left(), footer.right(), footer.top(), Color32::from_white_alpha(18));
        let previous = ERect::from_center_size(Pos2::new(footer.center().x - 90.0, footer.center().y), EV::new(76.0, 28.0));
        let next = ERect::from_center_size(Pos2::new(footer.center().x + 90.0, footer.center().y), EV::new(76.0, 28.0));
        let ready = st.pending_url.is_none();
        if slide_button(ui, previous, "present-slide-prev", "Previous", ready && slide_target(&slides, st.player.current, SlideMove::Previous).is_some()) {
            move_slide(&mut st, &slides, SlideMove::Previous, now, &ctx);
        }
        if slide_button(ui, next, "present-slide-next", "Next", ready && slide_target(&slides, st.player.current, SlideMove::Next).is_some()) {
            move_slide(&mut st, &slides, SlideMove::Next, now, &ctx);
        }
        let index = slides.iter().position(|id| *id == st.player.current).map(|i| i + 1).unwrap_or(0);
        let count = ERect::from_center_size(footer.center(), EV::new(84.0, 28.0));
        record(ui, "present-slide-count", count);
        p.text(count.center(), Align2::CENTER_CENTER, format!("{index} / {}", slides.len()), theme::font_body(), theme::TEXT_SECONDARY);
    }

    // ---- stage geometry
    let area = if st.fullscreen {
        full
    } else {
        ERect::from_min_max(Pos2::new(full.left(), bar.bottom()), Pos2::new(full.right(), (full.bottom() - footer_height).max(bar.bottom())))
    };
    let padding = if st.fullscreen { 0.0 } else { theme::PRESENT_PADDING };
    let cur_size = st.player.size_of(st.player.current);
    let (stage, scale) = match crate::proto::device_viewport(&app.doc.prototype_device) {
        Some((dev, radius)) => {
            let (screen, r, stage, k) = device_layout(area, cur_size, dev, radius, padding);
            st.device_screen = Some((screen, r));
            (stage, k)
        }
        None => {
            st.device_screen = None;
            stage_layout(area, cur_size, padding)
        }
    };
    st.stage = stage;
    st.scale = scale;
    let px = scale * ctx.pixels_per_point();

    // ---- input
    let over_widget = widgets::pointer_over_popup(&ctx);
    let (pos, pressed, down, wheel, events) =
        ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.smooth_scroll_delta, i.events.clone()));
    let in_area = pos.filter(|q| area.contains(*q) && !over_widget && !(st.show_vars && st.vars_rect.contains(*q)));
    let target = in_area.and_then(|q| target_at(&st, q, now));
    st.player.pointer_move(target);
    if let Some(q) = in_area {
        if pressed {
            st.drag = Some(Drag { start: q, last: q, target, moved: false, scrolling: false });
            st.player.pointer_down(target);
        }
    }
    if let Some(mut d) = st.drag.take() {
        let q = pos.unwrap_or(d.last);
        if down {
            if !d.moved && (q - d.start).length() > 4.0 {
                d.moved = true;
                d.scrolling = !st.player.drag(d.target);
            }
            if d.moved {
                let delta = (d.last - q) / scale;
                if d.scrolling {
                    let frame = d.target.and_then(|t| st.player.top_frame_of(t)).unwrap_or(st.player.current);
                    st.player.scroll_by(frame, Vec2::new(delta.x, delta.y));
                } else {
                    // On drag: a dragged overlay follows the pointer, Scroll to follows it too
                    st.player.drag_by(Vec2::new(delta.x, delta.y));
                }
            }
            d.last = q;
            st.drag = Some(d);
        } else {
            let t = in_area.and_then(|q| target_at(&st, q, now));
            st.player.pointer_up(t.or(if d.moved { None } else { d.target }), d.moved);
        }
    }
    if wheel != EV::ZERO {
        if let Some(q) = in_area {
            let t = target_at(&st, q, now);
            let frame = t.and_then(|t| st.player.top_frame_of(t)).unwrap_or(st.player.current);
            st.player.scroll_by(frame, Vec2::new(-wheel.x / scale, -wheel.y / scale));
        }
    }
    for ev in events {
        if let Event::Key { key, pressed: true, modifiers, repeat: false, .. } = ev {
            if key == egui::Key::Escape {
                if st.pending_url.is_some() {
                    st.pending_url = None;
                } else if st.fullscreen {
                    st.set_fullscreen(&ctx, false);
                } else {
                    st.close = true;
                }
            } else if st.pending_url.is_none() {
                if is_deck && slide_key(key, modifiers).is_some() {
                    continue;
                }
                match gamepad_button_for_key(key, modifiers) {
                    Some(button) => st.player.gamepad(GamepadDevice::Generic, button),
                    None => st.player.key(&key_name(key, modifiers)),
                }
            }
        }
    }
    // Hand cursor over anything that reacts.
    if target.is_some_and(|t| st.player.is_interactive(t)) {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    for e in std::mem::take(&mut st.player.effects) {
        let Effect::OpenUrl { url, new_tab } = e;
        st.pending_url = Some((url, new_tab));
    }

    // ---- paint
    st.videos.update(&mut st.player, now);
    paint_stage(&mut st, &ctx, &p, stage, px, scale, now, app);
    paint_video_badges(&st, &p.with_clip_rect(stage));
    if st.show_vars {
        st.vars_rect = paint_vars_panel(ui, &st.player, area);
    } else {
        st.vars_rect = ERect::NOTHING;
    }
    if let Some((url, _)) = st.pending_url.clone() {
        let (open, cancel) = confirm_bar(ui, full, &url);
        if cancel {
            st.pending_url = None;
        } else if open {
            st.pending_url = None;
            match open_url(&ctx, &url) {
                Ok(()) => {}
                Err(e) => app.show_toast(e, true),
            }
        }
    }
    st.evict();
    if st.player.anim.is_some() || st.player.overlays.iter().any(|o| o.anim.is_some()) || st.drag.is_some() {
        ctx.request_repaint();
    } else if st.player.is_animating() {
        ctx.request_repaint_after(std::time::Duration::from_millis(40));
    }
    if !st.close {
        app.proto.present = Some(st);
    } else if st.fullscreen != st.initial_fullscreen {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(st.initial_fullscreen));
    }
}

fn page_bg(app: &App) -> Color32 {
    // A background stored with the prototype device wins.
    if let Some(c) = app.doc.prototype_device.background {
        return Color32::from_rgb((c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8);
    }
    // The prototype background is the page background unless it is still the editor default.
    match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => {
            let c = Color32::from_rgb((background.r * 255.0).round() as u8, (background.g * 255.0).round() as u8, (background.b * 255.0).round() as u8);
            if c == theme::rgb(theme::DEFAULT_PAGE_BG) {
                theme::PRESENT_BG
            } else {
                c
            }
        }
        _ => theme::PRESENT_BG,
    }
}

/// Deepest layer under the pointer; overlays first, then the current frame.
fn target_at(st: &PresentState, pos: Pos2, now: f64) -> Option<NodeId> {
    let pl = &st.player;
    for o in pl.overlays.iter().rev().filter(|o| !o.closing()) {
        let sz = pl.size_of(o.frame);
        let poseoff = o.transition.map(|t| geom::overlay_pose(t.kind, o.shown(now), sz).offset).unwrap_or(Vec2::ZERO);
        let origin = st.stage.min + EV::new((o.origin.x + o.drag.x + poseoff.x) * st.scale, (o.origin.y + o.drag.y + poseoff.y) * st.scale);
        let local = Vec2::new((pos.x - origin.x) / st.scale, (pos.y - origin.y) / st.scale);
        if let Some(h) = hit_layer(&pl.doc, o.frame, local, pl.scroll_of(o.frame)) {
            return Some(h);
        }
    }
    let local = Vec2::new((pos.x - st.stage.min.x) / st.scale, (pos.y - st.stage.min.y) / st.scale);
    hit_layer(&pl.doc, pl.current, local, pl.scroll_of(pl.current))
}

#[allow(clippy::too_many_arguments)]
fn paint_stage(st: &mut PresentState, ctx: &Context, p: &egui::Painter, stage: ERect, px: f32, scale: f32, now: f64, app: &App) {
    // Device bezel and screen behind the (letterboxed) frame.
    if let Some((screen, r)) = st.device_screen {
        let rr = |v: f32| theme::cr(v.clamp(0.0, 120.0) as u8);
        p.rect_filled(screen.expand(theme::PRESENT_BEZEL), rr(r + theme::PRESENT_BEZEL), Color32::BLACK);
        p.rect_filled(screen, rr(r), page_bg(app));
    }
    let clip = stage;
    let pc = p.with_clip_rect(clip);
    let cur = st.player.current;
    let scroll_of = |st: &PresentState, f: NodeId| {
        let s = st.player.scroll_of(f);
        Vec2::new(s.x, s.y)
    };
    let anim = st.player.anim.as_ref().map(|a| (a.from, a.to, a.kind, a.eased(now), a.snapshot.as_ref().map(|s| s.id)));
    match anim {
        None => {
            if let Some(l) = st.layers(ctx, None, 0, cur, px) {
                draw_layers(&pc, &l, stage, scroll_of(st, cur), 1.0);
            }
        }
        Some((from, to, kind, e, snap)) => {
            let smart_ok = kind == TransitionKind::SmartAnimate && paint_smart(st, ctx, &pc, stage, px, (from, to, snap.unwrap_or(0)), e);
            if !smart_ok {
                let size_px = Vec2::new(stage.width(), stage.height());
                let poses = geom::poses(if kind == TransitionKind::SmartAnimate { TransitionKind::Dissolve } else { kind }, e, size_px);
                let from_units = if snap.is_some() {
                    st.player.anim.as_ref().and_then(|a| a.snapshot.as_ref()).and_then(|s| s.doc.get(from)).map(|n| n.size).unwrap_or(Vec2::ZERO)
                } else {
                    st.player.size_of(from)
                };
                let from_rect = ERect::from_center_size(stage.center(), EV::new(from_units.x * scale, from_units.y * scale));
                let mut draws: Vec<(bool, geom::Pose)> = vec![(true, poses.from), (false, poses.to)];
                if !poses.to_on_top {
                    draws.reverse();
                }
                for (is_from, pose) in draws {
                    let off = EV::new(pose.offset.x, pose.offset.y);
                    if is_from {
                        let l = match snap {
                            Some(id) => {
                                let doc = st.player.anim.as_mut().and_then(|a| a.snapshot.take());
                                let l = st.layers(ctx, doc.as_ref().map(|s| &s.doc), id, from, px);
                                if let (Some(a), Some(d)) = (st.player.anim.as_mut(), doc) {
                                    a.snapshot = Some(d);
                                }
                                l
                            }
                            None => st.layers(ctx, None, 0, from, px),
                        };
                        if let Some(l) = l {
                            let sc = if snap.is_some() { Vec2::ZERO } else { scroll_of(st, from) };
                            draw_layers(&pc, &l, from_rect.translate(off), sc, pose.alpha);
                        }
                    } else if let Some(l) = st.layers(ctx, None, 0, to, px) {
                        draw_layers(&pc, &l, stage.translate(off), scroll_of(st, to), pose.alpha);
                    }
                }
            }
        }
    }

    // Overlays above the frame.
    let overlays: Vec<_> = st.player.overlays.clone();
    for o in overlays {
        let shown = o.shown(now);
        if let Some(bg) = o.background {
            let a = (bg.a * shown).clamp(0.0, 1.0);
            pc.rect_filled(stage, 0.0, Color32::from_rgba_unmultiplied((bg.r * 255.0) as u8, (bg.g * 255.0) as u8, (bg.b * 255.0) as u8, (a * 255.0) as u8));
        }
        let sz = st.player.size_of(o.frame);
        let pose = geom::overlay_pose(o.transition.map(|t| t.kind).unwrap_or(TransitionKind::Dissolve), shown, sz);
        let rect = ERect::from_min_size(
            stage.min + EV::new((o.origin.x + o.drag.x + pose.offset.x) * scale, (o.origin.y + o.drag.y + pose.offset.y) * scale),
            EV::new(sz.x * scale, sz.y * scale),
        );
        if let Some(l) = st.layers(ctx, None, 0, o.frame, px) {
            draw_layers(&pc, &l, rect, scroll_of(st, o.frame), pose.alpha);
        }
    }
}

/// "Unsupported codec" pill over the poster of every video this build cannot play.
fn paint_video_badges(st: &PresentState, p: &egui::Painter) {
    let pl = &st.player;
    for (node, reason) in st.videos.unsupported_nodes() {
        let Some(frame) = pl.top_frame_of(node).filter(|f| *f == pl.current) else { continue };
        let r = crate::proto::player::rect_in_frame(&pl.doc, node, frame);
        let s = pl.scroll_of(frame);
        let min = st.stage.min + EV::new((r.x - s.x) * st.scale, (r.y - s.y) * st.scale);
        let rect = ERect::from_min_size(min, EV::new(r.w * st.scale, r.h * st.scale));
        let font = theme::font_body();
        let w = widgets::text_width(p, &reason, &font) + 24.0;
        let pill = ERect::from_center_size(rect.center(), EV::new(w.min(rect.width().max(40.0)), 26.0));
        p.rect_filled(pill, theme::cr_pill(pill.height()), Color32::from_black_alpha(190));
        let shown = widgets::elide(p, &reason, &font, pill.width() - 16.0);
        p.text(pill.center(), Align2::CENTER_CENTER, shown, font, Color32::WHITE);
    }
}

/// Width of the variables panel.
const VARS_W: f32 = 264.0;
const VARS_ROW_H: f32 = 22.0;

/// The live variables panel: every variable with the value it has right now (changed ones in
/// brand blue), the mode of each collection and the last action errors. Read only; the document
/// is never touched. Returns its screen rect.
fn paint_vars_panel(ui: &mut Ui, player: &Player, area: ERect) -> ERect {
    let rows = player.variable_rows();
    let mut h = 44.0 + 8.0;
    let mut last_coll: Option<&str> = None;
    for r in &rows {
        if last_coll != Some(r.collection.as_str()) {
            h += 26.0;
            last_coll = Some(&r.collection);
        }
        h += VARS_ROW_H;
    }
    if rows.is_empty() {
        h += VARS_ROW_H;
    }
    if !player.errors.is_empty() {
        h += 12.0 + player.errors.len().min(4) as f32 * 16.0;
    }
    let h = h.min(area.height() - 16.0).max(60.0);
    let rect = ERect::from_min_size(Pos2::new(area.right() - VARS_W - 12.0, area.top() + 8.0), EV::new(VARS_W, h));
    record(ui, "present-vars-panel", rect);
    let p = ui.painter().clone();
    widgets::elevation_shadow(&p, rect, theme::RADIUS_FRAME);
    p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
    p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), egui::Stroke::new(1.0, theme::BORDER_MENU), egui::StrokeKind::Inside);
    let pc = p.with_clip_rect(rect);
    pc.text(Pos2::new(rect.left() + 16.0, rect.top() + 22.0), Align2::LEFT_CENTER, "Variables", theme::font_body_strong(), theme::TEXT);
    pc.text(Pos2::new(rect.right() - 16.0, rect.top() + 22.0), Align2::RIGHT_CENTER, "live, not saved", theme::font_caption(), theme::TEXT_TERTIARY);
    widgets::hline(&pc, rect.left(), rect.right(), rect.top() + 43.0, theme::BORDER_MENU);
    let mut y = rect.top() + 44.0 + 4.0;
    let mut last_coll: Option<&str> = None;
    if rows.is_empty() {
        pc.text(
            Pos2::new(rect.left() + 16.0, y + VARS_ROW_H / 2.0),
            Align2::LEFT_CENTER,
            "No variables in this file",
            theme::font_body(),
            theme::TEXT_TERTIARY,
        );
        y += VARS_ROW_H;
    }
    for r in &rows {
        if last_coll != Some(r.collection.as_str()) {
            last_coll = Some(&r.collection);
            let label = widgets::elide(&pc, &r.collection, &theme::font_caption_medium(), VARS_W - 120.0);
            pc.text(Pos2::new(rect.left() + 16.0, y + 15.0), Align2::LEFT_CENTER, label, theme::font_caption_medium(), theme::TEXT_SECONDARY);
            pc.text(Pos2::new(rect.right() - 16.0, y + 15.0), Align2::RIGHT_CENTER, &r.mode, theme::font_caption(), theme::TEXT_TERTIARY);
            y += 26.0;
        }
        let name = widgets::elide(&pc, &r.name, &theme::font_body(), VARS_W - 32.0 - 96.0);
        pc.text(Pos2::new(rect.left() + 16.0, y + VARS_ROW_H / 2.0), Align2::LEFT_CENTER, name, theme::font_body(), theme::TEXT);
        let col = if r.changed { theme::BG_BRAND } else { theme::TEXT_SECONDARY };
        let val = widgets::elide(&pc, &r.value, &theme::font_body(), 92.0);
        pc.text(Pos2::new(rect.right() - 16.0, y + VARS_ROW_H / 2.0), Align2::RIGHT_CENTER, val, theme::font_body(), col);
        y += VARS_ROW_H;
    }
    if !player.errors.is_empty() {
        y += 6.0;
        widgets::hline(&pc, rect.left(), rect.right(), y, theme::BORDER_MENU);
        y += 6.0;
        let skip = player.errors.len().saturating_sub(4);
        for e in player.errors.iter().skip(skip) {
            let t = widgets::elide(&pc, e, &theme::font_caption(), VARS_W - 32.0);
            pc.text(Pos2::new(rect.left() + 16.0, y + 8.0), Align2::LEFT_CENTER, t, theme::font_caption(), theme::DANGER);
            y += 16.0;
        }
    }
    rect
}

/// Paints a Smart Animate frame; false when the frames share no layers.
fn paint_smart(st: &mut PresentState, ctx: &Context, pc: &egui::Painter, stage: ERect, px: f32, ids: (NodeId, NodeId, u64), eased: f32) -> bool {
    let (from, to, snap) = ids;
    let key = (from, to, snap, st.player.frame_rev(to) + st.player.frame_rev(from));
    if st.smart.as_ref().map(|s| s.key) != Some(key) {
        let plan = match st.player.anim.as_ref().and_then(|a| a.snapshot.as_ref()) {
            Some(s) => SmartPlan::build(&s.doc, from, &st.player.doc, to),
            None => SmartPlan::build(&st.player.doc, from, &st.player.doc, to),
        };
        st.smart = Some(SmartRun { key, plan, tex: None });
    }
    let Some(run) = st.smart.as_mut() else { return false };
    let Some(plan) = run.plan.as_mut() else { return false };
    let (doc, root) = plan.apply(eased);
    let max_side = (ctx.input(|i| i.max_texture_side) as f32).min(MAX_TEX_SIDE);
    let size = doc.get(root).map(|n| n.size).unwrap_or(Vec2::ZERO);
    let px = px.min(max_side / size.x.max(size.y).max(1.0));
    let Some(pm) = designos_render::render_node(doc, root, px) else { return false };
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    match &mut run.tex {
        Some(t) => t.set(img, TextureOptions::LINEAR),
        None => run.tex = Some(ctx.load_texture("present smart", img, TextureOptions::LINEAR)),
    }
    if let Some(t) = &run.tex {
        pc.image(t.id(), stage, ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    true
}

/// "Open link?" confirmation; returns (open, cancel).
fn confirm_bar(ui: &mut Ui, full: ERect, url: &str) -> (bool, bool) {
    let p = ui.painter().clone();
    let w = 520.0f32.min(full.width() - 32.0);
    let r = ERect::from_min_size(Pos2::new(full.center().x - w / 2.0, full.bottom() - 88.0), EV::new(w, 56.0));
    widgets::elevation_shadow(&p, r, theme::RADIUS_FRAME);
    p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
    let shown = widgets::elide(&p, url, &theme::font_body(), w - 200.0);
    p.text(Pos2::new(r.left() + 16.0, r.top() + 18.0), Align2::LEFT_CENTER, "Open this link?", theme::font_body_strong(), theme::TEXT);
    p.text(Pos2::new(r.left() + 16.0, r.top() + 38.0), Align2::LEFT_CENTER, shown, theme::font_body(), theme::TEXT_SECONDARY);
    let ob = ERect::from_min_size(Pos2::new(r.right() - 140.0, r.top() + 16.0), EV::new(64.0, 24.0));
    let cb = ERect::from_min_size(Pos2::new(r.right() - 72.0, r.top() + 16.0), EV::new(60.0, 24.0));
    record(ui, "present-url-open", ob);
    record(ui, "present-url-cancel", cb);
    let or = ui.interact(ob, ui.id().with("url-open"), Sense::click());
    p.rect_filled(ob, theme::cr_pill(ob.height()), if or.hovered() { theme::BG_BRAND_HOVER } else { theme::BG_BRAND });
    p.text(ob.center(), Align2::CENTER_CENTER, "Open", theme::font_body(), Color32::WHITE);
    let cr = ui.interact(cb, ui.id().with("url-cancel"), Sense::click());
    widgets::button_bg(&p, cb, &cr, false, theme::cr(theme::RADIUS_CONTROL));
    p.text(cb.center(), Align2::CENTER_CENTER, "Cancel", theme::font_body(), theme::TEXT);
    (or.clicked(), cr.clicked())
}

#[allow(dead_code)]
fn _icons_used(p: &egui::Painter) {
    crate::icons::paint(p, Icon::Play, Pos2::ZERO, 1.0, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slides_follow_declared_order_without_wrapping() {
        let mut doc = Document::new("deck");
        let a = doc.alloc_id();
        let b = doc.alloc_id();
        let c = doc.alloc_id();
        let slides = [c, a, b];
        assert_eq!(slide_target(&slides, c, SlideMove::Next), Some(a));
        assert_eq!(slide_target(&slides, a, SlideMove::Previous), Some(c));
        assert_eq!(slide_target(&slides, c, SlideMove::Previous), None);
        assert_eq!(slide_target(&slides, b, SlideMove::Next), None);
        assert_eq!(slide_target(&slides, a, SlideMove::First), Some(c));
        assert_eq!(slide_target(&slides, a, SlideMove::Last), Some(b));
        assert_eq!(slide_target(&[], a, SlideMove::First), None);
        assert_eq!(slide_target(&[a], a, SlideMove::Next), None);
        assert!(slide_key(egui::Key::Space, egui::Modifiers::NONE).is_some());
        assert!(slide_key(egui::Key::ArrowRight, egui::Modifiers::ALT).is_none());
    }

    #[test]
    fn stage_fits_and_centers() {
        let avail = ERect::from_min_size(Pos2::new(0.0, 48.0), EV::new(1000.0, 652.0));
        let (r, s) = stage_layout(avail, Vec2::new(360.0, 640.0), 32.0);
        assert!((s - (652.0 - 64.0) / 640.0).abs() < 1e-4, "height limited: {s}");
        assert!((r.center().x - 500.0).abs() < 1e-3 && (r.center().y - 374.0).abs() < 1e-3);
        assert!((r.width() / r.height() - 360.0 / 640.0).abs() < 1e-4);
        // wide frame is width limited
        let (_, s) = stage_layout(avail, Vec2::new(3000.0, 100.0), 32.0);
        assert!((s - 936.0 / 3000.0).abs() < 1e-4);
        // tiny frames do not blow up past the maximum scale
        let (_, s) = stage_layout(avail, Vec2::new(10.0, 10.0), 32.0);
        assert_eq!(s, theme::PRESENT_MAX_SCALE);
    }

    #[test]
    fn keyboard_stands_in_for_a_controller_only_with_alt_shift() {
        let both = egui::Modifiers { alt: true, shift: true, ..Default::default() };
        assert_eq!(gamepad_button_for_key(egui::Key::Z, both), Some("A"));
        assert_eq!(gamepad_button_for_key(egui::Key::ArrowUp, both), Some("DPadUp"));
        assert_eq!(gamepad_button_for_key(egui::Key::Z, egui::Modifiers::NONE), None);
        assert_eq!(gamepad_button_for_key(egui::Key::Z, egui::Modifiers::SHIFT), None);
        assert_eq!(gamepad_button_for_key(egui::Key::K, both), None);
    }

    #[test]
    fn urls_are_filtered() {
        assert!(url_allowed("https://example.com"));
        assert!(url_allowed("  HTTP://x.y"));
        assert!(url_allowed("mailto:a@b.c"));
        assert!(!url_allowed("file:///etc/passwd"));
        assert!(!url_allowed("javascript:alert(1)"));
        assert!(!url_allowed("calc.exe"));
    }

    #[test]
    fn key_names_follow_trigger_format() {
        assert_eq!(key_name(egui::Key::Space, egui::Modifiers::NONE), "Space");
        assert_eq!(key_name(egui::Key::K, egui::Modifiers::COMMAND), "Ctrl+K");
        assert_eq!(key_name(egui::Key::Num1, egui::Modifiers::SHIFT), "Shift+1");
        assert!(crate::proto::player::keys_match("Ctrl+K", &key_name(egui::Key::K, egui::Modifiers::COMMAND)));
    }
}
