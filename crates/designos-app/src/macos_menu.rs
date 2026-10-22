//! macOS: Cmd+Q and the app menu's Quit item go through the same path as closing the window.
//!
//! winit builds a default app menu whose Quit item sends `terminate:` to `NSApp` (winit-0.30.13
//! `platform_impl/macos/menu.rs:71`). AppKit takes the Cmd+Q key for that item before egui sees it, and `terminate:`
//! ends the process through `applicationWillTerminate:` without a window close request, so the "Save changes"
//! questions and the autosave flush that a close runs (`Shell::draw_window`) never ran. [`install`] keeps winit's
//! menu and points only its Quit item at a [`QuitTarget`], which asks for the close.
//!
//! A minimized window needs one more step. eframe runs `App::ui` only while the window is visible
//! (eframe-0.35.0 `native/epi_integration.rs:292`), and it ends the app on a close request that the frame did not
//! answer with `CancelClose` (`:302-310`), so a close taken while the window is hidden skips the question. [`request_quit`]
//! brings the window back and keeps the quit pending; the shell sends the close from the first frame that can draw
//! the question ([`send_pending_quit`]).

use egui::{Context, ViewportCommand, ViewportId};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};

/// A Quit chosen while the window was hidden, waiting for the window to be visible. It lives in the egui context, so
/// every window (and every test) has its own.
#[derive(Clone, Copy, Default)]
struct QuitPending;

/// Is the window drawn? This is eframe's own test for running `App::ui` (`native/wgpu_integration.rs:648`): a
/// minimized or occluded window is not, and a window that has not reported yet counts as visible. `input_for` reads
/// what the last frame was told, which an `Occluded` event brings up to date. It reads the viewport map directly,
/// because `InputState::viewport` panics when no frame has run.
fn window_visible(ctx: &Context) -> bool {
    ctx.input_for(ViewportId::ROOT, |input| input.raw.viewports.get(&ViewportId::ROOT).and_then(|info| info.visible()).unwrap_or(true))
}

/// What the Quit item does: ask for the close of the window, as egui's own quit shortcut and the window's red button
/// do. The shell answers it like any close: it asks about each document that has unsaved edits, flushes the
/// autosave, and keeps the app open on Cancel.
///
/// While the window is hidden no frame draws the question, so the close is not asked for yet: the window is brought
/// back, the quit is kept pending, and `Shell::draw_window` asks for the close on the first visible frame
/// ([`send_pending_quit`]). Quit chosen again while it waits changes nothing, except that it asks for the window again.
pub(crate) fn request_quit(ctx: &Context) {
    // Between frames a command waits in the root viewport until the next pass hands it to eframe; sending one also asks
    // for that pass, so an idle window wakes up. The flag is set before any command goes out, and no egui lock is held
    // across a send: `send_viewport_cmd_to` takes the context lock that `data_mut` holds.
    if window_visible(ctx) {
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    ctx.data_mut(|data| data.insert_temp(egui::Id::NULL, QuitPending));
    // `Minimized(false)` leaves the Dock. `Focus` covers a window that is occluded but not minimized; winit ignores it
    // while the window is still miniaturized (winit-0.30.13 `window_delegate.rs:1573-1583`).
    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
}

/// Called by every frame that draws the window (`Shell::draw_window`). Turns a pending quit into the close request,
/// once, when the window is visible; the next frame answers it, so the question and Cancel work as for any close.
/// The window coming back wakes eframe (`Occluded(false)` asks for a repaint), so nothing here polls.
pub(crate) fn send_pending_quit(ctx: &Context) {
    // `draw_window` also runs for a hidden root when a child window is visible, so it checks the root itself.
    if !window_visible(ctx) {
        return;
    }
    if ctx.data_mut(|data| data.remove_temp::<QuitPending>(egui::Id::NULL)).is_some() {
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
    }
}

struct QuitTargetIvars {
    ctx: Context,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and `QuitTarget` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[ivars = QuitTargetIvars]
    struct QuitTarget;

    impl QuitTarget {
        // SAFETY: an action method takes the sender (an object or nil) and returns nothing.
        #[unsafe(method(requestQuit:))]
        fn request_quit_action(&self, _sender: Option<&AnyObject>) {
            request_quit(&self.ivars().ctx);
        }
    }
);

impl QuitTarget {
    fn new(ctx: Context) -> Retained<Self> {
        let this = Self::alloc().set_ivars(QuitTargetIvars { ctx });
        // SAFETY: the signature of `NSObject`'s `init` is correct.
        unsafe { msg_send![super(this), init] }
    }
}

/// The first item under `menu` (searching its submenus) whose action is `terminate:`.
fn terminate_item(menu: &NSMenu) -> Option<Retained<NSMenuItem>> {
    for index in 0..menu.numberOfItems() {
        let Some(item) = menu.itemAtIndex(index) else { continue };
        if item.action() == Some(sel!(terminate:)) {
            return Some(item);
        }
        if let Some(found) = item.submenu().as_deref().and_then(terminate_item) {
            return Some(found);
        }
    }
    None
}

/// Points the Quit item of the app menu at the window close of the app that draws into `ctx`. It keeps its title and
/// its Cmd+Q key, and every other item of winit's menu (About, Services, Hide, Hide Others, Show All) is untouched.
/// Call it once on the main thread, after winit has built the menu: the app creator of `eframe::run_native` runs from
/// winit's `resumed` event, which follows `menu::initialize`. Returns false when there is no menu or no Quit item to
/// point, in which case Quit keeps ending the app without the question.
pub(crate) fn install(ctx: &Context) -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    let Some(menu) = NSApplication::sharedApplication(mtm).mainMenu() else { return false };
    let Some(item) = terminate_item(&menu) else { return false };
    let target = QuitTarget::new(ctx.clone());
    let any: &AnyObject = &target;
    // SAFETY: `requestQuit:` is a method of `QuitTarget` with the action signature.
    unsafe {
        item.setTarget(Some(any));
        item.setAction(Some(sel!(requestQuit:)));
    }
    // An item holds its target weakly. This one is needed for the life of the process, so it is never released.
    std::mem::forget(target);
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// What AppKit does when the Quit item is picked or Cmd+Q is pressed: it sends the item's action, `requestQuit:`,
    /// to the item's target. This sends it to a target for `ctx` through the Objective-C runtime, as AppKit does.
    pub(crate) fn pick_quit(ctx: &Context) {
        let target = QuitTarget::new(ctx.clone());
        let sender: Option<&AnyObject> = None;
        // SAFETY: `requestQuit:` takes one object argument and returns nothing.
        let _: () = unsafe { msg_send![&*target, requestQuit: sender] };
    }

    /// `Shell::draw_window` also runs for a hidden root while a child window is visible. It must not take the quit
    /// then: the close goes out once, on the first frame in which the root itself is visible.
    #[test]
    fn a_pending_quit_waits_for_the_root_window_and_goes_out_once() {
        let ctx = Context::default();
        let frame = |hidden: bool| {
            let mut raw = egui::RawInput::default();
            raw.viewports.entry(ViewportId::ROOT).or_default().occluded = Some(hidden);
            ctx.run_ui(raw, |ui| send_pending_quit(ui.ctx())).viewport_output[&ViewportId::ROOT].commands.clone()
        };
        frame(true);
        request_quit(&ctx);
        assert_eq!(frame(true), vec![ViewportCommand::Minimized(false), ViewportCommand::Focus], "the window is asked to come back");
        assert!(frame(true).is_empty(), "no close while the root is hidden, and nothing else either");
        assert_eq!(frame(false), vec![ViewportCommand::Close], "the root is visible: the quit asks for the close");
        assert!(frame(false).is_empty(), "once");
    }

    /// No frame has run yet, so egui knows nothing about the window: it counts as visible, as in eframe, and Quit
    /// asks for the close at once instead of waiting for a window that never reported.
    #[test]
    fn quit_before_the_first_frame_asks_for_the_close() {
        let ctx = Context::default();
        request_quit(&ctx);
        let out = ctx.run_ui(egui::RawInput::default(), |_| {});
        assert!(out.viewport_output[&ViewportId::ROOT].commands.contains(&ViewportCommand::Close));
    }
}
