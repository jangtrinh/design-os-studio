//! `designos FILE --select SELECTOR ...` as a script sees it (#1127): the node it names, or exit 2 with a message
//! that says what was searched and a file that is exactly as it was. Runs the real binary.

use designos_doc::{Document, Node, NodeId, Rect, TextStyle};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_designos");

/// Two cards in a board at (100, 60). Ids: 1 page, 2 `Board` (Figma id 12:1), 3 `Card A` (12:30), 4 `Title`,
/// 5 `Card B` (12:34, page position 480, 200, 360 x 220), 6 `Title` (I12:34;56:78).
fn two_cards_json() -> String {
    let mut doc = Document::new("two-cards");
    let page = doc.add_page("Page 1");
    let mut add = |parent: NodeId, mut node: Node, name: &str, source_id: Option<&str>| {
        node.name = name.into();
        node.source_id = source_id.map(str::to_string);
        doc.insert(parent, node).unwrap();
    };
    add(page, Node::frame(NodeId(2), Rect::new(100.0, 60.0, 900.0, 500.0)), "Board", Some("12:1"));
    add(NodeId(2), Node::frame(NodeId(3), Rect::new(20.0, 40.0, 360.0, 220.0)), "Card A", Some("12:30"));
    add(NodeId(3), Node::text(NodeId(4), Rect::new(16.0, 16.0, 120.0, 24.0), "Card A", TextStyle::default()), "Title", None);
    add(NodeId(2), Node::frame(NodeId(5), Rect::new(380.0, 140.0, 360.0, 220.0)), "Card B", Some("12:34"));
    add(NodeId(5), Node::text(NodeId(6), Rect::new(16.0, 16.0, 120.0, 24.0), "Card B", TextStyle::default()), "Title", Some("I12:34;56:78"));
    doc.to_json().expect("serialize the fixture")
}

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("designos_cli_select_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("two-cards.designos"), two_cards_json()).unwrap();
        Self(dir)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn doc_bytes(&self) -> Vec<u8> {
        std::fs::read(self.path("two-cards.designos")).unwrap()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `--screenshot` opens a window, which a Linux machine without a display cannot do (winit:
/// "neither WAYLAND_DISPLAY nor DISPLAY is set"), so these runs skip there, as on the Linux CI runner.
/// The messages themselves are covered headlessly by the `designos` unit tests (`main.rs`, `select_target`).
fn no_display() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none()
}

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("start designos: {e}"));
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if child.try_wait().expect("poll child").is_some() {
            return child.wait_with_output().expect("collect output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("designos {args:?} did not exit within 60 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn inspect(dir: &Dir, selector: &[&str]) -> (Value, Output) {
    let mut args = vec!["two-cards.designos"];
    args.extend_from_slice(selector);
    args.push("--inspect-json");
    let out = run(&dir.0, &args);
    assert!(out.status.success(), "{selector:?}: {}", stderr(&out));
    (serde_json::from_str(&stdout(&out)).unwrap_or_else(|e| panic!("{selector:?}: {e}: {}", stdout(&out))), out)
}

#[test]
fn select_by_id_figma_id_and_name_reach_the_same_card() {
    let dir = Dir::new("same_card");
    let card_b_css = {
        let out = run(&dir.0, &["two-cards.designos", "--select", "Card B", "--codegen", "css"]);
        assert!(out.status.success(), "{}", stderr(&out));
        stdout(&out)
    };
    assert!(card_b_css.contains("width: 360px") && card_b_css.contains("height: 220px"), "{card_b_css}");
    for selector in ["id:5", "fig:12:34", "name:Card B", "5"] {
        let out = run(&dir.0, &["two-cards.designos", "--select", selector, "--codegen", "css"]);
        assert!(out.status.success(), "{selector}: {}", stderr(&out));
        assert_eq!(stdout(&out), card_b_css, "{selector}");
    }
    let default = run(&dir.0, &["two-cards.designos", "--codegen", "css"]);
    assert!(stdout(&default).contains("width: 900px"), "without --select the first screen is still the default: {}", stdout(&default));
}

#[test]
fn inspect_json_with_select_reports_the_selection_in_page_coordinates() {
    let dir = Dir::new("inspect_selection");
    let (v, _) = inspect(&dir, &["--select", "Card B"]);
    let s = &v["selection"];
    assert_eq!(s["id"], 5);
    assert_eq!(s["name"], "Card B");
    assert_eq!(s["kind"], "Frame");
    assert_eq!(s["page_id"], 1);
    assert_eq!(s["source_id"], "12:34");
    assert_eq!((s["x"].as_f64(), s["y"].as_f64()), (Some(480.0), Some(200.0)), "the page position, not the one inside Board: {s}");
    assert_eq!((s["width"].as_f64(), s["height"].as_f64()), (Some(360.0), Some(220.0)));
    let screens = v["pages"][0]["screens"].as_array().unwrap();
    assert_eq!(screens[0]["id"], 2);
    assert_eq!(screens[0]["source_id"], "12:1", "a screen carries its Figma id so an agent can map it to a Studio id");

    let (v, _) = inspect(&dir, &["--select", "fig:I12:34;56:78"]);
    assert_eq!(v["selection"]["id"], 6);
}

#[test]
fn inspect_json_without_select_has_no_selection() {
    let dir = Dir::new("inspect_plain");
    let (v, _) = inspect(&dir, &[]);
    assert!(v.get("selection").is_none(), "{v}");
    assert!(v["pages"][0]["screens"][0].get("source_id").is_some());
    let (v, _) = inspect(&dir, &["--select", "none"]);
    assert!(v.get("selection").is_none(), "`none` selects nothing: {v}");
}

#[test]
fn an_unknown_select_exits_2_says_what_was_searched_and_leaves_the_file_alone() {
    let dir = Dir::new("miss");
    std::fs::write(dir.path("spec.json"), r#"{"kind":"frame","name":"Probe","width":40,"height":40}"#).unwrap();
    let before = dir.doc_bytes();
    let commands: [&[&str]; 5] =
        [&["--inspect-json"], &["--codegen", "css"], &["--export-spec", "out.json"], &["--add-comment", "Make it orange"], &["--apply-spec", "spec.json"]];
    for command in commands {
        let mut args = vec!["two-cards.designos", "--select", "Card Z"];
        args.extend_from_slice(command);
        let out = run(&dir.0, &args);
        assert_eq!(out.status.code(), Some(2), "{command:?}: {}", stderr(&out));
        let err = stderr(&out);
        assert!(err.contains("--select \"Card Z\" matched no node (searched 6 layers on 1 page by name"), "{command:?}: {err}");
        assert!(stdout(&out).is_empty(), "{command:?} prints nothing for another node: {}", stdout(&out));
        assert_eq!(dir.doc_bytes(), before, "{command:?} left the document as it was");
        assert!(!dir.path("out.json").exists(), "{command:?} wrote no export");
    }
}

#[test]
fn a_malformed_or_empty_select_exits_2() {
    let dir = Dir::new("malformed");
    for selector in ["id:abc", "id:-1", "id:1.5", "id:999", "fig:", "fig:0:0", ""] {
        let out = run(&dir.0, &["two-cards.designos", "--select", selector, "--inspect-json"]);
        assert_eq!(out.status.code(), Some(2), "{selector:?}: {}", stderr(&out));
        assert!(stdout(&out).is_empty(), "{selector:?}: {}", stdout(&out));
    }
}

#[test]
fn several_layers_with_one_name_print_a_note_naming_the_chosen_id() {
    let dir = Dir::new("several");
    let (v, out) = inspect(&dir, &["--select", "Title"]);
    assert_eq!(v["selection"]["id"], 4, "the first in page order");
    assert!(stderr(&out).contains("matched 2 layers; selected id 4 (first in page order; use id:<n> for another)"), "{}", stderr(&out));
}

#[test]
fn a_bare_number_selects_the_node_and_says_how_to_be_exact() {
    let dir = Dir::new("bare");
    let (v, out) = inspect(&dir, &["--select", "5"]);
    assert_eq!(v["selection"]["id"], 5);
    assert!(stderr(&out).contains("--select 5 matched no layer name; selected node id 5 (use --select id:5)"), "{}", stderr(&out));
}

#[test]
fn a_screenshot_with_an_unknown_select_exits_2_and_writes_no_png() {
    if no_display() {
        eprintln!("skipped: --screenshot needs a display");
        return;
    }
    let dir = Dir::new("shot_miss");
    let out = run(&dir.0, &["two-cards.designos", "--select", "Card Z", "--size", "800x600", "--no-toast", "--screenshot", "shot.png"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("--select \"Card Z\" matched no node"), "{}", stderr(&out));
    assert!(!dir.path("shot.png").exists(), "no PNG is written");
}
