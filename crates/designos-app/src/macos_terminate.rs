//! macOS: Dock Quit, logout, shutdown and the `quit` Apple Event go through the window close when edits are unsaved.
//!
//! [`crate::macos_menu::install`] redirects the app menu's Quit item (and Cmd+Q) to [`crate::macos_menu::request_quit`],
//! which asks for the close of the window. The Dock menu's Quit item, a system logout or shutdown, and `osascript`
//! send `terminate:` directly to `NSApp` instead. AppKit then asks `[NSApp delegate]` for
//! `applicationShouldTerminate:`. winit's `WinitApplicationDelegate` does not implement that method
//! (winit-0.30.13 `platform_impl/macos/app_state.rs:47-74`), so AppKit defaults to `NSTerminateNow` and ends the
//! process through `applicationWillTerminate:` without raising a window close request.
//!
//! [`install`] adds `applicationShouldTerminate:` to the delegate class on the main thread at startup, and [`sync`]
//! keeps the current [`TerminateDecision`] in the egui context as the document state changes. When any open or parked
//! document has unsaved edits (or an open user edit or unflushed library autosave), `applicationShouldTerminate:`
//! asks for the window close via [`crate::macos_menu::request_quit`] and returns `NSTerminateCancel`, so the same
//! "Save changes to <name>?" question and library autosave flush run as for Cmd+Q. When nothing is unsaved (or the
//! quit has already been approved), it returns `NSTerminateNow`.

use crate::app::App;
use egui::Context;
use objc2::encode::Encode;
use objc2::runtime::{AnyClass, AnyObject, Imp, ProtocolObject, Sel};
use objc2::{sel, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSApplicationTerminateReply};
use std::cell::RefCell;
use std::ffi::CString;
use std::sync::OnceLock;

/// What `applicationShouldTerminate:` should do for the current application state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminateDecision {
    /// At least one document has unsaved edits, an open user edit, an unflushed library autosave, or an open leave
    /// question: cancel immediate termination and route through the window close so library edits flush and non-library
    /// documents ask "Save changes to <name>?".
    CancelAndAsk,
    /// Nothing is unsaved (or the user has already answered every question for this quit): terminate immediately.
    QuitNow,
}

thread_local! {
    static ACTIVE_CTX: RefCell<Option<Context>> = const { RefCell::new(None) };
}

static MAIN_CTX: OnceLock<Context> = OnceLock::new();

/// Pure decision function: inspects `app` without hashing or allocating and decides whether `terminate:` must cancel
/// and route through the window close (`CancelAndAsk`) or let AppKit terminate at once (`QuitNow`).
pub(crate) fn terminate_decision(app: &App) -> TerminateDecision {
    if app.leave.quit_approved {
        return TerminateDecision::QuitNow;
    }
    if app.leave.is_asking() || app.terminal.live_patch.pending_doc.is_some() {
        return TerminateDecision::CancelAndAsk;
    }
    if app.home.is_none()
        && (app.has_unsaved_edits()
            || crate::live_patch_apply::user_edit_in_progress(app)
            || app.decks.notes_draft != app.decks.notes_saved
            || (app.lib_file.is_some() && app.autosave.save_failed_at.is_some()))
    {
        return TerminateDecision::CancelAndAsk;
    }
    if (0..app.tabs.entries.len()).any(|i| {
        let entry = &app.tabs.entries[i];
        entry.kind != crate::tabs::ProjectTabKind::Project
            && (crate::tabs::tab_is_unsaved(app, i)
                || !entry.hist.is_clean()
                || entry.dirty
                || (entry.lib_file.is_some() && entry.disk.save_failed_at.is_some()))
    }) {
        return TerminateDecision::CancelAndAsk;
    }
    TerminateDecision::QuitNow
}

/// Records `ctx` and the latest [`TerminateDecision`] for `app` so `applicationShouldTerminate:` can read it without
/// borrowing `Shell` while AppKit is inside `terminate:`.
pub(crate) fn sync(ctx: &Context, app: &App) {
    let decision = terminate_decision(app);
    ctx.data_mut(|data| data.insert_temp(egui::Id::NULL, decision));
    ACTIVE_CTX.with(|slot| *slot.borrow_mut() = Some(ctx.clone()));
}

/// Runs the synced [`TerminateDecision`] for `ctx`: on [`TerminateDecision::CancelAndAsk`], asks for the window close
/// (bringing a hidden window back first) and returns `NSTerminateCancel`; on [`TerminateDecision::QuitNow`], returns
/// `NSTerminateNow`.
pub(crate) fn should_terminate(ctx: &Context) -> NSApplicationTerminateReply {
    let decision = ctx.data_mut(|data| data.get_temp::<TerminateDecision>(egui::Id::NULL)).unwrap_or(TerminateDecision::QuitNow);
    match decision {
        TerminateDecision::CancelAndAsk => {
            crate::macos_menu::request_quit(ctx);
            NSApplicationTerminateReply::TerminateCancel
        }
        TerminateDecision::QuitNow => NSApplicationTerminateReply::TerminateNow,
    }
}

extern "C-unwind" fn application_should_terminate(_this: *mut AnyObject, _cmd: Sel, _sender: *mut AnyObject) -> NSApplicationTerminateReply {
    let ctx = ACTIVE_CTX.with(|slot| slot.borrow().clone()).or_else(|| MAIN_CTX.get().cloned());
    match ctx {
        Some(ctx) => should_terminate(&ctx),
        None => NSApplicationTerminateReply::TerminateNow,
    }
}

/// Adds or replaces `applicationShouldTerminate:` on `cls` and binds `ctx` as the active context on this thread.
fn install_on_class(cls: &AnyClass, ctx: &Context) -> bool {
    let Ok(types) =
        CString::new(format!("{}{}{}{}", NSApplicationTerminateReply::ENCODING, <*mut AnyObject>::ENCODING, Sel::ENCODING, <*mut AnyObject>::ENCODING,))
    else {
        return false;
    };
    ACTIVE_CTX.with(|slot| *slot.borrow_mut() = Some(ctx.clone()));
    let _ = MAIN_CTX.set(ctx.clone());
    // SAFETY: `application_should_terminate` uses the Objective-C IMP C-unwind calling convention with signature
    // `(self, _cmd, sender) -> NSApplicationTerminateReply`, matching the runtime type encoding in `types`.
    // `cls` is a valid registered Objective-C class, and `class_replaceMethod` is thread-safe in the Objective-C runtime.
    unsafe {
        let imp: Imp =
            std::mem::transmute::<extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> NSApplicationTerminateReply, Imp>(application_should_terminate);
        objc2::ffi::class_replaceMethod(cls as *const AnyClass as *mut AnyClass, sel!(applicationShouldTerminate:), imp, types.as_ptr());
    }
    cls.instance_method(sel!(applicationShouldTerminate:)).is_some()
}

/// Installs `applicationShouldTerminate:` on `NSApp`'s delegate class so Dock Quit, logout, shutdown, and the `quit`
/// Apple Event ask about unsaved edits through the window close path. Call once on the main thread after winit has set
/// its application delegate.
pub(crate) fn install(ctx: &Context) -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    let app = NSApplication::sharedApplication(mtm);
    let Some(delegate) = app.delegate() else { return false };
    let delegate_obj: &ProtocolObject<dyn NSApplicationDelegate> = &delegate;
    let cls = AnyObject::class(delegate_obj.as_ref());
    if !install_on_class(cls, ctx) {
        return false;
    }
    // Re-assign the delegate so AppKit refreshes any cached selector-response flags for `applicationShouldTerminate:`.
    app.setDelegate(Some(delegate_obj));
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use designos_doc::{Document, Node, Rect as DRect};
    use objc2::rc::Retained;
    use objc2::runtime::NSObject;
    use objc2::{define_class, msg_send, ClassType};

    define_class!(
        // SAFETY: `NSObject` has no subclassing requirements, and `TestTerminateDelegate` does not implement `Drop`.
        #[unsafe(super(NSObject))]
        struct TestTerminateDelegate;
    );

    impl TestTerminateDelegate {
        fn new() -> Retained<Self> {
            // SAFETY: `NSObject`'s `new` takes no arguments and returns a retained instance.
            unsafe { msg_send![Self::class(), new] }
        }
    }

    fn seeded_app() -> App {
        let mut doc = Document::new("Outside");
        let page = doc.add_page("P");
        let id = doc.alloc_id();
        doc.insert(page, Node::rectangle(id, DRect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        let mut app = App::new(doc);
        app.home = None;
        app.page = page;
        app.selection = vec![id];
        app
    }

    /// Sends `applicationShouldTerminate:` through the Objective-C runtime to a delegate class wired with
    /// [`install_on_class`], as AppKit does when `NSApp` receives `terminate:`.
    pub(crate) fn send_terminate_to_delegate(ctx: &Context) -> NSApplicationTerminateReply {
        assert!(install_on_class(TestTerminateDelegate::class(), ctx));
        let delegate = TestTerminateDelegate::new();
        let sender: Option<&AnyObject> = None;
        // SAFETY: `install_on_class` registered `applicationShouldTerminate:` on `TestTerminateDelegate` with one
        // object argument and an `NSApplicationTerminateReply` return value.
        unsafe { msg_send![&*delegate, applicationShouldTerminate: sender] }
    }

    #[test]
    fn terminate_decision_cancels_for_dirty_or_unflushed_documents_and_quits_when_clean_or_approved() {
        let mut app = seeded_app();
        assert_eq!(terminate_decision(&app), TerminateDecision::QuitNow, "a clean document can terminate at once");

        app.duplicate_selection();
        assert_eq!(terminate_decision(&app), TerminateDecision::CancelAndAsk, "an unsaved edit outside the library must cancel immediate termination and ask");

        app.leave.quit_approved = true;
        assert_eq!(terminate_decision(&app), TerminateDecision::QuitNow, "once the user has answered the quit question, termination proceeds");
        app.leave.quit_approved = false;

        // A library document with an edit not yet written by autosave must also cancel and run the close frame so
        // `may_quit` flushes the autosave before the process exits.
        app.lib_file = Some(crate::home::LibFile { id: "lib-1".into(), project_id: "proj-1".into(), project: "Project".into(), name: "In Library".into() });
        assert_eq!(
            terminate_decision(&app),
            TerminateDecision::CancelAndAsk,
            "a library document with an unwritten edit cancels immediate termination so the close frame flushes autosave"
        );

        app.hist.mark_clean();
        app.autosave.dot.adopt(false);
        assert_eq!(terminate_decision(&app), TerminateDecision::QuitNow, "a library document whose edits are saved can terminate at once");

        // An unflushed speaker-notes draft in a deck also requires the close frame to flush before exit.
        app.decks.notes_draft = "Speaker note".into();
        assert_eq!(terminate_decision(&app), TerminateDecision::CancelAndAsk, "an unflushed speaker-notes draft cancels immediate termination");
        app.decks.notes_saved = app.decks.notes_draft.clone();
        assert_eq!(terminate_decision(&app), TerminateDecision::QuitNow);

        // A dirty parked tab while on Home (or while another tab is active) also cancels immediate termination.
        let mut parked_doc = Document::new("Parked");
        let parked_page = parked_doc.add_page("P");
        let parked_node = parked_doc.alloc_id();
        parked_doc.insert(parked_page, Node::rectangle(parked_node, DRect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        app.open_new_tab("Parked", parked_doc, crate::tabs::ProjectTabKind::Design);
        app.page = parked_page;
        app.selection = vec![parked_node];
        app.duplicate_selection();
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.switch_to(0, &mut app);
        app.tabs = tabs;
        app.hist.mark_clean();
        app.autosave.dot.adopt(false);
        assert_eq!(
            terminate_decision(&app),
            TerminateDecision::CancelAndAsk,
            "an unsaved edit in a parked tab cancels immediate termination even when the active tab is clean"
        );
    }

    #[test]
    fn objective_c_delegate_returns_terminate_cancel_and_requests_close_when_dirty() {
        let ctx = Context::default();
        let mut app = seeded_app();
        sync(&ctx, &app);
        assert_eq!(send_terminate_to_delegate(&ctx), NSApplicationTerminateReply::TerminateNow);

        app.duplicate_selection();
        sync(&ctx, &app);
        assert_eq!(send_terminate_to_delegate(&ctx), NSApplicationTerminateReply::TerminateCancel);
        let out = ctx.run_ui(egui::RawInput::default(), |_| {});
        assert!(
            out.viewport_output[&egui::ViewportId::ROOT].commands.contains(&egui::ViewportCommand::Close),
            "cancelling immediate termination posts a window close request into the egui loop"
        );
    }
}
