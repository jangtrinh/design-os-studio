use crate::theme;
use egui::Color32;

/// Focus band colour. The 2 px gap and the outer radius are drawn by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingTone {
    Brand,
    Error,
}

pub const fn ring_color(tone: RingTone) -> Color32 {
    match tone {
        RingTone::Brand => theme::FOCUS_RING,
        RingTone::Error => theme::FOCUS_RING_ERROR,
    }
}
