use super::guard_tests::contrast;
use super::table_tests::doc;
use super::*;
use crate::theme;
use egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Border,
    Icon,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Floor {
    Resting,
    Placeholder,
    Edge,
    Carried,
}

pub(super) struct Exempt {
    row: &'static str,
    part: Part,
    color: Color32,
    states: &'static [State],
    reason: &'static str,
    floor: Floor,
}

const REST: &str = "resting boundary";
const HOLD: &str = "placeholder";
const EDGE: &str = "decorative edge";
const CARRIED: &str = "decorative: fill and label carry the state";
static DH: &[State] = &[State::Default, State::Hover];
static DHF: &[State] = &[State::Default, State::Hover, State::Focused];
static PRESSED: &[State] = &[State::Pressed];
static SELECTED: &[State] = &[State::Selected];
static ANY: &[State] = &[];

fn row(row: &'static str, part: Part, color: Color32, states: &'static [State], reason: &'static str, floor: Floor) -> Exempt {
    Exempt { row, part, color, states, reason, floor }
}

pub(super) fn exemptions() -> Vec<Exempt> {
    use Floor::*;
    use Part::*;
    vec![
        row("Checkbox/Box", Border, theme::BORDER_STRONG, DHF, REST, Resting),
        row("Input/Field", Border, theme::BORDER_STRONG, DH, REST, Resting),
        row("Input/PropertyField", Border, theme::BORDER_STRONG, DH, REST, Resting),
        row("Input/Search", Border, theme::BORDER_STRONG, DH, REST, Resting),
        row("Select/Trigger", Border, theme::BORDER_STRONG, DH, REST, Resting),
        row("Button/Secondary", Border, theme::BORDER_STRONG, DHF, REST, Resting),
        row("Button/Secondary", Border, theme::BORDER_STRONG, PRESSED, CARRIED, Carried),
        row("Tabs/Segmented", Border, theme::BORDER_STRONG, SELECTED, CARRIED, Carried),
        row("Radio/Card", Icon, theme::BORDER_STRONG, DHF, REST, Resting),
        row("Input/Search", Text, theme::TEXT_PLACEHOLDER, DH, HOLD, Placeholder),
        row("Tag/Chip", Border, theme::BORDER_STRONG, ANY, EDGE, Edge),
        row("EmptyState/Default", Border, theme::BORDER_STRONG, ANY, EDGE, Edge),
        row("Badge/StatusCount", Border, theme::BADGE_NEUTRAL_BORDER, ANY, EDGE, Edge),
        row("Badge/Neutral", Border, theme::BADGE_NEUTRAL_BORDER, ANY, EDGE, Edge),
        row("Badge/Success", Border, theme::BADGE_SUCCESS_BORDER, ANY, EDGE, Edge),
        row("Badge/Warning", Border, theme::BADGE_WARNING_BORDER, ANY, EDGE, Edge),
        row("Badge/Error", Border, theme::BADGE_ERROR_BORDER, ANY, EDGE, Edge),
        row("Tile", Border, theme::BORDER_STRONG, ANY, EDGE, Edge),
        row("Radio/Card", Border, theme::BORDER, DHF, EDGE, Edge),
        row("Menu/Panel", Border, theme::BORDER_MENU, ANY, EDGE, Edge),
        row("Menu/Context", Border, theme::BORDER_MENU, ANY, EDGE, Edge),
        row("Popover/Panel", Border, theme::BORDER_MENU, ANY, EDGE, Edge),
        row("Dialog/CommandPalette", Border, theme::BORDER_MENU, ANY, EDGE, Edge),
        row("Toast/Default", Border, theme::BORDER_MENU, ANY, EDGE, Edge),
        row("Section/Header", Border, theme::BORDER, ANY, EDGE, Edge),
        row("Loading/Inline", Border, theme::BORDER, ANY, EDGE, Edge),
        row("Slider/Handle", Border, theme::BG, ANY, EDGE, Edge),
        row("Toggle/Switch", Border, theme::BORDER, DHF, EDGE, Edge),
        row("Swatch/Color", Border, theme::BORDER_TRANSLUCENT, DHF, EDGE, Edge),
    ]
}

fn find<'a>(table: &'a [Exempt], row: &str, part: Part, state: State) -> Option<&'a Exempt> {
    table.iter().find(|entry| entry.row == row && entry.part == part && (entry.states.is_empty() || entry.states.contains(&state)))
}

fn over(fg: Color32, bg: Color32) -> Color32 {
    if fg.a() == 255 {
        return fg;
    }
    let inv = u32::from(255 - fg.a());
    let ch = |front: u8, back: u8| (u32::from(front) + u32::from(back) * inv / 255) as u8;
    Color32::from_rgba_premultiplied(ch(fg.r(), bg.r()), ch(fg.g(), bg.g()), ch(fg.b(), bg.b()), 255)
}

struct Hit<'a> {
    bad: &'a mut Vec<String>,
    table: &'a [Exempt],
    row: &'a str,
    state: State,
    where_: &'a str,
}

fn note(hit: &mut Hit<'_>, part: Part, color: Color32, bg: Color32, surface: Color32) {
    if color.a() == 0 {
        return;
    }
    let found = find(hit.table, hit.row, part, hit.state).map(|entry| (entry.reason, entry.color, entry.floor));
    let Some((reason, named, floor)) = found else {
        let floor = match part {
            Part::Text => 4.5,
            Part::Icon | Part::Border => 3.0,
        };
        let ratio = contrast(over(color, bg), bg);
        if ratio < floor {
            hit.bad.push(format!("{} {:?} {part:?} {ratio:.2} {}", hit.row, hit.state, hit.where_));
        }
        return;
    };
    if reason.is_empty() || color != named {
        hit.bad.push(format!("{} {:?} {part:?} named colour {}", hit.row, hit.state, hit.where_));
        return;
    }
    match floor {
        Floor::Resting => {
            let strong_hex = doc()["constants"]["BORDER_STRONG"].as_str() == Some("#404040");
            if !strong_hex || named != theme::BORDER_STRONG {
                hit.bad.push(format!("{} resting boundary hex {}", hit.row, hit.where_));
            }
            let ratio = contrast(named, surface);
            if ratio < 1.7 {
                hit.bad.push(format!("{} {:?} resting {ratio:.2} {}", hit.row, hit.state, hit.where_));
            }
            if bg == theme::BG_TERTIARY || bg == theme::BG_PRESSED {
                hit.bad.push(format!("{} {:?} resting on a filled state {}", hit.row, hit.state, hit.where_));
            }
        }
        Floor::Placeholder => {
            if bg == theme::BG_TERTIARY || bg == theme::BG_PRESSED {
                hit.bad.push(format!("{} {:?} placeholder on a filled state {}", hit.row, hit.state, hit.where_));
            }
            let ratio = contrast(over(color, bg), bg);
            if ratio < 3.0 {
                hit.bad.push(format!("{} {:?} placeholder {ratio:.2} {}", hit.row, hit.state, hit.where_));
            }
        }
        Floor::Edge => {}
        Floor::Carried => {
            if reason != CARRIED {
                hit.bad.push(format!("{} {:?} carried reason {}", hit.row, hit.state, hit.where_));
            }
        }
    }
}

pub(super) fn score(bad: &mut Vec<String>, table: &[Exempt], row: &str, state: State, look: Look, surface: Color32, where_: &str) {
    if state == State::Disabled {
        return;
    }
    let bg = if look.fill.a() == 255 { look.fill } else { surface };
    let mut hit = Hit { bad, table, row, state, where_ };
    note(&mut hit, Part::Text, look.text, bg, surface);
    note(&mut hit, Part::Icon, look.icon, bg, surface);
    note(&mut hit, Part::Border, look.border, bg, surface);
    if look.mark.a() == 255 && look.icon.a() == 255 {
        let ratio = contrast(look.mark, look.icon);
        if ratio < 3.0 {
            hit.bad.push(format!("{} {:?} mark {ratio:.2} {}", hit.row, hit.state, hit.where_));
        }
    }
}
