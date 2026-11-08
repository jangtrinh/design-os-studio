//! Generates native export fixtures (SwiftUI, UIKit, Jetpack Compose, Android XML) for the
//! CI compile-check harness (`scripts/ci/native-export-compile-check.sh`, Issue #1457).
//!
//! When `NATIVE_COMPILE_DUMP_DIR` is set in the environment, `dump_native_compile_fixtures`
//! writes all generated source and resource files into that directory.

use std::path::{Path, PathBuf};

use designos_doc::{
    create_instance, instances_of, AutoLayout, CodeSyntaxPlatform, Color, ComponentPropDef, Document, Effect, FrameProps, GridTrack, LayoutAlign,
    LayoutDirection, LineHeight, Node, NodeId, NodeKind, OverrideField, Paint, PropKind, PropValue, Rect, Sizing, TextAlignH, TextAutoResize, TextStyle,
    Variable, VariableCollection, VariableKind, VariableMode, VariableValue, Vec2,
};
use designos_export::{android_xml_for, compose_for, swiftui_for, uikit_for};

struct SceneFixture {
    name: &'static str,
    doc: Document,
    root: NodeId,
    instance: Option<NodeId>,
}

fn new_stack(al: AutoLayout, size: Vec2) -> (Document, NodeId) {
    let mut doc = Document::new("native-compile-stack");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut n = Node::frame(id, Rect::new(0.0, 0.0, size.x, size.y));
    n.name = "StackFrame".into();
    n.fills = vec![Paint::solid(Color::rgb(0xF5, 0xF5, 0xFA))];
    n.corner_radii = [8.0; 4];
    n.kind = NodeKind::Frame(FrameProps { clips_content: true, auto_layout: Some(al), ..Default::default() });
    let id = doc.insert(page, n).unwrap();
    (doc, id)
}

fn new_grid(al: AutoLayout, size: Vec2) -> (Document, NodeId) {
    let mut doc = Document::new("native-compile-grid");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, size.x, size.y));
    frame.name = "GridFrame".into();
    frame.fills.clear();
    frame.kind =
        NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(AutoLayout { direction: LayoutDirection::Grid, ..al }), ..Default::default() });
    let grid = doc.insert(page, frame).unwrap();
    (doc, grid)
}

#[allow(clippy::too_many_arguments)]
fn add_box(doc: &mut Document, parent: NodeId, name: &str, w: f32, h: f32, sh: Sizing, sv: Sizing, rgb: (u8, u8, u8)) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
    n.name = name.into();
    n.fills = vec![Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2))];
    n.layout_child.horizontal = sh;
    n.layout_child.vertical = sv;
    doc.insert(parent, n).unwrap()
}

fn add_item(doc: &mut Document, grid: NodeId, w: f32, h: f32, horiz: Sizing, vert: Sizing) -> NodeId {
    let idx = doc.get(grid).map(|n| n.children.len() + 1).unwrap_or(1);
    add_box(doc, grid, &format!("Item {idx}"), w, h, horiz, vert, (0x35, 0x82, 0xFF))
}

fn scene_01_golden_card() -> SceneFixture {
    let mut d = Document::new("golden-card");
    let page = d.add_page("Page 1");
    d.variable_collections.insert(
        "c1".into(),
        VariableCollection {
            id: "c1".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["primary".into(), "muted".into(), "md".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let var = |id: &str, name: &str, kind, values: Vec<(&str, VariableValue)>| Variable {
        id: id.into(),
        name: name.into(),
        collection: "c1".into(),
        kind,
        values: values.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    };
    d.variables.insert(
        "primary".into(),
        var(
            "primary",
            "Color/Primary",
            VariableKind::Color,
            vec![("light", VariableValue::Color(Color::rgb(0x0D, 0x99, 0xFF))), ("dark", VariableValue::Color(Color::rgb(0x4D, 0xB8, 0xFF)))],
        ),
    );
    d.variables.insert(
        "muted".into(),
        var(
            "muted",
            "Color/Text/Muted",
            VariableKind::Color,
            vec![("light", VariableValue::Color(Color::rgb(0x66, 0x66, 0x66))), ("dark", VariableValue::Color(Color::rgb(0xAA, 0xAA, 0xAA)))],
        ),
    );
    d.variables.insert(
        "md".into(),
        var("md", "Spacing/Md", VariableKind::Number, vec![("light", VariableValue::Number(12.0)), ("dark", VariableValue::Number(12.0))]),
    );

    let comp_id = d.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "ActionButton".into();
    comp.size = Vec2::new(80.0, 32.0);
    comp.component.props = vec![
        ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("ActionButton".into()) },
        ComponentPropDef { name: "Disabled".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) },
    ];
    let lib_page = d.add_page("Components");
    let comp_id = d.insert(lib_page, comp).unwrap();

    let card_id = d.alloc_id();
    let mut card = Node::frame(card_id, Rect::new(0.0, 0.0, 312.0, 200.0));
    card.name = "Card".into();
    card.corner_radii = [8.0; 4];
    card.stroke.paints = vec![Paint::solid(Color::rgb(0xE5, 0xE5, 0xE5))];
    card.stroke.weight = 1.0;
    card.effects = vec![Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 64),
        offset: Vec2::new(0.0, 4.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }];
    if let NodeKind::Frame(f) = &mut card.kind {
        f.clips_content = true;
        f.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 8.0,
            padding: [16.0; 4],
            counter_sizing: Sizing::Fixed,
            primary_sizing: Sizing::Fixed,
            ..Default::default()
        });
    }
    let card_id = d.insert(page, card).unwrap();

    let title_id = d.alloc_id();
    let mut title = Node::text(
        title_id,
        Rect::new(0.0, 0.0, 120.0, 24.0),
        "Hello world",
        TextStyle {
            font_family: "Inter".into(),
            font_weight: 600,
            font_size: 20.0,
            fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))],
            ..Default::default()
        },
    );
    title.name = "Title".into();
    d.insert(card_id, title).unwrap();

    let body_id = d.alloc_id();
    let mut body = Node::text(
        body_id,
        Rect::new(0.0, 0.0, 280.0, 20.0),
        "Body copy",
        TextStyle {
            font_family: "Inter".into(),
            font_size: 14.0,
            line_height: LineHeight::Pixels(20.0),
            align_h: TextAlignH::Center,
            fills: vec![Paint::solid(Color::rgb(0x66, 0x66, 0x66))],
            ..Default::default()
        },
    );
    body.name = "Body".into();
    body.layout_child.horizontal = Sizing::Fill;
    body.variables.bindings.insert("fills/0".into(), "muted".into());
    if let NodeKind::Text(t) = &mut body.kind {
        t.auto_resize = TextAutoResize::Height;
    }
    d.insert(card_id, body).unwrap();

    let row_id = d.alloc_id();
    let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 280.0, 48.0));
    row.name = "Actions".into();
    row.corner_radii = [4.0; 4];
    row.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
    row.variables.bindings.insert("fills/0".into(), "primary".into());
    row.variables.bindings.insert("itemSpacing".into(), "md".into());
    row.layout_child.horizontal = Sizing::Fill;
    if let NodeKind::Frame(f) = &mut row.kind {
        f.clips_content = false;
        f.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Horizontal,
            item_spacing: 12.0,
            padding: [8.0, 12.0, 8.0, 12.0],
            primary_align: LayoutAlign::Center,
            counter_align: LayoutAlign::Center,
            ..Default::default()
        });
    }
    let row_id = d.insert(card_id, row).unwrap();

    let btn_id = d.alloc_id();
    let mut btn = Node::new(btn_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None });
    btn.name = "ActionButton".into();
    btn.size = Vec2::new(80.0, 32.0);
    btn.component.values.insert("Label".into(), PropValue::Text("Save".into()));
    btn.component.values.insert("Disabled".into(), PropValue::Bool(true));
    d.insert(row_id, btn).unwrap();

    let dot_id = d.alloc_id();
    let mut dot = Node::ellipse(dot_id, Rect::new(0.0, 0.0, 24.0, 24.0));
    dot.name = "StatusDot".into();
    dot.fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
    d.insert(row_id, dot).unwrap();

    d.relayout_subtree(card_id);
    SceneFixture { name: "01_golden_card", doc: d, root: card_id, instance: None }
}

fn scene_02_grid_equal_tracks() -> SceneFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 12.0,
        counter_spacing: 12.0,
        padding: [12.0, 12.0, 12.0, 12.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, g) = new_grid(al, Vec2::new(324.0, 180.0));
    for _ in 0..6 {
        add_item(&mut doc, g, 80.0, 36.0, Sizing::Fill, Sizing::Fixed);
    }
    doc.relayout_subtree(g);
    SceneFixture { name: "02_grid_equal_tracks", doc, root: g, instance: None }
}

fn scene_03_grid_unequal_tracks() -> SceneFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, g) = new_grid(al, Vec2::new(500.0, 140.0));
    add_item(&mut doc, g, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, g, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, g, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(g);
    SceneFixture { name: "03_grid_unequal_tracks", doc, root: g, instance: None }
}

fn scene_04_grid_fixed_first_column() -> SceneFixture {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(200.0);
    let (mut doc, g) = new_grid(al, Vec2::new(440.0, 180.0));
    for _ in 0..6 {
        add_item(&mut doc, g, 50.0, 40.0, Sizing::Fill, Sizing::Fixed);
    }
    doc.relayout_subtree(g);
    SceneFixture { name: "04_grid_fixed_first_column", doc, root: g, instance: None }
}

fn scene_05_grid_hugging_grid() -> SceneFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 8.0,
        counter_spacing: 8.0,
        padding: [8.0, 8.0, 8.0, 8.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, g) = new_grid(al, Vec2::new(500.0, 500.0));
    add_item(&mut doc, g, 100.0, 28.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, g, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, g, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(g);
    SceneFixture { name: "05_grid_hugging_grid", doc, root: g, instance: None }
}

fn scene_06_grid_fixed_height_auto_rows() -> SceneFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 12.0,
        counter_spacing: 16.0,
        padding: [16.0, 16.0, 16.0, 16.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, g) = new_grid(al, Vec2::new(260.0, 240.0));
    add_item(&mut doc, g, 50.0, 24.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 24.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 36.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 36.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 28.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, g, 50.0, 28.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(g);
    SceneFixture { name: "06_grid_fixed_height_auto_rows", doc, root: g, instance: None }
}

fn scene_07_grid_row_and_column_spans() -> SceneFixture {
    let al = AutoLayout {
        grid_columns: 4,
        grid_rows: 0,
        item_spacing: 12.0,
        counter_spacing: 12.0,
        padding: [12.0, 12.0, 12.0, 12.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, g) = new_grid(al, Vec2::new(428.0, 220.0));
    let hero = add_box(&mut doc, g, "HeroTile", 196.0, 40.0, Sizing::Fixed, Sizing::Fill, (0x35, 0x82, 0xFF));
    doc.get_mut(hero).unwrap().layout_child.col_span = 2;
    doc.get_mut(hero).unwrap().layout_child.row_span = 2;
    let top_wide = add_box(&mut doc, g, "TopWide", 196.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
    doc.get_mut(top_wide).unwrap().layout_child.col_span = 2;
    add_box(&mut doc, g, "MidA", 92.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
    add_box(&mut doc, g, "MidB", 92.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
    let footer = add_box(&mut doc, g, "FooterBar", 404.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
    doc.get_mut(footer).unwrap().layout_child.grid_column = Some(0);
    doc.get_mut(footer).unwrap().layout_child.grid_row = Some(2);
    doc.get_mut(footer).unwrap().layout_child.col_span = 4;
    doc.get_mut(footer).unwrap().layout_child.row_span = 1;
    doc.relayout_subtree(g);
    SceneFixture { name: "07_grid_row_and_column_spans", doc, root: g, instance: None }
}

fn scene_08_grid_with_absolute_child() -> SceneFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [10.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, g) = new_grid(al, Vec2::new(210.0, 80.0));
    add_box(&mut doc, g, "CellA", 80.0, 36.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x66, 0xCC));
    add_box(&mut doc, g, "CellB", 80.0, 36.0, Sizing::Fill, Sizing::Fixed, (0x4D, 0x80, 0xE6));
    let badge = add_box(&mut doc, g, "Badge", 24.0, 18.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    {
        let b = doc.get_mut(badge).unwrap();
        b.layout_child.absolute = true;
        b.transform.tx = 15.0;
        b.transform.ty = 25.0;
    }
    doc.relayout_subtree(g);
    SceneFixture { name: "08_grid_with_absolute_child", doc, root: g, instance: None }
}

fn scene_09_stack_fill_width_fixed_height() -> SceneFixture {
    let v_al = AutoLayout {
        direction: LayoutDirection::Vertical,
        item_spacing: 8.0,
        padding: [12.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, vstack) = new_stack(v_al, Vec2::new(320.0, 200.0));
    add_box(&mut doc, vstack, "FillWidthFixedHeightLeaf", 100.0, 44.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x66, 0xCC));
    let sub_al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_align: LayoutAlign::Center,
        counter_align: LayoutAlign::Center,
        item_spacing: 6.0,
        primary_sizing: Sizing::Fill,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let sub_id = doc.alloc_id();
    let mut sub = Node::frame(sub_id, Rect::new(0.0, 0.0, 100.0, 64.0));
    sub.name = "SubRow".into();
    sub.fills.clear();
    sub.layout_child.horizontal = Sizing::Fill;
    sub.layout_child.vertical = Sizing::Fixed;
    sub.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(sub_al), ..Default::default() });
    let sub_id = doc.insert(vstack, sub).unwrap();
    add_box(&mut doc, sub_id, "FixedWidthFillHeightLeaf", 120.0, 20.0, Sizing::Fixed, Sizing::Fill, (0x1A, 0xB3, 0x80));
    doc.relayout_subtree(vstack);
    SceneFixture { name: "09_stack_fill_width_fixed_height", doc, root: vstack, instance: None }
}

fn scene_10_column_with_absolute_child() -> SceneFixture {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::SpaceBetween,
        counter_align: LayoutAlign::Start,
        item_spacing: 8.0,
        padding: [16.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(300.0, 200.0));
    add_box(&mut doc, frame, "Header", 180.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x26, 0x59, 0xBF));
    add_box(&mut doc, frame, "Footer", 200.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x40, 0x73, 0xCC));
    let badge = add_box(&mut doc, frame, "CornerBadge", 28.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x39, 0x46));
    {
        let n = doc.get_mut(badge).unwrap();
        n.layout_child.absolute = true;
        n.transform.tx = 260.0;
        n.transform.ty = 12.0;
    }
    doc.relayout_subtree(frame);
    SceneFixture { name: "10_column_with_absolute_child", doc, root: frame, instance: None }
}

fn scene_11_column_with_underlay_and_hidden_child() -> SceneFixture {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::Start,
        counter_align: LayoutAlign::Start,
        item_spacing: 10.0,
        padding: [12.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(280.0, 180.0));
    doc.variable_collections.insert(
        "c_flags".into(),
        VariableCollection {
            id: "c_flags".into(),
            name: "Flags".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Default".into() }],
            default_mode: "m".into(),
            variables: vec!["hide_flag".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "hide_flag".into(),
        Variable {
            id: "hide_flag".into(),
            name: "Flags/Show".into(),
            collection: "c_flags".into(),
            kind: VariableKind::Boolean,
            values: [("m".to_string(), VariableValue::Boolean(false))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: [(CodeSyntaxPlatform::Android, "Tokens.show".to_string())].into_iter().collect(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    let bg = add_box(&mut doc, frame, "Underlay", 280.0, 180.0, Sizing::Fixed, Sizing::Fixed, (0xE0, 0xE7, 0xFF));
    {
        let n = doc.get_mut(bg).unwrap();
        n.layout_child.absolute = true;
        n.transform.tx = 0.0;
        n.transform.ty = 0.0;
    }
    add_box(&mut doc, frame, "RowOne", 200.0, 28.0, Sizing::Fixed, Sizing::Fixed, (0x1D, 0x4E, 0xD8));
    let hidden = add_box(&mut doc, frame, "HiddenBanner", 200.0, 28.0, Sizing::Fixed, Sizing::Fixed, (0x93, 0x33, 0xEA));
    doc.get_mut(hidden).unwrap().variables.bindings.insert("visible".into(), "hide_flag".into());
    add_box(&mut doc, frame, "RowTwo", 200.0, 28.0, Sizing::Fixed, Sizing::Fixed, (0x25, 0x63, 0xEB));
    doc.relayout_subtree(frame);
    SceneFixture { name: "11_column_with_underlay_and_hidden_child", doc, root: frame, instance: None }
}

fn scene_12_slot_dialog_component() -> SceneFixture {
    let mut d = Document::new("slot-dialog");
    let page = d.add_page("Page 1");
    let comp_id = d.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "Dialog".into();
    comp.size = Vec2::new(240.0, 160.0);
    if let NodeKind::Component(f) = &mut comp.kind {
        f.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 8.0,
            padding: [12.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        });
    }
    comp.component.props.push(ComponentPropDef { name: "Header#1:0".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    comp.component.props.push(ComponentPropDef { name: "Body#1:1".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    let comp_id = d.insert(page, comp).unwrap();

    for (i, (slot_name, default_text)) in [("Header", Some("Default Title")), ("Body", None)].into_iter().enumerate() {
        let sid = d.alloc_id();
        let mut slot = Node::frame(sid, Rect::new(0.0, 0.0, 216.0, 48.0));
        slot.name = slot_name.into();
        slot.component.is_slot = true;
        slot.component.bindings.insert(OverrideField::SlotContent, format!("{slot_name}#1:{i}"));
        if let NodeKind::Frame(f) = &mut slot.kind {
            f.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                item_spacing: 4.0,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            });
        }
        let sid = d.insert(comp_id, slot).unwrap();
        if let Some(txt) = default_text {
            let tid = d.alloc_id();
            let style =
                TextStyle { font_family: "Inter".into(), font_size: 14.0, fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))], ..Default::default() };
            let mut tnode = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 20.0), txt, style);
            tnode.name = txt.into();
            d.insert(sid, tnode).unwrap();
        }
    }

    create_instance(&mut d, comp_id, page, Vec2::new(300.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp_id)[0];
    for (idx, prop, content) in [(0usize, "Header#1:0", "Custom Heading"), (1usize, "Body#1:1", "Custom Body Copy")] {
        let copy_slot = d.get(inst).unwrap().children[idx];
        for c in d.get(copy_slot).unwrap().children.clone() {
            d.remove(c).unwrap();
        }
        let tid = d.alloc_id();
        let style = TextStyle { font_family: "Inter".into(), font_size: 14.0, fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))], ..Default::default() };
        let mut tnode = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 20.0), content, style);
        tnode.name = content.into();
        tnode.component.is_slot_content = true;
        let tid = d.insert(copy_slot, tnode).unwrap();
        d.get_mut(inst).unwrap().component.values.insert(prop.into(), PropValue::Slot(vec![tid]));
    }
    d.relayout_subtree(comp_id);
    d.relayout_subtree(inst);
    SceneFixture { name: "12_slot_dialog_component", doc: d, root: comp_id, instance: Some(inst) }
}

fn build_all_fixtures() -> Vec<SceneFixture> {
    vec![
        scene_01_golden_card(),
        scene_02_grid_equal_tracks(),
        scene_03_grid_unequal_tracks(),
        scene_04_grid_fixed_first_column(),
        scene_05_grid_hugging_grid(),
        scene_06_grid_fixed_height_auto_rows(),
        scene_07_grid_row_and_column_spans(),
        scene_08_grid_with_absolute_child(),
        scene_09_stack_fill_width_fixed_height(),
        scene_10_column_with_absolute_child(),
        scene_11_column_with_underlay_and_hidden_child(),
        scene_12_slot_dialog_component(),
    ]
}

fn indent_lines(src: &str, prefix: &str) -> String {
    src.lines().map(|l| if l.is_empty() { String::new() } else { format!("{prefix}{l}") }).collect::<Vec<_>>().join("\n")
}

fn wrap_swiftui(scene: &SceneFixture) -> String {
    let preamble = r#"import SwiftUI

enum Tokens {
    static let spacingMd: CGFloat = 12
    static let spacingRowGap: CGFloat = 20
    static let show: Bool = false
}

struct ActionButton: View {
    var label: String = "ActionButton"
    var disabled: Bool = false
    var body: some View { EmptyView() }
}
"#;
    let root_code = swiftui_for(&scene.doc, scene.root);
    if let Some(inst) = scene.instance {
        let inst_code = indent_lines(&swiftui_for(&scene.doc, inst), "        ");
        format!("{preamble}\n{root_code}\n\nstruct FixtureScreen: View {{\n    var body: some View {{\n{inst_code}\n    }}\n}}\n")
    } else {
        let body = indent_lines(&root_code, "        ");
        format!("{preamble}\nstruct FixtureView: View {{\n    var body: some View {{\n{body}\n    }}\n}}\n")
    }
}

fn wrap_uikit(scene: &SceneFixture) -> String {
    let preamble = r#"import UIKit

enum Tokens {
    static let spacingMd: CGFloat = 12
    static let spacingRowGap: CGFloat = 20
    static let show: Bool = false
}

final class ActionButton: UIView {
    var label: String = "ActionButton"
    var disabled: Bool = false
}

final class Dialog: UIView {}
"#;
    let body = indent_lines(&uikit_for(&scene.doc, scene.root), "    ");
    format!("{preamble}\nfunc buildFixtureView() {{\n{body}\n}}\n")
}

fn wrap_compose(scene: &SceneFixture, idx: usize) -> String {
    let preamble = format!("package designos.ci.compilecheck.f{idx:02}\n\nimport androidx.compose.runtime.Composable\nimport androidx.compose.stubs.*\n");
    let root_code = compose_for(&scene.doc, scene.root);
    if let Some(inst) = scene.instance {
        let inst_code = indent_lines(&compose_for(&scene.doc, inst), "    ");
        format!("{preamble}\n{root_code}\n\n@Composable\nfun FixtureSlotScreen() {{\n{inst_code}\n}}\n")
    } else {
        let body = indent_lines(&root_code, "    ");
        format!("{preamble}\n@Composable\nfun FixtureView() {{\n{body}\n}}\n")
    }
}

fn split_android_xml_files(raw: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("<!-- ").and_then(|s| s.strip_suffix(" -->")) {
            if rest.starts_with("res/") && rest.ends_with(".xml") {
                if let Some(prev) = current_path.take() {
                    let body = current_lines.join("\n").trim().to_string();
                    files.push((prev, format!("{body}\n")));
                    current_lines.clear();
                }
                current_path = Some(rest.to_string());
                continue;
            }
        }
        current_lines.push(line);
    }
    if let Some(prev) = current_path.take() {
        let body = current_lines.join("\n").trim().to_string();
        files.push((prev, format!("{body}\n")));
    }
    files
}

fn dump_fixtures_to_dir(out_dir: &Path) {
    let swiftui_dir = out_dir.join("swiftui");
    let uikit_dir = out_dir.join("uikit");
    let compose_dir = out_dir.join("compose");
    let android_dir = out_dir.join("android-xml");
    for d in [&swiftui_dir, &uikit_dir, &compose_dir, &android_dir] {
        let _ = std::fs::remove_dir_all(d);
        std::fs::create_dir_all(d).expect("create target fixture dir");
    }

    let scenes = build_all_fixtures();
    assert_eq!(scenes.len(), 12);

    for (idx, scene) in scenes.iter().enumerate() {
        let name = scene.name;

        let swiftui_src = wrap_swiftui(scene);
        std::fs::write(swiftui_dir.join(format!("{name}.swift")), &swiftui_src).expect("write swiftui fixture");

        let uikit_src = wrap_uikit(scene);
        std::fs::write(uikit_dir.join(format!("{name}.swift")), &uikit_src).expect("write uikit fixture");

        let compose_src = wrap_compose(scene, idx + 1);
        std::fs::write(compose_dir.join(format!("{name}.kt")), &compose_src).expect("write compose fixture");

        let xml_raw = android_xml_for(&scene.doc, scene.root);
        let xml_files = split_android_xml_files(&xml_raw);
        assert!(!xml_files.is_empty(), "android_xml_for produced no res/*.xml sections for {name}");
        for (rel_path, xml_body) in xml_files {
            let full_path = android_dir.join(name).join(rel_path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).expect("create android resource subdir");
            }
            std::fs::write(&full_path, &xml_body).expect("write android xml resource file");
        }
    }
}

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn dump_native_compile_fixtures() {
    if let Some(dir) = std::env::var_os("NATIVE_COMPILE_DUMP_DIR") {
        let path = PathBuf::from(dir);
        dump_fixtures_to_dir(&path);
        println!("native_compile_fixtures: dumped 12 scenes to {}", path.display());
        return;
    }

    let guard = TempDirGuard(std::env::temp_dir().join(format!("designos-native-compile-dump-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&guard.0);
    dump_fixtures_to_dir(&guard.0);
    assert!(guard.0.join("swiftui/01_golden_card.swift").is_file());
    assert!(guard.0.join("uikit/01_golden_card.swift").is_file());
    assert!(guard.0.join("compose/01_golden_card.kt").is_file());
    assert!(guard.0.join("android-xml/01_golden_card/res/layout/card.xml").is_file());
    assert!(guard.0.join("android-xml/01_golden_card/res/drawable/card_background.xml").is_file());
}
