//! Every visual constant of the editor chrome lives here.
//! Existing values retain the legacy Figma UI3 measurements in `assets/ui-spec.json`;
//! new editor work targets DesignOS's own visual design, not Figma screenshot parity.
//! Nothing outside this module may hardcode colors, sizes, radii or fonts.

use egui::{Color32, CornerRadius, FontFamily, FontId};

/// Opaque color from `0xRRGGBB`.
pub const fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Color from `0xRRGGBB` plus alpha 0..=255 (premultiplied internally).
pub const fn rgba(c: u32, a: u8) -> Color32 {
    let r = ((c >> 16) & 0xFF) * a as u32 / 255;
    let g = ((c >> 8) & 0xFF) * a as u32 / 255;
    let b = (c & 0xFF) * a as u32 / 255;
    Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, a)
}

// ---------------------------------------------------------------- colors (Studio dark chrome)
pub const BG: Color32 = rgb(0x0A0A0A);
pub const BG_PRIMARY: Color32 = rgb(0x0A0A0A);
pub const BG_SECONDARY: Color32 = rgb(0x171717);
pub const BG_TERTIARY: Color32 = rgb(0x262626);
pub const BG_HOVER: Color32 = rgb(0x171717);
pub const BG_PRESSED: Color32 = rgb(0x404040);
pub const BG_MENU: Color32 = rgb(0x0A0A0A);
// Selection is a neutral fill (P2): the selected tool, tab and row. Accent colour is for the
// one primary action and for live state; canvas selection stays blue.
pub const BG_SELECTED: Color32 = rgb(0x262626);
pub const BG_SELECTED_HOVER: Color32 = rgb(0x262626);
pub const BG_SELECTED_SECONDARY: Color32 = rgb(0x171717);
/// Highlight of selected text inside a text field (egui `selection.bg_fill`).
pub const BG_TEXT_SELECTION: Color32 = rgb(0x431407);
pub const BG_BRAND: Color32 = rgb(0xE36323);
pub const BG_BRAND_HOVER: Color32 = rgb(0xF97316);
pub const BG_BRAND_PRESSED: Color32 = rgb(0xEA580C);
pub const BG_TOOLBAR: Color32 = rgb(0x0A0A0A);
pub const BG_TOOLBAR_HOVER: Color32 = rgb(0x171717);
pub const BG_TOOLBAR_SELECTED: Color32 = BG_SELECTED;
pub const BG_TOOLBAR_MODE_SWITCHER: Color32 = rgb(0x171717);
pub const BG_SELECTED_AREA: Color32 = rgba(0x3582FF, 0x33);
pub const BORDER: Color32 = rgb(0x262626);
#[allow(dead_code)]
pub const BORDER_SUBTLE: Color32 = rgb(0x262626);
pub const BORDER_MENU: Color32 = rgb(0x262626);
pub const BORDER_SELECTED: Color32 = rgb(0xFB923C);
pub const BORDER_TRANSLUCENT: Color32 = rgba(0xFFFFFF, 0x1A);
pub const TEXT: Color32 = rgb(0xFAFAFA);
pub const TEXT_SECONDARY: Color32 = rgb(0xD4D4D4);
pub const TEXT_TERTIARY: Color32 = rgb(0xA3A3A3);
pub const TEXT_DISABLED: Color32 = rgba(0xD4D4D4, 0x80);
pub const TEXT_BRAND: Color32 = rgb(0xF97316);
pub const TEXT_COMPONENT: Color32 = rgb(0x22D3EE);
pub const ICON: Color32 = rgb(0xFFFFFF);
pub const ICON_SECONDARY: Color32 = rgb(0xD4D4D4);
pub const ICON_TERTIARY: Color32 = rgb(0xA3A3A3);
pub const ICON_SELECTED: Color32 = ICON;
pub const DANGER: Color32 = rgb(0xEF4444);
pub const INFO: Color32 = rgb(0x0D99FF);
pub const SUCCESS: Color32 = rgb(0x22C55E);
pub const WARNING: Color32 = rgb(0xEAB308);
#[allow(dead_code)] // spec colour, kept for parity with ui-spec.json
pub const BG_MENU_HOVER: Color32 = rgb(0x171717);
#[allow(dead_code)] // spec colour, kept for parity with ui-spec.json
pub const BG_TRANSPARENT_HOVER: Color32 = rgba(0xFFFFFF, 0x1A);
pub const BORDER_STRONG: Color32 = rgb(0x404040);
#[allow(dead_code)] // spec colour, kept for parity with ui-spec.json
pub const COMPONENT: Color32 = rgb(0x06B6D4);
#[allow(dead_code)] // spec colour, kept for parity with ui-spec.json
pub const TEXT_ON_LIGHT_CANVAS: Color32 = rgb(0x0A0A0A);
pub const SHADOW_COLOR: Color32 = rgba(0x000000, 0x00);
pub const SHADOW_COLOR_STRONG: Color32 = rgba(0x000000, 0x00);
/// Elevation 400: drop shadow colour, 0.5px inset ring (#ffffff59 at half coverage) and top edge.
/// Shared helpers no longer paint the shadow or the top edge, so those two stay for the contract.
#[allow(dead_code)]
pub const ELEVATION_400_SHADOW: Color32 = rgba(0x000000, 0x00);
pub const ELEVATION_400_RING: Color32 = rgb(0x262626);
#[allow(dead_code)]
pub const ELEVATION_400_TOP: Color32 = rgba(0xFFFFFF, 0x00);
pub const INNER_HIGHLIGHT: Color32 = rgb(0x262626);
pub const CHECKER_A: Color32 = rgb(0xFFFFFF);
pub const CHECKER_B: Color32 = rgb(0xCCCCCC);

// Studio dark colour roles beyond the legacy set. Constants no call site reads yet carry `#[allow(dead_code)]`;
// the surface that first uses one removes the attribute.
pub const BG_SECONDARY_HOVER: Color32 = rgb(0x262626);
#[allow(dead_code)]
pub const TEXT_SECONDARY_HOVER: Color32 = rgb(0xE5E5E5);
#[allow(dead_code)]
pub const TEXT_TERTIARY_HOVER: Color32 = rgb(0xD4D4D4);
#[allow(dead_code)]
pub const TEXT_QUATERNARY: Color32 = rgb(0xA3A3A3);
#[allow(dead_code)]
pub const TEXT_PLACEHOLDER: Color32 = rgb(0x737373);
#[allow(dead_code)]
pub const ICON_QUATERNARY: Color32 = rgb(0x737373);
#[allow(dead_code)]
pub const TEXT_ERROR: Color32 = rgb(0xF87171);
#[allow(dead_code)]
pub const TEXT_WARNING: Color32 = rgb(0xFACC15);
#[allow(dead_code)]
pub const TEXT_SUCCESS: Color32 = rgb(0x4ADE80);
#[allow(dead_code)]
pub const TEXT_ON_BRAND: Color32 = rgb(0x0A0A0A);
#[allow(dead_code)]
pub const FOCUS_RING: Color32 = rgb(0xF97316);
#[allow(dead_code)]
pub const BG_ERROR_SOLID: Color32 = rgb(0xDC2626);
#[allow(dead_code)]
pub const TEXT_WHITE: Color32 = rgb(0xFFFFFF);
#[allow(dead_code)]
pub const ICON_BRAND_PRIMARY: Color32 = rgb(0xF97316);
#[allow(dead_code)]
pub const ICON_SECONDARY_HOVER: Color32 = rgb(0xE5E5E5);
#[allow(dead_code)]
pub const ICON_QUATERNARY_HOVER: Color32 = rgb(0xA3A3A3);
#[allow(dead_code)]
pub const ICON_WHITE: Color32 = rgb(0xFFFFFF);
#[allow(dead_code)]
pub const BG_PRIMARY_ALT: Color32 = rgb(0x171717);
#[allow(dead_code)]
pub const BG_PRIMARY_SOLID: Color32 = rgb(0x262626);
#[allow(dead_code)]
pub const BG_ERROR_SOLID_HOVER: Color32 = rgb(0xB91C1C);
#[allow(dead_code)]
pub const BADGE_NEUTRAL_BG: Color32 = rgb(0x171717);
#[allow(dead_code)]
pub const BADGE_NEUTRAL_BORDER: Color32 = rgb(0x404040);
#[allow(dead_code)]
pub const BADGE_NEUTRAL_TEXT: Color32 = rgb(0xD4D4D4);
#[allow(dead_code)]
pub const BG_SUCCESS_PRIMARY: Color32 = rgb(0x052E16);
#[allow(dead_code)]
pub const BG_WARNING_PRIMARY: Color32 = rgb(0x422006);
#[allow(dead_code)]
pub const BG_ERROR_PRIMARY: Color32 = rgb(0x450A0A);
#[allow(dead_code)]
pub const BADGE_SUCCESS_BORDER: Color32 = rgb(0x166534);
#[allow(dead_code)]
pub const BADGE_WARNING_BORDER: Color32 = rgb(0x854D0E);
#[allow(dead_code)]
pub const BADGE_ERROR_BORDER: Color32 = rgb(0x991B1B);
#[allow(dead_code)]
pub const BADGE_SUCCESS_TEXT: Color32 = rgb(0x86EFAC);
#[allow(dead_code)]
pub const BADGE_WARNING_TEXT: Color32 = rgb(0xFDE047);
#[allow(dead_code)]
pub const BADGE_ERROR_TEXT: Color32 = rgb(0xFCA5A5);
#[allow(dead_code)]
pub const BG_OVERLAY: Color32 = rgba(0x262626, 0xB3);
#[allow(dead_code)]
pub const PROVIDER_CLAUDE: Color32 = rgb(0xD97757);
#[allow(dead_code)]
pub const PROVIDER_CODEX: Color32 = rgb(0xFAFAFA);
#[allow(dead_code)]
pub const PROVIDER_GROK: Color32 = rgb(0xE5E5E5);
#[allow(dead_code)]
pub const PROVIDER_GEMINI: Color32 = rgb(0x60A5FA);
#[allow(dead_code)]
pub const BG_ERROR_SOLID_PRESSED: Color32 = rgb(0x991B1B);
#[allow(dead_code)]
pub const TILE_DESIGN_BG: Color32 = rgb(0x172554);
#[allow(dead_code)]
pub const TILE_DESIGN_GLYPH: Color32 = rgb(0x60A5FA);
#[allow(dead_code)]
pub const TILE_BOARD_BG: Color32 = rgb(0x451A03);
#[allow(dead_code)]
pub const TILE_BOARD_GLYPH: Color32 = rgb(0xFBBF24);
#[allow(dead_code)]
pub const TILE_DECK_BG: Color32 = rgb(0x052E16);
#[allow(dead_code)]
pub const TILE_DECK_GLYPH: Color32 = rgb(0x4ADE80);
#[allow(dead_code)]
pub const TILE_SPEC_BG: Color32 = rgb(0x431407);
#[allow(dead_code)]
pub const TILE_SPEC_GLYPH: Color32 = rgb(0xFB923C);
#[allow(dead_code)]
pub const DEV_READY_HOVER: Color32 = rgb(0x30B870);
#[allow(dead_code)]
pub const HUD_POINTER_FILL: Color32 = rgb(0x111216);
#[allow(dead_code)]
pub const PLAYGROUND_GRID_DOT: Color32 = rgba(0xFAFAFA, 0x12);
#[allow(dead_code)]
pub const SWATCH_PLACEHOLDER: Color32 = rgb(0x404040);
#[allow(dead_code)]
pub const TYPE_PREVIEW_BASELINE: Color32 = rgba(0x0D99FF, 0x6E);
#[allow(dead_code)]
pub const CONTENT_HANDLE_FILL: Color32 = CANVAS_HANDLE_FILL;
#[allow(dead_code)]
pub const CONTENT_HANDLE_RING: Color32 = rgba(0x000000, 0x4D);
#[allow(dead_code)]
pub const CONTENT_HANDLE_CASING: Color32 = rgba(0x000000, 0x66);
#[allow(dead_code)]
pub const CONTENT_HANDLE_AXIS: Color32 = rgba(0xFFFFFF, 0xB2);
#[allow(dead_code)]
pub const CONTENT_HANDLE_STOP_IDLE: Color32 = rgba(0xFFFFFF, 0xCC);
#[allow(dead_code)]
pub const CONTENT_MARK_SHADOW: Color32 = rgba(0x000000, 0x6E);
#[allow(dead_code)]
pub const CONTENT_MARK_LIGHT: Color32 = rgba(0xFFFFFF, 0xDC);
#[allow(dead_code)]
pub const READER_LIGHT_BG_PAGE: Color32 = rgb(0xFAFAFA);
#[allow(dead_code)]
pub const READER_LIGHT_DOC_SHEET: Color32 = rgb(0xFFFFFF);
#[allow(dead_code)]
pub const READER_LIGHT_SURFACE: Color32 = rgb(0xFFFFFF);
#[allow(dead_code)]
pub const READER_LIGHT_SURFACE_2: Color32 = rgb(0xF5F5F5);
#[allow(dead_code)]
pub const READER_LIGHT_SURFACE_3: Color32 = rgb(0xFAFAFA);
#[allow(dead_code)]
pub const READER_LIGHT_CODE_BG: Color32 = rgb(0xFAFAFA);
#[allow(dead_code)]
pub const READER_LIGHT_BORDER: Color32 = rgb(0xE5E5E5);
#[allow(dead_code)]
pub const READER_LIGHT_HAIRLINE: Color32 = rgb(0xF5F5F5);
#[allow(dead_code)]
pub const READER_LIGHT_TEXT: Color32 = rgb(0x171717);
#[allow(dead_code)]
pub const READER_LIGHT_TEXT_MUTED: Color32 = rgb(0x404040);
#[allow(dead_code)]
pub const READER_LIGHT_TEXT_SUBTLE: Color32 = rgb(0x525252);
#[allow(dead_code)]
pub const READER_LIGHT_DOT_OK: Color32 = rgb(0x16A34A);
#[allow(dead_code)]
pub const READER_LIGHT_DOT_WARN: Color32 = rgb(0xCA8A04);
#[allow(dead_code)]
pub const READER_LIGHT_DOT_NEUTRAL: Color32 = rgb(0xA3A3A3);
#[allow(dead_code)]
pub const READER_LIGHT_HIGHLIGHT: Color32 = rgb(0xFEFCE8);
#[allow(dead_code)]
pub const FOCUS_RING_ERROR: Color32 = rgb(0xEF4444);
#[allow(dead_code)]
pub const BORDER_ERROR_SUBTLE: Color32 = rgb(0xEF4444);
#[allow(dead_code)]
pub const BORDER_ERROR: Color32 = rgb(0xF87171);
#[allow(dead_code)]
pub const TEXT_ERROR_HOVER: Color32 = rgb(0xFCA5A5);

// canvas overlays (EaseUI BrainWave Primary Blue #3582FF & Cyan #06B6D4)
pub const CANVAS_SELECTION: Color32 = rgb(0x3582FF);
pub const CANVAS_COMPONENT: Color32 = rgb(0x06B6D4);
pub const CANVAS_HANDLE_FILL: Color32 = rgb(0xFFFFFF);
pub const CANVAS_SIZE_BADGE_TEXT: Color32 = rgb(0xFFFFFF);
pub const CANVAS_LABEL: Color32 = rgba(0x000000, 0x80);
pub const CANVAS_LABEL_DARK: Color32 = rgba(0xFFFFFF, 0x99);
pub const CANVAS_MARQUEE_FILL: Color32 = BG_SELECTED_AREA;
pub const GUIDE_COLOR: Color32 = rgb(0xFE5938);
pub const GUIDE_TEXT: Color32 = rgb(0xFFFFFF);
pub const PEN_POINT: Color32 = rgb(0xFFFFFF);
pub const DROP_INDICATOR: Color32 = rgb(0xE36323);
/// Four agent presence colours (amber, emerald, pink, sky) and the dark label drawn on them.
pub const AGENT_1: Color32 = rgb(0xFFA629);
pub const AGENT_2: Color32 = rgb(0x14AE5C);
pub const AGENT_3: Color32 = rgb(0xEE46BC);
pub const AGENT_4: Color32 = rgb(0x38BDF8);
pub const AGENT_COLORS: [Color32; 4] = [AGENT_1, AGENT_2, AGENT_3, AGENT_4];
pub const AGENT_LABEL_TEXT: Color32 = rgb(0x0A0A0A);
pub const LIGHT_BG_THRESHOLD: f32 = 0.5;

// ---------------------------------------------------------------- fonts
/// Chrome captions: sublabels, rail labels, counts, chips (P5). The same size as the body: colour and
/// position tell a caption from a name.
pub const FONT_SIZE_CAPTION: f32 = 12.0;
/// Canvas overlays only (rulers, canvas pills, annotation marks): the declared exception to the chrome floor.
pub const FONT_SIZE_CANVAS: f32 = 9.0;
/// Rows, menu items and values in the editor panels.
pub const FONT_SIZE_BODY: f32 = 12.0;
/// Panel titles.
pub const FONT_SIZE_LARGE: f32 = 14.0;
pub const FONT_SIZE_TITLE: f32 = 20.0;
/// Line heights: body text and captions both 18px, the line of the 12 step (P5). A label box is one caption line.
pub const LINE_BODY: f32 = 18.0;
pub const LINE_CAPTION: f32 = 18.0;
pub const CHEVRON_SMALL: f32 = 10.0;
pub const FONT_SIZE_LABEL_CANVAS: f32 = 11.0;
// Text steps of the UI type scale: each size beside the line height it is set on. Chrome rows and labels
// take the 12 step, panel titles the 14 step; Home and dialogs set their text on the 14 step.
#[allow(dead_code)] // staged: the surfaces that set text on the step name it
pub const FONT_SIZE_TEXT_XS: f32 = 12.0;
#[allow(dead_code)] // staged: button labels and Home and dialog text
pub const FONT_SIZE_TEXT_SM: f32 = 14.0;
#[allow(dead_code)] // staged: dialog titles
pub const FONT_SIZE_TEXT_MD: f32 = 16.0;
#[allow(dead_code)] // staged
pub const FONT_SIZE_TEXT_LG: f32 = 18.0;
#[allow(dead_code)] // staged
pub const FONT_SIZE_TEXT_XL: f32 = 20.0;
#[allow(dead_code)] // staged: the line height of the 12 step
pub const LINE_TEXT_XS: f32 = 18.0;
#[allow(dead_code)] // staged: the line height of the 14 step
pub const LINE_TEXT_SM: f32 = 20.0;
#[allow(dead_code)] // staged
pub const LINE_TEXT_MD: f32 = 24.0;
#[allow(dead_code)] // staged
pub const LINE_TEXT_LG: f32 = 28.0;
#[allow(dead_code)] // staged
pub const LINE_TEXT_XL: f32 = 30.0;
pub const FAMILY_STRONG: &str = "designos-strong";
pub const UI_FONT_FAMILY: &str = "Inter";
/// Variable-font instances of the bundled Inter: regular, medium and semibold are the three UI weights.
pub const UI_FONT_WEIGHT: f32 = 400.0;
pub const UI_FONT_WEIGHT_STRONG: f32 = 600.0;
pub const UI_FONT_WEIGHT_MEDIUM: f32 = 500.0;
pub const FAMILY_MEDIUM: &str = "designos-medium";
/// Frame labels on the canvas are document overlays, not chrome: they keep the weight that matches Figma's
/// canvas labels, so their elision points do not move with the chrome weights.
pub const CANVAS_LABEL_FONT_WEIGHT: f32 = 450.0;
pub const FAMILY_CANVAS_LABEL: &str = "designos-canvas-label";
/// The UI monospace role (numbers, hex codes, token paths): bundled Roboto Mono at its regular instance.
pub const UI_MONO_FAMILY: &str = "Roboto Mono";
pub const UI_MONO_WEIGHT: f32 = 400.0;
/// Inter's optical-size axis at text sizes.
pub const UI_FONT_OPSZ: f32 = 14.0;

pub fn font_caption() -> FontId {
    FontId::new(FONT_SIZE_CAPTION, FontFamily::Proportional)
}
pub fn font_caption_medium() -> FontId {
    FontId::new(FONT_SIZE_CAPTION, FontFamily::Name(FAMILY_MEDIUM.into()))
}
pub fn font_caption_strong() -> FontId {
    FontId::new(FONT_SIZE_CAPTION, FontFamily::Name(FAMILY_STRONG.into()))
}
pub fn font_canvas() -> FontId {
    FontId::new(FONT_SIZE_CANVAS, FontFamily::Proportional)
}
pub fn font_canvas_medium() -> FontId {
    FontId::new(FONT_SIZE_CANVAS, FontFamily::Name(FAMILY_MEDIUM.into()))
}
pub fn font_canvas_strong() -> FontId {
    FontId::new(FONT_SIZE_CANVAS, FontFamily::Name(FAMILY_STRONG.into()))
}
pub fn font_body() -> FontId {
    FontId::new(FONT_SIZE_BODY, FontFamily::Proportional)
}
pub fn font_body_medium() -> FontId {
    FontId::new(FONT_SIZE_BODY, FontFamily::Name(FAMILY_MEDIUM.into()))
}
/// Menu rows: Inter at the medium instance.
pub fn font_menu() -> FontId {
    FontId::new(FONT_SIZE_BODY, FontFamily::Name(FAMILY_MEDIUM.into()))
}
pub fn font_body_strong() -> FontId {
    FontId::new(FONT_SIZE_BODY, FontFamily::Name(FAMILY_STRONG.into()))
}
pub fn font_large() -> FontId {
    FontId::new(FONT_SIZE_LARGE, FontFamily::Proportional)
}
pub fn font_large_strong() -> FontId {
    FontId::new(FONT_SIZE_LARGE, FontFamily::Name(FAMILY_STRONG.into()))
}
/// Page titles on the home screen.
pub fn font_title() -> FontId {
    FontId::new(FONT_SIZE_TITLE, FontFamily::Name(FAMILY_STRONG.into()))
}
pub fn font_mono() -> FontId {
    FontId::new(FONT_SIZE_BODY, FontFamily::Monospace)
}

/// The three UI weights, by name: regular 400, medium 500, semibold 600. Bold is a display weight and has no
/// chrome constant.
#[allow(dead_code)] // staged: surfaces set text through `font_text` as they adopt the text steps
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextWeight {
    Regular,
    Medium,
    Semibold,
}

/// Inter at `size` in one of the three UI weights, for a text step such as `FONT_SIZE_TEXT_SM`.
#[allow(dead_code)] // staged: first used by the button labels
pub fn font_text(size: f32, weight: TextWeight) -> FontId {
    FontId::new(size, weight_family(weight))
}

fn weight_family(weight: TextWeight) -> FontFamily {
    use std::sync::{Arc, OnceLock};
    match weight {
        TextWeight::Regular => FontFamily::Proportional,
        TextWeight::Medium => {
            static NAME: OnceLock<Arc<str>> = OnceLock::new();
            FontFamily::Name(NAME.get_or_init(|| Arc::from(FAMILY_MEDIUM)).clone())
        }
        TextWeight::Semibold => {
            static NAME: OnceLock<Arc<str>> = OnceLock::new();
            FontFamily::Name(NAME.get_or_init(|| Arc::from(FAMILY_STRONG)).clone())
        }
    }
}

/// Frame, component and section labels above top-level frames on the canvas: overlay text that keeps its
/// own size and weight (`FONT_SIZE_LABEL_CANVAS`, `CANVAS_LABEL_FONT_WEIGHT`) whatever the chrome scale is.
pub fn font_canvas_label() -> FontId {
    FontId::new(FONT_SIZE_LABEL_CANVAS, FontFamily::Name(FAMILY_CANVAS_LABEL.into()))
}

// ---------------------------------------------------------------- radii
// Five radius roles (design/principles.md P4). A shape inset `x` px inside a frame takes
// `concentric(outer, x)`; a pill takes `radius_full(height)`. The chain 16 > 12 > 8 > 6 > 2 never grows
// inwards: a frame holds cards, a card holds controls, a control holds a segment tile.
/// Tiny marks: swatches, canvas badges.
pub const RADIUS_MARK: u8 = 2;
/// The tile inside a segmented control: `RADIUS_CONTROL - INSET_SEGMENT`.
pub const RADIUS_SEGMENT: u8 = 6;
/// Controls, fields, tool buttons, rows and the hover pills of a frame: `RADIUS_FRAME - INSET_FRAME`.
pub const RADIUS_CONTROL: u8 = 8;
/// Cards and nested panels that hold controls at a 4px inset: `RADIUS_XL - 4 = RADIUS_CONTROL`.
#[allow(dead_code)] // staged: first used by the card surfaces
pub const RADIUS_XL: u8 = 12;
/// Floating chrome: menus, palettes, popovers, modals, toasts, toolbars.
pub const RADIUS_FRAME: u8 = 16;

pub fn cr(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}
pub fn cr_each(nw: u8, ne: u8, se: u8, sw: u8) -> CornerRadius {
    CornerRadius { nw, ne, se, sw }
}

/// Radius of a shape inset `inset` px inside a frame of radius `outer_radius`, so both
/// corner arcs share one centre (concentric). Clamped at 0 when the inset swallows the arc.
/// Pass the stroke width in `inset` when the frame strokes `StrokeKind::Inside`.
#[allow(dead_code)] // no call site yet: later changes derive nested radii through it
pub fn concentric(outer_radius: f32, inset: f32) -> f32 {
    (outer_radius - inset).max(0.0)
}

/// Pill-shaped corners for a chip, badge, status mark or button of the given height (P4).
pub fn cr_pill(height: f32) -> CornerRadius {
    cr(radius_full(height).round() as u8)
}

/// Radius that rounds both ends of a control of the given height into a half circle.
#[allow(dead_code)] // no call site yet: pills are adopted per surface in later changes
pub fn radius_full(height: f32) -> f32 {
    (height * 0.5).max(0.0)
}

// ---------------------------------------------------------------- spacing
// Every gap names the relationship it separates and takes that rung (design/principles.md P3).
// Rungs may share a value but never swap order. Every value is a step of the spacing scale
// (0, 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128, 160).
/// Parts of one fact: icon and label, a label and the field it names.
pub const SPACE_FACT: f32 = 4.0;
/// Stacked rows in one group.
pub const SPACE_ROW: f32 = 4.0;
/// Peers side by side: field columns, buttons in a bar, chips, tabs.
pub const SPACE_INLINE: f32 = 8.0;
/// Vertical advance after the instance source row.
pub const INSTANCE_SOURCE_STEP: f32 = 28.0;
/// Vertical advance after the instance description row.
pub const INSTANCE_DESCRIPTION_STEP: f32 = 48.0;
/// Gap between the source label and its component icon.
pub const INSTANCE_SOURCE_ICON_GAP: f32 = 6.0;
/// Group to group inside a section.
pub const SPACE_GROUP: f32 = 12.0;
/// Block to block in one region: cards in a grid.
#[allow(dead_code)] // rung of the ladder; first adopted when the Home rhythm literals retire
pub const SPACE_BLOCK: f32 = 16.0;
/// Section to section in a panel.
#[allow(dead_code)] // rung of the ladder; first adopted when the Home rhythm literals retire
pub const SPACE_SECTION: f32 = 24.0;
/// Region to region: Home header to grid, Home section to section.
#[allow(dead_code)] // rung of the ladder; first adopted when the Home rhythm literals retire
pub const SPACE_REGION: f32 = 48.0;

// Insets: padding inside an element's own frame (P3, P10). A child's inset never exceeds its parent's.
pub const INSET_SEGMENT: f32 = 2.0;
pub const INSET_CONTROL: f32 = 8.0;
pub const INSET_FRAME: f32 = 8.0;
pub const INSET_BUTTON: f32 = 12.0;
pub const INSET_PANEL: f32 = 16.0;
pub const INSET_POPOVER: f32 = 16.0;
pub const INSET_MODAL: f32 = 24.0;

// Control heights: a separate family the ladder never re-values (P3). Buttons and inputs come in 32, 36
// and 40 (`CONTROL_XS`, `CONTROL_SM`, `CONTROL_MD`); badges in 22, 24 and 28.
/// Legacy: its sites mix buttons, chips and hit boxes, so each surface re-points the ones it owns
/// (button or row -> `CONTROL_XS`, chip -> a badge height, hit box -> `TARGET_MIN`); a ratchet stops new uses.
pub const CONTROL_DENSE: f32 = 24.0;
/// Legacy, see `CONTROL_DENSE`.
pub const CONTROL_STANDARD: f32 = 28.0;
/// Editor buttons, rows and fields.
pub const CONTROL_XS: f32 = 32.0;
/// Home and dialog buttons and inputs.
#[allow(dead_code)] // staged: first used by the Home and dialog surfaces
pub const CONTROL_SM: f32 = 36.0;
/// Panel title rows; the larger Home and dialog buttons and inputs.
pub const CONTROL_MD: f32 = 40.0;
/// Badge heights: small, medium and large status and count marks.
#[allow(dead_code)] // staged: first used by the chip and pill surfaces
pub const BADGE_SM: f32 = 22.0;
pub const BADGE_MD: f32 = 24.0;
#[allow(dead_code)] // staged
pub const BADGE_LG: f32 = 28.0;
/// Smallest pointer target (WCAG 2.5.8 AA).
pub const TARGET_MIN: f32 = 24.0;

// ---------------------------------------------------------------- shell layout
pub const RAIL_WIDTH: f32 = 48.0;
pub const BORDER_WIDTH: f32 = 1.0;
pub const RAIL_MENU_BUTTON_X: f32 = 12.0;
pub const RAIL_MENU_BUTTON_Y: f32 = 8.0;
pub const RAIL_MENU_BUTTON_SIZE: f32 = 32.0;
pub const RAIL_ITEM_SIZE: f32 = 56.0;
pub const RAIL_FIRST_Y: f32 = 56.0;
pub const RAIL_GROUP_GAP: f32 = 16.0;
pub const RAIL_ICON_SIZE: f32 = 20.0;
/// 1px separators under the logo and before the last group (y relative to the rail top / item cell).
pub const RAIL_SEPARATOR_Y: f32 = 48.0;
pub const RAIL_TILE: f32 = 32.0;

pub const LEFT_PANEL_WIDTH: f32 = 240.0;
pub const LEFT_HEADER_HEIGHT: f32 = 65.0;
pub const LEFT_FILE_NAME_Y: f32 = 16.0;
pub const LEFT_SUBLABEL_Y: f32 = 40.0;
pub const LEFT_ACTION_Y: f32 = 12.0;
pub const LEFT_SIDEBAR_BTN_RIGHT_PAD: f32 = 12.0;
#[allow(dead_code)] // Left-panel clusters use their own pitch. This one stays at the frozen value.
pub const PAGES_SEARCH_DX: f32 = 28.0;
pub const PANEL_TITLE_ROW_HEIGHT: f32 = CONTROL_MD;
pub const PANEL_ACTION_BUTTON: f32 = TARGET_MIN;
pub const PANEL_ACTION_RIGHT_PAD: f32 = 8.0;
pub const PANEL_PADDING_X: f32 = INSET_PANEL;
pub const PAGE_ROW_HEIGHT: f32 = CONTROL_XS;
pub const PAGE_ROW_INSET: f32 = INSET_FRAME;
pub const PAGE_ROW_TEXT_PAD: f32 = 8.0;
pub const PAGES_MAX_VISIBLE_ROWS: usize = 5;
pub const PAGES_SEPARATOR_HIT: f32 = 6.0;
pub const PAGES_SEPARATOR_HIT_HALF: f32 = PAGES_SEPARATOR_HIT * 0.5;
/// Pointer band centred on a 1px drag divider. The painted line stays 1px.
pub const DRAG_DIVIDER_HIT: f32 = TARGET_MIN;
pub const PAGES_SEPARATOR_HIGHLIGHT_STROKE: f32 = 2.0;
pub const PAGES_SEPARATOR_STEP: f32 = 8.0;
pub const PAGES_SEPARATOR_SHIFT_MULT: f32 = 5.0;
pub const LAYERS_MIN_HEIGHT: f32 = 120.0;
pub const LEFT_DEV_READY_SECTION_STEP: f32 = 54.0;
pub const LAYER_FILTER_ROW_HEIGHT: f32 = CONTROL_XS;
pub const PAGE_MENU_WIDTH: f32 = 160.0;
pub const LAYER_HEADER_HEIGHT: f32 = CONTROL_MD;
pub const LAYER_ROW_HEIGHT: f32 = CONTROL_XS;
pub const LAYER_INDENT: f32 = 16.0;
pub const LAYER_CHEVRON_BOX: f32 = 24.0;
pub const LAYER_CHEVRON_PAD: f32 = 8.0;
pub const LAYER_ICON_SIZE: f32 = 16.0;
pub const LAYER_ICON_GAP: f32 = 0.0;
pub const LAYER_TOGGLE_SIZE: f32 = 24.0;
pub const LAYER_DROP_THICKNESS: f32 = 2.0;

// Find and replace panel and the layers name filter (left sidebar).
pub const FIND_FIELD_H: f32 = CONTROL_XS;
pub const FIND_ROW_GAP: f32 = 8.0;
pub const FIND_TOGGLE_SIZE: f32 = 22.0;
#[allow(dead_code)]
pub const FIND_BUTTON_H: f32 = 24.0;
pub const FIND_STATUS_H: f32 = 32.0;
pub const FIND_GROUP_ROW_H: f32 = CONTROL_XS;
/// Highlight behind the matched characters of a result or a filtered layer name.
pub const FIND_HIGHLIGHT: Color32 = rgba(0x0D99FF, 0x59);
pub const FIND_HIGHLIGHT_SELECTED: Color32 = rgba(0x7CC4F8, 0x66);
/// Characters of context kept before a match in a result excerpt.
pub const FIND_EXCERPT_LEAD: usize = 12;

pub const RIGHT_PANEL_WIDTH: f32 = 240.0;
/// The present button sits at the right edge, level with the zoom dropdown (8px from the edge).
pub const RIGHT_PRESENT_FROM_RIGHT: f32 = 56.0;
/// Outlined play triangle of the present button.
#[allow(dead_code)]
pub const PRESENT_ICON_W: f32 = 15.0;
#[allow(dead_code)]
pub const PRESENT_ICON_H: f32 = 16.0;
#[allow(dead_code)]
pub const PRESENT_ICON_STROKE: f32 = 1.2;
pub const RIGHT_BTN_Y: f32 = 8.0;
pub const RIGHT_BTN_SIZE: f32 = CONTROL_XS;
pub const RIGHT_PRESENT_CHEVRON_W: f32 = 16.0;
pub const RIGHT_TABS_Y: f32 = 48.0;
pub const RIGHT_TABS_H: f32 = CONTROL_DENSE;
pub const RIGHT_TABS_X: f32 = 9.0;
pub const RIGHT_TAB_PAD_X: f32 = 8.0;
pub const RIGHT_ZOOM_FROM_RIGHT: f32 = 63.0;
pub const RIGHT_ZOOM_W: f32 = 55.0;
pub const RIGHT_CONTENT_TOP: f32 = 81.0;
pub const RIGHT_SELECTION_HEADER_Y: f32 = 93.0;
pub const RIGHT_NAME_FIELD_X: f32 = 9.0;
pub const RIGHT_NAME_FIELD_W: f32 = 136.0;
pub const RIGHT_NAME_ACTION_X: [f32; 3] = [153.0, 181.0, 209.0];
pub const SECTION_TITLE_HEIGHT: f32 = CONTROL_MD;
pub const SECTION_CONTENT_X: f32 = 17.0;
pub const SECTION_CONTENT_W: f32 = 184.0;
pub const SECTION_TRAILING_X: f32 = 209.0;
pub const FIELD_HEIGHT: f32 = CONTROL_XS;
pub const FIELD_WIDTH_HALF: f32 = 88.0;
pub const FIELD_COL_X: [f32; 2] = [17.0, 113.0];
pub const FIELD_PREFIX_W: f32 = 24.0;
/// Prefix labels (X, Y, W, ...) start this far inside the field.
pub const FIELD_PREFIX_X: f32 = 8.0;
/// Prefix icon slot (16px glyph at x+8); the value then starts at x+24.
pub const FIELD_PREFIX_ICON: f32 = 16.0;
/// Glyph box at the slot's left edge (x+8..x+20): Lucide fills its box edge to edge where
/// Figma's art has margins, so the glyph is smaller to leave a gap before the value.
pub const FIELD_PREFIX_GLYPH: f32 = 12.0;
/// Fields without a prefix start their value 8px in.
pub const FIELD_PLAIN_X: f32 = 8.0;
/// Gap between a value and a worded suffix such as " ms".
pub const FIELD_SUFFIX_GAP: f32 = 4.0;
pub const DISTRIBUTE_BTN_X: f32 = 209.0;
pub const SUBLABEL_HEIGHT: f32 = LINE_CAPTION;
/// The label box of a labelled field group: one caption line, `SPACE_GROUP` below the field above and
/// `SPACE_FACT` above the field it names, so the label reads as part of the field it names (P9).
pub const LABEL_BOX_H: f32 = LINE_CAPTION;
pub const SEGMENT_W: f32 = 29.0;
pub const SEGMENT_GROUP_GAP: f32 = 8.0;
pub const SWATCH_SIZE: f32 = 16.0;
pub const SWATCH_RADIUS: u8 = RADIUS_MARK;
pub const SWATCH_ROW_X: f32 = 17.0;
pub const STYLE_ROW_H: f32 = 32.0;
/// Colour-style rows: circle swatch centre (from the row's left) and label x.
pub const STYLE_SWATCH: f32 = 16.0;
pub const STYLE_SWATCH_CX: f32 = 16.0;
pub const STYLE_LABEL_X: f32 = 33.0;
/// Gaps in the page-level Styles section (measured from the Figma capture).
pub const STYLES_LABEL_GAP: f32 = 8.0;
pub const STYLES_GROUP_GAP: f32 = 14.0;
pub const STYLES_BOTTOM_PAD: f32 = 13.0;
/// Gap between a group label and its first row.
pub const STYLES_ROW_GAP: f32 = 9.0;
/// Page background field group: offset below the title, swatch, hex and opacity columns.
pub const PAGE_FIELD_DY: f32 = 4.0;
pub const PAGE_BOTTOM_PAD: f32 = 16.0;
pub const PAGE_SWATCH: f32 = 12.0;
pub const PAGE_SWATCH_CX: f32 = 12.0;
pub const PAGE_HEX_X: f32 = 24.0;
/// Hex text field width in swatch + hex rows (e.g. the Prototype background).
pub const HEX_FIELD_W: f32 = 72.0;
pub const PAGE_DIVIDER_X: f32 = 129.0;
pub const PAGE_OPACITY_PAD: f32 = 8.0;
pub const PAGE_PERCENT_W: f32 = 16.0;
pub const PAGE_PERCENT_RIGHT: f32 = 7.0;
pub const STYLE_FOLDER_ROW_H: f32 = CONTROL_XS;
pub const STYLE_INDENT: f32 = 12.0;
pub const OPACITY_FIELD_W: f32 = 48.0;

pub const TOOLBELT_HEIGHT: f32 = 48.0;
pub const TOOLBELT_BOTTOM_MARGIN: f32 = 12.0;
pub const TOOLBELT_PADDING: f32 = 8.0;
pub const TOOL_BUTTON: f32 = CONTROL_XS;
pub const TOOL_CHEVRON_W: f32 = 16.0;
pub const TOOL_CHEVRON_GAP: f32 = 0.0;
/// Glyph size of the dropdown chevrons (the 16px box holds a ~6px wide arrow).
pub const TOOL_CHEVRON_ICON: f32 = 11.0;
/// The chevron glyph sits this far right of its button's right edge.
pub const TOOL_CHEVRON_CENTER_DX: f32 = 11.0;
pub const TOOL_GROUP_GAP: f32 = 8.0;
pub const TOOL_ICON_SIZE: f32 = 20.0;
/// Mode switcher: 4 segments (draw, design, prototype, dev) in a padded rounded group.
pub const MODE_SEGMENTS: usize = 4;
pub const MODE_SWITCH_PAD: f32 = INSET_SEGMENT;
pub const MODE_SWITCH_W: f32 = 29.5;
/// 1px divider before the switcher: distance after the last tool, and before the switcher.
pub const TOOLBAR_DIVIDER_GAP_BEFORE: f32 = 13.0;
pub const TOOLBAR_DIVIDER_GAP_AFTER: f32 = 8.0;

pub const MENU_WIDTH: f32 = 200.0;
pub const MENU_ITEM_HEIGHT: f32 = CONTROL_XS;
/// Vertical padding above the first and below the last row.
pub const MENU_PADDING: f32 = INSET_FRAME;
/// Highlight pill: inset from the panel edges, corner radius.
pub const MENU_PILL_INSET: f32 = INSET_FRAME;
pub const BG_MENU_SELECTED: Color32 = BG_SELECTED;
/// Columns (x from the panel's left edge): check, icon box, label.
pub const MENU_CHECK_X: f32 = 12.0;
pub const MENU_CHECK_BOX: f32 = 16.0;
pub const MENU_ICON_X_CHECKED: f32 = 32.0;
pub const MENU_ICON_BOX: f32 = 24.0;
pub const MENU_LABEL_X_ICON: f32 = 60.0;
pub const MENU_LABEL_X_PLAIN: f32 = 16.0;
/// Label x for rows that have a check column but no icon.
pub const MENU_LABEL_X_CHECK: f32 = 32.0;
/// Icon x when there is no check column; the label follows 4px after the box.
pub const MENU_ICON_X_PLAIN: f32 = 12.0;
pub const MENU_ICON_LABEL_GAP: f32 = 4.0;
pub const MENU_SHORTCUT_PAD: f32 = 16.0;
pub const MENU_CHEVRON_BOX: f32 = 24.0;
pub const MENU_CHEVRON_PAD: f32 = 8.0;
pub const MENU_SEP_MARGIN: f32 = 8.0;
/// Space kept between the label and a right-aligned shortcut / chevron.
pub const MENU_LABEL_GAP: f32 = 16.0;
#[allow(dead_code)] // Shortcuts paint TEXT_QUATERNARY. This role stays in the colour table.
pub const MENU_SHORTCUT_HIGHLIGHT: Color32 = rgb(0xE5E5E5);
pub const MENU_ICON_SIZE: f32 = 16.0;
pub const MENU_CHEVRON_SIZE: f32 = 12.0;
/// Submenus overlap their parent by this much horizontally.
pub const SUBMENU_OVERLAP: f32 = 0.0;
/// Toolbar dropdown: width, gap right of the tool button, overlap into the toolbar.
pub const TOOL_MENU_WIDTH: f32 = 232.0;
pub const TOOL_MENU_DX: f32 = 2.5;
pub const TOOL_MENU_OVERLAP: f32 = 4.0;
/// Main (logo) menu placement and width.
pub const MAIN_MENU_X: f32 = 12.0;
pub const MAIN_MENU_Y: f32 = 44.0;
pub const MAIN_MENU_WIDTH: f32 = 210.0;
#[allow(dead_code)]
pub const PALETTE_WIDTH: f32 = 320.0;
pub const PERMISSIONS_MODAL_WIDTH: f32 = 360.0;
#[allow(dead_code)]
pub const PALETTE_TOP: f32 = 64.0;
pub const TOAST_HEIGHT: f32 = 40.0;
pub const TOAST_BOTTOM: f32 = 72.0;
pub const TOAST_SECONDS: f64 = 4.0;
/// A toast with a "Show in folder" action: long enough to read a full path and reach for the button.
pub const TOAST_REVEAL_SECONDS: f64 = 10.0;
// ---------------------------------------------------------------- comments and version history
/// Comment pin: bubble diameter and the white ring around it.
pub const COMMENT_PIN: f32 = 32.0;
pub const COMMENT_PIN_RING: f32 = 2.0;
pub const COMMENT_PIN_HIT_PAD: f32 = 2.0;
/// Resolved pins are greyed out.
pub const COMMENT_PIN_RESOLVED: Color32 = rgb(0x757575);
/// Avatar colours picked from the author's name.
pub const AVATAR_COLORS: [Color32; 6] = [rgb(0xF24822), rgb(0xFFA629), rgb(0x14AE5C), rgb(0x0C8CE9), rgb(0x06B6D4), rgb(0xEE46BC)];
pub const COMMENT_AVATAR: f32 = 24.0;
/// Thread popover: width, gap to its pin, inner padding, header height.
pub const COMMENT_POP_W: f32 = 300.0;
pub const COMMENT_POP_GAP: f32 = 8.0;
pub const COMMENT_PAD: f32 = 12.0;
pub const COMMENT_HEADER_H: f32 = 36.0;
#[allow(dead_code)]
pub const COMMENT_BTN_H: f32 = 24.0;
pub const COMMENT_BOX_MIN_H: f32 = 56.0;
pub const COMMENT_BOX_MAX_H: f32 = 160.0;
#[allow(dead_code)]
pub const COMMENT_ICON_BTN: f32 = 20.0;
/// Comments panel (right side): filter row height and thread card padding.
pub const COMMENT_PANEL_HEADER_H: f32 = 48.0;
pub const COMMENT_FILTER_H: f32 = 24.0;
pub const COMMENT_CARD_PAD: f32 = 12.0;
/// Version history panel rows and the "Viewing" banner.
pub const HISTORY_ROW_H: f32 = 44.0;
pub const HISTORY_DAY_H: f32 = 28.0;
pub const HISTORY_BANNER_H: f32 = 40.0;

// ---------------------------------------------------------------- canvas
pub const CANVAS_OUTLINE_WIDTH: f32 = 1.0;
pub const CANVAS_HOVER_WIDTH: f32 = 1.0;
pub const CANVAS_HANDLE_SIZE: f32 = 7.0;
pub const CANVAS_HANDLE_BORDER: f32 = 1.0;
pub const CANVAS_ROTATE_ZONE: f32 = 12.0;
pub const CANVAS_HANDLE_HIT: f32 = 6.0;
pub const CANVAS_BADGE_OFFSET: f32 = 4.0;
pub const CANVAS_BADGE_PAD_X: f32 = 4.0;
pub const CANVAS_BADGE_HEIGHT: f32 = 16.0;
pub const CANVAS_BADGE_RADIUS: u8 = RADIUS_MARK;
pub const CANVAS_LABEL_GAP: f32 = 4.0;
pub const CANVAS_LABEL_MAX_ZOOM_HIDE: f32 = 0.05;
pub const SNAP_THRESHOLD_PX: f32 = 5.0;
pub const GUIDE_WIDTH: f32 = 1.0;
pub const GUIDE_X_SIZE: f32 = 3.0;
pub const GUIDE_TEXT_PAD: f32 = 3.0;
pub const GUIDE_TEXT_HEIGHT: f32 = 14.0;
pub const PEN_POINT_SIZE: f32 = 4.0;
pub const PEN_LINE_WIDTH: f32 = 1.5;
/// Insertion line shown while reordering inside an auto-layout frame.
pub const AL_INDICATOR_WIDTH: f32 = 2.0;
/// Vector edit mode: vertex square, handle dot, hit radii (px).
pub const VEDIT_POINT_SIZE: f32 = 6.0;
pub const VEDIT_HANDLE_RADIUS: f32 = 3.0;
pub const VEDIT_HIT: f32 = 6.0;
pub const VEDIT_SEGMENT_HIT: f32 = 5.0;
pub const DRAG_THRESHOLD_PX: f32 = 3.0;
pub const ZOOM_MIN: f32 = 0.02;
pub const ZOOM_MAX: f32 = 256.0;
pub const FIT_PADDING: f32 = 64.0;
pub const TEXT_CARET_WIDTH: f32 = 1.0;
/// Rulers (Shift+R): 20px strips on the top and left of the canvas.
pub const RULER_SIZE: f32 = 20.0;
pub const RULER_FONT_SIZE: f32 = FONT_SIZE_CANVAS;
/// Tick labels: white at 70% (#ffffffb2).
pub const RULER_TEXT: Color32 = rgba(0xFFFFFF, 0xB2);
pub const RULER_TICK: Color32 = rgba(0xFFFFFF, 0x66);
/// Selection extent highlighted on the rulers.
pub const RULER_SELECTION_FILL: Color32 = rgba(0x0D99FF, 0x4D);
/// User guides dragged from the rulers (#f24822).
pub const RULER_GUIDE: Color32 = rgb(0xF24822);
/// Pixel grid lines: #ffffff1a over dark pages, black at the same alpha over light ones.
pub const PIXEL_GRID_LIGHT_LINES: Color32 = rgba(0xFFFFFF, 0x1A);
pub const PIXEL_GRID_DARK_LINES: Color32 = rgba(0x000000, 0x1A);
/// The pixel grid appears from this zoom (800%).
pub const PIXEL_GRID_MIN_ZOOM: f32 = 8.0;
pub fn font_ruler() -> FontId {
    FontId::new(RULER_FONT_SIZE, FontFamily::Proportional)
}
/// Selected text highlight: #0c8ce9 at 35% alpha.
pub const TEXT_SELECTION: Color32 = Color32::from_rgba_premultiplied(4, 49, 81, 89);
/// Find matches on the canvas text: #0c8ce9 at 35% alpha, like a selection.
pub const FIND_CANVAS_MATCH: Color32 = TEXT_SELECTION;
/// Outline of the current find match.
pub const FIND_CANVAS_MATCH_CURRENT: Color32 = BG_BRAND;
/// At most this many matches in view are highlighted per frame.
pub const FIND_CANVAS_MATCH_MAX: usize = 600;
pub const NUDGE: f32 = 1.0;
pub const NUDGE_BIG: f32 = 10.0;

// ---------------------------------------------------------------- dev mode
/// Syntax colours of the Inspect code blocks.
pub const SYNTAX_PLAIN: Color32 = TEXT;
pub const SYNTAX_PROPERTY: Color32 = rgb(0x7DD3FC);
pub const SYNTAX_KEYWORD: Color32 = rgb(0xF472B6);
pub const SYNTAX_STRING: Color32 = rgb(0x4ADE80);
pub const SYNTAX_NUMBER: Color32 = rgb(0xFB923C);
pub const SYNTAX_TYPE: Color32 = rgb(0xFACC15);
pub const SYNTAX_FUNCTION: Color32 = rgb(0x60A5FA);
pub const SYNTAX_COMMENT: Color32 = rgb(0xA3A3A3);
pub const SYNTAX_PUNCT: Color32 = rgb(0xD4D4D4);
/// Dev status pills: Ready for dev, Completed, Changed.
pub const DEV_READY: Color32 = rgb(0x14AE5C);
pub const DEV_COMPLETED: Color32 = rgb(0x06B6D4);
pub const DEV_CHANGED: Color32 = rgb(0xFFA629);
/// In-progress agent badge ("Agent working" / "<agent> working") (`E15-7`).
pub const STATUS_INFO_TEXT: Color32 = rgb(0x0D99FF);
pub const STATUS_INFO_BG: Color32 = rgba(0x0D99FF, 0x24);
pub const STATUS_INFO_BORDER: Color32 = rgba(0x0D99FF, 0x59);
/// Box model diagram: margin, padding and content tints (as in Figma's Inspect).
pub const BOX_MARGIN: Color32 = rgb(0xF9CB9C);
pub const BOX_PADDING: Color32 = rgb(0xB6D7A8);
pub const BOX_CONTENT: Color32 = rgb(0x9FC5E8);
pub const BOX_TEXT: Color32 = rgb(0x1E1E1E);
/// Annotation callouts on the canvas.
pub const ANNOTATION_BG: Color32 = rgb(0xFFE08A);
pub const ANNOTATION_TEXT: Color32 = rgb(0x1E1E1E);
pub const ANNOTATION_LINE: Color32 = rgb(0xFFB800);
/// Measurement overlay red (distances between layers).
pub const MEASURE: Color32 = rgb(0xF24822);
pub const CODE_LINE_HEIGHT: f32 = LINE_BODY;

// ---------------------------------------------------------------- prototype
/// Noodles, hotspots and flow flags use the brand blue.
pub const PROTO_BLUE: Color32 = BG_BRAND;
pub const PROTO_NOODLE_WIDTH: f32 = 2.0;
pub const PROTO_HOTSPOT_RADIUS: f32 = 6.0;
pub const PROTO_HOTSPOT_RING: f32 = 2.0;
pub const PROTO_HOTSPOT_HIT: f32 = 10.0;
pub const PROTO_FLAG_HEIGHT: f32 = 20.0;
pub const PROTO_FLAG_GAP: f32 = 4.0;
pub const PROTO_ROW_HEIGHT: f32 = 44.0;
pub const PROTO_POPOVER_WIDTH: f32 = 240.0;
pub const PRESENT_BG: Color32 = rgb(0x0A0A0A);
pub const PRESENT_BAR_HEIGHT: f32 = 48.0;
pub const PRESENT_PADDING: f32 = 32.0;
/// Thickness of the plain device bezel drawn around the device screen when presenting.
pub const PRESENT_BEZEL: f32 = 10.0;
pub const PRESENT_MAX_SCALE: f32 = 4.0;

// ---------------------------------------------------------------- app defaults
pub const DEFAULT_WINDOW: (f32, f32) = (1600.0, 1088.0);
pub const MIN_WINDOW: (f32, f32) = (900.0, 600.0);
pub const APP_TITLE: &str = "design:os studio";
pub const DEFAULT_SHAPE_SIZE: f32 = 100.0;
pub const DEFAULT_TEXT_SIZE: f32 = 16.0;
pub const DEFAULT_PAGE_BG: u32 = 0x5A5A5A;
pub const DEFAULT_STROKE: u32 = 0x000000;
pub const DEFAULT_FRAME_FILL: u32 = 0xFFFFFF;
pub const DEFAULT_SHAPE_FILL: u32 = 0xD9D9D9;
pub const AUTO_LAYOUT_SPACING: f32 = 10.0;
pub const AUTO_LAYOUT_PADDING: f32 = 10.0;
pub const PASTE_OFFSET: f32 = 10.0;

/// Applies the Figma-like dark visuals to egui so any stock widget we do use
/// (text edits, scroll bars, popups) matches the chrome.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = BG_PRIMARY_SOLID;
    v.extreme_bg_color = BG;
    v.faint_bg_color = BG_SECONDARY;
    v.override_text_color = None;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.window_stroke = egui::Stroke::NONE;
    v.window_corner_radius = cr(RADIUS_CONTROL);
    v.menu_corner_radius = cr(RADIUS_CONTROL);
    v.selection.bg_fill = BG_TEXT_SELECTION;
    v.selection.stroke = egui::Stroke::new(1.0, BORDER_SELECTED);
    v.text_cursor.stroke = egui::Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.bg_fill = BG;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = cr(RADIUS_CONTROL);
        w.fg_stroke = egui::Stroke::new(1.0, TEXT);
    }
    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER_STRONG);
    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.weak_bg_fill = BG_HOVER;
    v.widgets.active.bg_fill = BG_PRESSED;
    v.widgets.active.weak_bg_fill = BG_PRESSED;
    style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    style.spacing.button_padding = egui::vec2(0.0, 0.0);
    style.spacing.interact_size = egui::vec2(0.0, 0.0);
    let pad = egui::Margin::symmetric(INSET_BUTTON as i8, INSET_FRAME as i8);
    style.spacing.window_margin = pad;
    style.spacing.menu_margin = pad;
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.active_handle_opacity = 0.225;
    style.spacing.scroll.interact_handle_opacity = 0.225;
    style.text_styles.insert(egui::TextStyle::Body, font_body());
    style.text_styles.insert(egui::TextStyle::Button, font_body());
    style.text_styles.insert(egui::TextStyle::Small, font_caption());
    style.text_styles.insert(egui::TextStyle::Monospace, font_mono());
    ctx.set_global_style(style);
}

/// Installs the bundled Inter variable font (regular, medium, strong and canvas-label instances) and Roboto Mono.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let inter = designos_fonts::bundled().iter().find(|a| a.family == UI_FONT_FAMILY && !a.italic).expect("bundled Inter");
    let instance = |weight: f32| {
        let tweak = egui::FontTweak { coords: egui::epaint::text::VariationCoords::new([("wght", weight), ("opsz", UI_FONT_OPSZ)]), ..Default::default() };
        std::sync::Arc::new(egui::FontData::from_static(inter.data).tweak(tweak))
    };
    fonts.font_data.insert("ui-regular".into(), instance(UI_FONT_WEIGHT));
    fonts.font_data.insert("ui-strong".into(), instance(UI_FONT_WEIGHT_STRONG));
    fonts.font_data.insert("ui-medium".into(), instance(UI_FONT_WEIGHT_MEDIUM));
    fonts.font_data.insert("ui-canvas-label".into(), instance(CANVAS_LABEL_FONT_WEIGHT));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "ui-regular".into());
    let mut strong_family = vec!["ui-strong".to_string()];
    strong_family.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(FAMILY_STRONG.into()), strong_family);
    let mut medium_family = vec!["ui-medium".to_string()];
    medium_family.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(FAMILY_MEDIUM.into()), medium_family);
    let mut canvas_label_family = vec!["ui-canvas-label".to_string()];
    canvas_label_family.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(FAMILY_CANVAS_LABEL.into()), canvas_label_family);
    // Monospace role: bundled Roboto Mono first, egui's default mono face as the glyph fallback.
    let mono = designos_fonts::bundled().iter().find(|a| a.family == UI_MONO_FAMILY && !a.italic).expect("bundled Roboto Mono");
    let mono_tweak = egui::FontTweak { coords: egui::epaint::text::VariationCoords::new([("wght", UI_MONO_WEIGHT)]), ..Default::default() };
    fonts.font_data.insert("ui-mono".into(), std::sync::Arc::new(egui::FontData::from_static(mono.data).tweak(mono_tweak)));
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "ui-mono".into());
    ctx.set_fonts(fonts);
}

/// Perceived luminance test used to choose label colors on the canvas.
pub fn is_light(c: Color32) -> bool {
    let l = (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) / 255.0;
    l > LIGHT_BG_THRESHOLD
}

// ---------------------------------------------------------------- projects library (home screen)
pub const HOME_SIDEBAR_W: f32 = 264.0;
pub const HOME_PAD: f32 = 32.0;
pub const HOME_HEADER_H: f32 = 64.0;
pub const HOME_CARD_W: f32 = 224.0;
pub const HOME_THUMB_H: f32 = 140.0;
pub const HOME_CARD_GAP: f32 = 24.0;
pub const HOME_FOLDER_H: f32 = 48.0;
pub const HOME_LIST_ROW_H: f32 = 44.0;
pub const HOME_THUMB_BG: Color32 = rgb(0x262626);
// ---------------------------------------------------------------- variables view
// Measured on the Figma UI3 local-variables captures (800x600, dpr 1).
/// Sidebar content width; the 1px border follows it in the full view.
pub const VAR_SIDEBAR_W: f32 = 240.0;
/// Top bar height of the full view (excluding its 1px bottom border).
pub const VAR_BAR_H: f32 = 48.0;
/// Floating panel: size and default top-left.
pub const VAR_FLOAT_W: f32 = 642.0;
pub const VAR_FLOAT_H: f32 = 380.0;
pub const VAR_FLOAT_Y: f32 = 8.0;
pub const VAR_FLOAT_BAR_H: f32 = 40.0;
pub const VAR_HEAD_H: f32 = CONTROL_MD;
pub const VAR_ROW_H: f32 = CONTROL_MD;
pub const VAR_ROW_PITCH: f32 = 41.0;
/// Name column width (the 1px grid line follows it).
pub const VAR_NAME_COL: f32 = 200.0;
pub const VAR_MODE_MIN: f32 = 140.0;
pub const VAR_MODE_MAX: f32 = 280.0;
pub const VAR_ADD_COL: f32 = 39.0;
pub const VAR_BOTTOM_H: f32 = 41.0;
pub const VAR_SCROLL_H: f32 = 11.0;
pub const VAR_SCROLL_THUMB: Color32 = rgb(0x737373);
pub const VAR_SEARCH_W: f32 = 174.0;
pub const VAR_FILTER_W: f32 = 25.0;
pub const VAR_SIDE_ROW: f32 = CONTROL_XS;
pub const VAR_PILL_H: f32 = BADGE_MD;
pub const VAR_ICON_X: f32 = 16.0;
pub const VAR_NAME_X: f32 = 44.0;
pub const VAR_SWATCH: f32 = 16.0;
pub const VAR_HEX_X: f32 = 30.0;
pub const VAR_HEX_W: f32 = 146.0;
pub const VAR_OPACITY_W: f32 = 62.0;
pub const VAR_OPACITY_RIGHT: f32 = 74.0;
#[allow(dead_code)] // legacy field height; call sites read FIELD_HEIGHT
pub const VAR_FIELD_H: f32 = 24.0;
pub const VAR_POPOVER_W: f32 = 264.0;

// ---------------------------------------------------------------- board (whiteboard) mode
/// Sticky note and section palette: eight pastel colours (`0xRRGGBB`), the first is the default.
pub const BOARD_PALETTE: [u32; 8] = [0xFFE58F, 0xFFC48A, 0xFFB3B3, 0xF2B8E6, 0xD5C3FF, 0xB6D4FF, 0xA8E6DD, 0xC4EBA6];
/// Ink colours for connectors, markers and text on a board (dark first).
pub const BOARD_INK: [u32; 8] = [0x1E1E1E, 0x757575, 0xF24822, 0xFFA629, 0x14AE5C, 0x0C8CE9, 0x06B6D4, 0xEE46BC];
/// Canvas colour of a board and the dots drawn on it.
pub const BOARD_BG: u32 = 0xF4F4F4;
pub const BOARD_DOT: Color32 = rgba(0x000000, 0x2E);
/// Distance between canvas dots in page units; the grid doubles while it would be denser than the minimum on screen.
pub const BOARD_DOT_SPACING: f32 = 24.0;
pub const BOARD_DOT_MIN_PX: f32 = 14.0;
/// Magnet dots on a hovered shape: radius, the gap from the shape's side, and the grab radius (screen px).
pub const BOARD_MAGNET_RADIUS: f32 = 5.0;
pub const BOARD_MAGNET_GAP: f32 = 12.0;
pub const BOARD_MAGNET_HIT: f32 = 10.0;
/// New items.
pub const BOARD_SHAPE_W: f32 = 160.0;
pub const BOARD_SHAPE_H: f32 = 120.0;
pub const BOARD_STAMP_SIZE: f32 = 64.0;
pub const BOARD_STICKY_GAP: f32 = 24.0;
pub const BOARD_MARKER_WIDTH: f32 = 6.0;
pub const BOARD_HIGHLIGHTER_WIDTH: f32 = 24.0;
pub const BOARD_HIGHLIGHTER_ALPHA: f32 = 0.4;
/// Floating contextual toolbar: height, padding, gap to the selection, swatch size.
pub const BOARD_BAR_H: f32 = 40.0;
pub const BOARD_BAR_PAD: f32 = 6.0;
pub const BOARD_BAR_GAP: f32 = 12.0;
pub const BOARD_BAR_BTN: f32 = 28.0;
pub const BOARD_SWATCH: f32 = 18.0;
pub const BOARD_BAR_SEP: f32 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concentric_subtracts_the_inset() {
        assert_eq!(concentric(14.0, 8.0), 6.0);
        assert_eq!(concentric(6.0, 2.0), 4.0);
        assert_eq!(concentric(5.0, 0.0), 5.0);
    }

    #[test]
    fn concentric_clamps_at_zero_when_the_inset_reaches_the_arc() {
        assert_eq!(concentric(6.0, 6.0), 0.0);
        assert_eq!(concentric(6.0, 16.0), 0.0);
    }

    #[test]
    fn concentric_grows_for_a_negative_inset() {
        assert_eq!(concentric(6.0, -2.0), 8.0);
    }

    #[test]
    fn concentric_never_returns_nan() {
        assert_eq!(concentric(f32::NAN, 4.0), 0.0);
    }

    #[test]
    fn radius_full_is_half_the_height() {
        assert_eq!(radius_full(24.0), 12.0);
        assert_eq!(radius_full(28.0), 14.0);
        assert_eq!(radius_full(0.0), 0.0);
    }

    #[test]
    fn radius_full_clamps_a_negative_height() {
        assert_eq!(radius_full(-8.0), 0.0);
    }

    const LADDER: [f32; 7] = [SPACE_FACT, SPACE_ROW, SPACE_INLINE, SPACE_GROUP, SPACE_BLOCK, SPACE_SECTION, SPACE_REGION];
    const INSETS: [f32; 7] = [INSET_SEGMENT, INSET_CONTROL, INSET_FRAME, INSET_BUTTON, INSET_PANEL, INSET_POPOVER, INSET_MODAL];
    /// The control heights that are component sizes rather than spacing steps.
    const CONTROLS: [f32; 7] = [CONTROL_XS, CONTROL_SM, CONTROL_MD, BADGE_SM, BADGE_MD, BADGE_LG, TARGET_MIN];
    /// The spacing scale (2px and 6px included) and the sizes controls come in.
    const SPACING_SCALE: [f32; 17] = [0.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 40.0, 48.0, 64.0, 80.0, 96.0, 128.0, 160.0];
    const BUTTON_AND_INPUT_SIZES: [f32; 5] = [32.0, 36.0, 40.0, 44.0, 48.0];
    const BADGE_SIZES: [f32; 3] = [22.0, 24.0, 28.0];
    const RADIUS_SCALE: [f32; 10] = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0];

    #[test]
    fn ladder_insets_and_controls_sit_on_the_scale_steps() {
        // Two kinds of value, two sets: a gap or inset is a spacing step; a control height is a component size.
        for v in LADDER.into_iter().chain(INSETS) {
            assert!(SPACING_SCALE.contains(&v), "{v} is not a spacing step");
        }
        for v in CONTROLS {
            assert!(
                BUTTON_AND_INPUT_SIZES.contains(&v) || BADGE_SIZES.contains(&v) || v == TARGET_MIN,
                "{v} is neither a button or input size, a badge size nor the pointer floor"
            );
        }
    }

    #[test]
    fn the_ladder_never_inverts() {
        for pair in LADDER.windows(2) {
            assert!(pair[0] <= pair[1], "{} is followed by the smaller {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn a_group_sits_at_least_twice_its_inner_gap_from_its_neighbours() {
        // (outer gap, the inner gap it must clear twice over)
        for (outer, inner) in [(SPACE_GROUP, SPACE_FACT), (SPACE_BLOCK, SPACE_INLINE), (SPACE_REGION, SPACE_BLOCK)] {
            assert!(outer >= 2.0 * inner, "{outer} is not twice {inner}");
        }
    }

    #[test]
    fn a_child_inset_never_exceeds_its_parent_inset() {
        let chain = [INSET_MODAL, INSET_POPOVER, INSET_PANEL, INSET_BUTTON, INSET_FRAME, INSET_CONTROL, INSET_SEGMENT];
        assert!(chain.windows(2).all(|w| w[0] >= w[1]), "{chain:?} is not non-increasing");
    }

    #[test]
    fn control_heights_are_the_button_and_badge_sizes() {
        assert_eq!([CONTROL_XS, CONTROL_SM, CONTROL_MD], [32.0, 36.0, 40.0]);
        assert_eq!([BADGE_SM, BADGE_MD, BADGE_LG], [22.0, 24.0, 28.0]);
        assert_eq!(TARGET_MIN, 24.0);
        // The legacy heights keep their names and values until each surface re-points its sites.
        assert_eq!([CONTROL_DENSE, CONTROL_STANDARD], [24.0, 28.0]);
        assert_eq!(CONTROL_DENSE, TARGET_MIN, "the densest control sits exactly on the pointer floor");
        // Rows, menu items and Inspector fields are the editor button size.
        assert_eq!(PAGE_ROW_HEIGHT, CONTROL_XS);
        assert_eq!(MENU_ITEM_HEIGHT, CONTROL_XS);
        assert_eq!(FIELD_HEIGHT, CONTROL_XS);
        assert_eq!(VAR_PILL_H, BADGE_MD, "a pill is a badge, not a control");
        assert_eq!(RIGHT_TABS_H, CONTROL_DENSE, "inspector tab strips are 24px, not 28px");
    }

    #[test]
    fn nested_radii_are_concentric() {
        assert_eq!(RADIUS_FRAME as f32 - INSET_FRAME, RADIUS_CONTROL as f32);
        assert_eq!(RADIUS_CONTROL as f32 - INSET_SEGMENT, RADIUS_SEGMENT as f32);
        assert_eq!(concentric(RADIUS_FRAME as f32, INSET_FRAME), RADIUS_CONTROL as f32);
        assert_eq!(concentric(RADIUS_CONTROL as f32, INSET_SEGMENT), RADIUS_SEGMENT as f32);
        // The chain from a frame down to a segment tile never grows.
        let chain = [RADIUS_FRAME, RADIUS_CONTROL, RADIUS_SEGMENT, RADIUS_MARK];
        assert!(chain.windows(2).all(|w| w[0] > w[1]), "{chain:?} is not strictly decreasing");
        // A card holding controls at a 4px inset sits between the frame and the control.
        let full_chain = [RADIUS_FRAME, RADIUS_XL, RADIUS_CONTROL, RADIUS_SEGMENT, RADIUS_MARK];
        assert_eq!(full_chain, [16, 12, 8, 6, 2]);
        assert!(full_chain.windows(2).all(|w| w[0] > w[1]), "{full_chain:?} is not strictly decreasing");
        assert_eq!(concentric(RADIUS_XL as f32, 4.0), RADIUS_CONTROL as f32);
        for r in full_chain {
            assert!(RADIUS_SCALE.contains(&(r as f32)), "{r} is not a radius step");
        }
        // A pill inside a pill: a 32px track holding a 28px thumb at a 2px inset.
        assert_eq!(radius_full(32.0) - 2.0, radius_full(28.0));
    }

    #[test]
    fn menu_pills_inset_by_the_frame_inset() {
        assert_eq!(MENU_PILL_INSET, INSET_FRAME);
        assert_eq!(concentric(RADIUS_FRAME as f32, MENU_PILL_INSET), RADIUS_CONTROL as f32);
    }

    #[test]
    fn pill_radius_is_half_the_height() {
        assert_eq!(cr_pill(20.0), cr(10));
        assert_eq!(cr_pill(26.0), cr(13));
        assert_eq!(cr_pill(28.0), cr(14));
    }

    #[test]
    fn type_roles_are_the_text_steps() {
        // The role chain never shrinks, and the canvas size is the only one in the set below the chrome floor of 12.
        let chain = [FONT_SIZE_CANVAS, FONT_SIZE_CAPTION, FONT_SIZE_BODY, FONT_SIZE_LARGE, FONT_SIZE_TITLE];
        assert!(chain.windows(2).all(|w| w[0] <= w[1]), "{chain:?} is not non-decreasing");
        assert_eq!([FONT_SIZE_CAPTION, FONT_SIZE_BODY, FONT_SIZE_LARGE, FONT_SIZE_TITLE], [12.0, 12.0, 14.0, 20.0]);
        assert_eq!(FONT_SIZE_CANVAS, 9.0);
        assert_eq!(RULER_FONT_SIZE, FONT_SIZE_CANVAS);
        let scanned = [
            FONT_SIZE_CAPTION,
            FONT_SIZE_BODY,
            FONT_SIZE_LARGE,
            FONT_SIZE_TITLE,
            FONT_SIZE_TEXT_XS,
            FONT_SIZE_TEXT_SM,
            FONT_SIZE_TEXT_MD,
            FONT_SIZE_TEXT_LG,
            FONT_SIZE_TEXT_XL,
            FONT_SIZE_CANVAS,
        ];
        let below_floor: Vec<f32> = scanned.into_iter().filter(|s| *s < 12.0).collect();
        assert_eq!(below_floor, [FONT_SIZE_CANVAS], "only the canvas size sits below the chrome floor");
        // Each text step with the line height it is set on.
        let steps = [
            (FONT_SIZE_TEXT_XS, LINE_TEXT_XS, 12.0, 18.0),
            (FONT_SIZE_TEXT_SM, LINE_TEXT_SM, 14.0, 20.0),
            (FONT_SIZE_TEXT_MD, LINE_TEXT_MD, 16.0, 24.0),
            (FONT_SIZE_TEXT_LG, LINE_TEXT_LG, 18.0, 28.0),
            (FONT_SIZE_TEXT_XL, LINE_TEXT_XL, 20.0, 30.0),
        ];
        for (size, line, want_size, want_line) in steps {
            assert_eq!((size, line), (want_size, want_line));
        }
    }

    #[test]
    fn caption_and_body_are_one_font_and_the_weights_tell_the_helpers_apart() {
        assert_eq!(font_caption(), font_body(), "colour and position tell a caption from a name, not the face");
        assert_eq!(font_caption_medium(), font_body_medium());
        assert_eq!(font_caption_strong(), font_body_strong());
        assert_ne!(font_body(), font_body_medium());
        assert_ne!(font_body_medium(), font_body_strong());
    }

    #[test]
    fn font_text_sets_a_step_in_one_of_the_three_weights() {
        assert_eq!(font_text(FONT_SIZE_TEXT_XS, TextWeight::Regular), font_body());
        assert_eq!(font_text(FONT_SIZE_TEXT_XS, TextWeight::Medium), font_body_medium());
        assert_eq!(font_text(FONT_SIZE_TEXT_XS, TextWeight::Semibold), font_body_strong());
        // Button labels are the 14 step in semibold at every button size.
        assert_eq!(font_text(FONT_SIZE_TEXT_SM, TextWeight::Semibold), font_large_strong());
    }

    #[test]
    fn line_heights_fit_their_roles() {
        for (line, size, ratio) in [(LINE_CAPTION, FONT_SIZE_CAPTION, 1.3), (LINE_BODY, FONT_SIZE_BODY, 1.4)] {
            assert!(line >= size * ratio, "line {line} is too tight for {size}px text");
        }
        assert_eq!([LINE_CAPTION, LINE_BODY], [18.0, 18.0]);
        assert_eq!(LABEL_BOX_H, LINE_CAPTION);
        // A label box plus the gap below it stays inside the 32px field slot it introduces.
        let slot = std::hint::black_box(FIELD_HEIGHT);
        assert!(LABEL_BOX_H + SPACE_FACT <= slot);
    }

    #[test]
    fn only_three_ui_weights_exist() {
        assert_eq!([UI_FONT_WEIGHT, UI_FONT_WEIGHT_MEDIUM, UI_FONT_WEIGHT_STRONG], [400.0, 500.0, 600.0]);
    }

    #[test]
    fn canvas_labels_keep_their_own_size_and_weight() {
        // Frame labels are document overlays: neither the chrome size nor a chrome weight.
        assert_eq!(FONT_SIZE_LABEL_CANVAS, 11.0);
        assert_eq!(CANVAS_LABEL_FONT_WEIGHT, 450.0);
        assert!(![UI_FONT_WEIGHT, UI_FONT_WEIGHT_MEDIUM, UI_FONT_WEIGHT_STRONG].contains(&CANVAS_LABEL_FONT_WEIGHT));
        let font = font_canvas_label();
        assert_eq!(font.size, FONT_SIZE_LABEL_CANVAS);
        assert_eq!(font.family, FontFamily::Name(FAMILY_CANVAS_LABEL.into()));
        // Installed: the family lays text out, and at weight 450 it is wider than the 400 chrome regular.
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let sample = "A frame with a very long name that overflows";
        let width_of = |font: FontId| ctx.fonts_mut(|f| f.layout_no_wrap(sample.to_string(), font, Color32::WHITE).size().x);
        let label = width_of(font_canvas_label());
        let chrome = width_of(FontId::new(FONT_SIZE_LABEL_CANVAS, FontFamily::Proportional));
        assert!(label > chrome, "canvas label {label}px should be wider than regular {chrome}px");
    }

    #[test]
    fn selection_fills_are_neutral() {
        for c in [BG_SELECTED, BG_SELECTED_HOVER, BG_SELECTED_SECONDARY, BG_MENU_SELECTED, BG_TOOLBAR_SELECTED] {
            let [r, g, b, _] = c.to_array();
            assert!(r.abs_diff(b) <= 12 && g.abs_diff(b) <= 12, "selection fill {c:?} carries an accent hue");
        }
        assert_ne!(BG_SELECTED, BG_BRAND);
    }

    #[test]
    fn the_monospace_role_is_the_bundled_roboto_mono() {
        assert_eq!(UI_MONO_FAMILY, "Roboto Mono");
        assert!(designos_fonts::bundled().iter().any(|a| a.family == UI_MONO_FAMILY && !a.italic));
    }

    fn srgb_channel_lum(c: u8) -> f64 {
        let x = c as f64 / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    }

    fn relative_luminance(c: Color32) -> f64 {
        let [r, g, b, _] = c.to_srgba_unmultiplied();
        0.2126 * srgb_channel_lum(r) + 0.7152 * srgb_channel_lum(g) + 0.0722 * srgb_channel_lum(b)
    }

    fn composite_over(fg: Color32, bg: Color32) -> Color32 {
        let [fr, fg_c, fb, fa] = fg.to_srgba_unmultiplied();
        if fa == 255 {
            return fg;
        }
        let [br, bg_c, bb, _] = bg.to_srgba_unmultiplied();
        let a = fa as f64 / 255.0;
        let blend = |f: u8, b: u8| -> u8 { (f as f64 * a + b as f64 * (1.0 - a)).round() as u8 };
        Color32::from_rgb(blend(fr, br), blend(fg_c, bg_c), blend(fb, bb))
    }

    fn wcag_contrast(fg: Color32, bg: Color32) -> f64 {
        let eff_bg = composite_over(bg, BG);
        let eff_fg = composite_over(fg, eff_bg);
        let l1 = relative_luminance(eff_fg);
        let l2 = relative_luminance(eff_bg);
        let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Hue in degrees and HSL saturation of a colour.
    fn rgb_hue_sat(c: Color32) -> (f64, f64) {
        let [r, g, b, a] = c.to_srgba_unmultiplied();
        if a == 0 {
            return (0.0, 0.0);
        }
        let (rf, gf, bf) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        let max = rf.max(gf).max(bf);
        let min = rf.min(gf).min(bf);
        let d = max - min;
        if d <= 1e-6 || max <= 1e-6 {
            return (0.0, 0.0);
        }
        // HSL saturation (contract: HSL, not HSV; pale purples such as #E9D5FF are S 1.00 in HSL, 0.16 in HSV).
        let sat = d / (1.0 - (max + min - 1.0).abs());
        let mut hue = if (max - rf).abs() < 1e-6 {
            60.0 * (((gf - bf) / d) % 6.0)
        } else if (max - gf).abs() < 1e-6 {
            60.0 * (((bf - rf) / d) + 2.0)
        } else {
            60.0 * (((rf - gf) / d) + 4.0)
        };
        if hue < 0.0 {
            hue += 360.0;
        }
        (hue, sat)
    }

    /// Colours written as literals in one `pub const NAME: Color32 = …;` / `[Color32; N]` line of `src`
    /// (`rgb(0x…)`, `rgba(0x…, …)`, `from_rgba_premultiplied(r, g, b, a)`). An alias carries none: its target is listed itself.
    fn declared_colour_literals(src: &str) -> Vec<(String, Vec<Color32>)> {
        let mut out = Vec::new();
        for line in src.lines() {
            let Some(rest) = line.strip_prefix("pub const ") else { continue };
            let Some((name, rhs)) = rest.split_once(": ") else { continue };
            let Some((ty, expr)) = rhs.split_once(" = ") else { continue };
            if ty != "Color32" && !ty.starts_with("[Color32;") {
                continue;
            }
            let mut colours = Vec::new();
            let mut tail = expr;
            while let Some(i) = tail.find("(0x") {
                let digits = &tail[i + 3..];
                if let Some(hex) = digits.get(..6).and_then(|h| u32::from_str_radix(h, 16).ok()) {
                    colours.push(Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8));
                }
                tail = digits;
            }
            if let Some(i) = expr.find("from_rgba_premultiplied(") {
                let args = &expr[i + "from_rgba_premultiplied(".len()..];
                let nums: Vec<u8> = args.split(')').next().unwrap_or("").split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if let [r, g, b, a] = nums[..] {
                    colours.push(Color32::from_rgba_premultiplied(r, g, b, a));
                }
            }
            out.push((name.to_string(), colours));
        }
        out
    }

    /// Chrome colour constants of `src` inside the Purple Ban band (hue 255-300 deg, HSL saturation above 0.25).
    /// `BOARD_*` is board content and exempt.
    fn purple_constants(src: &str) -> Vec<String> {
        let mut hits = Vec::new();
        for (name, colours) in declared_colour_literals(src) {
            if name.starts_with("BOARD_") {
                continue;
            }
            for col in colours {
                let (hue, sat) = rgb_hue_sat(col);
                if (255.0..=300.0).contains(&hue) && sat > 0.25 {
                    hits.push(format!("{name} ({col:?}) hue={hue:.1} sat={sat:.2}"));
                }
            }
        }
        hits
    }

    #[test]
    fn no_chrome_colour_is_purple() {
        // The list is read from this file's own source above the test module, so a new constant cannot escape the scan.
        let src = include_str!("theme.rs");
        let decls = &src[..src.find("#[cfg(test)]").expect("test module marker")];
        let declared = declared_colour_literals(decls);
        let names: Vec<&str> = declared.iter().map(|(n, _)| n.as_str()).collect();
        for sentinel in ["BG", "SYNTAX_KEYWORD", "AVATAR_COLORS", "TEXT_SELECTION", "READER_LIGHT_HIGHLIGHT"] {
            assert!(names.contains(&sentinel), "scan did not reach {sentinel}");
        }
        assert!(declared.len() > 150, "scan found only {} colour constants", declared.len());
        let hits = purple_constants(decls);
        assert!(hits.is_empty(), "Purple Ban: {hits:?}");
    }

    #[test]
    fn purple_constants_flags_a_scalar_an_array_and_a_pale_purple_but_not_board_content() {
        // Declarations are assembled from parts so the Python parity gate, which reads this file, never sees them.
        let decl = |name: &str, ty: &str, expr: &str| format!("pub const {name}: {ty} = {expr};\n");
        let fixture = [
            decl("OK", "Color32", "rgb(0x0A0A0A)"),
            decl("SCALAR", "Color32", "rgb(0xC792EA)"),
            decl("PALE", "Color32", "rgba(0xE9D5FF, 0x80)"),
            decl("WITH_ARRAY", "[Color32; 2]", "[rgb(0x0A0A0A), rgb(0x8B5CF6)]"),
            decl("BOARD_DOT", "Color32", "rgb(0xD5C3FF)"),
            decl("ALIAS", "Color32", "SCALAR"),
        ]
        .concat();
        let hits = purple_constants(&fixture);
        let hit_names: Vec<&str> = hits.iter().map(|h| h.split(' ').next().unwrap_or("")).collect();
        assert_eq!(hit_names, ["SCALAR", "PALE", "WITH_ARRAY"], "{hits:?}");
    }

    #[test]
    fn purple_scan_sees_pale_purples_that_an_hsv_scan_misses() {
        // #E9D5FF is HSL saturation 1.00 but HSV saturation 0.16: a scan on HSV lets it through.
        for hex in [0xE9D5FF_u32, 0xD5C3FF, 0xF3E8FF] {
            let c = Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
            let (hue, sat) = rgb_hue_sat(c);
            assert!((255.0..=300.0).contains(&hue) && sat > 0.25, "{hex:06X} slips past the Purple Ban scan: hue={hue:.1}, sat={sat:.2}");
        }
    }

    #[test]
    fn brand_label_passes_aa() {
        for (bg_name, bg) in [("BG_BRAND", BG_BRAND), ("BG_BRAND_HOVER", BG_BRAND_HOVER), ("BG_BRAND_PRESSED", BG_BRAND_PRESSED)] {
            let ratio = wcag_contrast(TEXT_ON_BRAND, bg);
            assert!(ratio >= 4.5, "TEXT_ON_BRAND on {bg_name} fails AA: {ratio:.2}:1 < 4.5:1");
        }
    }

    /// `BG_OVERLAY` drawn over white, straight alpha, the conservative C-1.40 / C-1.72 backdrop.
    fn bg_overlay_on_white() -> Color32 {
        assert_eq!(BG_OVERLAY, rgba(0x262626, 0xB3));
        let (h, a) = (0x26u32, 0xB3u32);
        let ch = (h as f64 * a as f64 / 255.0 + 255.0 * (255 - a) as f64 / 255.0).round() as u8;
        Color32::from_rgb(ch, ch, ch)
    }

    fn opaque_hex(c: Color32) -> u32 {
        let [r, g, b, a] = c.to_srgba_unmultiplied();
        assert_eq!(a, 255, "{c:?} is translucent");
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    #[test]
    fn chrome_contrast_pairs() {
        let star_bg = bg_overlay_on_white();
        let pairs = [
            ("C-1.1", TEXT, BG, 4.5),
            ("C-1.2", TEXT, BG_SECONDARY, 4.5),
            ("C-1.3", TEXT, BG_SELECTED, 4.5),
            ("C-1.4", TEXT, BG_PRESSED, 4.5),
            ("C-1.5", TEXT_SECONDARY, BG, 4.5),
            ("C-1.6", TEXT_SECONDARY, BG_PRESSED, 4.5),
            ("C-1.7", TEXT_TERTIARY, BG, 4.5),
            ("C-1.8", TEXT_TERTIARY, BG_SECONDARY, 4.5),
            ("C-1.9", TEXT_TERTIARY, BG_SELECTED, 4.5),
            ("C-1.10", TEXT_BRAND, BG, 4.5),
            ("C-1.11", TEXT_BRAND, BG_SECONDARY, 4.5),
            ("C-1.12", TEXT_ERROR, BG, 4.5),
            ("C-1.13", TEXT_ERROR, BG_SECONDARY, 4.5),
            ("C-1.14", TEXT_ON_BRAND, BG_BRAND, 4.5),
            ("C-1.15", TEXT_ON_BRAND, BG_BRAND_HOVER, 4.5),
            ("C-1.16", TEXT_ON_BRAND, BG_BRAND_PRESSED, 4.5),
            ("C-1.17", TEXT_WHITE, BG_ERROR_SOLID, 4.5),
            ("C-1.18", TEXT_WHITE, BG_ERROR_SOLID_HOVER, 4.5),
            ("C-1.19", TEXT, BG_PRIMARY_SOLID, 4.5),
            ("C-1.20", TEXT_SECONDARY, BG_PRIMARY_SOLID, 4.5),
            ("C-1.21", BADGE_SUCCESS_TEXT, BG_SUCCESS_PRIMARY, 4.5),
            ("C-1.22", BADGE_WARNING_TEXT, BG_WARNING_PRIMARY, 4.5),
            ("C-1.23", BADGE_ERROR_TEXT, BG_ERROR_PRIMARY, 4.5),
            ("C-1.24", BADGE_NEUTRAL_TEXT, BADGE_NEUTRAL_BG, 4.5),
            ("C-1.27", ICON_TERTIARY, BG_SELECTED, 3.0),
            ("C-1.28", ICON_QUATERNARY, BG_SECONDARY, 3.0),
            ("C-1.29", FOCUS_RING, BG, 3.0),
            ("C-1.30", BORDER_SELECTED, BG_SECONDARY, 3.0),
            ("C-1.31", TILE_DESIGN_GLYPH, TILE_DESIGN_BG, 4.5),
            ("C-1.32", TILE_BOARD_GLYPH, TILE_BOARD_BG, 4.5),
            ("C-1.33", TILE_DECK_GLYPH, TILE_DECK_BG, 4.5),
            ("C-1.34", TILE_SPEC_GLYPH, TILE_SPEC_BG, 4.5),
            ("C-1.35", TEXT_ON_BRAND, DEV_READY, 4.5),
            ("C-1.36", DEV_READY, BG, 4.5),
            ("C-1.37", BORDER_SELECTED, BG, 3.0),
            ("C-1.38", BORDER_SELECTED, HOME_THUMB_BG, 3.0),
            ("C-1.39", TEXT_SECONDARY, BG_SECONDARY_HOVER, 4.5),
            ("C-1.40", TEXT_WARNING, star_bg, 3.0),
            ("C-1.41", AGENT_LABEL_TEXT, AGENT_1, 4.5),
            ("C-1.42", AGENT_LABEL_TEXT, AGENT_2, 4.5),
            ("C-1.43", AGENT_LABEL_TEXT, AGENT_3, 4.5),
            ("C-1.44", AGENT_LABEL_TEXT, AGENT_4, 4.5),
            ("C-1.45", TEXT_QUATERNARY, BG_PRIMARY, 4.5),
            ("C-1.46", STATUS_INFO_TEXT, STATUS_INFO_BG, 4.5),
            ("C-1.47", PROVIDER_CLAUDE, BG_PRIMARY, 3.0),
            ("C-1.48", PROVIDER_CODEX, BG_PRIMARY, 3.0),
            ("C-1.49", PROVIDER_GROK, BG_PRIMARY, 3.0),
            ("C-1.50", PROVIDER_GEMINI, BG_PRIMARY, 3.0),
            ("C-1.51", ICON_TERTIARY, BG_PRIMARY, 3.0),
            ("C-1.52", TEXT_SECONDARY_HOVER, BG_MENU_HOVER, 4.5),
            ("C-1.53", TEXT_QUATERNARY, BG_MENU, 4.5),
            ("C-1.54", TEXT, BG_PRIMARY_ALT, 4.5),
            ("C-1.55", TEXT_TERTIARY, BG_MENU, 4.5),
            ("C-1.56", TEXT_ON_LIGHT_CANVAS, CANVAS_COMPONENT, 4.5),
            ("C-1.57", TEXT_SECONDARY, BG_MENU, 4.5),
            ("C-1.58", TEXT_TERTIARY, BG_SECONDARY_HOVER, 4.5),
            ("C-1.59", WARNING, BG_MENU, 3.0),
            ("C-1.60", DANGER, BG_MENU, 3.0),
            ("C-1.61", INFO, BG_MENU, 3.0),
            ("C-1.62", SUCCESS, BG_MENU, 3.0),
            ("C-1.63", TEXT_ON_BRAND, DEV_READY_HOVER, 4.5),
            ("C-1.64", READER_LIGHT_TEXT_SUBTLE, READER_LIGHT_SURFACE_2, 4.5),
            ("C-1.65", TEXT_ERROR_HOVER, BG_ERROR_PRIMARY, 4.5),
            ("C-1.66", BORDER_ERROR, BG_ERROR_PRIMARY, 3.0),
            ("C-1.67", BORDER_ERROR_SUBTLE, BG, 3.0),
            ("C-1.68", BORDER_ERROR_SUBTLE, BG_ERROR_PRIMARY, 3.0),
            ("C-1.69", FOCUS_RING_ERROR, BG, 3.0),
            ("C-1.70", TEXT_ERROR, BG_SECONDARY, 4.5),
            ("C-1.71", TEXT_ERROR, BG_SELECTED, 4.5),
            ("C-1.72", ICON_SECONDARY, star_bg, 3.0),
        ];
        assert_eq!(pairs.len(), 70, "C-1 pass rows");
        for (id, fg, bg, min) in pairs {
            let ratio = wcag_contrast(fg, bg);
            assert!(ratio >= min, "{id} contrast {ratio:.2}:1 < min {min:.1}:1 ({fg:?} on {bg:?})");
        }
        // Floor rows are exceptions, not passes: exact hex and a floor, never a 4.5 pass.
        let floors = [
            ("E-1.a", TEXT_PLACEHOLDER, BG, 3.0, 0x737373, 0x0A0A0A),
            ("E-1.b", TEXT_PLACEHOLDER, BG_SECONDARY, 3.0, 0x737373, 0x171717),
            ("E-2.a", BORDER_STRONG, BG, 1.7, 0x404040, 0x0A0A0A),
            ("E-2.b", BORDER_STRONG, BG_SECONDARY, 1.7, 0x404040, 0x171717),
        ];
        assert_eq!(floors.len(), 4);
        for (id, fg, bg, floor, fh, bh) in floors {
            assert_eq!(opaque_hex(fg), fh, "{id} foreground");
            assert_eq!(opaque_hex(bg), bh, "{id} background");
            let ratio = wcag_contrast(fg, bg);
            assert!(ratio >= floor, "{id} {ratio:.2}:1 is under floor {floor}");
        }
    }
}
