//! Screenshot hover tips use the same frame a live hover tooltip draws.
use crate::theme;
use egui::{Event, Pos2, RawInput, Rect, Sense, Vec2};

fn screen() -> RawInput {
    RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), ..Default::default() }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TipChrome {
    fill: egui::Color32,
    radius: egui::CornerRadius,
    stroke_w: i32,
    stroke: egui::Color32,
    pad: [i32; 4],
}

fn tip_chrome(shapes: &[egui::epaint::ClippedShape], needle: &str) -> TipChrome {
    let text = shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Text(text) if text.galley.job.text.contains(needle) => Some(text.galley.rect.translate(text.pos.to_vec2())),
        _ => None,
    });
    let text = text.unwrap_or_else(|| panic!("missing hover tip {needle}"));
    let frame = shapes.iter().filter_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.fill.a() > 0 && shape.rect.contains(text.center()) => Some(shape),
        _ => None,
    });
    let frame = frame.min_by(|a, b| a.rect.area().total_cmp(&b.rect.area())).unwrap_or_else(|| panic!("no frame around {needle}"));
    let pad = |gap: f32| gap.round() as i32;
    TipChrome {
        fill: frame.fill,
        radius: frame.corner_radius,
        stroke_w: frame.stroke.width.round() as i32,
        stroke: frame.stroke.color,
        pad: [
            pad(text.left() - frame.rect.left()),
            pad(frame.rect.right() - text.right()),
            pad(text.top() - frame.rect.top()),
            pad(frame.rect.bottom() - text.bottom()),
        ],
    }
}

fn themed() -> egui::Context {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    theme::install_fonts(&ctx);
    ctx
}

fn live_hover_tip(ctx: &egui::Context) -> Vec<egui::epaint::ClippedShape> {
    let rect = Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(96.0, theme::TARGET_MIN));
    let mut shapes = Vec::new();
    for (time, moved) in [(0.0, true), (0.8, false), (1.2, false)] {
        let mut raw = screen();
        raw.time = Some(time);
        if moved {
            raw.events.push(Event::PointerMoved(rect.center()));
        }
        shapes = ctx
            .run_ui(raw, |ui| {
                let response = ui.interact(rect, ui.id().with("live-hover-tip"), Sense::hover());
                response.on_hover_text("Live hover copy");
            })
            .shapes;
    }
    shapes
}

fn style_hover_tip(ctx: &egui::Context) -> Vec<egui::epaint::ClippedShape> {
    let mut doc = designos_doc::Document::new("t");
    doc.add_page("p");
    let fx = designos_doc::Style::Effect(vec![designos_doc::Effect::LayerBlur { radius: 4.0, visible: true }]);
    doc.styles.insert("1186:100350".into(), fx);
    doc.style_meta
        .insert("1186:100350".into(), designos_doc::StyleMeta { name: "   ".into(), property: designos_doc::StyleProperty::Effect, ..Default::default() });
    let mut app = crate::app::App::new(doc);
    app.props.style_menu = Some(designos_doc::StyleProperty::Effect);
    crate::panels::style_picker::set_screenshot_hover_tip(true);
    let btn = Rect::from_min_size(egui::pos2(120.0, 48.0), Vec2::splat(theme::TARGET_MIN));
    let mut shapes = Vec::new();
    for time in [0.0, 0.5] {
        let mut raw = screen();
        raw.time = Some(time);
        shapes = ctx
            .run_ui(raw, |ui| {
                crate::panels::style_picker::style_button(&mut app, ui, &[], designos_doc::StyleProperty::Effect, btn, btn.right());
            })
            .shapes;
    }
    shapes
}

fn libraries_hover_tip(ctx: &egui::Context) -> Vec<egui::epaint::ClippedShape> {
    let mut paint = designos_doc::Paint::solid(designos_doc::Color::BLACK);
    let mut state = crate::widgets::picker::State::new(&paint);
    state.tab = crate::widgets::picker::Tab::Libraries;
    let styles = [crate::widgets::picker::StyleItem {
        key: "brand".into(),
        name: "Brand".into(),
        paint: paint.clone(),
        description: "Libraries hover copy".into(),
        library: None,
    }];
    let env = crate::widgets::picker::Env { paint_types: true, sources: &[], styles: &styles, can_add_style: false };
    crate::widgets::picker::set_screenshot_libraries_hover(true);
    let mut shapes = Vec::new();
    for time in [0.0, 0.5] {
        let mut raw = screen();
        raw.time = Some(time);
        shapes = ctx
            .run_ui(raw, |ui| {
                crate::widgets::picker::show(ui, egui::pos2(24.0, 24.0), &mut state, &mut paint, &env);
            })
            .shapes;
    }
    shapes
}

struct ClearHoverTips;

impl Drop for ClearHoverTips {
    fn drop(&mut self) {
        crate::panels::style_picker::set_screenshot_hover_tip(false);
        crate::widgets::picker::set_screenshot_libraries_hover(false);
    }
}

#[test]
fn hover_tips_match_the_live_tooltip_frame() {
    let _clear = ClearHoverTips;
    let live = tip_chrome(&live_hover_tip(&themed()), "Live hover copy");
    let style = tip_chrome(&style_hover_tip(&themed()), "1186:100350");
    let libraries = tip_chrome(&libraries_hover_tip(&themed()), "Libraries hover copy");
    assert_eq!(style, live, "style picker hover tip");
    assert_eq!(libraries, live, "libraries hover tip");
}
