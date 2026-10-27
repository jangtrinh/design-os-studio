//! The Home screen must not offer the spec bundle: the spec reader is switched off, so
//! the header button and the template card only led to a blank canvas.
//!
//! The tests draw the real `main_view::show` into a headless egui context, read the text
//! it paints and the widget rects it records, and click through it. `main_view::show` is
//! used instead of `home::show` so the pushed actions are observed and never applied
//! (`Import` would open a native dialog and `+ Design file` would leave Home).

use super::{main_view, Act, Home, Section};
use crate::theme;
use designos_library::Library;
use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use std::path::PathBuf;

/// Temporary library root, removed on drop.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_home_spec_hidden_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Self(p)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A Home screen drawn frame by frame in a headless egui context.
struct HomeSim {
    ctx: Context,
    lib: Library,
    home: Home,
    time: f64,
    pointer: Pos2,
    shapes: Vec<ClippedShape>,
    _dir: TmpDir,
}

/// The area `main_view::show` draws into, like the window's main area next to the sidebar.
fn main_rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 760.0))
}

impl HomeSim {
    fn new(section: Section) -> Self {
        let dir = TmpDir::new();
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let lib = Library::open(&dir.0).expect("open a temporary library");
        let mut home = Home::new();
        home.refresh(&lib);
        home.section = section;
        let mut sim = Self { ctx, lib, home, time: 0.0, pointer: Pos2::ZERO, shapes: Vec::new(), _dir: dir };
        sim.frame(vec![]);
        sim.frame(vec![]);
        sim
    }

    /// Draws one frame and returns the actions the screen pushed in it.
    fn frame(&mut self, events: Vec<Event>) -> Vec<Act> {
        self.time += 0.05;
        let raw = RawInput { screen_rect: Some(main_rect()), time: Some(self.time), events, ..Default::default() };
        let mut acts = Vec::new();
        let (lib, home) = (&self.lib, &mut self.home);
        let out = self.ctx.run_ui(raw, |ui| main_view::show(lib, home, ui, main_rect(), &mut acts));
        self.shapes = out.shapes;
        acts
    }

    /// Presses and releases the primary button at `p`; returns every action pushed meanwhile.
    fn click(&mut self, p: Pos2) -> Vec<Act> {
        self.pointer = p;
        let button = |pressed| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let mut acts = self.frame(vec![Event::PointerMoved(p)]);
        acts.extend(self.frame(vec![button(true)]));
        acts.extend(self.frame(vec![button(false)]));
        acts.extend(self.frame(vec![]));
        // Anything the click opened (menus, selection) must not shape the next probe.
        self.home.filter_menu = None;
        self.home.sort_menu = None;
        self.home.menu = None;
        self.home.selected = None;
        self.home.drag = None;
        acts
    }

    /// Every string painted in the last frame.
    fn painted_text(&self) -> Vec<String> {
        fn walk(shape: &Shape, out: &mut Vec<String>) {
            match shape {
                Shape::Text(t) => out.push(t.galley.text().to_owned()),
                Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    fn has_text(&self, needle: &str) -> bool {
        let needle = needle.to_ascii_lowercase();
        self.painted_text().iter().any(|t| t.to_ascii_lowercase().contains(&needle))
    }

    /// Where the named widget was drawn, if it was drawn at all.
    fn rect_of(&self, name: &str) -> Option<Rect> {
        self.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
    }

    fn click_widget(&mut self, name: &str) -> Vec<Act> {
        let r = self.rect_of(name).unwrap_or_else(|| panic!("nothing recorded for {name}"));
        self.click(r.center())
    }

    /// Clicks along the header strip, where every header button sits, every 8 px.
    fn sweep_header(&mut self) -> Vec<Act> {
        let y = main_rect().top() + theme::HOME_HEADER_H / 2.0;
        let mut acts = Vec::new();
        let mut x = main_rect().left() + 4.0;
        while x < main_rect().right() {
            acts.extend(self.click(Pos2::new(x, y)));
            x += 8.0;
        }
        acts
    }

    /// Clicks a 40 px grid over the area below the header; a template card is 240 x 188, so each one is hit.
    fn sweep_body(&mut self) -> Vec<Act> {
        let mut acts = Vec::new();
        let mut y = main_rect().top() + theme::HOME_HEADER_H + 40.0;
        while y < main_rect().bottom() {
            let mut x = main_rect().left() + 20.0;
            while x < main_rect().right() {
                acts.extend(self.click(Pos2::new(x, y)));
                x += 40.0;
            }
            y += 40.0;
        }
        acts
    }
}

#[test]
fn header_has_no_spec_bundle_button() {
    let sim = HomeSim::new(Section::Recents);

    // The walk sees the other header buttons, so an empty result cannot mean "nothing was drawn".
    for label in ["+ Design file", "+ FigJam board", "Import"] {
        assert!(sim.has_text(label), "the header must still show {label:?}; painted: {:?}", sim.painted_text());
    }
    assert!(sim.rect_of("home-new-file").is_some() && sim.rect_of("home-new-board").is_some() && sim.rect_of("home-import").is_some());

    assert!(!sim.has_text("spec bundle"), "the header must not show a spec bundle button; painted: {:?}", sim.painted_text());
    assert!(sim.rect_of("home-new-spec").is_none(), "no widget may be recorded as the spec bundle button");
}

#[test]
fn templates_view_has_no_spec_bundle_card() {
    let sim = HomeSim::new(Section::Templates);

    let cards = ["home-template-design", "home-template-board", "home-template-deck", "home-template-starter"];
    for card in cards {
        assert!(sim.rect_of(card).is_some(), "template card {card} must still be drawn");
    }
    for needle in ["spec bundle", "open spec hub", "spec driven"] {
        assert!(!sim.has_text(needle), "the templates view must not show {needle:?}; painted: {:?}", sim.painted_text());
    }
    assert!(sim.rect_of("home-template-specs").is_none(), "no widget may be recorded as the spec bundle card");
}

#[test]
fn no_click_on_home_pushes_open_specs() {
    for section in [Section::Recents, Section::Templates] {
        let mut sim = HomeSim::new(section.clone());
        let mut acts = sim.sweep_header();
        acts.extend(sim.sweep_body());
        assert!(!acts.is_empty(), "the sweep must reach real buttons in {section:?}, or it proves nothing");
        assert!(!acts.contains(&Act::OpenSpecs), "a click in {section:?} pushed Act::OpenSpecs");
    }
}

#[test]
fn other_home_actions_still_work() {
    let mut recents = HomeSim::new(Section::Recents);
    assert!(recents.click_widget("home-new-file").contains(&Act::NewFile));
    assert!(recents.click_widget("home-new-board").contains(&Act::NewBoard));
    assert!(recents.click_widget("home-import").contains(&Act::Import));

    let mut templates = HomeSim::new(Section::Templates);
    for (card, expected) in [
        ("home-template-design", Act::NewFile),
        ("home-template-board", Act::NewBoard),
        ("home-template-deck", Act::NewDeck),
        ("home-template-starter", Act::OpenStarterSample),
    ] {
        let acts = templates.click_widget(card);
        assert!(acts.contains(&expected), "{card} must push {expected:?}; pushed {acts:?}");
    }
}
