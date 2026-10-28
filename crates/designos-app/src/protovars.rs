//! Built-in prototype with variables (`--fixture proto-vars`): a small shop screen whose buttons
#![cfg(any(test, feature = "fixtures"))]
//! set variables, a conditional that checks out at three items, a mode switch and a
//! hover-interactive component. Used for screenshots and the Present tests.

use designos_doc::*;

pub const FIXTURE: &str = "proto-vars";

// Fixture builder: a flat positional signature is the clearest here.
#[allow(clippy::too_many_arguments)]
fn text(d: &mut Document, parent: NodeId, x: f32, y: f32, w: f32, s: &str, size: f32, weight: u16, color: Color, name: &str) -> NodeId {
    let id = d.alloc_id();
    let style = TextStyle { font_size: size, font_weight: weight, fills: vec![Paint::solid(color)], ..Default::default() };
    let mut n = Node::text(id, Rect::new(x, y, w, size * 1.3), s, style);
    n.name = name.into();
    d.insert(parent, n).unwrap()
}

/// A button: a rounded frame with its label inside, so clicks on the text reach the frame.
fn button(d: &mut Document, parent: NodeId, rect: Rect, name: &str, fill: Color, label: &str) -> NodeId {
    let id = d.alloc_id();
    let mut n = Node::frame(id, rect);
    n.name = name.into();
    n.corner_radii = [10.0; 4];
    n.fills = vec![Paint::solid(fill)];
    let b = d.insert(parent, n).unwrap();
    text(d, b, 16.0, rect.h / 2.0 - 9.0, rect.w - 32.0, label, 14.0, 600, Color::WHITE, &format!("{name} label"));
    b
}

fn click(actions: Vec<Action>) -> Reaction {
    Reaction { trigger: Trigger::OnClick, actions, transition: None }
}

/// The ids the fixture's tests and screenshot setup need.
pub struct Ids {
    pub count: String,
    pub label: String,
    pub member: String,
    pub accent: String,
    pub collection: String,
    pub dark: String,
}

pub fn ids(d: &Document) -> Ids {
    let by = |name: &str| d.variables.values().find(|v| v.name == name).map(|v| v.id.clone()).unwrap_or_default();
    let coll = d.variable_collections.values().next().cloned().unwrap();
    Ids {
        count: by("count"),
        label: by("label"),
        member: by("member"),
        accent: by("accent"),
        collection: coll.id.clone(),
        dark: coll.modes.iter().find(|m| m.name == "Dark").map(|m| m.id.clone()).unwrap_or_default(),
    }
}

pub fn document() -> Document {
    let mut d = Document::new("Shop prototype");
    let page = d.add_page("Shop");
    if let Some(n) = d.nodes.get_mut(&page) {
        n.kind = NodeKind::Page { background: Color::rgb(0x1E, 0x1E, 0x1E) };
    }

    // ---- variables
    let mut store = VariableStore::default();
    let coll = store.add_collection("State");
    let light = store.collections[&coll].default_mode.clone();
    let dark = store.add_mode(&coll, "Dark").unwrap();
    let count = store.add_variable(&coll, "count", VariableKind::Number).unwrap();
    let label = store.add_variable(&coll, "label", VariableKind::String).unwrap();
    let member = store.add_variable(&coll, "member", VariableKind::Boolean).unwrap();
    let accent_id = store.add_variable(&coll, "accent", VariableKind::Color).unwrap();
    store.set_value(&count, &light, VariableValue::Number(0.0));
    store.set_value(&count, &dark, VariableValue::Number(0.0));
    store.set_value(&label, &light, VariableValue::String("Your cart is empty".into()));
    store.set_value(&label, &dark, VariableValue::String("Your cart is empty".into()));
    store.set_value(&member, &light, VariableValue::Boolean(false));
    store.set_value(&member, &dark, VariableValue::Boolean(false));
    store.set_value(&accent_id, &light, VariableValue::Color(Color::rgb(0x0C, 0x8C, 0xE9)));
    store.set_value(&accent_id, &dark, VariableValue::Color(Color::rgb(0xF2, 0xC9, 0x4C)));
    let _ = store.command().apply(&mut d);

    // ---- screens
    let cart = {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(0.0, 0.0, 390.0, 640.0));
        f.name = "Cart".into();
        f.fills = vec![Paint::solid(Color::rgb(0x2C, 0x2C, 0x2C))];
        f.corner_radii = [24.0; 4];
        d.insert(page, f).unwrap()
    };
    let done = {
        let id = d.alloc_id();
        let mut f = Node::frame(id, Rect::new(480.0, 0.0, 390.0, 640.0));
        f.name = "Checkout".into();
        f.fills = vec![Paint::solid(Color::rgb(0x14, 0x6B, 0x3A))];
        f.corner_radii = [24.0; 4];
        d.insert(page, f).unwrap()
    };
    text(&mut d, done, 32.0, 280.0, 320.0, "Thanks for your order!", 24.0, 700, Color::WHITE, "Thanks");
    let back = button(&mut d, done, Rect::new(32.0, 560.0, 326.0, 48.0), "Start over", Color::rgb(0x0C, 0x8C, 0xE9), "Start over");

    text(&mut d, cart, 32.0, 40.0, 326.0, "Shop", 28.0, 700, Color::WHITE, "Title");
    let status = text(&mut d, cart, 32.0, 96.0, 326.0, "Your cart is empty", 16.0, 400, Color::rgb(0xCF, 0xCF, 0xCF), "Status");
    let badge = {
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(32.0, 144.0, 326.0, 8.0));
        n.name = "Accent bar".into();
        n.corner_radii = [4.0; 4];
        n.fills = vec![Paint::solid(Color::rgb(0x0C, 0x8C, 0xE9))];
        d.insert(cart, n).unwrap()
    };
    let banner = text(&mut d, cart, 32.0, 176.0, 326.0, "Member prices applied", 14.0, 600, Color::rgb(0xF2, 0xC9, 0x4C), "Member banner");
    let add = button(&mut d, cart, Rect::new(32.0, 400.0, 326.0, 48.0), "Add", Color::rgb(0x0C, 0x8C, 0xE9), "Add item");
    let join = button(&mut d, cart, Rect::new(32.0, 464.0, 160.0, 44.0), "Join", Color::rgb(0x4A, 0x4A, 0x4A), "Toggle member");
    let theme = button(&mut d, cart, Rect::new(198.0, 464.0, 160.0, 44.0), "Dark mode", Color::rgb(0x4A, 0x4A, 0x4A), "Dark mode");
    let reset = button(&mut d, cart, Rect::new(32.0, 528.0, 326.0, 44.0), "Reset", Color::rgb(0x4A, 0x4A, 0x4A), "Reset");

    // ---- bindings
    let bind = |d: &mut Document, node: NodeId, key: &str, var: &str| {
        d.get_mut(node).unwrap().variables.bindings.insert(key.into(), var.into());
    };
    bind(&mut d, status, "characters", &label);
    bind(&mut d, banner, "visible", &member);
    let accent = ids(&d).accent;
    bind(&mut d, badge, "fills/0", &accent);
    if let Some(n) = d.get_mut(banner) {
        n.visible = false;
    }

    // ---- interactions
    let ids = ids(&d);
    let items = |n: &str| Expression::binary(BinaryOp::Add, Expression::text(n), Expression::var(ids.count.clone()));
    d.get_mut(add).unwrap().prototype.reactions.push(Reaction {
        trigger: Trigger::OnClick,
        actions: vec![
            Action::SetVariable {
                variable: ids.count.clone(),
                value: Expression::binary(BinaryOp::Add, Expression::var(ids.count.clone()), Expression::number(1.0)),
            },
            Action::Conditional {
                branches: vec![(
                    Expression::binary(BinaryOp::Ge, Expression::var(ids.count.clone()), Expression::number(3.0)),
                    vec![Action::Navigate { dest: done, preserve_scroll: false }],
                )],
                otherwise: vec![Action::SetVariable { variable: ids.label.clone(), value: items("Items in cart: ") }],
            },
        ],
        transition: Some(Transition { kind: TransitionKind::Dissolve, easing: Easing::EaseOut, duration_ms: 250 }),
    });
    d.get_mut(join)
        .unwrap()
        .prototype
        .reactions
        .push(click(vec![Action::SetVariable { variable: ids.member.clone(), value: Expression::logical_not(Expression::var(ids.member.clone())) }]));
    d.get_mut(theme).unwrap().prototype.reactions.push(click(vec![Action::SetVariableMode { collection: ids.collection.clone(), mode: ids.dark.clone() }]));
    d.get_mut(reset).unwrap().prototype.reactions.push(click(vec![
        Action::SetVariable { variable: ids.count.clone(), value: Expression::number(0.0) },
        Action::SetVariable { variable: ids.label.clone(), value: Expression::text("Your cart is empty") },
    ]));
    d.get_mut(back).unwrap().prototype.reactions.push(click(vec![
        Action::SetVariable { variable: ids.count.clone(), value: Expression::number(0.0) },
        Action::SetVariable { variable: ids.label.clone(), value: Expression::text("Your cart is empty") },
        Action::Navigate { dest: cart, preserve_scroll: false },
    ]));
    d.flows.push(Flow { name: "Shopping".into(), page, start: cart, description: String::new() });
    d
}

/// Presses "Add item" twice so the Present screenshot shows live, changed values.
pub fn prime_present(app: &mut crate::app::App) {
    let Some(add) = app.doc.nodes.values().find(|n| n.name == "Add").map(|n| n.id) else { return };
    if let Some(st) = app.proto.present.as_mut() {
        for _ in 0..2 {
            st.player.pointer_up(Some(add), false);
        }
        st.show_vars = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_wired() {
        let d = document();
        let ids = ids(&d);
        assert!(!ids.count.is_empty() && !ids.dark.is_empty() && !ids.accent.is_empty());
        let add = d.nodes.values().find(|n| n.name == "Add").unwrap();
        assert!(matches!(add.prototype.reactions[0].actions[1], Action::Conditional { .. }));
        let status = d.nodes.values().find(|n| n.name == "Status").unwrap();
        assert_eq!(status.variables.bindings.get("characters"), Some(&ids.label));
        for r in d.nodes.values().flat_map(|n| &n.prototype.reactions) {
            for a in &r.actions {
                if let Action::SetVariable { value, .. } = a {
                    assert!(value.within_limits());
                }
            }
        }
    }
}
