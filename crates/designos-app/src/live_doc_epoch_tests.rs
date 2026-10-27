//! The agent's log orders its document switches against its edits: an edit made before the
//! agent opened another document never lands on that document, however the editor's polls and
//! its busy canvas happen to batch the log.

use crate::live_sync_interaction_tests::{apply_pending, live, Live};
use designos_doc::live_patch::{DocEpoch, LIVE_PATCH_VERSION};
use designos_doc::{Document, Node, NodeId, Rect};
use serde_json::json;

/// A second document whose only layer has the same id as the editor's hero layer, so an edit
/// that crosses over cannot miss.
fn other_document_with_colliding_id(hero: NodeId) -> (Document, NodeId) {
    let mut other = Document::new("Other");
    let page = other.add_page("Page");
    let theirs = other.alloc_id();
    assert_eq!(theirs, hero, "the repro needs both documents to number their layer alike");
    other.insert(page, Node::frame(theirs, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    other.get_mut(theirs).unwrap().name = "Fresh".into();
    (other, theirs)
}

fn agent_opens(l: &mut Live, other: &Document) {
    std::fs::write(l.workspace.join("other.designos"), other.to_json().unwrap()).unwrap();
    l.session.call("designos_open_file", json!({ "path": "other.designos", "discard_changes": true })).expect("open_file");
}

fn name_of(l: &Live, id: NodeId) -> String {
    l.app.doc.get(id).map(|n| n.name.clone()).unwrap_or_default()
}

fn toast(l: &Live) -> String {
    l.app.toast.as_ref().map(|t| t.text.clone()).unwrap_or_default()
}

#[test]
fn an_edit_made_before_the_agent_opened_another_document_does_not_land_on_it_when_both_arrive_in_one_poll() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    l.agent_renames(hero, "Agent hero");
    agent_opens(&mut l, &other);
    l.deliver();
    assert_eq!(l.app.doc.name, "Other");
    assert_eq!(name_of(&l, theirs), "Fresh", "the old document's rename landed on the new one");
    assert!(toast(&l).contains("1 earlier edit"), "the toast must say an edit was dropped: {:?}", toast(&l));
    l.finish();
}

#[test]
fn an_edit_made_before_the_agent_opened_another_document_does_not_land_on_it_during_a_drag() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    l.start_drag();
    l.agent_renames(hero, "Agent hero");
    agent_opens(&mut l, &other);
    l.deliver();
    assert_eq!(l.app.doc.name, "Live", "the document is not replaced in the middle of a drag");
    l.app.canvas.inter = crate::canvas::Interaction::None;
    apply_pending(&mut l.app);
    assert_eq!(l.app.doc.name, "Other");
    assert_eq!(name_of(&l, theirs), "Fresh", "the old document's rename landed on the new one");
    assert!(toast(&l).contains("1 earlier edit"), "the toast must say an edit was dropped: {:?}", toast(&l));
    l.finish();
}

#[test]
fn edits_made_after_the_switch_land_on_the_new_document_in_the_same_poll() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    l.agent_renames(hero, "Agent hero");
    agent_opens(&mut l, &other);
    l.agent_renames(theirs, "Agent fresh");
    l.deliver();
    assert_eq!(name_of(&l, theirs), "Agent fresh");
    assert!(toast(&l).contains("1 earlier edit"), "{:?}", toast(&l));
    l.finish();
}

#[test]
fn an_edit_already_applied_before_the_switch_is_not_counted_as_dropped() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    l.agent_renames(hero, "Agent hero");
    l.deliver();
    assert_eq!(name_of(&l, hero), "Agent hero");
    agent_opens(&mut l, &other);
    l.deliver();
    assert_eq!(name_of(&l, theirs), "Fresh");
    assert!(!toast(&l).contains("earlier edit"), "{:?}", toast(&l));
    l.finish();
}

#[test]
fn only_the_last_of_several_switches_in_one_poll_is_opened_and_edits_between_them_are_dropped() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    agent_opens(&mut l, &other);
    l.agent_renames(theirs, "Edit on the middle document");
    let mut last = Document::new("Last");
    let page = last.add_page("Page");
    let fresh = last.alloc_id();
    assert_eq!(fresh, hero);
    last.insert(page, Node::frame(fresh, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    last.get_mut(fresh).unwrap().name = "Last fresh".into();
    std::fs::write(l.workspace.join("last.designos"), last.to_json().unwrap()).unwrap();
    l.session.call("designos_open_file", json!({ "path": "last.designos", "discard_changes": true })).expect("open_file");
    l.deliver();
    assert_eq!(l.app.doc.name, "Last");
    assert_eq!(name_of(&l, fresh), "Last fresh");
    assert!(toast(&l).contains("1 earlier edit"), "{:?}", toast(&l));
    l.finish();
}

/// A document switch the editor cannot honour: the log says it was written by a newer version.
fn append_unsupported_switch(l: &Live) {
    use std::io::Write;
    let switch = DocEpoch { v: LIVE_PATCH_VERSION + 1, doc_epoch: 1, bytes: 0, hash: 0 };
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&l.app.terminal.live_patch.path).unwrap();
    writeln!(log, "{}", serde_json::to_string(&switch).unwrap()).unwrap();
}

/// A line from a build newer than this one, as its writer would append it.
fn append_newer_edit(l: &Live) {
    use std::io::Write;
    let line = format!(r#"{{"v":{},"ops":[{{"from_the_future":{{"id":1}}}}]}}"#, LIVE_PATCH_VERSION + 1);
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&l.app.terminal.live_patch.path).unwrap();
    writeln!(log, "{line}").unwrap();
}

#[test]
fn a_newer_edit_stops_what_follows_it_until_the_agent_opens_another_document() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    l.agent_renames(hero, "Before the newer edit");
    append_newer_edit(&l);
    // The newer edit may have removed or reshaped the hero layer; this later rename must not land on it.
    l.agent_renames(hero, "After the newer edit");
    l.deliver();
    assert_eq!(name_of(&l, hero), "Before the newer edit", "the edit before the newer one applies, the one after it does not");
    assert_eq!(toast(&l), "Agent edit not applied. Update Studio and designos-mcp to the same version.");
    // A document switch ends the fence.
    agent_opens(&mut l, &other);
    l.agent_renames(theirs, "Fresh agent");
    l.deliver();
    assert_eq!(l.app.doc.name, "Other");
    assert_eq!(name_of(&l, theirs), "Fresh agent", "edits on the new document apply again");
    l.finish();
}

#[test]
fn edits_dropped_on_a_failed_switch_are_still_reported_when_a_later_switch_in_the_same_poll_loads() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    append_unsupported_switch(&l);
    l.agent_renames(hero, "Edit made on the unloadable document");
    agent_opens(&mut l, &other);
    l.deliver();
    assert_eq!(l.app.doc.name, "Other");
    assert_eq!(name_of(&l, theirs), "Fresh");
    let shown = toast(&l);
    assert!(shown.contains("1 earlier edit made on the previous document dropped"), "the dropped edit is reported: {shown:?}");
    assert_eq!(shown.matches("earlier edit").count(), 1, "reported once: {shown:?}");
    assert!(!shown.contains("not applied"), "reported once, not also as a failed load: {shown:?}");
    l.finish();
}

#[test]
fn edits_dropped_after_a_failed_load_are_reported_when_the_next_poll_brings_a_switch_that_loads() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    agent_opens(&mut l, &other);
    // The editor cannot read what the agent published.
    std::fs::remove_file(l.dir.join("doc-sync.designos")).unwrap();
    l.deliver();
    assert!(toast(&l).contains("Cannot read the document"), "the failed load is reported: {:?}", toast(&l));
    l.agent_renames(theirs, "Edit made while the document could not be loaded");
    let (mut second, _) = other_document_with_colliding_id(l.dragged);
    second.name = "Second".into();
    agent_opens(&mut l, &second);
    l.deliver();
    assert_eq!(l.app.doc.name, "Second");
    let shown = toast(&l);
    assert_eq!(shown.matches("1 earlier edit").count(), 1, "the dropped edit is reported once: {shown:?}");
    l.finish();
}

#[test]
fn edits_dropped_on_a_failed_switch_are_reported_when_the_good_switch_waits_for_a_drag_to_end() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    l.start_drag();
    append_unsupported_switch(&l);
    l.agent_renames(hero, "Edit made on the unloadable document");
    agent_opens(&mut l, &other);
    l.deliver();
    assert_eq!(l.app.doc.name, "Live", "the document is not replaced in the middle of a drag");
    l.app.canvas.inter = crate::canvas::Interaction::None;
    apply_pending(&mut l.app);
    assert_eq!(l.app.doc.name, "Other");
    assert_eq!(name_of(&l, theirs), "Fresh");
    let shown = toast(&l);
    assert_eq!(shown.matches("1 earlier edit").count(), 1, "the dropped edit is reported once: {shown:?}");
    l.finish();
}

#[test]
fn an_unloadable_document_is_reported_and_the_edits_made_on_it_are_dropped_not_misapplied() {
    let mut l = live();
    let (other, theirs) = other_document_with_colliding_id(l.dragged);
    let hero = l.dragged;
    agent_opens(&mut l, &other);
    // The editor cannot read what the agent published.
    std::fs::remove_file(l.dir.join("doc-sync.designos")).unwrap();
    l.agent_renames(theirs, "Agent fresh");
    l.deliver();
    assert_eq!(l.app.doc.name, "Live");
    assert_eq!(name_of(&l, hero), "Hero", "an edit made on the unreadable document landed on the open one");
    assert!(toast(&l).contains("agent edit"), "the dropped edit is reported: {:?}", toast(&l));
    l.finish();
}

/// Two raw file writes in the session folder (what a sandboxed command can do without any MCP call) replace the document the
/// window shows. The skill Ask-mode Claude loads from the private plugin must not change with that document's name: Claude
/// acts on frontmatter keys, `!` placeholders and `@` paths in a skill, so a name in it would be text the document's author
/// chose, running with the user's rights.
#[test]
fn a_document_switched_in_by_raw_session_file_writes_leaves_the_private_plugin_skill_as_it_was() {
    for &name in crate::live_sync_tests::HOSTILE_NAMES {
        let mut l = live();
        l.app.terminal.permission_mode = crate::agent_permissions::AgentPermissionMode::Ask;
        let skill_path = crate::agent_approval_channel::claude_plugin_dir_path(&l.dir)
            .expect("a private channel folder")
            .join(crate::agent_permissions::CLAUDE_PLUGIN_SKILL_PATH);
        // What the plugin holds while the window shows its own, plainly named document.
        crate::live_patch_apply::publish_live_context(&mut l.app);
        l.app.terminal.flush_live_sync();
        let plain_skill = std::fs::read_to_string(&skill_path).expect("plugin skill written by the first sync");
        let mut forged = Document::new(name);
        let _ = forged.add_page("Page 1");
        let bytes = forged.to_json().unwrap();
        std::fs::write(l.dir.join("doc-sync.designos"), &bytes).unwrap();
        {
            use std::io::Write;
            let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&l.app.terminal.live_patch.path).unwrap();
            writeln!(log, "{}", serde_json::to_string(&DocEpoch::of(1, bytes.as_bytes())).unwrap()).unwrap();
        }
        l.deliver();
        crate::live_patch_apply::apply_pending_live_patches(&mut l.app);
        assert_eq!(&l.app.doc.name, name, "the forged switch did not replace the document");
        crate::live_patch_apply::publish_live_context(&mut l.app);
        l.app.terminal.flush_live_sync();
        let skill = std::fs::read_to_string(&skill_path).expect("plugin skill written by the sync");
        assert_eq!(skill, plain_skill, "the skill Ask-mode Claude loads changed with the document name {name:?}");
        crate::live_sync_tests::assert_skill_frontmatter_is_name_and_fixed_description(&skill);
        l.finish();
    }
}
