//! Which popups are open, for a surface that draws before them in the frame.
//!
//! The Agents panel draws before the inspector, the context menus and the dialogs of the same frame, yet its terminal must
//! not take the `Esc`, `Enter` and arrow keys a popup is about to read. egui has no "is a popup open" query, and the open
//! state of many popups lives in egui temp data under ids their host builds, so the panel cannot read it.
//!
//! Every shared popup helper therefore calls [`mark_open`] each time it draws a popup that stays open, and so does each
//! popover built by hand (the colour picker, the font dropdown, the prototype and Variables popovers), and the panel asks
//! [`open_recently`]. Only the number of the pass is stored. A popup whose host stops drawing (its node was deleted, its
//! view closed) stops counting after one frame, so it can never leave the terminal deaf, and a popup that closed in a frame
//! is not marked in that frame. The exceptions are a board menu pick and a `dropdown` closed by its own anchor: they mark in
//! the pass that closes them, and the follow-up pass egui runs after any input consumes that mark before the next key.
//!
//! Only a popup that closes on `Esc` and on an outside press may mark itself. One that does neither, like the Check designs
//! panel, would keep the terminal deaf until the user found its close button.
//!
//! Cost: one write of a `u64` into a slot that already exists (`get_temp_mut_or_default` keeps the boxed value after the
//! first insert), so no allocation and no repaint request per frame. Nothing is stored while no popup is open.

use egui::{Context, Id};

fn slot() -> Id {
    Id::new("popup-open-pass")
}

/// Marks a popup as open in the current pass. Call it where a popup is drawn and stays open.
///
/// The pass number is read before the write: `data_mut` holds the context lock, and `cumulative_pass_nr` takes it too.
pub fn mark_open(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr() + 1;
    ctx.data_mut(|d| *d.get_temp_mut_or_default::<u64>(slot()) = pass);
}

/// True when a popup was drawn open in the current pass or the one before it. Pass `p` stores `p + 1`, so `0` means never.
pub fn open_recently(ctx: &Context) -> bool {
    let now = ctx.cumulative_pass_nr() + 1;
    ctx.data(|d| d.get_temp::<u64>(slot())).is_some_and(|seen| seen + 1 >= now)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One pass of `ctx`; `draw` runs inside it. Returns the repaint delay the pass asked for.
    fn pass(ctx: &Context, draw: impl FnOnce(&Context)) -> std::time::Duration {
        let mut draw = Some(draw);
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(draw) = draw.take() {
                draw(ui.ctx());
            }
        });
        out.viewport_output.get(&egui::ViewportId::ROOT).map_or(std::time::Duration::MAX, |v| v.repaint_delay)
    }

    #[test]
    fn nothing_is_open_before_a_popup_is_marked() {
        let ctx = Context::default();
        pass(&ctx, |ctx| assert!(!open_recently(ctx)));
        pass(&ctx, |ctx| assert!(!open_recently(ctx)));
    }

    #[test]
    fn a_marked_popup_counts_in_its_own_pass_and_the_next_and_then_stops() {
        let ctx = Context::default();
        pass(&ctx, mark_open);
        // The next pass is the one the Agents panel asks in, before the popup draws again.
        pass(&ctx, |ctx| assert!(open_recently(ctx), "marked in the previous pass"));
        // That pass did not mark it again: the host stopped drawing, so it counts for no longer.
        pass(&ctx, |ctx| assert!(!open_recently(ctx), "not marked in the previous pass"));
        pass(&ctx, |ctx| assert!(!open_recently(ctx)));
    }

    #[test]
    fn a_popup_drawn_open_every_pass_keeps_counting() {
        let ctx = Context::default();
        for _ in 0..4 {
            pass(&ctx, |ctx| {
                let _ = open_recently(ctx);
                mark_open(ctx);
            });
        }
        pass(&ctx, |ctx| assert!(open_recently(ctx)));
    }

    #[test]
    fn marking_requests_no_repaint() {
        let ctx = Context::default();
        pass(&ctx, |_| {});
        let without = pass(&ctx, |_| {});
        let with = pass(&ctx, |ctx| {
            mark_open(ctx);
            let _ = open_recently(ctx);
        });
        assert_eq!(with, without, "an open popup must not make an idle window repaint more than it would anyway");
    }
}
