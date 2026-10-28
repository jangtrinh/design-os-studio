//! `--select` on the headless agent CLI (#1127): a command acts on the node it was told to act on, or it stops with an
//! error and leaves every file as it was. The fixture here is shared with the resolver and the window tests.

use designos_doc::{Document, Node, NodeId, Rect, TextStyle};
use std::path::{Path, PathBuf};

/// Two cards in a board. Ids: 1 page, 2 `Board`, 3 `Card A`, 4 `Title`, 5 `Card B`, 6 `Title`.
/// `Board` sits at (100, 60), so a card's page position differs from its parent-relative one:
/// `Card B` is at (480, 200) in the page and 360 x 220 big. Figma ids: `Board` 12:1, `Card A` 12:30, `Card B` 12:34,
/// the second `Title` I12:34;56:78 (like an instance sublayer); the first `Title` has none.
pub(crate) fn two_cards() -> Document {
    let mut doc = Document::new("two-cards");
    let page = doc.add_page("Page 1");
    assert_eq!(page, NodeId(1));
    let mut add = |parent: u64, mut node: Node, name: &str, source_id: Option<&str>| {
        node.name = name.into();
        node.source_id = source_id.map(str::to_string);
        doc.insert(NodeId(parent), node).unwrap();
    };
    add(1, Node::frame(NodeId(2), Rect::new(100.0, 60.0, 900.0, 500.0)), "Board", Some("12:1"));
    add(2, Node::frame(NodeId(3), Rect::new(20.0, 40.0, 360.0, 220.0)), "Card A", Some("12:30"));
    add(3, Node::text(NodeId(4), Rect::new(16.0, 16.0, 120.0, 24.0), "Card A", TextStyle::default()), "Title", None);
    add(2, Node::frame(NodeId(5), Rect::new(380.0, 140.0, 360.0, 220.0)), "Card B", Some("12:34"));
    add(5, Node::text(NodeId(6), Rect::new(16.0, 16.0, 120.0, 24.0), "Card B", TextStyle::default()), "Title", Some("I12:34;56:78"));
    doc
}

/// A directory of its own under the system temp dir, removed when dropped.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("designos_select_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Saves [`two_cards`] as `two-cards.designos` and returns its path.
    pub(crate) fn two_cards(&self) -> PathBuf {
        let path = self.path("two-cards.designos");
        crate::io::save_path(&two_cards(), &path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn text(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Runs `designos ARGS...` in-process. A refusal comes back as the error text.
pub(crate) fn run(args: &[&str]) -> Result<(), String> {
    let opts = crate::cli::parse(args.iter().map(|a| a.to_string()))?;
    crate::run_agent_cli(&opts)
}

fn spec_name(spec_file: &Path) -> String {
    let spec: serde_json::Value = serde_json::from_slice(&std::fs::read(spec_file).unwrap()).unwrap();
    spec["name"].as_str().unwrap().to_string()
}

#[test]
fn export_spec_with_unknown_select_fails_and_writes_nothing() {
    let dir = Scratch::new("export_miss");
    let doc = dir.two_cards();
    let out = dir.path("spec.json");
    let err = run(&[text(&doc), "--select", "Card Z", "--export-spec", text(&out)]).expect_err("an unknown --select stops the command");
    assert!(err.contains("Card Z"), "the message names the selector: {err}");
    assert!(!out.exists(), "no other node is exported in its place");
}

#[test]
fn apply_spec_with_unknown_select_leaves_the_file_unchanged() {
    let dir = Scratch::new("apply_miss");
    let doc = dir.two_cards();
    let spec = dir.path("probe.spec.json");
    std::fs::write(&spec, r#"{"kind":"frame","name":"Probe","width":40,"height":40}"#).unwrap();
    let before = std::fs::read(&doc).unwrap();
    let err = run(&[text(&doc), "--select", "Card Z", "--apply-spec", text(&spec)]).expect_err("an unknown --select stops the command");
    assert!(err.contains("Card Z"), "{err}");
    assert_eq!(std::fs::read(&doc).unwrap(), before, "the document is not touched, nothing lands on the first page");
}

#[test]
fn codegen_and_add_comment_with_unknown_select_fail() {
    let dir = Scratch::new("codegen_comment_miss");
    let doc = dir.two_cards();
    let before = std::fs::read(&doc).unwrap();
    let err = run(&[text(&doc), "--select", "Card Z", "--codegen", "css"]).expect_err("codegen must not describe the first screen instead");
    assert!(err.contains("Card Z"), "{err}");
    let err = run(&[text(&doc), "--select", "Card Z", "--add-comment", "Make it orange"]).expect_err("a comment must not anchor to the first page instead");
    assert!(err.contains("Card Z"), "{err}");
    assert_eq!(std::fs::read(&doc).unwrap(), before, "no comment was saved");
}

#[test]
fn inspect_json_with_unknown_select_fails() {
    let dir = Scratch::new("inspect_miss");
    let doc = dir.two_cards();
    let err = run(&[text(&doc), "--select", "Card Z", "--inspect-json"]).expect_err("--inspect-json must not ignore --select");
    assert!(err.contains("Card Z"), "{err}");
}

#[test]
fn select_by_id_figma_id_and_name_export_the_same_card() {
    let dir = Scratch::new("export_hit");
    let doc = dir.two_cards();
    for selector in ["id:5", "fig:12:34", "name:Card B", "Card B"] {
        let out = dir.path("spec.json");
        let _ = std::fs::remove_file(&out);
        run(&[text(&doc), "--select", selector, "--export-spec", text(&out)]).unwrap_or_else(|e| panic!("{selector}: {e}"));
        assert_eq!(spec_name(&out), "Card B", "{selector}");
    }
}

#[test]
fn select_by_figma_id_in_a_round_tripped_fig() {
    let dir = Scratch::new("fig_round_trip");
    let doc = dir.two_cards();
    let fig = dir.path("two-cards.fig");
    run(&[text(&doc), "--export-fig", text(&fig)]).unwrap();
    // The ids a reopened .fig carries are the ones to address it by.
    let reopened = crate::io::load_path(&fig).unwrap();
    let card_b = reopened.nodes.values().find(|n| n.name == "Card B").expect("Card B survives the round trip");
    let source_id = card_b.source_id.clone().expect("a reopened .fig keeps a Figma id on its nodes");
    let out = dir.path("spec.json");
    run(&[text(&fig), "--select", &format!("fig:{source_id}"), "--export-spec", text(&out)]).unwrap();
    assert_eq!(spec_name(&out), "Card B");
}

#[test]
fn inspect_json_reports_the_selection_and_screen_source_ids() {
    let mut doc = two_cards();
    let load_next_id = doc.peek_next_id();
    let target = crate::select_target::resolve(&mut doc, "Card B", load_next_id).unwrap();
    let v = crate::inspect_json(&doc, target.node, target.id_stable);
    let s = &v["selection"];
    assert_eq!(s["id"], 5);
    assert_eq!(s["id_stable"], true, "a node that existed at load is stable: {s}");
    assert_eq!(
        (s["name"].as_str(), s["kind"].as_str(), s["page_id"].as_u64(), s["source_id"].as_str()),
        (Some("Card B"), Some("Frame"), Some(1), Some("12:34"))
    );
    assert_eq!((s["x"].as_f64(), s["y"].as_f64()), (Some(480.0), Some(200.0)), "the page position, not the one inside Board: {s}");
    assert_eq!((s["width"].as_f64(), s["height"].as_f64()), (Some(360.0), Some(220.0)));
    assert_eq!(v["pages"][0]["screens"][0]["source_id"], "12:1", "a screen carries its Figma id");
    assert!(crate::inspect_json(&doc, None, true).get("selection").is_none(), "no --select, no selection key");
}

#[test]
fn screenshot_exit_code_for_a_select_miss() {
    assert_eq!(crate::exit_code(true, true), 2, "a miss is exit 2");
    assert_eq!(crate::exit_code(true, false), 2, "a miss takes precedence over a PNG that was not captured");
    assert_eq!(crate::exit_code(false, false), 1, "no PNG is exit 1, as before");
    assert_eq!(crate::exit_code(false, true), 0);
}
