//! `--select` and `--zoom fit` at launch (#1127): the window and `--screenshot` select the node the selector names,
//! frame it on request, and say so when the selector names nothing.

use super::*;
use crate::agent_cli_select_tests::Scratch;

/// A window on `two_cards` opened as `designos two-cards.designos ARGS...`.
fn opened(dir: &Scratch, args: &[&str]) -> Sim {
    let mut argv = vec![dir.two_cards().to_string_lossy().into_owned()];
    argv.extend(args.iter().map(|a| a.to_string()));
    Sim::with(crate::cli::parse(argv).unwrap())
}

#[test]
fn gui_select_by_id_selects_the_node() {
    let dir = Scratch::new("gui_select");
    for selector in ["id:5", "fig:12:34", "name:Card B", "Card B", "5"] {
        let s = opened(&dir, &["--select", selector]);
        assert_eq!(s.app().selection, vec![NodeId(5)], "{selector}");
        assert!(s.shell.select_miss.is_none(), "{selector} is a hit");
    }
    let s = opened(&dir, &["--select", "fig:I12:34;56:78"]);
    assert_eq!(s.app().selection, vec![NodeId(6)], "a Figma id with colons and a semicolon");
    let s = opened(&dir, &["--select", "none"]);
    assert!(s.app().selection.is_empty());
    assert!(s.shell.select_miss.is_none(), "`none` selects nothing on purpose");
}

/// Page 0 grows one node the first time the window expands it, the way a deferred page does on open.
#[derive(Debug)]
struct PageZeroHydrator {
    done: bool,
    created: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl designos_doc::PageHydrator for PageZeroHydrator {
    fn hydrate_page(&mut self, doc: &mut designos_doc::Document, page: NodeId) -> bool {
        if self.done || doc.pages.first().copied() != Some(page) {
            return false;
        }
        self.done = true;
        let id = doc.alloc_id();
        self.created.store(id.0, std::sync::atomic::Ordering::SeqCst);
        let node = designos_doc::Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
        doc.insert(page, node).is_ok()
    }

    fn hydrate_all(&mut self, doc: &mut designos_doc::Document) -> bool {
        let Some(page) = doc.pages.first().copied() else { return false };
        self.hydrate_page(doc, page)
    }

    fn is_hydrated(&self, _page: NodeId) -> bool {
        self.done
    }
}

#[test]
fn gui_rejects_a_page0_hydrated_id_like_the_cli() {
    let dir = Scratch::new("gui_page0_watermark");
    let launched = opened(&dir, &["--select", "id:7"]);
    let msg = launched.shell.select_miss.as_deref().expect("id 7 is at the load watermark");
    assert!(msg.contains("id 7 belongs to an expanded instance and is not stable across runs"), "{msg}");
    assert!(launched.app().selection.is_empty());

    let mut s = opened(&dir, &[]);
    let mut doc = crate::agent_cli_select_tests::two_cards();
    let watermark = doc.peek_next_id();
    let created = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    doc.page_hydrator = Some(std::sync::Arc::new(std::sync::Mutex::new(PageZeroHydrator { done: false, created: created.clone() })));
    s.shell.app.replace_document(doc, None);
    let id = created.load(std::sync::atomic::Ordering::SeqCst);
    assert_ne!(id, 0, "opening the file expanded a node on page 0");
    assert_eq!(s.app().load_next_id, watermark, "the watermark is recorded before that expansion");
    assert!(s.app().doc.get(NodeId(id)).is_some(), "the expanded node stays in the document");
    let raw = format!("id:{id}");
    let load_next_id = s.app().load_next_id;
    let err = crate::select_target::resolve_flag(&mut s.shell.app.doc, Some(&raw), load_next_id).expect_err("id: of a node page 0 just created");
    assert!(err.to_string().contains("not stable across runs"), "{err}");
}

#[test]
fn gui_select_of_a_page_name_switches_to_the_page_and_selects_nothing() {
    let dir = Scratch::new("gui_select_page");
    let s = opened(&dir, &["--select", "Page 1"]);
    assert!(s.app().selection.is_empty());
    assert_eq!(s.app().page, NodeId(1));
    assert!(s.shell.select_miss.is_none());
}

#[test]
fn gui_select_miss_is_recorded_for_exit() {
    let dir = Scratch::new("gui_select_miss_shot");
    let png = dir.path("shot.png");
    let s = opened(&dir, &["--select", "Card Z", "--screenshot", png.to_str().unwrap()]);
    let msg = s.shell.select_miss.as_deref().expect("a miss is recorded for the exit code");
    assert!(msg.contains("--select \"Card Z\" matched no node"), "{msg}");
    assert!(s.sent_commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)), "the capture window closes at once");
    assert!(!s.sent_commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(..))), "no capture is asked for");
    assert!(!png.exists(), "no PNG is written for a selector that matched nothing");
}

#[test]
fn an_edit_text_miss_in_a_capture_run_is_recorded_like_a_select_miss() {
    let dir = Scratch::new("gui_edit_text_miss");
    let png = dir.path("shot.png");
    let s = opened(&dir, &["--edit-text", "Nope", "--screenshot", png.to_str().unwrap()]);
    let msg = s.shell.select_miss.as_deref().expect("a text layer that is not there is a miss in a capture run");
    assert_eq!(msg, "--edit-text \"Nope\" matched no text layer");
    assert!(s.sent_commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)), "the capture window closes at once");
    assert!(!png.exists());

    let hit = opened(&dir, &["--edit-text", "Title", "--screenshot", png.to_str().unwrap()]);
    assert!(hit.shell.select_miss.is_none(), "an existing text layer is not a miss");
}

#[test]
fn an_interactive_launch_with_a_select_miss_stays_open_and_says_so() {
    let dir = Scratch::new("gui_select_miss_open");
    let s = opened(&dir, &["--select", "Card Z"]);
    assert!(s.app().selection.is_empty());
    assert!(!s.sent_commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)), "a person is not thrown out of the app for a typo");
    let toast = s.app().toast.as_ref().expect("an error toast names the miss");
    assert!(toast.error);
    assert_eq!(toast.text, "No layer matches \"Card Z\". Check the name, or use id: or fig:");
}

/// How much of the canvas the selection fills along its larger side (measured against the canvas side that
/// matches it), and whether it lies inside the canvas.
fn framing(s: &Sim, id: NodeId) -> (f32, bool) {
    let app = s.app();
    let b = designos_doc::node_world_bounds(&app.doc, id);
    let (a, c) = (app.view.to_screen(app.canvas_rect.min, DV::new(b.x, b.y)), app.view.to_screen(app.canvas_rect.min, DV::new(b.x + b.w, b.y + b.h)));
    let rect = Rect::from_two_pos(a, c);
    let fill = if rect.width() >= rect.height() { rect.width() / app.canvas_rect.width() } else { rect.height() / app.canvas_rect.height() };
    (fill, app.canvas_rect.contains_rect(rect))
}

#[test]
fn zoom_fit_frames_the_selection() {
    let dir = Scratch::new("zoom_fit");
    let s = opened(&dir, &["--select", "Card B", "--zoom", "fit"]);
    assert_eq!(s.app().selection, vec![NodeId(5)]);
    let (fill, inside) = framing(&s, NodeId(5));
    assert!(inside, "the selection lies inside the canvas");
    assert!(fill >= 0.8, "the selection fills {fill:.2} of the canvas, wanted at least 0.80");
}

#[test]
fn without_zoom_fit_the_page_fit_stays_as_it_was() {
    let dir = Scratch::new("zoom_default");
    let plain = opened(&dir, &[]);
    let selected = opened(&dir, &["--select", "Card B"]);
    assert_eq!(selected.app().view.zoom, plain.app().view.zoom, "--select alone does not reframe a capture");
    assert_eq!((selected.app().view.offset.x, selected.app().view.offset.y), (plain.app().view.offset.x, plain.app().view.offset.y));
    let (fill, _) = framing(&selected, NodeId(5));
    assert!(fill < 0.8, "Card B is a part of the page here, filling {fill:.2}");
}

#[test]
fn zoom_fit_without_a_selection_fits_the_page() {
    let dir = Scratch::new("zoom_fit_page");
    let plain = opened(&dir, &[]);
    let fit = opened(&dir, &["--zoom", "fit"]);
    assert_eq!(fit.app().view.zoom, plain.app().view.zoom);
}
