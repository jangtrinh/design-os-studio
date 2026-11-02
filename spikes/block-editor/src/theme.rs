//! THROWAWAY: the handful of Studio chrome constants the spike needs, copied from
//! `crates/designos-app/src/theme.rs` (that crate is bin-only, so it cannot be a path dependency).
//! Fonts come from the real `designos-fonts` crate, so Vietnamese glyph coverage is the shipping one.

use egui::{Color32, FontFamily, FontId};

pub const BG: Color32 = Color32::from_rgb(0x0A, 0x0A, 0x0A);
pub const BG_SECONDARY: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
pub const BG_SELECTED: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
pub const BG_BRAND: Color32 = Color32::from_rgb(0xE3, 0x63, 0x23);
pub const TEXT: Color32 = Color32::from_rgb(0xFA, 0xFA, 0xFA);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xD4, 0xD4, 0xD4);
pub const TEXT_TERTIARY: Color32 = Color32::from_rgb(0xA3, 0xA3, 0xA3);
pub const TEXT_PLACEHOLDER: Color32 = Color32::from_rgb(0x73, 0x73, 0x73);
pub const ICON_QUATERNARY: Color32 = Color32::from_rgb(0x73, 0x73, 0x73);
pub const BG_TEXT_SELECTION: Color32 = Color32::from_rgb(0x43, 0x14, 0x07);

pub const FAMILY_STRONG: &str = "spike-strong";
pub const FAMILY_MEDIUM: &str = "spike-medium";

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn strong(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_STRONG.into()))
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_MEDIUM.into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Same recipe as Studio's `install_fonts`: Inter variable font, one instance per weight, as separate
/// egui families. A bold run is therefore a different `FontId`, not a synthetic bold.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let inter = designos_fonts::bundled().iter().find(|a| a.family == "Inter" && !a.italic).expect("bundled Inter");
    let instance = |weight: f32| {
        let tweak = egui::FontTweak { coords: egui::epaint::text::VariationCoords::new([("wght", weight), ("opsz", 14.0)]), ..Default::default() };
        std::sync::Arc::new(egui::FontData::from_static(inter.data).tweak(tweak))
    };
    fonts.font_data.insert("ui-regular".into(), instance(400.0));
    fonts.font_data.insert("ui-medium".into(), instance(500.0));
    fonts.font_data.insert("ui-strong".into(), instance(600.0));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "ui-regular".into());
    let base = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (name, data) in [(FAMILY_STRONG, "ui-strong"), (FAMILY_MEDIUM, "ui-medium")] {
        let mut fam = vec![data.to_string()];
        fam.extend(base.clone());
        fonts.families.insert(FontFamily::Name(name.into()), fam);
    }
    let mono = designos_fonts::bundled().iter().find(|a| a.family == "Roboto Mono" && !a.italic).expect("bundled Roboto Mono");
    fonts.font_data.insert("ui-mono".into(), std::sync::Arc::new(egui::FontData::from_static(mono.data)));
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "ui-mono".into());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.extreme_bg_color = BG;
    v.selection.bg_fill = BG_TEXT_SELECTION;
    v.selection.stroke = egui::Stroke::new(1.0, TEXT);
    v.text_cursor.stroke = egui::Stroke::new(1.5, TEXT);
    v.text_cursor.blink = false; // idle 0 % CPU: a blinking caret schedules a repaint twice a second
    v.override_text_color = Some(TEXT_SECONDARY);
    ctx.set_global_style(style);
}
