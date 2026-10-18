//! Recolouring text through one door. The renderer shapes the base style and every style run as
//! separate styles and paints the run's own fills, so a colour written to the base alone does not
//! change what is drawn once the text has runs.

use crate::{PaintKind, TextProps};

/// Sets paint `index` of the base text style and of every style run to `kind`.
///
/// All-or-nothing: returns `false` and changes nothing unless the base style and every run have a
/// paint at `index`. Fill properties other than the kind (opacity, visibility, blend) are kept.
pub fn set_text_fills_everywhere(text: &mut TextProps, index: usize, kind: PaintKind) -> bool {
    if text.style.fills.len() <= index || text.runs.iter().any(|r| r.style.fills.len() <= index) {
        return false;
    }
    text.style.fills[index].kind = kind.clone();
    for run in &mut text.runs {
        run.style.fills[index].kind = kind.clone();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Paint, TextRun, TextStyle};

    fn text_with_runs(base: Vec<Paint>, runs: Vec<Vec<Paint>>) -> TextProps {
        let style = |fills| TextStyle { fills, ..TextStyle::default() };
        TextProps {
            content: "Hello".into(),
            style: style(base),
            runs: runs.into_iter().enumerate().map(|(i, f)| TextRun { start: i, end: i + 1, style: style(f) }).collect(),
            ..TextProps::default()
        }
    }

    fn solid(v: u8) -> Paint {
        Paint::solid(Color::rgb(v, v, v))
    }

    #[test]
    fn the_base_style_and_every_run_get_the_new_colour() {
        let mut t = text_with_runs(vec![solid(0x77)], vec![vec![solid(0x77)], vec![solid(0x77)]]);
        assert!(set_text_fills_everywhere(&mut t, 0, PaintKind::Solid(Color::BLACK)));
        assert_eq!(t.style.fills[0].kind, PaintKind::Solid(Color::BLACK));
        assert!(t.runs.iter().all(|r| r.style.fills[0].kind == PaintKind::Solid(Color::BLACK)));
    }

    #[test]
    fn opacity_visibility_and_the_other_paints_are_kept() {
        let mut hidden_second = solid(0x10);
        hidden_second.visible = false;
        let mut half = solid(0x77);
        half.opacity = 0.5;
        let mut t = text_with_runs(vec![half.clone(), hidden_second.clone()], vec![vec![half, hidden_second.clone()]]);
        assert!(set_text_fills_everywhere(&mut t, 0, PaintKind::Solid(Color::WHITE)));
        assert_eq!(t.style.fills[0].opacity, 0.5);
        assert_eq!(t.runs[0].style.fills[0].opacity, 0.5);
        assert_eq!(t.style.fills[1], hidden_second);
        assert_eq!(t.runs[0].style.fills[1], hidden_second);
    }

    #[test]
    fn text_without_runs_only_changes_the_base() {
        let mut t = text_with_runs(vec![solid(0x77)], vec![]);
        assert!(set_text_fills_everywhere(&mut t, 0, PaintKind::Solid(Color::BLACK)));
        assert_eq!(t.style.fills[0].kind, PaintKind::Solid(Color::BLACK));
    }

    #[test]
    fn a_missing_paint_index_on_the_base_or_on_any_run_changes_nothing() {
        let mut t = text_with_runs(vec![solid(0x77)], vec![vec![solid(0x77)], vec![]]);
        let before = t.clone();
        assert!(!set_text_fills_everywhere(&mut t, 0, PaintKind::Solid(Color::BLACK)), "one run has no paint");
        assert_eq!(t, before);
        assert!(!set_text_fills_everywhere(&mut t, 3, PaintKind::Solid(Color::BLACK)), "no such paint");
        assert_eq!(t, before);
    }
}
