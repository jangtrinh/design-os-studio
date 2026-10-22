//! macOS: Cmd+Q and the app menu's Quit item go through the same path as closing the window.
//!
//! winit builds a default app menu whose Quit item sends `terminate:` to `NSApp` (winit-0.30.13
//! `platform_impl/macos/menu.rs:71`). AppKit takes the Cmd+Q key for that item before egui sees it, and `terminate:`
//! ends the process through `applicationWillTerminate:` without a window close request, so the "Save changes"
//! questions and the autosave flush that a close runs (`Shell::draw_window`) never ran. [`install`] keeps winit's
//! menu and points only its Quit item at a [`QuitTarget`], which asks for the close.

use egui::{Context, ViewportCommand, ViewportId};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};

/// What the Quit item does: ask for the close of the window, as egui's own quit shortcut and the window's red button
/// do. The shell answers it like any close: it asks about each document that has unsaved edits, flushes the
/// autosave, and keeps the app open on Cancel.
pub(crate) fn request_quit(ctx: &Context) {
    // Between frames this waits in the root viewport until the next pass hands it to eframe; sending it also asks
    // for that pass, so an idle window wakes up.
    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
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
}
