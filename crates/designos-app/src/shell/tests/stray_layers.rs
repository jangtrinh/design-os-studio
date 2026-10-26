//! Layers the user never made: a rewritten `live-spec.json` and a stale in-app clipboard must not add copies.

use super::*;
use crate::terminal::TerminalAction;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_stray_layers_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create stray_layers scratch dir");
    dir.join(name)
}

fn spec_json(root: &str, label: &str) -> String {
    format!(
        r##"{{ "kind": "frame", "name": "{root}", "width": 200, "height": 120,
              "children": [ {{ "kind": "text", "name": "Label", "text": "{label}" }} ] }}"##
    )
}

/// The agent writes `json` to its live spec file and the editor hot-applies it, as one `ApplySpecFile` action.
fn hot_apply(s: &mut Sim, path: &PathBuf, json: &str) {
    std::fs::write(path, json).unwrap();
    crate::shell::execute_terminal_actions(&mut s.shell.app, vec![TerminalAction::ApplySpecFile(path.clone())]);
}

fn page_children(s: &Sim) -> Vec<NodeId> {
    let app = s.app();
    app.doc.get(app.page).unwrap().children.clone()
}

fn named(s: &Sim, name: &str) -> Vec<NodeId> {
    s.app().doc.nodes.values().filter(|n| n.name == name).map(|n| n.id).collect()
}

#[test]
fn rewriting_an_unchanged_live_spec_changes_nothing() {
    let path = tmp("unchanged.json");
    let mut s = Sim::new();
    hot_apply(&mut s, &path, &spec_json("Login", "Hello"));
    let (children, nodes, depth) = (page_children(&s), s.count(), s.app().hist.undo_depth());
    for _ in 0..3 {
        hot_apply(&mut s, &path, &spec_json("Login", "Hello"));
    }
    std::fs::remove_file(&path).ok();
    assert_eq!(page_children(&s), children, "no second copy on the page");
    assert_eq!(s.count(), nodes);
    assert_eq!(s.app().hist.undo_depth(), depth, "an unchanged spec leaves no undo step");
    assert_eq!(named(&s, "Login").len(), 1);
}

#[test]
fn a_changed_live_spec_replaces_its_earlier_output() {
    let path = tmp("changed.json");
    let mut s = Sim::new();
    let before = page_children(&s).len();
    hot_apply(&mut s, &path, &spec_json("Login", "Hello"));
    let first = named(&s, "Login")[0];
    let index = page_children(&s).iter().position(|c| *c == first).unwrap();
    hot_apply(&mut s, &path, &spec_json("Login", "Goodbye"));
    std::fs::remove_file(&path).ok();

    let now = page_children(&s);
    assert_eq!(now.len(), before + 1, "one output on the page, not two");
    let logins = named(&s, "Login");
    assert_eq!(logins.len(), 1, "the earlier output is gone");
    assert_ne!(logins[0], first);
    assert_eq!(now.iter().position(|c| *c == logins[0]), Some(index), "the new output takes the old one's place");
    assert_eq!(s.app().selection, vec![logins[0]]);
    let texts: Vec<String> = s.app().doc.nodes.values().filter_map(|n| n.kind.any_text().map(|t| t.content.clone())).collect();
    assert!(texts.iter().any(|t| t == "Goodbye") && !texts.iter().any(|t| t == "Hello"), "{texts:?}");

    // one undo brings the earlier output back
    s.cmd_key(Key::Z);
    assert_eq!(named(&s, "Login"), vec![first]);
    assert_eq!(page_children(&s).len(), before + 1);
}

#[test]
fn each_live_spec_file_keeps_its_own_output() {
    let (a, b) = (tmp("tab-a.json"), tmp("tab-b.json"));
    let mut s = Sim::new();
    let before = page_children(&s).len();
    hot_apply(&mut s, &a, &spec_json("Screen A", "a"));
    hot_apply(&mut s, &b, &spec_json("Screen B", "b"));
    hot_apply(&mut s, &a, &spec_json("Screen A", "a again"));
    std::fs::remove_file(&a).ok();
    std::fs::remove_file(&b).ok();
    assert_eq!(page_children(&s).len(), before + 2);
    assert_eq!(named(&s, "Screen A").len(), 1);
    assert_eq!(named(&s, "Screen B").len(), 1);
}

#[test]
fn a_changed_live_spec_never_replaces_a_layer_of_another_document() {
    let path = tmp("other-doc.json");
    let mut s = Sim::new();
    hot_apply(&mut s, &path, &spec_json("Login", "Hello"));
    let first = named(&s, "Login")[0];

    // The agent opened another document. Layer ids restart in every document, so this one has a layer with
    // the very id the earlier output had.
    let mut other = designos_doc::Document::new("Other");
    let page = other.add_page("Page");
    let mut theirs = None;
    while theirs != Some(first) {
        let id = other.alloc_id();
        if id == first {
            let mut n = designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
            n.name = "Keep me".into();
            other.insert(page, n).unwrap();
        }
        theirs = Some(id);
    }
    s.shell.app.replace_document(other, None);
    assert_eq!(named(&s, "Keep me"), vec![first], "precondition: the new document has a layer with the old output's id");

    hot_apply(&mut s, &path, &spec_json("Login", "Goodbye"));
    std::fs::remove_file(&path).ok();
    assert_eq!(named(&s, "Keep me"), vec![first], "the other document's layer survives");
    assert_eq!(named(&s, "Login").len(), 1, "the spec lands on the new document");
}

#[test]
fn text_from_another_app_never_pastes_the_stale_in_app_copy() {
    let mut s = Sim::new();
    let button = s.by_name("Button");
    s.shell.app.selection = vec![button];
    let json = s.shell.app.copy_selection().expect("copied");
    let buttons = named(&s, "Button").len();

    // the system clipboard now holds words from a browser, newer than the in-app copy
    s.frame(vec![Event::Paste("words from a browser".into())]);
    assert_eq!(named(&s, "Button").len(), buttons, "the copied layer did not come back");

    // Studio's own copy still pastes
    s.frame(vec![Event::Paste(json)]);
    assert_eq!(named(&s, "Button").len(), buttons + 1);
}
