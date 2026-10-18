//! Prototype mode: editing interactions (Prototype tab, canvas noodles) and
//! the player behind the Present view.

pub mod easing;
pub mod geom;
pub mod player;
pub mod smart;

use crate::app::App;
use designos_doc::*;
use player::top_level_frames;

/// Editing state of the Prototype tab and canvas.
#[derive(Default)]
pub struct ProtoState {
    /// Interaction being edited in the popover: (layer, reaction index).
    pub editing: Option<(NodeId, usize)>,
    /// Screen rect of the row that opened the popover (clicks on it do not close it).
    pub editing_anchor: Option<egui::Rect>,
    /// Popover height measured last frame (the background is drawn before the content).
    pub popover_h: f32,
    /// Flow whose name is being edited in the list.
    pub scroll: f32,
    pub content_h: f32,
    /// Scroll offset and content height of the Frame preset list in the Design tab.
    pub presets_scroll: f32,
    pub presets_content_h: f32,
    /// Hotspot drag started on the canvas: (source layer, pointer position).
    pub connect: Option<(NodeId, egui::Pos2)>,
    /// Layer whose hotspot the pointer is over.
    pub hot: Option<NodeId>,
    /// Deepest layer under the pointer while in prototype mode.
    pub hover: Option<NodeId>,
    pub present: Option<crate::present::PresentState>,
}

/// Category in Figma's Frame / Device preset catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetCategory {
    Phone,
    Tablet,
    Desktop,
    Presentation,
    Watch,
    Paper,
    SocialMedia,
}

/// A named frame or device preset with Figma's dimensions and corner radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePreset {
    pub category: PresetCategory,
    pub name: &'static str,
    pub width: f32,
    pub height: f32,
    pub corner_radius: f32,
}

/// Complete Figma frame and device preset catalogue across all categories.
pub const FRAME_PRESETS: &[FramePreset] = &[
    // Phone — iPhone 17 family
    FramePreset { category: PresetCategory::Phone, name: "iPhone 17", width: 402.0, height: 874.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 17 Air", width: 420.0, height: 912.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 17 Pro", width: 402.0, height: 874.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 17 Pro Max", width: 440.0, height: 956.0, corner_radius: 55.0 },
    // Phone — iPhone 16 family
    FramePreset { category: PresetCategory::Phone, name: "iPhone 16", width: 393.0, height: 852.0, corner_radius: 48.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 16 Plus", width: 430.0, height: 932.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 16 Pro", width: 402.0, height: 874.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 16 Pro Max", width: 440.0, height: 956.0, corner_radius: 55.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 16e", width: 390.0, height: 844.0, corner_radius: 47.0 },
    // Phone — iPhone 15 / 14 / 13 / SE
    FramePreset { category: PresetCategory::Phone, name: "iPhone 15", width: 393.0, height: 852.0, corner_radius: 48.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 15 Plus", width: 430.0, height: 932.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 15 Pro", width: 393.0, height: 852.0, corner_radius: 48.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 15 Pro Max", width: 430.0, height: 932.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 14 & 15 Pro", width: 393.0, height: 852.0, corner_radius: 48.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 14 & 15 Pro Max", width: 430.0, height: 932.0, corner_radius: 52.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 14", width: 390.0, height: 844.0, corner_radius: 47.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 14 Plus", width: 428.0, height: 926.0, corner_radius: 50.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 13 & 14", width: 390.0, height: 844.0, corner_radius: 47.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone 13 mini", width: 375.0, height: 812.0, corner_radius: 44.0 },
    FramePreset { category: PresetCategory::Phone, name: "iPhone SE", width: 320.0, height: 568.0, corner_radius: 0.0 },
    // Phone — Android
    FramePreset { category: PresetCategory::Phone, name: "Android Compact", width: 412.0, height: 917.0, corner_radius: 36.0 },
    FramePreset { category: PresetCategory::Phone, name: "Android Medium", width: 700.0, height: 840.0, corner_radius: 28.0 },
    FramePreset { category: PresetCategory::Phone, name: "Android Large", width: 360.0, height: 800.0, corner_radius: 32.0 },
    FramePreset { category: PresetCategory::Phone, name: "Android Small", width: 360.0, height: 640.0, corner_radius: 24.0 },
    // Tablet
    FramePreset { category: PresetCategory::Tablet, name: "iPad mini 8.3", width: 744.0, height: 1133.0, corner_radius: 28.0 },
    FramePreset { category: PresetCategory::Tablet, name: "iPad mini", width: 744.0, height: 1133.0, corner_radius: 28.0 },
    FramePreset { category: PresetCategory::Tablet, name: "iPad Pro 11\"", width: 834.0, height: 1194.0, corner_radius: 28.0 },
    FramePreset { category: PresetCategory::Tablet, name: "iPad Pro 12.9\"", width: 1024.0, height: 1366.0, corner_radius: 28.0 },
    FramePreset { category: PresetCategory::Tablet, name: "Surface Pro 8", width: 1440.0, height: 960.0, corner_radius: 16.0 },
    FramePreset { category: PresetCategory::Tablet, name: "Android Expanded", width: 1280.0, height: 800.0, corner_radius: 24.0 },
    // Desktop
    FramePreset { category: PresetCategory::Desktop, name: "MacBook Air", width: 1280.0, height: 832.0, corner_radius: 12.0 },
    FramePreset { category: PresetCategory::Desktop, name: "MacBook Pro 14\"", width: 1512.0, height: 982.0, corner_radius: 12.0 },
    FramePreset { category: PresetCategory::Desktop, name: "MacBook Pro 16\"", width: 1728.0, height: 1117.0, corner_radius: 12.0 },
    FramePreset { category: PresetCategory::Desktop, name: "Desktop", width: 1440.0, height: 1024.0, corner_radius: 8.0 },
    FramePreset { category: PresetCategory::Desktop, name: "Wireframes", width: 1440.0, height: 1024.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Desktop, name: "TV", width: 1280.0, height: 720.0, corner_radius: 0.0 },
    // Presentation
    FramePreset { category: PresetCategory::Presentation, name: "Slide 16:9", width: 1920.0, height: 1080.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Presentation, name: "Slide 4:3", width: 1024.0, height: 768.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Presentation, name: "Presentation", width: 1920.0, height: 1080.0, corner_radius: 0.0 },
    // Watch
    FramePreset { category: PresetCategory::Watch, name: "Apple Watch Series 10 42mm", width: 187.0, height: 223.0, corner_radius: 44.0 },
    FramePreset { category: PresetCategory::Watch, name: "Apple Watch Series 10 46mm", width: 208.0, height: 248.0, corner_radius: 48.0 },
    FramePreset { category: PresetCategory::Watch, name: "Apple Watch 41mm", width: 176.0, height: 215.0, corner_radius: 40.0 },
    FramePreset { category: PresetCategory::Watch, name: "Apple Watch 45mm", width: 198.0, height: 242.0, corner_radius: 44.0 },
    FramePreset { category: PresetCategory::Watch, name: "Apple Watch 42mm", width: 156.0, height: 195.0, corner_radius: 36.0 },
    FramePreset { category: PresetCategory::Watch, name: "Apple Watch 38mm", width: 136.0, height: 170.0, corner_radius: 32.0 },
    // Paper
    FramePreset { category: PresetCategory::Paper, name: "A4", width: 595.0, height: 842.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Paper, name: "A5", width: 420.0, height: 595.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Paper, name: "A6", width: 297.0, height: 420.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Paper, name: "Letter", width: 612.0, height: 792.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::Paper, name: "Tabloid", width: 792.0, height: 1224.0, corner_radius: 0.0 },
    // Social Media
    FramePreset { category: PresetCategory::SocialMedia, name: "Twitter post", width: 1200.0, height: 675.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Twitter header", width: 1500.0, height: 500.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Facebook post", width: 1200.0, height: 630.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Facebook cover", width: 820.0, height: 312.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Instagram post", width: 1080.0, height: 1080.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Instagram story", width: 1080.0, height: 1920.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Dribbble shot", width: 400.0, height: 300.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "Dribbble shot HD", width: 800.0, height: 600.0, corner_radius: 0.0 },
    FramePreset { category: PresetCategory::SocialMedia, name: "LinkedIn cover", width: 1584.0, height: 396.0, corner_radius: 0.0 },
];

/// Space between a new preset frame and the top-level frame it is stepped past (a multiple of 4).
const PRESET_FRAME_GAP: f32 = 40.0;

/// Looks up a [`FramePreset`] by exact name.
pub fn find_frame_preset(name: &str) -> Option<&'static FramePreset> {
    FRAME_PRESETS.iter().find(|p| p.name == name)
}

/// Returns all [`FramePreset`] entries in `category`, in catalogue order.
pub fn frame_presets_in_category(category: PresetCategory) -> Vec<&'static FramePreset> {
    FRAME_PRESETS.iter().filter(|p| p.category == category).collect()
}

/// Device presets: name, width, height, corner radius. The chosen preset is stored in
/// `Document::prototype_device`; Present letterboxes the start frame into its size.
pub const DEVICES: &[(&str, f32, f32, f32)] = &[
    ("None", 0.0, 0.0, 0.0),
    ("iPhone 15", 393.0, 852.0, 48.0),
    ("iPhone 15 Pro Max", 430.0, 932.0, 52.0),
    ("iPhone 17", 402.0, 874.0, 52.0),
    ("iPhone 17 Air", 420.0, 912.0, 52.0),
    ("iPhone 17 Pro", 402.0, 874.0, 52.0),
    ("iPhone 17 Pro Max", 440.0, 956.0, 55.0),
    ("iPhone 16", 393.0, 852.0, 48.0),
    ("iPhone 16 Plus", 430.0, 932.0, 52.0),
    ("iPhone 16 Pro", 402.0, 874.0, 52.0),
    ("iPhone 16 Pro Max", 440.0, 956.0, 55.0),
    ("iPhone 16e", 390.0, 844.0, 47.0),
    ("iPhone 15 Plus", 430.0, 932.0, 52.0),
    ("iPhone 15 Pro", 393.0, 852.0, 48.0),
    ("iPhone 14 & 15 Pro", 393.0, 852.0, 48.0),
    ("iPhone 14 & 15 Pro Max", 430.0, 932.0, 52.0),
    ("iPhone 14", 390.0, 844.0, 47.0),
    ("iPhone 14 Plus", 428.0, 926.0, 50.0),
    ("iPhone 13 & 14", 390.0, 844.0, 47.0),
    ("iPhone 13 mini", 375.0, 812.0, 44.0),
    ("iPhone SE", 320.0, 568.0, 0.0),
    ("Android Compact", 412.0, 917.0, 36.0),
    ("Android Medium", 700.0, 840.0, 28.0),
    ("Android Large", 360.0, 800.0, 32.0),
    ("Android Small", 360.0, 640.0, 24.0),
    ("Android Expanded", 1280.0, 800.0, 24.0),
    ("iPad mini 8.3", 744.0, 1133.0, 28.0),
    ("iPad mini", 744.0, 1133.0, 28.0),
    ("iPad Pro 11\"", 834.0, 1194.0, 28.0),
    ("iPad Pro 12.9\"", 1024.0, 1366.0, 28.0),
    ("Surface Pro 8", 1440.0, 960.0, 16.0),
    ("MacBook Air", 1280.0, 832.0, 12.0),
    ("MacBook Pro 14\"", 1512.0, 982.0, 12.0),
    ("MacBook Pro 16\"", 1728.0, 1117.0, 12.0),
    ("Desktop", 1440.0, 1024.0, 8.0),
    ("TV", 1280.0, 720.0, 0.0),
    ("Presentation", 1920.0, 1080.0, 0.0),
    ("Slide 16:9", 1920.0, 1080.0, 0.0),
    ("Slide 4:3", 1024.0, 768.0, 0.0),
    ("Apple Watch Series 10 42mm", 187.0, 223.0, 44.0),
    ("Apple Watch Series 10 46mm", 208.0, 248.0, 48.0),
    ("Apple Watch 41mm", 176.0, 215.0, 40.0),
    ("Apple Watch 45mm", 198.0, 242.0, 44.0),
    ("Apple Watch 42mm", 156.0, 195.0, 36.0),
    ("Apple Watch 38mm", 136.0, 170.0, 32.0),
];

/// Corner radius used for a stored device whose name is not a known preset.
const CUSTOM_DEVICE_RADIUS: f32 = 24.0;

/// Index of the stored device in [`DEVICES`] (0 = None). Unknown names map to `None`.
pub fn device_index(d: &PrototypeDevice) -> Option<usize> {
    match &d.preset {
        None => Some(0),
        Some(name) => DEVICES.iter().position(|p| p.0 == name && p.1 > 0.0),
    }
}

/// The stored device as `Command` payload for preset `i` (keeps the background).
pub fn device_for(i: usize, keep: &PrototypeDevice) -> PrototypeDevice {
    match DEVICES.get(i).filter(|d| d.1 > 0.0) {
        Some(d) => PrototypeDevice { preset: Some(d.0.to_string()), size: Some(Vec2::new(d.1, d.2)), background: keep.background },
        None => PrototypeDevice { preset: None, size: None, background: keep.background },
    }
}

/// Viewport size and corner radius to present in; `None` = no device frame.
pub fn device_viewport(d: &PrototypeDevice) -> Option<(Vec2, f32)> {
    d.preset.as_ref()?;
    let known = d.preset.as_ref().and_then(|n| {
        DEVICES.iter().find(|p| p.0 == n && p.1 > 0.0).map(|p| (p.1, p.2, p.3)).or_else(|| find_frame_preset(n).map(|p| (p.width, p.height, p.corner_radius)))
    });
    let size = d.size.or_else(|| known.map(|p| Vec2::new(p.0, p.1)))?;
    (size.x > 0.0 && size.y > 0.0).then(|| (size, known.map(|p| p.2).unwrap_or(CUSTOM_DEVICE_RADIUS)))
}

impl App {
    /// Persists the device as one undo step.
    pub fn set_prototype_device(&mut self, device: PrototypeDevice) {
        if device == self.doc.prototype_device {
            return;
        }
        if let Err(e) = self.hist.exec(&mut self.doc, Command::SetPrototypeDevice { device }) {
            self.show_toast(format!("Edit failed: {e}"), true);
        }
        self.changed();
    }

    /// Creates a new top-level frame on the active page sized and named from a [`FramePreset`]. It opens
    /// centred on the viewport, moved right past any top-level frame it would otherwise cover; when that
    /// pushes its centre off screen the view pans to it (the zoom stays).
    pub fn create_preset_frame(&mut self, preset_name: &str) -> Option<NodeId> {
        let preset = find_frame_preset(preset_name)?;
        let at = self.preset_frame_origin(preset.width, preset.height);
        let id = self.doc.alloc_id();
        let mut frame = Node::frame(id, Rect::new(at.x, at.y, preset.width, preset.height));
        frame.name = preset.name.to_string();
        frame.corner_radii = [preset.corner_radius; 4];
        let index = self.doc.get(self.page).map(|p| p.children.len()).unwrap_or(0);
        self.hist.exec(&mut self.doc, Command::Insert { parent: self.page, index, nodes: vec![frame] }).ok()?;
        self.selection = vec![id];
        self.pan_to_show_centre_of(id);
        self.changed();
        Some(id)
    }

    /// Pans (never zooms) so the centre of `id` is on screen, centring it the way `zoom_on_content` does.
    fn pan_to_show_centre_of(&mut self, id: NodeId) {
        let b = crate::util::world_aabb(&self.doc, id);
        let centre = Vec2::new(b.x + b.w / 2.0, b.y + b.h / 2.0);
        let (top_left, bottom_right) =
            (self.view.to_page(self.canvas_rect.min, self.canvas_rect.min), self.view.to_page(self.canvas_rect.min, self.canvas_rect.max));
        if (top_left.x..=bottom_right.x).contains(&centre.x) && (top_left.y..=bottom_right.y).contains(&centre.y) {
            return;
        }
        let size = self.canvas_rect.size();
        self.view.offset = Vec2::new(centre.x - size.x / 2.0 / self.view.zoom, centre.y - size.y / 2.0 / self.view.zoom);
    }

    /// Top-left page position for a new `w` x `h` top-level frame: its centre on the viewport centre
    /// (whole pixels), then stepped right of every top-level frame it overlaps.
    fn preset_frame_origin(&self, w: f32, h: f32) -> Vec2 {
        let centre = self.view.to_page(self.canvas_rect.min, self.canvas_rect.center());
        let mut at = Rect::new((centre.x - w / 2.0).round(), (centre.y - h / 2.0).round(), w, h);
        let siblings: Vec<Rect> =
            self.doc.get(self.page).map(|p| p.children.iter().map(|c| crate::util::world_aabb(&self.doc, *c)).collect()).unwrap_or_default();
        // A step clears every frame it overlapped and only moves right, so it ends within `siblings.len()` steps.
        for _ in 0..=siblings.len() {
            let Some(right) = siblings.iter().filter(|s| s.intersects(&at)).map(|s| s.x + s.w).reduce(f32::max) else { break };
            at.x = (right + PRESET_FRAME_GAP).ceil();
        }
        Vec2::new(at.x, at.y)
    }
}

// ------------------------------------------------------------------ labels

pub const TRIGGERS: &[&str] =
    &["On click", "On drag", "While hovering", "While pressing", "Mouse enter", "Mouse leave", "Mouse down", "Mouse up", "After delay", "Key", "Gamepad"];

/// Controllers a gamepad trigger can be bound to, in menu order.
pub const GAMEPAD_DEVICES: &[(&str, GamepadDevice)] = &[
    ("Any controller", GamepadDevice::Generic),
    ("Xbox", GamepadDevice::Xbox),
    ("PlayStation", GamepadDevice::PlayStation),
    ("Switch Pro", GamepadDevice::SwitchPro),
];

pub fn gamepad_device_index(d: GamepadDevice) -> usize {
    GAMEPAD_DEVICES.iter().position(|(_, x)| *x == d).unwrap_or(0)
}

pub fn trigger_index(t: &Trigger) -> usize {
    match t {
        Trigger::OnClick => 0,
        Trigger::OnDrag => 1,
        Trigger::WhileHovering => 2,
        Trigger::WhilePressing => 3,
        Trigger::MouseEnter { .. } => 4,
        Trigger::MouseLeave { .. } => 5,
        Trigger::MouseDown { .. } => 6,
        Trigger::MouseUp { .. } => 7,
        Trigger::AfterDelay { .. } => 8,
        Trigger::KeyPress { .. } => 9,
        Trigger::GamepadPress { .. } => 10,
    }
}

/// A fresh trigger of the i-th kind, keeping the delay of `prev` when both have one.
pub fn trigger_of_index(i: usize, prev: &Trigger) -> Trigger {
    let d = trigger_delay(prev).unwrap_or(match i {
        8 => 800,
        _ => 0,
    });
    match i {
        0 => Trigger::OnClick,
        1 => Trigger::OnDrag,
        2 => Trigger::WhileHovering,
        3 => Trigger::WhilePressing,
        4 => Trigger::MouseEnter { delay_ms: d },
        5 => Trigger::MouseLeave { delay_ms: d },
        6 => Trigger::MouseDown { delay_ms: d },
        7 => Trigger::MouseUp { delay_ms: d },
        8 => Trigger::AfterDelay { delay_ms: d },
        9 => Trigger::KeyPress { keys: if let Trigger::KeyPress { keys } = prev { keys.clone() } else { "Space".into() } },
        _ => match prev {
            Trigger::GamepadPress { .. } => prev.clone(),
            _ => Trigger::GamepadPress { device: GamepadDevice::Generic, buttons: "A".into() },
        },
    }
}

pub fn trigger_delay(t: &Trigger) -> Option<u32> {
    match t {
        Trigger::MouseEnter { delay_ms }
        | Trigger::MouseLeave { delay_ms }
        | Trigger::MouseDown { delay_ms }
        | Trigger::MouseUp { delay_ms }
        | Trigger::AfterDelay { delay_ms } => Some(*delay_ms),
        _ => None,
    }
}

pub fn with_delay(t: &Trigger, ms: u32) -> Trigger {
    match t {
        Trigger::MouseEnter { .. } => Trigger::MouseEnter { delay_ms: ms },
        Trigger::MouseLeave { .. } => Trigger::MouseLeave { delay_ms: ms },
        Trigger::MouseDown { .. } => Trigger::MouseDown { delay_ms: ms },
        Trigger::MouseUp { .. } => Trigger::MouseUp { delay_ms: ms },
        Trigger::AfterDelay { .. } => Trigger::AfterDelay { delay_ms: ms },
        other => other.clone(),
    }
}

pub fn trigger_label(t: &Trigger) -> String {
    match t {
        Trigger::KeyPress { keys } if !keys.is_empty() => format!("Key {keys}"),
        Trigger::GamepadPress { buttons, .. } if !buttons.is_empty() => format!("Gamepad {buttons}"),
        Trigger::AfterDelay { delay_ms } => format!("After delay {delay_ms}ms"),
        other => TRIGGERS[trigger_index(other)].to_string(),
    }
}

pub const ACTIONS: &[&str] = &[
    "Navigate to",
    "Back",
    "Open overlay",
    "Swap overlay",
    "Close overlay",
    "Scroll to",
    "Open link",
    "Change to",
    "Set variable",
    "Set variable mode",
    "Conditional",
    "Control video",
];

pub fn action_index(a: &Action) -> usize {
    match a {
        Action::Navigate { .. } => 0,
        Action::Back => 1,
        Action::OpenOverlay { .. } => 2,
        Action::SwapOverlay { .. } => 3,
        Action::CloseOverlay => 4,
        Action::ScrollTo { .. } => 5,
        Action::OpenUrl { .. } => 6,
        Action::ChangeTo { .. } => 7,
        Action::SetVariable { .. } => 8,
        Action::SetVariableMode { .. } => 9,
        Action::Conditional { .. } => 10,
        Action::MediaControl { .. } => 11,
    }
}

pub fn action_dest(a: &Action) -> Option<NodeId> {
    match a {
        Action::Navigate { dest, .. }
        | Action::OpenOverlay { dest, .. }
        | Action::SwapOverlay { dest }
        | Action::ScrollTo { dest, .. }
        | Action::ChangeTo { dest }
        | Action::MediaControl { dest, .. } => Some(*dest),
        _ => None,
    }
}

/// A new action of the i-th kind, keeping the destination of `prev` when it has one.
/// `doc` supplies the first variable / collection / video for the variable and media actions.
pub fn action_of_index(doc: &Document, i: usize, prev: &Action, fallback_dest: NodeId) -> Action {
    let dest = action_dest(prev).unwrap_or(fallback_dest);
    match i {
        0 => Action::Navigate { dest, preserve_scroll: false },
        1 => Action::Back,
        2 => Action::OpenOverlay { dest, position: OverlayPosition::Center, offset: Vec2::ZERO, close_on_click_outside: true, background: None },
        3 => Action::SwapOverlay { dest },
        4 => Action::CloseOverlay,
        5 => Action::ScrollTo { dest, offset: Vec2::ZERO },
        6 => Action::OpenUrl { url: "https://".into(), new_tab: true },
        7 => Action::ChangeTo { dest },
        8 => default_set_variable(doc),
        9 => {
            let coll = doc.variable_collections.values().next();
            Action::SetVariableMode {
                collection: coll.map(|c| c.id.clone()).unwrap_or_default(),
                mode: coll.map(|c| c.default_mode.clone()).unwrap_or_default(),
            }
        }
        10 => Action::Conditional { branches: vec![(Expression::boolean(true), vec![Action::Back])], otherwise: Vec::new() },
        _ => Action::MediaControl { dest: video_layers(doc).first().copied().unwrap_or(dest), op: MediaOp::TogglePlay },
    }
}

/// "Set variable" on the first variable, with a value that suits its type.
pub fn default_set_variable(doc: &Document) -> Action {
    let first = doc.variable_collections.values().flat_map(|c| c.variables.iter()).find_map(|id| doc.variables.get(id));
    match first {
        Some(v) => Action::SetVariable { variable: v.id.clone(), value: default_value_for(v.kind, v, doc) },
        None => Action::SetVariable { variable: String::new(), value: Expression::number(0.0) },
    }
}

/// A starting expression for `kind`: "<variable> + 1" for numbers, a plain literal otherwise.
pub fn default_value_for(kind: VariableKind, v: &Variable, _doc: &Document) -> Expression {
    match kind {
        VariableKind::Number => Expression::binary(BinaryOp::Add, Expression::var(v.id.clone()), Expression::number(1.0)),
        VariableKind::String => Expression::text(""),
        VariableKind::Boolean => Expression::logical_not(Expression::var(v.id.clone())),
        VariableKind::Color => Expression::Literal(ExprValue::Color(Color::BLACK)),
    }
}

/// Layers with a video fill, in document order.
pub fn video_layers(doc: &Document) -> Vec<NodeId> {
    doc.nodes.values().filter(|n| !n.prototype.videos.is_empty()).map(|n| n.id).collect()
}

pub const MEDIA_OPS: &[&str] = &["Play", "Pause", "Toggle play/pause", "Mute", "Unmute", "Toggle mute", "Skip by", "Skip to time", "Loop", "Don't loop"];

pub fn media_op_index(op: &MediaOp) -> usize {
    match op {
        MediaOp::Play => 0,
        MediaOp::Pause => 1,
        MediaOp::TogglePlay => 2,
        MediaOp::Mute => 3,
        MediaOp::Unmute => 4,
        MediaOp::ToggleMute => 5,
        MediaOp::SkipBy(_) => 6,
        MediaOp::SeekTo(_) => 7,
        MediaOp::SetLoop(true) => 8,
        MediaOp::SetLoop(false) => 9,
    }
}

pub fn media_op_of_index(i: usize, prev: &MediaOp) -> MediaOp {
    let secs = match prev {
        MediaOp::SkipBy(s) | MediaOp::SeekTo(s) => *s,
        _ => 5.0,
    };
    match i {
        0 => MediaOp::Play,
        1 => MediaOp::Pause,
        2 => MediaOp::TogglePlay,
        3 => MediaOp::Mute,
        4 => MediaOp::Unmute,
        5 => MediaOp::ToggleMute,
        6 => MediaOp::SkipBy(secs),
        7 => MediaOp::SeekTo(secs.max(0.0)),
        8 => MediaOp::SetLoop(true),
        _ => MediaOp::SetLoop(false),
    }
}

/// Whether the action plays an animation (so the editor shows the transition).
pub fn action_animates(a: &Action) -> bool {
    match a {
        Action::Conditional { branches, otherwise } => {
            branches.iter().any(|(_, acts)| acts.iter().any(action_animates)) || otherwise.iter().any(action_animates)
        }
        Action::Back
        | Action::CloseOverlay
        | Action::OpenUrl { .. }
        | Action::SetVariable { .. }
        | Action::SetVariableMode { .. }
        | Action::MediaControl { .. } => false,
        _ => true,
    }
}

/// Calls `f` on every action, descending into conditional branches.
pub fn walk_actions<'a>(actions: &'a [Action], f: &mut impl FnMut(&'a Action)) {
    for a in actions {
        f(a);
        if let Action::Conditional { branches, otherwise } = a {
            for (_, acts) in branches {
                walk_actions(acts, f);
            }
            walk_actions(otherwise, f);
        }
    }
}

pub fn action_label(doc: &Document, a: &Action) -> String {
    let name = |id: &NodeId| doc.get(*id).map(|n| n.name.clone()).unwrap_or_else(|| "(missing)".into());
    match a {
        Action::Navigate { dest, .. } => format!("Navigate to {}", name(dest)),
        Action::Back => "Back".into(),
        Action::OpenOverlay { dest, .. } => format!("Open overlay {}", name(dest)),
        Action::SwapOverlay { dest } => format!("Swap overlay {}", name(dest)),
        Action::CloseOverlay => "Close overlay".into(),
        Action::ScrollTo { dest, .. } => format!("Scroll to {}", name(dest)),
        Action::OpenUrl { url, .. } => format!("Open link {url}"),
        Action::ChangeTo { dest } => format!("Change to {}", name(dest)),
        Action::SetVariable { variable, value } => format!("Set {} = {}", variable_name(doc, variable), expression_text(doc, value)),
        Action::SetVariableMode { collection, mode } => {
            let c = doc.variable_collections.get(collection);
            let m = c.and_then(|c| c.modes.iter().find(|m| &m.id == mode)).map(|m| m.name.clone()).unwrap_or_else(|| "(missing)".into());
            format!("Set mode {}: {m}", c.map(|c| c.name.as_str()).unwrap_or("(missing)"))
        }
        Action::Conditional { branches, otherwise } => {
            let first = branches.first().map(|(c, _)| expression_text(doc, c)).unwrap_or_default();
            let tail = if otherwise.is_empty() { "" } else { ", else" };
            format!("If {first}{tail}")
        }
        Action::MediaControl { dest, op } => format!("{} {}", MEDIA_OPS[media_op_index(op)], name(dest)),
    }
}

// ------------------------------------------------------------- expressions

pub fn variable_name(doc: &Document, id: &str) -> String {
    doc.variables.get(id).map(|v| v.name.clone()).unwrap_or_else(|| if id.is_empty() { "(none)".into() } else { "(missing)".into() })
}

/// Text form of an expression, with variable names.
pub fn expression_text(doc: &Document, e: &Expression) -> String {
    e.to_source(&|id| doc.variables.get(id).map(|v| v.name.clone()))
}

/// Finds a variable id from a name as typed in an expression: exact name or id, else a unique
/// case-insensitive name, else a unique last segment of a grouped name ("Cart/count" for count).
pub fn find_variable(doc: &Document, name: &str) -> Option<String> {
    if doc.variables.contains_key(name) {
        return Some(name.to_string());
    }
    let by = |pred: &dyn Fn(&Variable) -> bool| {
        let hits: Vec<&Variable> = doc.variables.values().filter(|v| pred(v)).collect();
        (hits.len() == 1).then(|| hits[0].id.clone())
    };
    if let Some(id) = doc.variables.values().find(|v| v.name == name).map(|v| v.id.clone()) {
        return Some(id);
    }
    by(&|v| v.name.eq_ignore_ascii_case(name)).or_else(|| by(&|v| v.name.rsplit('/').next().is_some_and(|last| last.eq_ignore_ascii_case(name))))
}

/// Parses and type-checks expression text typed in the editor. `expected` is the type the value
/// must have (the variable's type for Set variable, boolean for conditions).
pub fn parse_expression_text(doc: &Document, text: &str, expected: Option<VariableKind>) -> Result<Expression, String> {
    let e = parse_expression(text, &|n| find_variable(doc, n)).map_err(|e| e.to_string())?;
    let ty = e.type_of(&|id| doc.variables.get(id).map(|v| v.kind))?;
    match expected {
        // Anything can be stored in a text variable (numbers and booleans are written out).
        Some(VariableKind::String) | None => {}
        Some(k) if ExprType::from(k) != ty => return Err(format!("Expected a {} value, not {}", ExprType::from(k).name(), ty.name())),
        Some(_) => {}
    }
    Ok(e)
}

/// Same as [`parse_expression_text`] for a condition.
pub fn parse_condition_text(doc: &Document, text: &str) -> Result<Expression, String> {
    let e = parse_expression_text(doc, text, None)?;
    let ty = e.type_of(&|id| doc.variables.get(id).map(|v| v.kind))?;
    if ty != ExprType::Boolean {
        return Err(format!("A condition must be true or false, not {}", ty.name()));
    }
    Ok(e)
}

pub const ANIMATIONS: &[&str] = &["Instant", "Dissolve", "Smart animate", "Move in", "Move out", "Push", "Slide in", "Slide out"];

pub fn animation_index(t: &Option<Transition>) -> usize {
    match t.map(|t| t.kind) {
        None => 0,
        Some(TransitionKind::Dissolve) => 1,
        Some(TransitionKind::SmartAnimate) => 2,
        Some(TransitionKind::MoveIn(_)) => 3,
        Some(TransitionKind::MoveOut(_)) => 4,
        Some(TransitionKind::Push(_)) => 5,
        Some(TransitionKind::SlideIn(_)) => 6,
        Some(TransitionKind::SlideOut(_)) => 7,
    }
}

pub fn transition_direction(k: TransitionKind) -> Option<Direction> {
    match k {
        TransitionKind::MoveIn(d) | TransitionKind::MoveOut(d) | TransitionKind::Push(d) | TransitionKind::SlideIn(d) | TransitionKind::SlideOut(d) => Some(d),
        _ => None,
    }
}

pub fn with_direction(k: TransitionKind, d: Direction) -> TransitionKind {
    match k {
        TransitionKind::MoveIn(_) => TransitionKind::MoveIn(d),
        TransitionKind::MoveOut(_) => TransitionKind::MoveOut(d),
        TransitionKind::Push(_) => TransitionKind::Push(d),
        TransitionKind::SlideIn(_) => TransitionKind::SlideIn(d),
        TransitionKind::SlideOut(_) => TransitionKind::SlideOut(d),
        other => other,
    }
}

/// The transition for the i-th animation entry, keeping easing, duration and direction of `prev`.
pub fn transition_of_index(i: usize, prev: &Option<Transition>) -> Option<Transition> {
    let dir = prev.and_then(|t| transition_direction(t.kind)).unwrap_or(Direction::Left);
    let kind = match i {
        0 => return None,
        1 => TransitionKind::Dissolve,
        2 => TransitionKind::SmartAnimate,
        3 => TransitionKind::MoveIn(dir),
        4 => TransitionKind::MoveOut(dir),
        5 => TransitionKind::Push(dir),
        6 => TransitionKind::SlideIn(dir),
        _ => TransitionKind::SlideOut(dir),
    };
    Some(prev.map(|t| Transition { kind, ..t }).unwrap_or(Transition { kind, easing: Easing::EaseOut, duration_ms: 300 }))
}

pub fn direction_name(d: Direction) -> &'static str {
    match d {
        Direction::Left => "Left",
        Direction::Right => "Right",
        Direction::Top => "Top",
        Direction::Bottom => "Bottom",
    }
}

pub fn position_name(p: OverlayPosition) -> &'static str {
    match p {
        OverlayPosition::Center => "Center",
        OverlayPosition::TopLeft => "Top left",
        OverlayPosition::TopCenter => "Top center",
        OverlayPosition::TopRight => "Top right",
        OverlayPosition::BottomLeft => "Bottom left",
        OverlayPosition::BottomCenter => "Bottom center",
        OverlayPosition::BottomRight => "Bottom right",
        OverlayPosition::Manual => "Manual",
    }
}

pub const OVERFLOWS: &[(&str, Overflow)] = &[
    ("No scrolling", Overflow::None),
    ("Horizontal scrolling", Overflow::Horizontal),
    ("Vertical scrolling", Overflow::Vertical),
    ("Horizontal & vertical", Overflow::Both),
];

// ------------------------------------------------------------------ queries

/// Top-level frame (child of its page) that contains `id`, or `id` itself.
pub fn top_frame(doc: &Document, mut id: NodeId) -> Option<NodeId> {
    loop {
        let p = doc.get(id)?.parent?;
        if matches!(doc.get(p)?.kind, NodeKind::Page { .. }) {
            return Some(id);
        }
        id = p;
    }
}

/// Frames a reaction on `src` may target: the top-level frames of its page.
pub fn destination_frames(doc: &Document, src: NodeId) -> Vec<NodeId> {
    let Some(page) = doc.page_of(src) else { return Vec::new() };
    top_level_frames(doc, page)
}

/// (target frame, is overlay) pairs of every connection `id` draws a noodle to.
pub fn connections(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let Some(n) = doc.get(id) else { return Vec::new() };
    let mut out = Vec::new();
    for r in &n.prototype.reactions {
        walk_actions(&r.actions, &mut |a| {
            if let Action::Navigate { dest, .. } | Action::OpenOverlay { dest, .. } | Action::SwapOverlay { dest } = a {
                if let Some(f) = top_frame(doc, *dest) {
                    if doc.page_of(f) == doc.page_of(id) && !out.contains(&f) {
                        out.push(f);
                    }
                }
            }
        });
    }
    out
}

pub fn has_interactions(doc: &Document, id: NodeId) -> bool {
    doc.get(id).is_some_and(|n| !n.prototype.reactions.is_empty())
}

/// Next flow name: "Flow N" with N one above the highest existing "Flow N".
pub fn next_flow_name(flows: &[Flow]) -> String {
    let max = flows.iter().filter_map(|f| f.name.strip_prefix("Flow ").and_then(|n| n.trim().parse::<u32>().ok())).max().unwrap_or(0);
    format!("Flow {}", max.max(flows.len() as u32) + 1)
}

// ------------------------------------------------------------------- edits

impl App {
    /// Edits the prototype settings of `ids` as one undo step.
    pub fn proto_edit(&mut self, ids: &[NodeId], mut f: impl FnMut(&mut PrototypeProps)) {
        self.edit_nodes(ids, |n| f(&mut n.prototype));
    }

    /// Replaces the flow list as one undo step.
    pub fn proto_set_flows(&mut self, flows: Vec<Flow>) {
        if flows == self.doc.flows {
            return;
        }
        if let Err(e) = self.hist.exec(&mut self.doc, Command::SetFlows { flows }) {
            self.show_toast(format!("Edit failed: {e}"), true);
        }
        self.changed();
    }

    pub fn proto_add_flow(&mut self, start: NodeId) {
        let Some(page) = self.doc.page_of(start) else { return };
        let mut flows = self.doc.flows.clone();
        if flows.iter().any(|f| f.start == start) {
            return;
        }
        let name = next_flow_name(&flows);
        flows.push(Flow { name, page, start, description: String::new() });
        self.proto_set_flows(flows);
    }

    /// Default reaction for the + button: On click -> navigate to the next frame.
    pub fn proto_default_reaction(&self, src: NodeId) -> Reaction {
        let frames = destination_frames(&self.doc, src);
        let own = top_frame(&self.doc, src);
        let dest =
            own.and_then(|o| frames.iter().position(|f| *f == o)).map(|i| frames[(i + 1) % frames.len()]).or_else(|| frames.first().copied()).unwrap_or(src);
        Reaction {
            trigger: Trigger::OnClick,
            actions: vec![Action::Navigate { dest, preserve_scroll: false }],
            transition: Some(Transition { kind: TransitionKind::Dissolve, easing: Easing::EaseOut, duration_ms: 300 }),
        }
    }

    /// Adds a reaction to `src` and opens it in the editor.
    pub fn proto_add_reaction(&mut self, src: NodeId) {
        let r = self.proto_default_reaction(src);
        let mut idx = 0;
        self.proto_edit(&[src], |p| {
            p.reactions.push(r.clone());
            idx = p.reactions.len() - 1;
        });
        self.proto.editing = Some((src, idx));
    }

    /// Drag-from-hotspot: On click -> Navigate to `dest_frame` (instant, Figma's default).
    pub fn proto_connect(&mut self, src: NodeId, dest_frame: NodeId) {
        let r = Reaction { trigger: Trigger::OnClick, actions: vec![Action::Navigate { dest: dest_frame, preserve_scroll: false }], transition: None };
        self.proto_edit(&[src], |p| p.reactions.push(r.clone()));
    }

    /// Opens the Present view from the selected frame, else the page's first flow, else the first frame.
    pub fn start_present(&mut self) {
        crate::decks::ui::flush_notes(self);
        let start = if self.is_deck() {
            self.active_slide()
        } else {
            self.selection
                .first()
                .and_then(|s| top_frame(&self.doc, *s))
                .or_else(|| self.doc.flows.iter().find(|f| f.page == self.page).map(|f| f.start))
                .or_else(|| top_level_frames(&self.doc, self.page).first().copied())
        };
        match start {
            Some(s) => self.proto.present = Some(crate::present::PresentState::new(&self.doc, s)),
            None => self.show_toast(if self.is_deck() { "Nothing to present: add a slide first" } else { "Nothing to present: add a frame first" }, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc3() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut f = Vec::new();
        for i in 0..3 {
            let id = d.alloc_id();
            f.push(d.insert(page, Node::frame(id, Rect::new(i as f32 * 500.0, 0.0, 400.0, 800.0))).unwrap());
        }
        let id = d.alloc_id();
        let b = d.insert(f[0], Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        (d, f[0], f[1], f[2], b)
    }

    #[test]
    fn deck_presentation_starts_the_active_or_selected_slide() {
        let (mut doc, a, b, _, nested) = doc3();
        doc.kind = DocumentKind::Deck;
        for id in [a, b] {
            doc.get_mut(id).unwrap().slide = Some(SlideProps::default());
        }
        let mut app = App::new(doc);
        app.selection.clear();
        app.decks.active = Some(b);
        app.start_present();
        assert_eq!(app.proto.present.as_ref().unwrap().player.current, b);
        app.proto.present = None;
        app.selection = vec![nested];
        app.start_present();
        assert_eq!(app.proto.present.as_ref().unwrap().player.current, a);
    }

    #[test]
    fn trigger_round_trips_through_indexes() {
        for i in 0..TRIGGERS.len() {
            let t = trigger_of_index(i, &Trigger::OnClick);
            assert_eq!(trigger_index(&t), i);
        }
        let t = trigger_of_index(4, &Trigger::AfterDelay { delay_ms: 250 });
        assert_eq!(t, Trigger::MouseEnter { delay_ms: 250 }, "delay carries over");
        assert_eq!(trigger_of_index(8, &Trigger::OnClick), Trigger::AfterDelay { delay_ms: 800 });
        assert_eq!(with_delay(&t, 90), Trigger::MouseEnter { delay_ms: 90 });
        assert_eq!(with_delay(&Trigger::OnClick, 90), Trigger::OnClick);
    }

    #[test]
    fn action_round_trips_and_keeps_destination() {
        let prev = Action::Navigate { dest: NodeId(7), preserve_scroll: true };
        let doc = Document::new("t");
        for i in 0..ACTIONS.len() {
            assert_eq!(action_index(&action_of_index(&doc, i, &prev, NodeId(1))), i);
        }
        assert_eq!(action_dest(&action_of_index(&doc, 2, &prev, NodeId(1))), Some(NodeId(7)));
        assert_eq!(action_dest(&action_of_index(&doc, 0, &Action::Back, NodeId(1))), Some(NodeId(1)));
        assert!(!action_animates(&Action::Back) && action_animates(&prev));
    }

    #[test]
    fn animation_picker_keeps_duration_and_direction() {
        let prev = Some(Transition { kind: TransitionKind::Push(Direction::Top), easing: Easing::Linear, duration_ms: 777 });
        let t = transition_of_index(3, &prev).unwrap();
        assert_eq!(t.kind, TransitionKind::MoveIn(Direction::Top));
        assert_eq!((t.duration_ms, t.easing), (777, Easing::Linear));
        assert_eq!(transition_of_index(0, &prev), None);
        let fresh = transition_of_index(1, &None).unwrap();
        assert_eq!((fresh.kind, fresh.duration_ms), (TransitionKind::Dissolve, 300));
        for i in 0..ANIMATIONS.len() {
            assert_eq!(animation_index(&transition_of_index(i, &None)), i);
        }
        assert_eq!(with_direction(TransitionKind::Dissolve, Direction::Left), TransitionKind::Dissolve);
        assert_eq!(transition_direction(TransitionKind::SlideOut(Direction::Right)), Some(Direction::Right));
    }

    #[test]
    fn labels_name_destinations() {
        let (d, a, b, _c, _) = doc3();
        assert_eq!(action_label(&d, &Action::Navigate { dest: b, preserve_scroll: false }), "Navigate to Frame");
        assert_eq!(action_label(&d, &Action::Navigate { dest: NodeId(99), preserve_scroll: false }), "Navigate to (missing)");
        assert_eq!(trigger_label(&Trigger::AfterDelay { delay_ms: 5 }), "After delay 5ms");
        assert_eq!(trigger_label(&Trigger::KeyPress { keys: "K".into() }), "Key K");
        assert_eq!(trigger_label(&Trigger::WhileHovering), "While hovering");
        let _ = a;
    }

    #[test]
    fn connections_lists_destination_frames_once() {
        let (mut d, _a, b, c, btn) = doc3();
        let r = |dest| Reaction { trigger: Trigger::OnClick, actions: vec![Action::Navigate { dest, preserve_scroll: false }], transition: None };
        d.get_mut(btn).unwrap().prototype.reactions =
            vec![r(b), r(b), r(c), Reaction { trigger: Trigger::OnClick, actions: vec![Action::Back], transition: None }];
        assert_eq!(connections(&d, btn), vec![b, c]);
        assert!(has_interactions(&d, btn));
        assert!(connections(&d, b).is_empty());
    }

    fn vars_doc() -> Document {
        let mut d = Document::new("t");
        d.add_page("p");
        let mut s = VariableStore::default();
        let c = s.add_collection("Cart");
        s.add_variable(&c, "Counter/total", VariableKind::Number).unwrap();
        s.add_variable(&c, "name", VariableKind::String).unwrap();
        s.add_variable(&c, "ready", VariableKind::Boolean).unwrap();
        let _ = s.command().apply(&mut d);
        d
    }

    #[test]
    fn expression_text_resolves_names_ids_and_unique_short_names() {
        let d = vars_doc();
        let total = d.variables.values().find(|v| v.name == "Counter/total").unwrap().id.clone();
        assert_eq!(find_variable(&d, "Counter/total"), Some(total.clone()));
        assert_eq!(find_variable(&d, &total), Some(total.clone()), "ids work too");
        assert_eq!(find_variable(&d, "total"), Some(total.clone()), "last segment of a grouped name");
        assert_eq!(find_variable(&d, "NAME").as_deref(), d.variables.values().find(|v| v.name == "name").map(|v| v.id.as_str()));
        assert_eq!(find_variable(&d, "nope"), None);
        let e = parse_expression_text(&d, "{Counter/total} + $total * 2", None).unwrap();
        assert_eq!(expression_text(&d, &e), "{Counter/total} + {Counter/total} * 2");
        assert_eq!(variable_name(&d, &total), "Counter/total");
        assert_eq!(variable_name(&d, "x"), "(missing)");
        assert_eq!(variable_name(&d, ""), "(none)");
    }

    #[test]
    fn expression_text_is_checked_against_the_variable_type() {
        let d = vars_doc();
        let ok = |t: &str, k| parse_expression_text(&d, t, k).map(|_| ());
        assert!(ok("$total + 1", Some(VariableKind::Number)).is_ok());
        assert!(ok("\"x\"", Some(VariableKind::Number)).unwrap_err().contains("Expected a number"));
        assert!(ok("$ready", Some(VariableKind::Number)).is_err());
        assert!(ok("$total + 1", Some(VariableKind::String)).is_ok(), "numbers are written out into text variables");
        assert!(ok("true", Some(VariableKind::Boolean)).is_ok());
        assert!(ok("1 +", None).unwrap_err().contains("end of expression"));
        assert!(ok("$nope", None).unwrap_err().contains("Unknown variable"));
        assert!(parse_condition_text(&d, "$total > 3 && $ready").is_ok());
        assert!(parse_condition_text(&d, "$total + 3").unwrap_err().contains("true or false"));
    }

    #[test]
    fn new_actions_have_labels_and_defaults() {
        let d = vars_doc();
        let total = d.variables.values().find(|v| v.name == "Counter/total").unwrap().id.clone();
        let set = action_of_index(&d, 8, &Action::Back, NodeId(1));
        assert_eq!(action_label(&d, &set), "Set Counter/total = {Counter/total} + 1");
        let mode = action_of_index(&d, 9, &Action::Back, NodeId(1));
        assert!(action_label(&d, &mode).starts_with("Set mode Cart: "));
        let cond = action_of_index(&d, 10, &Action::Back, NodeId(1));
        assert_eq!(action_label(&d, &cond), "If true");
        let cond = Action::Conditional { branches: vec![(Expression::var(total), vec![])], otherwise: vec![Action::Back] };
        assert_eq!(action_label(&d, &cond), "If {Counter/total}, else");
        let media = action_of_index(&d, 11, &Action::Back, NodeId(5));
        assert_eq!(action_label(&d, &media), "Toggle play/pause (missing)");
        // nothing animates for variable and media actions; a conditional animates when a branch does
        assert!(!action_animates(&set) && !action_animates(&mode) && !action_animates(&media));
        let nav = Action::Navigate { dest: NodeId(1), preserve_scroll: false };
        let c2 = Action::Conditional { branches: vec![(Expression::boolean(true), vec![nav])], otherwise: vec![] };
        assert!(action_animates(&c2));
        // an empty document gives harmless placeholders
        let empty = Document::new("e");
        assert!(matches!(action_of_index(&empty, 8, &Action::Back, NodeId(1)), Action::SetVariable { variable, .. } if variable.is_empty()));
        assert!(matches!(action_of_index(&empty, 9, &Action::Back, NodeId(1)), Action::SetVariableMode { collection, .. } if collection.is_empty()));
    }

    #[test]
    fn media_ops_round_trip_through_the_menu_and_keep_seconds() {
        for i in 0..MEDIA_OPS.len() {
            assert_eq!(media_op_index(&media_op_of_index(i, &MediaOp::Play)), i);
        }
        assert_eq!(media_op_of_index(7, &MediaOp::SkipBy(12.0)), MediaOp::SeekTo(12.0));
        assert_eq!(media_op_of_index(6, &MediaOp::SeekTo(3.0)), MediaOp::SkipBy(3.0));
        assert_eq!(media_op_of_index(7, &MediaOp::SkipBy(-4.0)), MediaOp::SeekTo(0.0), "a time is never negative");
    }

    #[test]
    fn noodles_follow_navigation_inside_conditionals() {
        let (mut d, _a, b, c, btn) = doc3();
        let nav = |dest| Action::Navigate { dest, preserve_scroll: false };
        d.get_mut(btn).unwrap().prototype.reactions = vec![Reaction {
            trigger: Trigger::OnClick,
            actions: vec![Action::Conditional { branches: vec![(Expression::boolean(true), vec![nav(b)])], otherwise: vec![nav(c)] }],
            transition: None,
        }];
        assert_eq!(connections(&d, btn), vec![b, c]);
        let mut seen = 0;
        walk_actions(&d.get(btn).unwrap().prototype.reactions[0].actions, &mut |_| seen += 1);
        assert_eq!(seen, 3, "the conditional and both nested navigations");
    }

    #[test]
    fn gamepad_trigger_indexes_and_labels() {
        let t = trigger_of_index(10, &Trigger::OnClick);
        assert_eq!(t, Trigger::GamepadPress { device: GamepadDevice::Generic, buttons: "A".into() });
        assert_eq!(trigger_index(&t), 10);
        assert_eq!(trigger_label(&t), "Gamepad A");
        let keep = Trigger::GamepadPress { device: GamepadDevice::Xbox, buttons: "LB".into() };
        assert_eq!(trigger_of_index(10, &keep), keep);
        assert_eq!(gamepad_device_index(GamepadDevice::SwitchPro), 3);
    }

    #[test]
    fn flow_names_count_up() {
        let mk = |n: &str| Flow { name: n.into(), page: NodeId(1), start: NodeId(2), description: String::new() };
        assert_eq!(next_flow_name(&[]), "Flow 1");
        assert_eq!(next_flow_name(&[mk("Flow 1"), mk("Flow 4")]), "Flow 5");
        assert_eq!(next_flow_name(&[mk("Onboarding")]), "Flow 2");
    }

    #[test]
    fn default_reaction_targets_the_next_frame() {
        let (d, _a, b, _c, btn) = doc3();
        let app = App::new(d);
        let r = app.proto_default_reaction(btn);
        assert_eq!(r.trigger, Trigger::OnClick);
        assert_eq!(r.actions, vec![Action::Navigate { dest: b, preserve_scroll: false }]);
    }

    #[test]
    fn edits_are_single_undo_steps() {
        let (d, _a, b, c, btn) = doc3();
        let mut app = App::new(d);
        app.proto_add_reaction(btn);
        assert_eq!(app.doc.get(btn).unwrap().prototype.reactions.len(), 1);
        assert_eq!(app.proto.editing, Some((btn, 0)));
        app.proto_edit(&[btn], |p| p.reactions[0].actions = vec![Action::Navigate { dest: c, preserve_scroll: false }]);
        app.proto_connect(btn, b);
        assert_eq!(app.doc.get(btn).unwrap().prototype.reactions.len(), 2);
        app.undo();
        assert_eq!(app.doc.get(btn).unwrap().prototype.reactions.len(), 1);
        app.undo();
        assert!(matches!(app.doc.get(btn).unwrap().prototype.reactions[0].actions[0], Action::Navigate { dest, .. } if dest == b));
        app.undo();
        assert!(app.doc.get(btn).unwrap().prototype.reactions.is_empty());
        app.redo();
        assert_eq!(app.doc.get(btn).unwrap().prototype.reactions.len(), 1);
    }

    #[test]
    fn flows_are_undoable() {
        let (d, a, b, _c, _) = doc3();
        let mut app = App::new(d);
        app.proto_add_flow(a);
        app.proto_add_flow(a);
        assert_eq!(app.doc.flows.len(), 1, "a frame starts at most one flow");
        app.proto_add_flow(b);
        assert_eq!(app.doc.flows.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Flow 1", "Flow 2"]);
        app.undo();
        assert_eq!(app.doc.flows.len(), 1);
        app.undo();
        assert!(app.doc.flows.is_empty());
        app.redo();
        assert_eq!(app.doc.flows[0].start, a);
    }

    #[test]
    fn frame_and_device_presets_include_iphone_17_and_figma_catalogue() {
        let expected_iphone_17 =
            [("iPhone 17", 402.0, 874.0), ("iPhone 17 Air", 420.0, 912.0), ("iPhone 17 Pro", 402.0, 874.0), ("iPhone 17 Pro Max", 440.0, 956.0)];
        for (name, w, h) in expected_iphone_17 {
            let preset = find_frame_preset(name).unwrap_or_else(|| panic!("missing frame preset {name}"));
            assert_eq!((preset.width, preset.height), (w, h), "size mismatch for {name}");
            assert_eq!(preset.category, PresetCategory::Phone);
            let dev_idx = device_index(&PrototypeDevice { preset: Some(name.into()), size: None, background: Some(Color::BLACK) })
                .unwrap_or_else(|| panic!("missing device preset {name}"));
            let dev = device_for(dev_idx, &PrototypeDevice::default());
            assert_eq!(dev.size, Some(Vec2::new(w, h)));
        }

        let catalogue_samples = [
            ("iPhone 16", 393.0, 852.0, PresetCategory::Phone),
            ("iPhone 16 Plus", 430.0, 932.0, PresetCategory::Phone),
            ("iPhone 16 Pro", 402.0, 874.0, PresetCategory::Phone),
            ("iPhone 16 Pro Max", 440.0, 956.0, PresetCategory::Phone),
            ("iPhone 16e", 390.0, 844.0, PresetCategory::Phone),
            ("iPhone 14 & 15 Pro Max", 430.0, 932.0, PresetCategory::Phone),
            ("iPhone 14 & 15 Pro", 393.0, 852.0, PresetCategory::Phone),
            ("iPhone 13 & 14", 390.0, 844.0, PresetCategory::Phone),
            ("iPhone SE", 320.0, 568.0, PresetCategory::Phone),
            ("Android Compact", 412.0, 917.0, PresetCategory::Phone),
            ("Android Medium", 700.0, 840.0, PresetCategory::Phone),
            ("Android Large", 360.0, 800.0, PresetCategory::Phone),
            ("iPad mini 8.3", 744.0, 1133.0, PresetCategory::Tablet),
            ("iPad Pro 11\"", 834.0, 1194.0, PresetCategory::Tablet),
            ("iPad Pro 12.9\"", 1024.0, 1366.0, PresetCategory::Tablet),
            ("Surface Pro 8", 1440.0, 960.0, PresetCategory::Tablet),
            ("MacBook Air", 1280.0, 832.0, PresetCategory::Desktop),
            ("MacBook Pro 14\"", 1512.0, 982.0, PresetCategory::Desktop),
            ("MacBook Pro 16\"", 1728.0, 1117.0, PresetCategory::Desktop),
            ("Desktop", 1440.0, 1024.0, PresetCategory::Desktop),
            ("Wireframes", 1440.0, 1024.0, PresetCategory::Desktop),
            ("TV", 1280.0, 720.0, PresetCategory::Desktop),
            ("Slide 16:9", 1920.0, 1080.0, PresetCategory::Presentation),
            ("Slide 4:3", 1024.0, 768.0, PresetCategory::Presentation),
            ("Apple Watch Series 10 42mm", 187.0, 223.0, PresetCategory::Watch),
            ("Apple Watch Series 10 46mm", 208.0, 248.0, PresetCategory::Watch),
            ("Apple Watch 41mm", 176.0, 215.0, PresetCategory::Watch),
            ("Apple Watch 45mm", 198.0, 242.0, PresetCategory::Watch),
            ("A4", 595.0, 842.0, PresetCategory::Paper),
            ("A5", 420.0, 595.0, PresetCategory::Paper),
            ("A6", 297.0, 420.0, PresetCategory::Paper),
            ("Letter", 612.0, 792.0, PresetCategory::Paper),
            ("Tabloid", 792.0, 1224.0, PresetCategory::Paper),
            ("Twitter post", 1200.0, 675.0, PresetCategory::SocialMedia),
            ("Twitter header", 1500.0, 500.0, PresetCategory::SocialMedia),
            ("Facebook post", 1200.0, 630.0, PresetCategory::SocialMedia),
            ("Facebook cover", 820.0, 312.0, PresetCategory::SocialMedia),
            ("Instagram post", 1080.0, 1080.0, PresetCategory::SocialMedia),
            ("Instagram story", 1080.0, 1920.0, PresetCategory::SocialMedia),
            ("Dribbble shot", 400.0, 300.0, PresetCategory::SocialMedia),
            ("Dribbble shot HD", 800.0, 600.0, PresetCategory::SocialMedia),
            ("LinkedIn cover", 1584.0, 396.0, PresetCategory::SocialMedia),
        ];
        for (name, w, h, cat) in catalogue_samples {
            let p = find_frame_preset(name).unwrap_or_else(|| panic!("missing preset {name}"));
            assert_eq!((p.width, p.height, p.category), (w, h, cat), "mismatch for {name}");
            assert!(frame_presets_in_category(cat).iter().any(|x| x.name == name));
        }

        let mut d = Document::new("t");
        d.add_page("p");
        let mut app = App::new(d);
        let fid = app.create_preset_frame("iPhone 17 Pro Max").expect("created frame");
        let node = app.doc.get(fid).unwrap();
        assert_eq!(node.name, "iPhone 17 Pro Max");
        assert_eq!((node.size.x, node.size.y), (440.0, 956.0));
        assert_eq!(node.corner_radii, [55.0; 4]);
        let sharp = app.create_preset_frame("Wireframes").expect("created sharp frame");
        assert_eq!(app.doc.get(sharp).unwrap().corner_radii, [0.0; 4]);
    }

    #[test]
    fn device_dropdown_keeps_the_original_entries_first() {
        let head: Vec<&str> = DEVICES.iter().take(3).map(|d| d.0).collect();
        assert_eq!(head, ["None", "iPhone 15", "iPhone 15 Pro Max"], "stored index-based pickers keep pointing at the same device");
    }

    fn preset_app() -> App {
        let mut d = Document::new("t");
        d.add_page("p");
        let mut app = App::new(d);
        app.canvas_rect = egui::Rect::from_min_size(egui::Pos2::new(300.0, 0.0), egui::vec2(1000.0, 800.0));
        app
    }

    #[test]
    fn preset_frame_opens_centered_in_the_visible_viewport() {
        let mut app = preset_app();
        app.view = crate::app::View { offset: Vec2::new(4000.0, 4000.0), zoom: 0.5 };
        let id = app.create_preset_frame("iPhone 17").expect("created frame");
        let b = crate::util::world_aabb(&app.doc, id);
        assert_eq!((b.w, b.h), (402.0, 874.0));
        let view = app.view.to_page(app.canvas_rect.min, app.canvas_rect.center());
        assert!(
            (b.x + b.w / 2.0 - view.x).abs() <= 1.0 && (b.y + b.h / 2.0 - view.y).abs() <= 1.0,
            "frame center ({}, {}) must be the viewport center ({}, {})",
            b.x + b.w / 2.0,
            b.y + b.h / 2.0,
            view.x,
            view.y
        );
        assert_eq!((b.x.fract(), b.y.fract()), (0.0, 0.0), "placed on whole pixels");
        assert_eq!(app.selection, vec![id]);
        let count = app.doc.nodes.len();
        app.undo();
        assert_eq!(app.doc.nodes.len(), count - 1, "one undo removes the frame");
    }

    #[test]
    fn preset_frame_stays_in_view_when_existing_frames_cover_the_viewport_center() {
        let mut app = preset_app();
        let (page, existing) = (app.page, app.doc.alloc_id());
        app.doc.insert(page, Node::frame(existing, Rect::new(-1000.0, -200.0, 3000.0, 1200.0))).unwrap();
        let zoom = app.view.zoom;
        let id = app.create_preset_frame("iPhone 17").expect("created frame");
        let (b, covered) = (crate::util::world_aabb(&app.doc, id), crate::util::world_aabb(&app.doc, existing));
        assert!(!b.intersects(&covered), "the new frame {b:?} must not sit on top of the existing one {covered:?}");
        let (top_left, bottom_right) = (app.view.to_page(app.canvas_rect.min, app.canvas_rect.min), app.view.to_page(app.canvas_rect.min, app.canvas_rect.max));
        let centre = Vec2::new(b.x + b.w / 2.0, b.y + b.h / 2.0);
        assert!(
            (top_left.x..=bottom_right.x).contains(&centre.x) && (top_left.y..=bottom_right.y).contains(&centre.y),
            "the new frame's centre {centre:?} must be inside the visible page area {top_left:?}..{bottom_right:?}"
        );
        assert_eq!(app.view.zoom, zoom, "revealing the frame pans, it does not zoom");
    }

    #[test]
    fn successive_preset_frames_sit_side_by_side_on_the_grid() {
        let mut app = preset_app();
        let mut placed: Vec<Rect> = Vec::new();
        for name in ["iPhone 17", "Desktop", "A4", "iPhone 17"] {
            let id = app.create_preset_frame(name).expect("created frame");
            let b = crate::util::world_aabb(&app.doc, id);
            for earlier in &placed {
                assert!(!b.intersects(earlier), "{name} at {b:?} overlaps an earlier preset frame {earlier:?}");
            }
            placed.push(b);
        }
        let gap = placed[1].x - (placed[0].x + placed[0].w);
        assert!(gap > 0.0 && gap % 4.0 == 0.0, "the second frame starts {gap} px right of the first: a positive step on the 4 px grid");
    }
}
