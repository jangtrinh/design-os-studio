//! Tools, shortcuts, and the pure press-to-interaction state transitions.

use super::handles::{Handle, HandleHit};
use designos_doc::NodeId;

/// Whether the Arrow tool is offered: toolbar dropdown, Cmd+K, menus and the Shift+L key.
///
/// It stays off because the stroke model has no arrowhead, so the tool only drew a plain line.
/// Turn it on when arrowheads land (#859). The tool and its creation code are kept for that.
pub const ARROW_TOOL_ENABLED: bool = false;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Move,
    Hand,
    Scale,
    ShapeBuilder,
    Frame,
    Section,
    Slice,
    Rectangle,
    Line,
    Arrow,
    Ellipse,
    Polygon,
    Star,
    Image,
    Pen,
    Pencil,
    Text,
    Comment,
    // board tools
    Sticky,
    BoardShape,
    Connector,
    Stamp,
    Marker,
    Highlighter,
}

impl Tool {
    /// The tool to activate when `self` is asked for: a hidden tool falls back to Line, which
    /// is what the Arrow tool drew anyway.
    pub fn or_fallback(self) -> Tool {
        if self == Tool::Arrow && !ARROW_TOOL_ENABLED {
            Tool::Line
        } else {
            self
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Move => "Move",
            Tool::Hand => "Hand tool",
            Tool::Scale => "Scale",
            Tool::ShapeBuilder => "Shape builder",
            Tool::Frame => "Frame",
            Tool::Section => "Section",
            Tool::Slice => "Slice",
            Tool::Rectangle => "Rectangle",
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Ellipse => "Ellipse",
            Tool::Polygon => "Polygon",
            Tool::Star => "Star",
            Tool::Image => "Place image",
            Tool::Pen => "Pen",
            Tool::Pencil => "Pencil",
            Tool::Text => "Text",
            Tool::Comment => "Add comment",
            Tool::Sticky => "Sticky note",
            Tool::BoardShape => "Shape",
            Tool::Connector => "Connector",
            Tool::Stamp => "Stamp",
            Tool::Marker => "Marker",
            Tool::Highlighter => "Highlighter",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Tool::Move => "V",
            Tool::Hand => "H",
            Tool::Scale => "K",
            Tool::ShapeBuilder => "Shift+M",
            Tool::Frame => "F",
            Tool::Section => "Shift+S",
            Tool::Slice => "S",
            Tool::Rectangle => "R",
            Tool::Line => "L",
            Tool::Arrow => "Shift+L",
            Tool::Ellipse => "O",
            Tool::Polygon => "",
            Tool::Star => "",
            Tool::Image => "Ctrl+Shift+K",
            Tool::Pen => "P",
            Tool::Pencil => "Shift+P",
            Tool::Text => "T",
            Tool::Comment => "C",
            Tool::Sticky => "S",
            Tool::BoardShape => "R",
            Tool::Connector => "X",
            Tool::Stamp => "E",
            Tool::Marker => "P",
            Tool::Highlighter => "Shift+P",
        }
    }

    /// Shortcut key (no modifiers except shift) to tool.
    pub fn from_key(key: egui::Key, shift: bool) -> Option<Tool> {
        use egui::Key::*;
        Some(match (key, shift) {
            (V, false) => Tool::Move,
            (H, false) => Tool::Hand,
            (K, false) => Tool::Scale,
            (M, true) => Tool::ShapeBuilder,
            (F, false) => Tool::Frame,
            (S, true) => Tool::Section,
            (S, false) => Tool::Slice,
            (R, false) => Tool::Rectangle,
            (L, false) => Tool::Line,
            (L, true) => Tool::Arrow.or_fallback(),
            (O, false) => Tool::Ellipse,
            (P, false) => Tool::Pen,
            (P, true) => Tool::Pencil,
            (T, false) => Tool::Text,
            (C, false) => Tool::Comment,
            _ => return None,
        })
    }

    /// Shortcut keys of a board file (no modifiers except shift): V move, H hand, P marker,
    /// Shift+P highlighter, S sticky note, R shape, X connector, T text, Shift+S section, E stamp.
    pub fn from_key_board(key: egui::Key, shift: bool) -> Option<Tool> {
        use egui::Key::*;
        Some(match (key, shift) {
            (V, false) => Tool::Move,
            (H, false) => Tool::Hand,
            (P, false) => Tool::Marker,
            (P, true) => Tool::Highlighter,
            (S, false) => Tool::Sticky,
            (S, true) => Tool::Section,
            (R, false) => Tool::BoardShape,
            (X, false) => Tool::Connector,
            (T, false) => Tool::Text,
            (E, false) => Tool::Stamp,
            _ => return None,
        })
    }

    /// True for tools that create a node by dragging a box.
    pub fn is_shape_tool(self) -> bool {
        matches!(
            self,
            Tool::Frame
                | Tool::Section
                | Tool::Slice
                | Tool::Rectangle
                | Tool::Line
                | Tool::Arrow
                | Tool::Ellipse
                | Tool::Polygon
                | Tool::Star
                | Tool::Sticky
                | Tool::BoardShape
                | Tool::Stamp
        )
    }

    /// Freehand tools of a board: marker strokes are pencil strokes with a thick round pen.
    pub fn is_brush(self) -> bool {
        matches!(self, Tool::Marker | Tool::Highlighter)
    }
}

/// The shape group of the toolbar, without the Arrow tool unless [`ARROW_TOOL_ENABLED`].
const SHAPE_GROUP: &[Tool] = if ARROW_TOOL_ENABLED {
    &[Tool::Rectangle, Tool::Line, Tool::Arrow, Tool::Ellipse, Tool::Polygon, Tool::Star, Tool::Image]
} else {
    &[Tool::Rectangle, Tool::Line, Tool::Ellipse, Tool::Polygon, Tool::Star, Tool::Image]
};

/// Toolbar groups: first entry is the default tool shown on the button.
pub const GROUPS: &[&[Tool]] = &[
    &[Tool::Move, Tool::Hand, Tool::Scale, Tool::ShapeBuilder],
    &[Tool::Frame, Tool::Section, Tool::Slice],
    SHAPE_GROUP,
    &[Tool::Pen, Tool::Pencil],
    &[Tool::Text],
    &[Tool::Comment],
];

/// What the pointer is over when the primary button goes down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressTarget {
    Handle(HandleHit),
    /// A node that is part of the current selection.
    SelectedNode(NodeId),
    /// An unselected node.
    Node(NodeId),
    Empty,
}

/// What a press starts (the interaction is confirmed/refined on drag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressAction {
    Pan,
    Resize(Handle),
    Rotate(usize),
    /// Select (unless already selected) then drag-move. `select_first` is false
    /// when pressing on an already selected node (so multi-selection survives).
    MoveNode {
        select_first: bool,
    },
    Marquee,
    Draw(Tool),
    /// Brush over regions with the shape builder.
    Build,
    PenPoint,
    /// Drag a connector out of a node (or from empty canvas).
    Connect,
    TextCreate,
    Nothing,
}

/// Pure decision table for a primary-button press.
pub fn press_action(tool: Tool, target: PressTarget, space_held: bool, middle: bool) -> PressAction {
    if middle || space_held || tool == Tool::Hand {
        return PressAction::Pan;
    }
    match tool {
        Tool::Move | Tool::Scale => match target {
            PressTarget::Handle(HandleHit::Resize(h)) => PressAction::Resize(h),
            PressTarget::Handle(HandleHit::Rotate(c)) => PressAction::Rotate(c),
            PressTarget::SelectedNode(_) => PressAction::MoveNode { select_first: false },
            PressTarget::Node(_) => PressAction::MoveNode { select_first: true },
            PressTarget::Empty => PressAction::Marquee,
        },
        Tool::ShapeBuilder => PressAction::Build,
        Tool::Pen | Tool::Pencil | Tool::Marker | Tool::Highlighter => PressAction::PenPoint,
        Tool::Connector => PressAction::Connect,
        Tool::Text => PressAction::TextCreate,
        Tool::Comment | Tool::Image => PressAction::Nothing,
        t if t.is_shape_tool() => PressAction::Draw(t),
        _ => PressAction::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_map() {
        assert_eq!(Tool::from_key(egui::Key::R, false), Some(Tool::Rectangle));
        assert_eq!(Tool::from_key(egui::Key::S, true), Some(Tool::Section));
        assert_eq!(Tool::from_key(egui::Key::X, false), None);
    }

    #[test]
    fn arrow_tool_is_in_no_toolbar_group() {
        assert!(GROUPS.iter().all(|g| !g.contains(&Tool::Arrow)), "the Arrow tool draws a plain line, so no dropdown may offer it");
        assert!(GROUPS[2].contains(&Tool::Line), "Line stays in the shape group");
    }

    #[test]
    fn shift_l_selects_line_while_arrow_is_hidden() {
        assert_eq!(Tool::from_key(egui::Key::L, true), Some(Tool::Line));
        assert_eq!(Tool::from_key(egui::Key::L, false), Some(Tool::Line));
    }

    #[test]
    fn asking_for_the_hidden_arrow_tool_selects_line() {
        let mut app = crate::app::App::new(designos_doc::Document::new("t"));
        app.set_tool(Tool::Arrow);
        assert_eq!(app.tool, Tool::Line);
        app.set_tool(Tool::Ellipse);
        assert_eq!(app.tool, Tool::Ellipse, "other tools are untouched");
    }

    #[test]
    fn pan_overrides_everything() {
        let t = PressTarget::Node(NodeId(1));
        assert_eq!(press_action(Tool::Move, t, true, false), PressAction::Pan);
        assert_eq!(press_action(Tool::Rectangle, t, false, true), PressAction::Pan);
        assert_eq!(press_action(Tool::Hand, PressTarget::Empty, false, false), PressAction::Pan);
    }

    #[test]
    fn move_tool_transitions() {
        let n = NodeId(3);
        assert_eq!(press_action(Tool::Move, PressTarget::Empty, false, false), PressAction::Marquee);
        assert_eq!(press_action(Tool::Move, PressTarget::Node(n), false, false), PressAction::MoveNode { select_first: true });
        assert_eq!(press_action(Tool::Move, PressTarget::SelectedNode(n), false, false), PressAction::MoveNode { select_first: false });
        assert_eq!(press_action(Tool::Move, PressTarget::Handle(HandleHit::Resize(Handle::BR)), false, false), PressAction::Resize(Handle::BR));
        assert_eq!(press_action(Tool::Move, PressTarget::Handle(HandleHit::Rotate(1)), false, false), PressAction::Rotate(1));
    }

    #[test]
    fn drawing_tools_draw_even_over_nodes() {
        let t = PressTarget::Node(NodeId(1));
        assert_eq!(press_action(Tool::Ellipse, t, false, false), PressAction::Draw(Tool::Ellipse));
        assert_eq!(press_action(Tool::Frame, t, false, false), PressAction::Draw(Tool::Frame));
        assert_eq!(press_action(Tool::Text, t, false, false), PressAction::TextCreate);
        assert_eq!(press_action(Tool::Pen, t, false, false), PressAction::PenPoint);
    }

    #[test]
    fn groups_cover_all_default_tools() {
        assert!(GROUPS.iter().all(|g| !g.is_empty()));
        assert_eq!(GROUPS[0][0], Tool::Move);
    }
}
