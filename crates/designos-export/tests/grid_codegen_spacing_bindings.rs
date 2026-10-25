//! A grid frame has two gaps: `itemSpacing` is the column gap and `counterAxisSpacing` the row gap.
//! Binding one of them to a variable must leave the other axis at its own value, in CSS, in React
//! inline styles and in Tailwind classes.

use designos_doc::*;
use designos_export::{css_for, react_for, react_with, tailwind_for, ReactOptions};

/// A 3-column grid with a 16 px column gap and a 24 px row gap. `Spacing/Md` (16) and `Spacing/Lg`
/// (24) are bound to the axes picked by `item` / `counter`; `bare_tokens` gives both a WEB code
/// syntax, which CSS prints verbatim.
fn grid(item: bool, counter: bool, bare_tokens: bool) -> (Document, NodeId) {
    grid_with_row_gap(24.0, item, counter, bare_tokens)
}

/// [`grid`] with the row gap (`counterAxisSpacing`) set to `row_px`; 0 leaves it unset, so the row
/// takes the column gap.
fn grid_with_row_gap(row_px: f32, item: bool, counter: bool, bare_tokens: bool) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    d.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Mode".into() }],
            default_mode: "m".into(),
            variables: vec!["md".into(), "lg".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    for (id, name, px, token) in [("md", "Spacing/Md", 16.0, "tokens.space.md"), ("lg", "Spacing/Lg", 24.0, "tokens.space.lg")] {
        let code_syntax = if bare_tokens { [(CodeSyntaxPlatform::Web, token.to_string())].into_iter().collect() } else { Default::default() };
        d.variables.insert(
            id.into(),
            Variable {
                id: id.into(),
                name: name.into(),
                collection: "c".into(),
                kind: VariableKind::Number,
                values: [("m".to_string(), VariableValue::Number(px))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax,
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    }
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 480.0, 200.0));
    g.name = "Cards".into();
    g.fills.clear();
    let al = AutoLayout {
        direction: LayoutDirection::Grid,
        item_spacing: 16.0,
        counter_spacing: row_px,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: 3,
        ..Default::default()
    };
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    if item {
        g.variables.bindings.insert("itemSpacing".into(), "md".into());
    }
    if counter {
        g.variables.bindings.insert("counterAxisSpacing".into(), "lg".into());
    }
    let g = d.insert(page, g).unwrap();
    let cid = d.alloc_id();
    d.insert(g, Node::rectangle(cid, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
    (d, g)
}

/// The value of declaration `prop` in a CSS block.
fn decl(css: &str, prop: &str) -> String {
    let pre = format!("{prop}: ");
    css.lines().find_map(|l| l.strip_prefix(&pre)).map(|v| v.trim_end_matches(';').to_string()).unwrap_or_else(|| format!("<no {prop} in {css}>"))
}

/// The value of the `gap` entry of React inline styles.
fn react_gap(src: &str) -> String {
    src.lines().find_map(|l| l.trim().strip_prefix("gap: ")).map(|v| v.trim_end_matches(',').to_string()).unwrap_or_else(|| format!("<no gap in {src}>"))
}

/// The class tokens of the first element of Tailwind markup.
fn classes(html: &str) -> Vec<String> {
    let start = html.find("class=\"").or_else(|| html.find("className=\"")).map(|i| html[i..].find('"').unwrap() + i + 1).unwrap();
    let end = html[start..].find('"').unwrap() + start;
    html[start..end].split_whitespace().map(String::from).collect()
}

#[test]
fn css_unbound_gaps_keep_both_pixel_values() {
    let (d, g) = grid(false, false, false);
    assert_eq!(decl(&css_for(&d, g), "gap"), "24px 16px");
}

#[test]
fn css_item_spacing_binding_only_replaces_the_column_gap() {
    let (d, g) = grid(true, false, false);
    assert_eq!(decl(&css_for(&d, g), "gap"), "24px var(--Spacing-Md, 16px)");
    let (d, g) = grid(true, false, true);
    assert_eq!(decl(&css_for(&d, g), "gap"), "24px tokens.space.md");
}

#[test]
fn css_counter_spacing_binding_only_replaces_the_row_gap() {
    let (d, g) = grid(false, true, false);
    assert_eq!(decl(&css_for(&d, g), "gap"), "var(--Spacing-Lg, 24px) 16px");
    let (d, g) = grid(false, true, true);
    assert_eq!(decl(&css_for(&d, g), "gap"), "tokens.space.lg 16px");
}

#[test]
fn css_both_bindings_replace_both_gaps() {
    let (d, g) = grid(true, true, false);
    assert_eq!(decl(&css_for(&d, g), "gap"), "var(--Spacing-Lg, 24px) var(--Spacing-Md, 16px)");
    let (d, g) = grid(true, true, true);
    assert_eq!(decl(&css_for(&d, g), "gap"), "tokens.space.lg tokens.space.md");
}

#[test]
fn css_row_follows_the_item_variable_only_when_it_has_no_spacing_of_its_own() {
    // `counter_spacing == 0` is "unset": the solver reuses the item spacing for the row, so one variable drives both.
    let (d, g) = grid_with_row_gap(0.0, true, false, false);
    assert_eq!(decl(&css_for(&d, g), "gap"), "var(--Spacing-Md, 16px)");
    // A row spacing of 16 that happens to equal the column gap is still the row's own value.
    let (d, g) = grid_with_row_gap(16.0, true, false, false);
    assert_eq!(decl(&css_for(&d, g), "gap"), "16px var(--Spacing-Md, 16px)");
    let (d, g) = grid_with_row_gap(16.0, true, false, true);
    assert_eq!(decl(&css_for(&d, g), "gap"), "16px tokens.space.md");
}

#[test]
fn react_inline_row_gap_with_the_same_pixels_as_the_column_gap_stays_its_own() {
    let (d, g) = grid_with_row_gap(16.0, true, false, false);
    assert_eq!(react_gap(&react_for(&d, g)), "'16px var(--Spacing-Md, 16px)'");
    let (d, g) = grid_with_row_gap(0.0, true, false, false);
    assert_eq!(react_gap(&react_for(&d, g)), "'var(--Spacing-Md, 16px)'");
}

#[test]
fn tailwind_splits_the_axes_for_an_equal_row_gap_of_its_own_and_joins_them_for_an_unset_one() {
    let gap_classes = |row_px: f32| {
        let (d, g) = grid_with_row_gap(row_px, true, false, false);
        let from_tailwind: Vec<String> = classes(&tailwind_for(&d, g)).into_iter().filter(|c| c.starts_with("gap")).collect();
        let from_react: Vec<String> = classes(&react_with(&d, g, &ReactOptions { tailwind: true })).into_iter().filter(|c| c.starts_with("gap")).collect();
        assert_eq!(from_tailwind, from_react);
        from_tailwind
    };
    assert_eq!(gap_classes(16.0), ["gap-x-[var(--Spacing-Md)]", "gap-y-4"]);
    assert_eq!(gap_classes(0.0), ["gap-[var(--Spacing-Md)]"]);
}

#[test]
fn react_inline_styles_follow_the_css_gap() {
    for (item, counter, bare, expected) in [
        (false, false, false, "'24px 16px'"),
        (true, false, false, "'24px var(--Spacing-Md, 16px)'"),
        (true, false, true, "'24px tokens.space.md'"),
        (false, true, false, "'var(--Spacing-Lg, 24px) 16px'"),
        (false, true, true, "'tokens.space.lg 16px'"),
        (true, true, false, "'var(--Spacing-Lg, 24px) var(--Spacing-Md, 16px)'"),
    ] {
        let (d, g) = grid(item, counter, bare);
        assert_eq!(react_gap(&react_for(&d, g)), expected, "item={item} counter={counter} bare={bare}");
    }
}

#[test]
fn tailwind_keeps_one_class_per_axis() {
    for (item, counter, expected) in [
        (false, false, vec!["gap-x-4", "gap-y-6"]),
        (true, false, vec!["gap-x-[var(--Spacing-Md)]", "gap-y-6"]),
        (false, true, vec!["gap-x-4", "gap-y-[var(--Spacing-Lg)]"]),
        (true, true, vec!["gap-x-[var(--Spacing-Md)]", "gap-y-[var(--Spacing-Lg)]"]),
    ] {
        let (d, g) = grid(item, counter, false);
        let gap_classes: Vec<String> = classes(&tailwind_for(&d, g)).into_iter().filter(|c| c.starts_with("gap")).collect();
        assert_eq!(gap_classes, expected, "tailwind_for item={item} counter={counter}");
        let react = react_with(&d, g, &ReactOptions { tailwind: true });
        let gap_classes: Vec<String> = classes(&react).into_iter().filter(|c| c.starts_with("gap")).collect();
        assert_eq!(gap_classes, expected, "react tailwind item={item} counter={counter}");
    }
}
