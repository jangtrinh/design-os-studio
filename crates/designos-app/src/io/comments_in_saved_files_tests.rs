//! What a save says about comments: only the `.designos` format keeps them, so a save to `.fig` or `.pen` tells the
//! user once per save, and a save that loses nothing stays quiet.

use super::*;
use crate::app::App;
use designos_doc::{CommentThread, Document, Vec2};

const NOTICE: &str = "Comments left out: only .designos files keep them";

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_comment_notice_{}_{name}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A document with one page and, when `comments` is `Some(resolved)`, one comment thread.
fn doc_with(comments: Option<bool>) -> Document {
    let mut doc = Document::new("Doc");
    let page = doc.add_page("Page");
    if let Some(resolved) = comments {
        let anchor = crate::comments::model::make_anchor(&doc, page, Vec2::new(10.0, 10.0), None);
        let message = crate::comments::model::comment("m1".into(), "Kir", "Make this bigger");
        doc.comments.push(CommentThread { id: "t1".into(), anchor, messages: vec![message], resolved });
    }
    doc
}

fn save_as(app: &mut App, path: &Path) {
    app.save_with(true, &ScriptedDialogs::answering(DialogOutcome::Picked(path.to_path_buf())));
}

fn toast_of(app: &App) -> (String, bool) {
    let t = app.toast.as_ref().expect("a save always says how it went");
    (t.text.clone(), t.error)
}

#[test]
fn saving_comments_to_fig_or_pen_says_they_are_not_kept_and_still_saves() {
    let dir = scratch_dir("fig_pen");
    for name in ["Doc.fig", "Doc.pen", "Doc.FIG"] {
        let path = dir.join(name);
        let mut app = App::new(doc_with(Some(false)));
        save_as(&mut app, &path);
        assert!(path.is_file(), "{name}: the file is still written");
        assert_eq!(toast_of(&app), (format!("Saved {}. {NOTICE}", path.display()), true), "{name}: a warning, not a plain success");
        assert_eq!(app.doc.comments.len(), 1, "{name}: the open document keeps its comments");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_resolved_thread_is_still_a_comment_the_file_cannot_keep() {
    let dir = scratch_dir("resolved");
    let path = dir.join("Doc.fig");
    let mut app = App::new(doc_with(Some(true)));
    save_as(&mut app, &path);
    assert!(toast_of(&app).0.ends_with(NOTICE), "{}", toast_of(&app).0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_save_that_loses_no_comment_says_nothing_about_comments() {
    let dir = scratch_dir("quiet");
    // no comments to lose
    for name in ["Doc.fig", "Doc.pen", "Doc.designos"] {
        let path = dir.join(name);
        let mut app = App::new(doc_with(None));
        save_as(&mut app, &path);
        assert_eq!(toast_of(&app), (format!("Saved {}", path.display()), false), "{name}");
    }
    // the native writer keeps comments, in the native extension and in any other name it writes natively
    for name in ["Kept.designos", "Kept.json", "Kept"] {
        let mut app = App::new(doc_with(Some(false)));
        save_as(&mut app, &dir.join(name));
        let (text, error) = toast_of(&app);
        assert!(!text.contains(NOTICE) && text.starts_with("Saved ") && !error, "{name}: {text}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn every_save_to_fig_tells_again_because_every_save_leaves_the_comments_out_again() {
    let dir = scratch_dir("every_save");
    let mut app = App::new(doc_with(Some(false)));
    for name in ["First.fig", "Second.fig"] {
        app.toast = None;
        let path = dir.join(name);
        save_as(&mut app, &path);
        assert_eq!(toast_of(&app).0, format!("Saved {}. {NOTICE}", path.display()), "{name}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn only_the_fig_and_pen_writers_leave_comments_out_and_the_rule_matches_the_writer() {
    let doc = doc_with(Some(false));
    for (name, left_out) in
        [("a.fig", 1), ("a.FIG", 1), ("a.pen", 1), ("a.Pen", 1), ("a.designos", 0), ("a.json", 0), ("a", 0), ("dir.fig/a", 0), ("a.fig.designos", 0)]
    {
        assert_eq!(comment_threads_left_out(&doc, Path::new(name)), left_out, "{name}");
    }
    // the writer and the rule read one extension test: what the rule says is left out is what the file lacks
    let dir = scratch_dir("rule_matches_writer");
    for name in ["rule.fig", "rule.pen", "rule.designos", "rule.json"] {
        let written = save_path(&doc, &dir.join(name)).unwrap();
        let on_disk = std::fs::read(&written).unwrap();
        let holds_comment = String::from_utf8_lossy(&on_disk).contains("Make this bigger");
        assert_eq!(holds_comment, comment_threads_left_out(&doc, &written) == 0, "{name}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_rule_counts_every_thread_including_resolved_ones_and_carries_the_pinned_copy() {
    let mut doc = doc_with(Some(true));
    let again = doc.comments[0].clone();
    doc.comments.push(CommentThread { id: "t2".into(), resolved: false, ..again });
    assert_eq!(comment_threads_left_out(&doc, Path::new("x.fig")), 2);
    assert_eq!(COMMENTS_ONLY_IN_NATIVE_NOTICE, NOTICE, "the copy these tests pin is the copy the code carries");
}
