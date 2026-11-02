//! An instance keeps its own aspect lock across a `.fig` save and reopen, and across a later sync of its main.
//!
//! The writer puts the lock into the instance's own override entry as `proportionsConstrained`; the importer has to
//! turn that key back into the lock value and the `LockAspect` override flag, or the reopened instance follows its
//! main again the next time the main is synced.
use designos_doc::*;
use designos_fig::{export_fig, import_fig};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn id_of(doc: &Document, name: &str) -> NodeId {
    find(doc, name).id
}

fn set_aspect_lock(doc: &mut Document, id: NodeId, locked: bool) {
    let mut n = doc.get(id).unwrap().clone();
    n.lock_aspect = locked;
    Command::Update { node: Box::new(n) }.apply(doc).unwrap();
}

fn flagged(doc: &Document, name: &str) -> bool {
    find(doc, name).component.overrides.contains(&OverrideField::LockAspect)
}

/// Two mains, `MainOpen` (not locked) and `MainLocked` (locked), each with two instances: one that sets its own lock
/// (the opposite of its main's) and one that follows its main.
fn mains_with_own_and_following_instances() -> Document {
    let mut doc = Document::new("lock-aspect-instances");
    let page = doc.add_page("Page 1");
    let mut mains = Vec::new();
    for (i, (name, locked)) in [("MainOpen", false), ("MainLocked", true)].into_iter().enumerate() {
        let frame_id = doc.alloc_id();
        let main = doc.insert(page, Node::frame(frame_id, Rect::new(0.0, 400.0 * i as f32, 100.0, 50.0))).unwrap();
        doc.get_mut(main).unwrap().name = name.into();
        create_component(&mut doc, main).apply(&mut doc).unwrap();
        set_aspect_lock(&mut doc, main, locked);
        mains.push(main);
    }
    let mut place = |main: NodeId, name: &str, x: f32, y: f32| {
        create_instance(&mut doc, main, page, Vec2::new(x, y)).apply(&mut doc).unwrap();
        let id = *doc.get(page).unwrap().children.last().unwrap();
        doc.get_mut(id).unwrap().name = name.into();
        id
    };
    let own_lock = place(mains[0], "LockedOverOpenMain", 200.0, 0.0);
    place(mains[0], "FollowsOpenMain", 350.0, 0.0);
    let own_unlock = place(mains[1], "OpenOverLockedMain", 200.0, 400.0);
    place(mains[1], "FollowsLockedMain", 350.0, 400.0);
    let n = doc.get_mut(own_lock).unwrap();
    n.lock_aspect = true;
    n.component.overrides.insert(OverrideField::LockAspect);
    let n = doc.get_mut(own_unlock).unwrap();
    n.lock_aspect = false;
    n.component.overrides.insert(OverrideField::LockAspect);
    doc
}

fn reopened() -> Document {
    let doc = mains_with_own_and_following_instances();
    assert!(find(&doc, "LockedOverOpenMain").lock_aspect && !find(&doc, "MainOpen").lock_aspect, "fixture: locked instance of an open main");
    assert!(!find(&doc, "OpenOverLockedMain").lock_aspect && find(&doc, "MainLocked").lock_aspect, "fixture: open instance of a locked main");
    import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig")
}

#[test]
fn an_instance_whose_lock_differs_from_its_main_keeps_both_the_lock_and_the_override_through_a_fig_reopen() {
    let doc = reopened();
    let locked = find(&doc, "LockedOverOpenMain");
    assert!(locked.lock_aspect, "an instance locked over an open main stays locked");
    assert!(flagged(&doc, "LockedOverOpenMain"), "the lock is recorded as the instance's own");
    let open = find(&doc, "OpenOverLockedMain");
    assert!(!open.lock_aspect, "an instance opened over a locked main stays open");
    assert!(flagged(&doc, "OpenOverLockedMain"), "the open state is recorded as the instance's own");
    assert!(!flagged(&doc, "FollowsOpenMain") && !flagged(&doc, "FollowsLockedMain"), "an instance that follows its main records no lock override");
    assert!(!find(&doc, "FollowsOpenMain").lock_aspect && find(&doc, "FollowsLockedMain").lock_aspect, "followers have their main's lock");
}

#[test]
fn a_reopened_instance_keeps_its_own_lock_when_its_main_is_locked_or_unlocked_and_synced() {
    let mut doc = reopened();
    let (open_main, locked_main) = (id_of(&doc, "MainOpen"), id_of(&doc, "MainLocked"));

    // Lock the open main and unlock the locked one, then sync both.
    set_aspect_lock(&mut doc, open_main, true);
    set_aspect_lock(&mut doc, locked_main, false);
    for main in [open_main, locked_main] {
        sync_instances(&mut doc, main).apply(&mut doc).unwrap();
    }
    assert!(find(&doc, "FollowsOpenMain").lock_aspect, "a follower gets the lock of the main that was locked");
    assert!(!find(&doc, "FollowsLockedMain").lock_aspect, "a follower gets the open state of the main that was unlocked");
    assert!(find(&doc, "LockedOverOpenMain").lock_aspect, "the instance that locked itself stays locked when its main is locked too");
    assert!(!find(&doc, "OpenOverLockedMain").lock_aspect, "the instance that opened itself stays open when its main is unlocked too");

    // Put the mains back: the own values now differ from the mains again, and must survive a second sync.
    set_aspect_lock(&mut doc, open_main, false);
    set_aspect_lock(&mut doc, locked_main, true);
    for main in [open_main, locked_main] {
        sync_instances(&mut doc, main).apply(&mut doc).unwrap();
    }
    assert!(find(&doc, "LockedOverOpenMain").lock_aspect, "the own lock survives the main being unlocked and synced");
    assert!(!find(&doc, "OpenOverLockedMain").lock_aspect, "the own open state survives the main being locked and synced");
    assert!(!find(&doc, "FollowsOpenMain").lock_aspect && find(&doc, "FollowsLockedMain").lock_aspect, "followers keep following");
}
