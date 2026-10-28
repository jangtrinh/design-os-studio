//! The Mobile page: welcome, home, spending and settings screens.

use super::icons::{smooth_line, Ic};
use super::*;

/// The screens and the layers the prototype links start from.
pub struct Screens {
    pub welcome: NodeId,
    pub home: NodeId,
    pub spending: NodeId,
    pub settings: NodeId,
    pub welcome_cta: NodeId,
    pub welcome_login: NodeId,
    pub home_balance: NodeId,
    pub spending_back: NodeId,
    pub spending_more: NodeId,
    pub settings_out: NodeId,
}

impl Screens {
    pub fn all(&self) -> Vec<NodeId> {
        vec![self.welcome, self.home, self.spending, self.settings]
    }
}

const GAP: f32 = 110.0;

fn violet() -> Color {
    hex(0x8D5CF6)
}

fn brand_gradient() -> Paint {
    grad_d(&[(0.0, hex(0x4C5BF0)), (1.0, violet())])
}

/// Status bar: time on the left, signal and battery on the right.
fn status_bar(s: &mut S, parent: NodeId, on_color: bool) -> NodeId {
    let ink = if on_color { WHITE } else { Cl::T(Tok::Ink) };
    let bar = s.row(parent, "Status bar", 0.0, [16.0, 28.0, 0.0, 28.0]);
    s.fixed_h(bar, 54.0);
    s.spread(bar, Center);
    s.text(bar, "9:41", "Text/Body strong", ink);
    let right = s.row(bar, "System", 8.0, [0.0; 4]);
    s.align(right, Start, Center);
    let signal = s.row(right, "Signal", 2.0, [0.0; 4]);
    s.align(signal, Start, End);
    for (i, h) in [4.0, 6.0, 8.0, 10.0].into_iter().enumerate() {
        let r = s.rect(signal, &format!("Bar {}", i + 1), 3.0, h, ink);
        s.radius(r, 1.0);
    }
    let batt = s.frame(right, "Battery");
    s.size(batt, 27.0, 12.0);
    let outline = s.rect(batt, "Body", 24.0, 12.0, ink);
    s.n(outline).fills = Vec::new();
    s.stroke(outline, ink, 1.0);
    s.opacity(outline, 0.45);
    s.radius(outline, 4.0);
    let level = s.rect(batt, "Level", 17.0, 8.0, ink);
    s.at(level, 2.0, 2.0);
    s.radius(level, 2.5);
    let cap = s.rect(batt, "Cap", 2.0, 4.0, ink);
    s.at(cap, 25.0, 4.0);
    s.opacity(cap, 0.45);
    bar
}

/// A phone-sized frame in vertical auto layout; returns (screen, content column).
fn phone(s: &mut S, page: NodeId, name: &str, x: f32, tab: Option<&str>, dark: bool) -> (NodeId, NodeId) {
    let root = s.screen_frame(page, name, x, 0.0, 390.0, 844.0, Tok::Bg);
    s.make_stack(root, V, 0.0, [0.0; 4]);
    s.fixed(root, 390.0, 844.0);
    s.radius(root, 36.0);
    if dark {
        let (th, dk) = (s.theme.clone(), s.dark.clone());
        s.n(root).variables.modes.insert(th, dk);
    }
    let bar = status_bar(s, root, false);
    s.fill_w(bar);
    let body = s.col(root, "Content", 16.0, [8.0, 20.0, 16.0, 20.0]);
    s.fill_w(body);
    s.fill_h(body);
    s.clip(body);
    s.bind_screen_padding(body);
    if let Some(t) = tab {
        let bar = s.inst(root, &format!("TabBar/{t}"));
        s.inst_fill_w(bar);
    }
    (root, body)
}

fn icon_button(s: &mut S, parent: NodeId, name: &str, ic: Ic) -> NodeId {
    let b = s.row(parent, name, 0.0, [0.0; 4]);
    s.fixed(b, 44.0, 44.0);
    s.centered(b);
    s.radius(b, 22.0);
    s.fill(b, Cl::T(Tok::Card));
    s.border(b);
    s.vicon(b, ic, 22.0, Cl::T(Tok::Ink));
    b
}

fn list_row(s: &mut S, parent: NodeId, ty: &str, ic: Ic, title: &str, sub: &str, value: &str) -> NodeId {
    let r = s.inst(parent, &format!("ListRow/{ty}"));
    s.set_text(r, "Title", title);
    s.set_text(r, "Subtitle", sub);
    s.set_text(r, "Value", value);
    s.set_icon(r, ic);
    s.inst_fill_w(r);
    r
}

fn card(s: &mut S, parent: NodeId, name: &str, pad: [f32; 4]) -> NodeId {
    let c = s.col(parent, name, 0.0, pad);
    s.fill_w(c);
    s.fill(c, Cl::T(Tok::Card));
    s.radius_var(c, true);
    s.border(c);
    s.effect(c, card_shadow(), Some("Shadow/Card"));
    c
}

fn section_header(s: &mut S, parent: NodeId, title: &str, action: &str) {
    let h = s.row(parent, "Section header", 0.0, [0.0; 4]);
    s.fill_w(h);
    s.spread(h, Center);
    s.text(h, title, "Text/Heading", Cl::T(Tok::Ink));
    s.text(h, action, "Text/Label", Cl::T(Tok::Brand));
}

// ------------------------------------------------------------------------- welcome

fn welcome(s: &mut S, page: NodeId, x: f32) -> (NodeId, NodeId, NodeId) {
    let root = s.screen_frame(page, "Welcome", x, 0.0, 390.0, 844.0, Tok::Bg);
    s.make_stack(root, V, 0.0, [0.0; 4]);
    s.fixed(root, 390.0, 844.0);
    s.radius(root, 36.0);

    // Hero illustration: gradients, circles and two cards, all vectors.
    let hero = s.frame(root, "Hero illustration");
    s.fixed_w(hero, 390.0);
    s.size(hero, 390.0, 440.0);
    s.fill_w(hero);
    s.n(hero).layout_child.vertical = Sizing::Fixed;
    s.fill_paint(hero, grad_d(&[(0.0, hex(0x4C5BF0)), (0.55, hex(0x6A5BF2)), (1.0, hex(0x9A5CF6))]));
    s.n(hero).corner_radii = [0.0, 0.0, 44.0, 44.0];
    s.clip(hero);
    for (name, x, y, d, a) in [("Glow 1", -90.0, -110.0, 300.0, 0.10), ("Glow 2", 220.0, 250.0, 250.0, 0.09), ("Glow 3", 292.0, 70.0, 110.0, 0.12)] {
        let c = s.ellipse(hero, name, d, d, white(a));
        s.at(c, x, y);
    }
    let bar = status_bar(s, hero, true);
    s.fixed_w(bar, 390.0);

    let back = s.frame(hero, "Card behind");
    s.size(back, 250.0, 158.0);
    s.fill(back, white(0.20));
    s.stroke(back, white(0.35), 1.0);
    s.radius(back, 22.0);
    s.place_rotated(back, 218.0, 214.0, 9.0);

    let main = s.frame(hero, "Card");
    s.size(main, 276.0, 172.0);
    s.fill_paint(main, grad_d(&[(0.0, hex(0x2B2F6E)), (1.0, hex(0x0F1030))]));
    s.style_fill(main, "Gradient/Night");
    s.radius(main, 24.0);
    s.effect(main, floating_shadow(), Some("Shadow/Floating"));
    let chip = s.rect(main, "Chip", 40.0, 30.0, Cl::C(hex(0xF5B544)));
    s.fill_paint(chip, grad_d(&[(0.0, hex(0xFFD58A)), (1.0, hex(0xF29A2E))]));
    s.radius(chip, 8.0);
    s.at(chip, 24.0, 26.0);
    let mark = s.frame(main, "Mark");
    s.size(mark, 30.0, 30.0);
    s.fill(mark, white(0.16));
    s.radius(mark, 10.0);
    s.at(mark, 222.0, 24.0);
    let m = s.text(mark, "M", "Text/Body strong", WHITE);
    s.at(m, 9.0, 4.0);
    let num = s.text(main, "4821  ••••  ••••  7305", "Text/Body strong", white(0.9));
    s.at(num, 24.0, 98.0);
    let who = s.text(main, "ALEX MORGAN", "Text/Micro", white(0.6));
    s.at(who, 24.0, 134.0);
    for (i, a) in [0.9, 0.55].into_iter().enumerate() {
        let ring = s.ellipse(main, "Ring", 30.0, 30.0, WHITE);
        s.n(ring).fills = Vec::new();
        s.stroke(ring, white(a), 2.0);
        s.at(ring, 214.0 + i as f32 * 18.0, 126.0);
    }
    s.place_rotated(main, 170.0, 238.0, -8.0);

    let pill = s.row(hero, "Payment received", 10.0, [8.0, 16.0, 8.0, 8.0]);
    s.align(pill, Start, Center);
    s.radius(pill, 99.0);
    s.fill(pill, Cl::T(Tok::Card));
    s.effect(pill, floating_shadow(), Some("Shadow/Floating"));
    let dot = s.row(pill, "Icon", 0.0, [0.0; 4]);
    s.fixed(dot, 34.0, 34.0);
    s.centered(dot);
    s.radius(dot, 17.0);
    s.fill(dot, Cl::T(Tok::PosSoft));
    s.vicon(dot, Ic::Request, 18.0, Cl::T(Tok::Pos));
    let txt = s.col(pill, "Text", 0.0, [0.0; 4]);
    s.text(txt, "+$240.00", "Text/Body strong", Cl::T(Tok::Ink));
    s.text(txt, "Received from Hollis", "Text/Label", Cl::T(Tok::Muted));
    s.at(pill, 24.0, 368.0);

    let coin = s.row(hero, "Coin", 0.0, [0.0; 4]);
    s.fixed(coin, 56.0, 56.0);
    s.centered(coin);
    s.radius(coin, 28.0);
    s.fill_paint(coin, grad_d(&[(0.0, hex(0xFFD58A)), (1.0, hex(0xF29A2E))]));
    s.stroke(coin, white(0.6), 3.0);
    s.effect(coin, floating_shadow(), None);
    s.text(coin, "$", "Text/Title", WHITE);
    s.at(coin, 296.0, 330.0);

    // Copy and actions.
    let copy = s.col(root, "Copy", 12.0, [32.0, 24.0, 0.0, 24.0]);
    s.fill_w(copy);
    s.fill_h(copy);
    s.text(copy, "Money, made calm.", "Text/Display", Cl::T(Tok::Ink));
    s.text_w(copy, "One place to spend, save and send. No hidden fees, no surprises.", "Text/Body", Cl::T(Tok::Muted), 342.0);

    let actions = s.col(root, "Actions", 12.0, [0.0, 24.0, 34.0, 24.0]);
    s.fill_w(actions);
    s.align(actions, Start, Center);
    let dots = s.row(actions, "Page dots", 6.0, [0.0, 0.0, 10.0, 0.0]);
    for i in 0..3 {
        let r = s.rect(dots, "Dot", if i == 0 { 22.0 } else { 6.0 }, 6.0, Cl::T(if i == 0 { Tok::Brand } else { Tok::Border }));
        s.radius(r, 3.0);
    }
    let cta = s.inst(actions, "Button/Primary/Large");
    s.set_text(cta, "Label", "Get started");
    s.set_prop(cta, "Show icon", PropValue::Bool(true));
    s.inst_fill_w(cta);
    s.n(cta).name = "Get started".into();
    let login = s.inst(actions, "Button/Ghost/Large");
    s.set_text(login, "Label", "I already have an account");
    s.inst_fill_w(login);
    (root, cta, login)
}

// ------------------------------------------------------------------------- home

fn balance_chart(s: &mut S, parent: NodeId) {
    let (w, h) = (310.0, 84.0);
    let chart = s.frame(parent, "Balance chart");
    s.size(chart, w, h);
    let data = [0.62, 0.55, 0.68, 0.42, 0.50, 0.26, 0.10];
    let pts: Vec<(f32, f32)> = data.iter().enumerate().map(|(i, v)| (7.0 + i as f32 * (w - 14.0) / 6.0, 8.0 + v * (h - 16.0))).collect();
    let mut area = smooth_line(&pts);
    area.push(PathCmd::LineTo(Vec2::new(pts[pts.len() - 1].0, h)));
    area.push(PathCmd::LineTo(Vec2::new(pts[0].0, h)));
    area.push(PathCmd::Close);
    let a = s.vector(chart, "Area", w, h, vec![Path { cmds: area, fill_rule: FillRule::NonZero }]);
    s.fill_paint(a, grad_v(&[(0.0, Color::WHITE.with_alpha(0.38)), (1.0, Color::WHITE.with_alpha(0.0))]));
    let l = s.vector(chart, "Line", w, h, vec![Path { cmds: smooth_line(&pts), fill_rule: FillRule::NonZero }]);
    s.stroke(l, WHITE, 2.5);
    let (lx, ly) = pts[pts.len() - 1];
    let ring = s.ellipse(chart, "Marker ring", 18.0, 18.0, white(0.28));
    s.at(ring, lx - 9.0, ly - 9.0);
    let dot = s.ellipse(chart, "Marker", 9.0, 9.0, WHITE);
    s.at(dot, lx - 4.5, ly - 4.5);
}

fn home(s: &mut S, page: NodeId, x: f32) -> (NodeId, NodeId) {
    let (root, body) = phone(s, page, "Home", x, Some("Home"), false);

    let hdr = s.row(body, "Header", 12.0, [0.0; 4]);
    s.fill_w(hdr);
    s.spread(hdr, Center);
    let left = s.row(hdr, "Greeting", 12.0, [0.0; 4]);
    s.align(left, Start, Center);
    s.inst(left, "Avatar/Small");
    let who = s.col(left, "Text", 0.0, [0.0; 4]);
    s.text(who, "Good morning,", "Text/Caption", Cl::T(Tok::Muted));
    s.text(who, "Alex Morgan", "Text/Heading", Cl::T(Tok::Ink));
    icon_button(s, hdr, "Notifications", Ic::Bell);

    let bal = s.col(body, "Balance card", 4.0, [20.0, 20.0, 16.0, 20.0]);
    s.fill_w(bal);
    s.fill_paint(bal, brand_gradient());
    s.style_fill(bal, "Gradient/Brand");
    s.radius(bal, 28.0);
    s.clip(bal);
    s.effect(
        bal,
        Effect::DropShadow {
            color: Color::rgba(76, 91, 240, 90),
            offset: Vec2::new(0.0, 14.0),
            radius: 28.0,
            spread: -8.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        },
        None,
    );
    let top = s.row(bal, "Top", 0.0, [0.0; 4]);
    s.fill_w(top);
    s.spread(top, Center);
    s.text(top, "Total balance", "Text/Caption", white(0.8));
    let tag = s.inst(top, "Tag/Positive");
    s.set_text(tag, "Label", "+4.2% this week");
    s.text(bal, "$12,480.20", "Text/Display", WHITE);
    let gap = s.frame(bal, "Gap");
    s.size(gap, 1.0, 4.0);
    balance_chart(s, bal);
    let days = s.row(bal, "Days", 0.0, [0.0; 4]);
    s.fill_w(days);
    s.spread(days, Center);
    for d in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
        s.text(days, d, "Text/Micro", white(0.7));
    }

    let qa = s.row(body, "Quick actions", 0.0, [0.0; 4]);
    s.fill_w(qa);
    s.spread(qa, Start);
    for (ic, label) in [(Ic::Send, "Send"), (Ic::Request, "Request"), (Ic::Swap, "Swap"), (Ic::More, "More")] {
        let item = s.col(qa, label, 8.0, [0.0; 4]);
        s.align(item, Start, Center);
        let tile = s.row(item, "Tile", 0.0, [0.0; 4]);
        s.fixed(tile, 64.0, 64.0);
        s.centered(tile);
        s.radius(tile, 22.0);
        s.fill(tile, Cl::T(Tok::Card));
        s.border(tile);
        s.effect(tile, card_shadow(), Some("Shadow/Card"));
        let c = s.icon_comps[&ic];
        s.instance(tile, c);
        s.text(item, label, "Text/Caption", Cl::T(Tok::Ink));
    }

    section_header(s, body, "Recent activity", "See all");
    let list = card(s, body, "Activity", [4.0, 0.0, 4.0, 0.0]);
    let rows = [
        (Ic::Coffee, "Corner Roast", "Coffee - Today, 8:12", "-$4.80", false),
        (Ic::Basket, "Greenleaf Market", "Groceries - Yesterday", "-$62.40", false),
        (Ic::Request, "Hollis Studio", "Salary - Mon, 6 Oct", "+$3,850.00", true),
    ];
    for (i, (ic, t, sub, v, pos)) in rows.into_iter().enumerate() {
        if i > 0 {
            s.divider(list);
        }
        let r = list_row(s, list, "Transaction", ic, t, sub, v);
        if pos {
            let val = s.find(r, "Value").expect("value");
            s.ov_text_color(val, Cl::T(Tok::Pos));
        }
    }
    (root, bal)
}

// ------------------------------------------------------------------------- spending

fn bar_chart(s: &mut S, parent: NodeId) {
    let (w, h) = (310.0, 160.0);
    let chart = s.frame(parent, "Weekly chart");
    s.size(chart, w, h);
    let (x0, top, bottom) = (40.0, 14.0, 124.0);
    for (i, label) in ["$400", "$200", "$0"].into_iter().enumerate() {
        let y = top + i as f32 * (bottom - top) / 2.0;
        let line = s.rect(chart, "Gridline", w - x0, 1.0, Cl::T(Tok::Border));
        s.at(line, x0, y);
        let t = s.text(chart, label, "Text/Label", Cl::T(Tok::Faint));
        s.at(t, 0.0, y - 8.0);
    }
    let values = [96.0, 168.0, 72.0, 352.0, 208.0, 264.0, 120.0];
    let pitch = (w - x0) / 7.0;
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    for (i, v) in values.into_iter().enumerate() {
        let bh = v / 400.0 * (bottom - top);
        let bx = x0 + i as f32 * pitch + (pitch - 24.0) / 2.0;
        let b = s.rect(chart, &format!("Bar {}", days[i]), 24.0, bh, Cl::T(Tok::BrandSoft));
        s.n(b).corner_radii = [8.0, 8.0, 3.0, 3.0];
        s.at(b, bx, bottom - bh);
        if i == 3 {
            s.fill_paint(b, grad_v(&[(0.0, hex(0x6A6CF6)), (1.0, hex(0x4C5BF0))]));
            s.n(b).variables.bindings.remove("fills/0");
            let tip = s.row(chart, "Tooltip", 0.0, [4.0, 9.0, 4.0, 9.0]);
            s.radius(tip, 9.0);
            s.fill(tip, Cl::T(Tok::Ink));
            s.text(tip, "$352", "Text/Label", Cl::T(Tok::Card));
            s.relayout(tip);
            let tw = s.n(tip).size.x;
            s.at(tip, bx + 12.0 - tw / 2.0, bottom - bh - 32.0);
        }
        let d = s.text(chart, days[i], "Text/Label", Cl::T(Tok::Muted));
        let dw = s.n(d).size.x;
        s.at(d, bx + 12.0 - dw / 2.0, bottom + 12.0);
    }
}

fn spending(s: &mut S, page: NodeId, x: f32) -> (NodeId, NodeId, NodeId) {
    let (root, body) = phone(s, page, "Spending", x, Some("Insights"), false);

    let hdr = s.row(body, "Header", 0.0, [0.0; 4]);
    s.fill_w(hdr);
    s.spread(hdr, Center);
    let back = icon_button(s, hdr, "Back", Ic::ChevronLeft);
    s.text(hdr, "Spending", "Text/Heading", Cl::T(Tok::Ink));
    let more = icon_button(s, hdr, "Settings", Ic::More);

    let seg = s.row(body, "Segmented control", 4.0, [4.0; 4]);
    s.fill_w(seg);
    s.fixed_h(seg, 44.0);
    s.fill(seg, Cl::T(Tok::Raised));
    s.radius(seg, 14.0);
    for (i, label) in ["Week", "Month", "Year"].into_iter().enumerate() {
        let seg_item = s.row(seg, label, 0.0, [0.0; 4]);
        s.fill_w(seg_item);
        s.fill_h(seg_item);
        s.centered(seg_item);
        s.radius(seg_item, 11.0);
        if i == 0 {
            s.fill(seg_item, Cl::T(Tok::Card));
            s.effect(seg_item, card_shadow(), None);
        }
        s.text(seg_item, label, "Text/Body strong", Cl::T(if i == 0 { Tok::Ink } else { Tok::Muted }));
    }

    let sum = s.col(body, "Summary", 4.0, [0.0; 4]);
    s.fill_w(sum);
    s.text(sum, "Total spent", "Text/Caption", Cl::T(Tok::Muted));
    s.text(sum, "$2,184.60", "Text/Display", Cl::T(Tok::Ink));
    let tag = s.inst(sum, "Tag/Positive");
    s.set_text(tag, "Label", "8.2% less than last week");

    let chart_card = card(s, body, "Chart card", [18.0, 20.0, 16.0, 20.0]);
    if let Some(al) = s.n(chart_card).kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.item_spacing = 8.0;
    }
    let head = s.row(chart_card, "Chart header", 0.0, [0.0; 4]);
    s.fill_w(head);
    s.spread(head, Center);
    s.text(head, "Daily spending", "Text/Body strong", Cl::T(Tok::Ink));
    s.text(head, "6 - 12 Oct", "Text/Caption", Cl::T(Tok::Muted));
    bar_chart(s, chart_card);

    let cats = card(s, body, "Categories", [4.0, 0.0, 4.0, 0.0]);
    for (i, (ic, t, sub, v)) in [
        (Ic::Basket, "Groceries", "12 payments", "$684.20"),
        (Ic::Bus, "Transport", "9 payments", "$318.50"),
        (Ic::Coffee, "Dining out", "7 payments", "$402.00"),
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            s.divider(cats);
        }
        list_row(s, cats, "Transaction", ic, t, sub, v);
    }
    (root, back, more)
}

// ------------------------------------------------------------------------- settings

fn settings(s: &mut S, page: NodeId, x: f32) -> (NodeId, NodeId) {
    let (root, body) = phone(s, page, "Settings", x, Some("Profile"), true);
    if let Some(al) = s.n(body).kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.item_spacing = 12.0;
    }
    s.text(body, "Settings", "Text/Title", Cl::T(Tok::Ink));

    let profile = card(s, body, "Profile", [16.0, 16.0, 16.0, 16.0]);
    if let Some(al) = s.n(profile).kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.direction = H;
        al.item_spacing = 14.0;
        al.counter_align = Center;
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Hug;
    }
    let av = s.inst(profile, "Avatar/Large");
    let info = s.col(profile, "Info", 2.0, [0.0; 4]);
    s.fill_w(info);
    s.text(info, "Alex Morgan", "Text/Heading", Cl::T(Tok::Ink));
    s.text(info, "alex.morgan@example.com", "Text/Caption", Cl::T(Tok::Muted));
    let gap = s.frame(info, "Gap");
    s.size(gap, 1.0, 4.0);
    let tag = s.inst(info, "Tag/Brand");
    s.set_text(tag, "Label", "Marlow Plus");
    s.vicon(profile, Ic::ChevronRight, 18.0, Cl::T(Tok::Faint));
    let _ = av;

    s.text(body, "Preferences", "Text/Micro", Cl::T(Tok::Muted));
    let g1 = card(s, body, "Preferences", [4.0, 0.0, 4.0, 0.0]);
    list_row(s, g1, "Toggle", Ic::Bell, "Notifications", "Push and email", "");
    s.divider(g1);
    list_row(s, g1, "Navigation", Ic::Moon, "Appearance", "", "Dark");
    s.divider(g1);
    let bio = list_row(s, g1, "Toggle", Ic::Shield, "Biometric unlock", "Face or fingerprint", "");
    let t = s.find(bio, "Toggle").expect("toggle");
    let off = s.variant("Toggle/Off");
    swap_instance(&mut s.d, t, off).apply(&mut s.d).expect("swap toggle");

    s.text(body, "Account", "Text/Micro", Cl::T(Tok::Muted));
    let g2 = card(s, body, "Account", [4.0, 0.0, 4.0, 0.0]);
    list_row(s, g2, "Navigation", Ic::Bank, "Linked accounts", "", "3");
    s.divider(g2);
    list_row(s, g2, "Navigation", Ic::Card, "Cards and limits", "", "");
    s.divider(g2);
    list_row(s, g2, "Navigation", Ic::Help, "Help centre", "", "");

    let out = s.inst(body, "Button/Ghost/Large");
    s.set_text(out, "Label", "Sign out");
    s.inst_fill_w(out);
    (root, out)
}

/// Builds the Mobile page.
pub fn build(s: &mut S) -> Screens {
    let page = s.page(0);
    let step = 390.0 + GAP;
    let (welcome, welcome_cta, welcome_login) = welcome(s, page, 0.0);
    let (home, home_balance) = home(s, page, step);
    let (spending, spending_back, spending_more) = spending(s, page, 2.0 * step);
    let (settings, settings_out) = settings(s, page, 3.0 * step);
    Screens { welcome, home, spending, settings, welcome_cta, welcome_login, home_balance, spending_back, spending_more, settings_out }
}
