//! "Agent failed to start": what an agent CLI tab shows when the CLI exits within seconds of being
//! opened. Diagnosis is a pure function of the CLI, its install state and its output; the panel is
//! painted over the terminal viewport with the Studio theme tokens and offers exactly one fix.

use std::time::Duration;

use egui::{Align2, Id, Painter, Pos2, Rect, Sense, Ui, Vec2};

use crate::terminal::{strip_ansi_codes, AgentCliPreset};
use crate::{theme, widgets};

/// A tab whose CLI exits this soon after it was spawned counts as "failed to start".
pub const START_FAILURE_WINDOW: Duration = Duration::from_secs(5);
/// How many of the CLI's last output lines the panel shows.
pub const TAIL_LINES: usize = 6;
const TAIL_LINE_MAX_CHARS: usize = 160;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartFailureCause {
    NotInstalled,
    UnknownFlag,
    NotLoggedIn,
    ExitedEarly,
}

/// The single action the panel offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixAction {
    /// Opens the CLI's install page in the browser.
    OpenInstallPage(&'static str),
    /// Opens a tab that runs the CLI's own sign-in command.
    RunLogin(&'static str),
    /// Opens a shell tab so the CLI can be run by hand (`--help`, update).
    OpenShell,
    /// Starts a fresh session for the same CLI when a resumed session failed to start.
    StartNewSession(AgentCliPreset),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartFailure {
    pub preset: AgentCliPreset,
    pub cause: StartFailureCause,
    pub exit_code: Option<u32>,
    pub output_tail: Vec<String>,
    pub fix: FixAction,
}

pub fn install_url(preset: AgentCliPreset) -> &'static str {
    match preset {
        AgentCliPreset::Claude => "https://docs.claude.com/en/docs/claude-code/setup",
        AgentCliPreset::Codex => "https://github.com/openai/codex",
        AgentCliPreset::Antigravity => "https://antigravity.google/",
        AgentCliPreset::Grok => "https://x.ai/cli",
        AgentCliPreset::Gemini => "https://github.com/google-gemini/gemini-cli",
        AgentCliPreset::OpenCode => "https://opencode.ai",
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => "https://github.com/jangtrinh/design-os-studio",
    }
}

pub fn login_command(preset: AgentCliPreset) -> &'static str {
    match preset {
        AgentCliPreset::Claude => "claude auth login",
        AgentCliPreset::Codex => "codex login",
        AgentCliPreset::Grok => "grok login",
        // `agy` and `gemini` have no dedicated login subcommand; their first screen is the sign-in picker.
        AgentCliPreset::Antigravity => "agy",
        AgentCliPreset::Gemini => "gemini",
        AgentCliPreset::OpenCode => "opencode auth login",
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => "",
    }
}

/// A tab counts as a failed start when its CLI exited within [`START_FAILURE_WINDOW`] of being
/// spawned and did not exit cleanly: a user who quits at once with `/exit` did not hit a failure.
pub fn counts_as_failed_start(since_spawn: Duration, exit_code: Option<u32>) -> bool {
    since_spawn <= START_FAILURE_WINDOW && exit_code != Some(0)
}

/// Non-empty output lines: escape sequences stripped, a carriage return keeps only the text after it
/// (progress redraws), long lines cut with an ellipsis.
fn clean_lines(raw: &str) -> Vec<String> {
    // A carriage return redraws the line, so only the text after the last one is visible: apply it
    // before the escape sequences are stripped (stripping drops the `\r`).
    raw.split('\n')
        .map(|l| strip_ansi_codes(l.rsplit('\r').find(|seg| !strip_ansi_codes(seg).trim().is_empty()).unwrap_or("")).trim_end().to_string())
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if l.chars().count() > TAIL_LINE_MAX_CHARS {
                let cut: String = l.chars().take(TAIL_LINE_MAX_CHARS - 1).collect();
                format!("{cut}…")
            } else {
                l
            }
        })
        .collect()
}

/// The last [`TAIL_LINES`] lines. When the line that explains the failure (the first one containing
/// any of `cause_needles`) scrolled out of them, as the error above a long usage dump does, it is
/// kept on top, followed by an ellipsis line.
pub fn output_excerpt(raw: &str, cause_needles: &[&str]) -> Vec<String> {
    let lines = clean_lines(raw);
    if lines.len() <= TAIL_LINES {
        return lines;
    }
    let cause_idx = lines.iter().position(|l| {
        let lower = l.to_ascii_lowercase();
        cause_needles.iter().any(|n| lower.contains(n))
    });
    match cause_idx {
        Some(i) if i < lines.len() - (TAIL_LINES - 2) => {
            let mut out = vec![lines[i].clone(), "…".to_string()];
            out.extend(lines[lines.len() - (TAIL_LINES - 2)..].iter().cloned());
            out
        }
        _ => lines[lines.len() - TAIL_LINES..].to_vec(),
    }
}

/// Classifies why `preset` exited right after it was opened. `installed` is whether the CLI's
/// binary resolves on `PATH`; `raw_output` is everything the PTY printed.
pub fn diagnose(preset: AgentCliPreset, installed: bool, exit_code: Option<u32>, raw_output: &str) -> StartFailure {
    let lower = strip_ansi_codes(raw_output).to_ascii_lowercase();
    const NOT_INSTALLED: &[&str] = &["command not found", ": not found", "is not recognized as an internal"];
    const UNKNOWN_FLAG: &[&str] = &[
        "unexpected argument",
        "flags provided but not defined",
        "unknown option",
        "unrecognized option",
        "unknown flag",
        "invalid option",
        "unknown argument",
        "unknown arguments",
        "invalid values",
    ];
    const NOT_LOGGED_IN: &[&str] = &[
        "not logged in",
        "please log in",
        "please login",
        "/login",
        "authentication required",
        "not authenticated",
        "unauthorized",
        "sign in",
        "invalid api key",
        "please set an auth method",
        "gemini_api_key",
        "no provider configured",
        "opencode auth",
    ];
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    let (cause, needles) = if has(UNKNOWN_FLAG) {
        (StartFailureCause::UnknownFlag, UNKNOWN_FLAG)
    } else if has(NOT_LOGGED_IN) {
        (StartFailureCause::NotLoggedIn, NOT_LOGGED_IN)
    } else if has(NOT_INSTALLED) || !installed {
        (StartFailureCause::NotInstalled, NOT_INSTALLED)
    } else {
        (StartFailureCause::ExitedEarly, &[][..])
    };
    let fix = match cause {
        StartFailureCause::NotInstalled => FixAction::OpenInstallPage(install_url(preset)),
        StartFailureCause::NotLoggedIn => FixAction::RunLogin(login_command(preset)),
        StartFailureCause::UnknownFlag | StartFailureCause::ExitedEarly => FixAction::OpenShell,
    };
    StartFailure { preset, cause, exit_code, output_tail: output_excerpt(raw_output, needles), fix }
}

impl StartFailure {
    fn cli(&self) -> &'static str {
        self.preset.binary_name()
    }

    /// Short statement of what went wrong.
    pub fn headline(&self) -> String {
        match self.cause {
            StartFailureCause::NotInstalled => format!("{} is not installed", self.preset.button_label()),
            StartFailureCause::UnknownFlag => format!("{} rejected a launch flag", self.cli()),
            StartFailureCause::NotLoggedIn => format!("{} is not signed in", self.preset.button_label()),
            StartFailureCause::ExitedEarly => match self.exit_code {
                Some(code) => format!("{} exited with code {code}", self.cli()),
                None => format!("{} exited right after it started", self.cli()),
            },
        }
    }

    /// One sentence on what to do about it.
    pub fn explanation(&self) -> String {
        match self.cause {
            StartFailureCause::NotInstalled => format!("Studio could not find `{}` on your PATH. Install it, then open the tab again.", self.cli()),
            StartFailureCause::UnknownFlag => {
                format!("This `{}` version does not accept an option Studio passed. Run it by hand to see what it supports.", self.cli())
            }
            StartFailureCause::NotLoggedIn => "Sign in once, then open the tab again.".to_string(),
            StartFailureCause::ExitedEarly => "The CLI stopped on its own. The last lines it printed are below.".to_string(),
        }
    }

    pub fn fix_label(&self) -> String {
        match &self.fix {
            FixAction::OpenInstallPage(_) => format!("Install {}", self.preset.button_label()),
            FixAction::RunLogin(_) => "Run login".to_string(),
            FixAction::OpenShell => "Open a shell".to_string(),
            FixAction::StartNewSession(_) => "Start a new session".to_string(),
        }
    }
}

const PAD: f32 = theme::INSET_PANEL;
const BTN_H: f32 = theme::CONTROL_STANDARD;
const CODE_PAD: f32 = theme::INSET_CONTROL;
const BTN_MIN_W: f32 = 96.0;

/// Paints the failure card over `rect` and returns the fix the user clicked, if any.
pub fn paint(ui: &mut Ui, rect: Rect, failure: &StartFailure, surface_id: &'static str) -> Option<FixAction> {
    let p: Painter = ui.painter().clone();
    p.rect_filled(rect, 0.0, theme::BG);

    let card_w = (rect.width() - theme::SPACE_SECTION).clamp(160.0, 460.0);
    let inner_w = card_w - 2.0 * PAD;
    let card_left = rect.center().x - card_w / 2.0;

    let title = p.layout(format!("{} failed to start", failure.preset.button_label()), theme::font_large_strong(), theme::TEXT, inner_w - theme::SPACE_BLOCK);
    let headline = p.layout(failure.headline(), theme::font_body_medium(), theme::DANGER, inner_w);
    let explanation = p.layout(failure.explanation(), theme::font_body(), theme::TEXT_SECONDARY, inner_w);
    let caption = p.layout_no_wrap("Last output".to_string(), theme::font_caption_medium(), theme::TEXT_SECONDARY);
    let code_lines: Vec<_> =
        failure.output_tail.iter().map(|l| p.layout(l.clone(), theme::font_mono(), theme::TEXT_SECONDARY, inner_w - 2.0 * CODE_PAD)).collect();
    let code_h = if code_lines.is_empty() { 0.0 } else { code_lines.iter().map(|g| g.size().y).sum::<f32>() + 2.0 * CODE_PAD };
    let fix_label = failure.fix_label();
    let fix_galley = p.layout_no_wrap(fix_label, theme::font_body_medium(), theme::TEXT_BRAND);
    let btn_w = (fix_galley.size().x + theme::SPACE_BLOCK * 2.0).clamp(BTN_MIN_W, inner_w);

    let gap = theme::SPACE_INLINE;
    let mut h = PAD + title.size().y + gap + headline.size().y + theme::SPACE_ROW + explanation.size().y;
    if code_h > 0.0 {
        h += theme::SPACE_BLOCK + caption.size().y + theme::SPACE_ROW + code_h;
    }
    h += theme::SPACE_BLOCK + BTN_H + PAD;
    let card = Rect::from_min_size(Pos2::new(card_left, rect.top() + theme::SPACE_BLOCK), Vec2::new(card_w, h));

    p.rect_filled(card, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    p.rect_stroke(card, theme::cr(theme::RADIUS_FRAME), egui::Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);

    let x = card.left() + PAD;
    let mut y = card.top() + PAD;
    p.circle_filled(Pos2::new(x + theme::SPACE_ROW, y + title.size().y / 2.0), theme::SPACE_ROW, theme::DANGER);
    let title_h = title.size().y;
    p.galley(Pos2::new(x + theme::SPACE_BLOCK, y), title, theme::TEXT);
    y += title_h + gap;
    let headline_h = headline.size().y;
    p.galley(Pos2::new(x, y), headline, theme::DANGER);
    y += headline_h + theme::SPACE_ROW;
    let explanation_h = explanation.size().y;
    p.galley(Pos2::new(x, y), explanation, theme::TEXT_SECONDARY);
    y += explanation_h;
    if code_h > 0.0 {
        y += theme::SPACE_BLOCK;
        let caption_h = caption.size().y;
        p.galley(Pos2::new(x, y), caption, theme::TEXT_SECONDARY);
        y += caption_h + theme::SPACE_ROW;
        let block = Rect::from_min_size(Pos2::new(x, y), Vec2::new(inner_w, code_h));
        p.rect_filled(block, theme::cr(theme::RADIUS_CONTROL), theme::BG);
        p.rect_stroke(block, theme::cr(theme::RADIUS_CONTROL), egui::Stroke::new(1.0, theme::BORDER_SUBTLE), egui::StrokeKind::Inside);
        let mut cy = block.top() + CODE_PAD;
        for g in code_lines {
            let gh = g.size().y;
            p.galley(Pos2::new(block.left() + CODE_PAD, cy), g, theme::TEXT_SECONDARY);
            cy += gh;
        }
        y += code_h;
    }
    y += theme::SPACE_BLOCK;

    let btn = Rect::from_min_size(Pos2::new(x, y), Vec2::new(btn_w, BTN_H));
    let name = format!("agents-start-failure-fix-{surface_id}");
    widgets::record(ui, &name, btn);
    let resp = ui.interact(btn, Id::new((surface_id, "agents-start-failure-fix")), Sense::click());
    // Brand text on the brand-tinted surface (8.1:1), not white on `BG_BRAND` (3.5:1).
    let bg = if resp.hovered() { theme::BG_SELECTED_HOVER } else { theme::BG_SELECTED };
    p.rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), bg);
    p.rect_stroke(btn, theme::cr(theme::RADIUS_CONTROL), egui::Stroke::new(1.0, theme::BORDER_SELECTED), egui::StrokeKind::Inside);
    p.text(btn.center(), Align2::CENTER_CENTER, failure.fix_label(), theme::font_body_medium(), theme::TEXT_BRAND);
    resp.clicked().then(|| failure.fix.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    fn d(preset: AgentCliPreset, installed: bool, code: Option<u32>, out: &str) -> StartFailure {
        diagnose(preset, installed, code, out)
    }

    #[test]
    fn unknown_flag_output_of_the_real_clis_is_recognised() {
        let agy = d(AgentCliPreset::Antigravity, true, Some(2), "flags provided but not defined: -mcp-config\nUsage of agy:\n  --add-dir  Add a directory\n");
        assert_eq!(agy.cause, StartFailureCause::UnknownFlag);
        assert_eq!(agy.fix, FixAction::OpenShell);
        let grok = d(
            AgentCliPreset::Grok,
            true,
            Some(2),
            "error: unexpected argument '--mcp-config' found\n\n  tip: to pass '--mcp-config' as a value, use '-- --mcp-config'\n",
        );
        assert_eq!(grok.cause, StartFailureCause::UnknownFlag);
        assert_eq!(grok.headline(), "grok rejected a launch flag");
    }

    #[test]
    fn missing_binary_offers_the_install_page() {
        let by_probe = d(AgentCliPreset::Claude, false, Some(127), "");
        assert_eq!(by_probe.cause, StartFailureCause::NotInstalled);
        assert_eq!(by_probe.fix, FixAction::OpenInstallPage("https://docs.claude.com/en/docs/claude-code/setup"));
        assert_eq!(by_probe.fix_label(), "Install Claude CLI");
        let by_output = d(AgentCliPreset::Codex, true, Some(127), "zsh:1: command not found: codex\n");
        assert_eq!(by_output.cause, StartFailureCause::NotInstalled);
        assert_eq!(by_output.fix, FixAction::OpenInstallPage("https://github.com/openai/codex"));
        assert!(by_output.explanation().contains("`codex`"));
    }

    #[test]
    fn signed_out_cli_offers_its_own_login() {
        let claude = d(AgentCliPreset::Claude, true, Some(1), "Invalid API key · Please run /login\n");
        assert_eq!(claude.cause, StartFailureCause::NotLoggedIn);
        assert_eq!(claude.fix, FixAction::RunLogin("claude auth login"));
        assert_eq!(claude.fix_label(), "Run login");
        assert_eq!(d(AgentCliPreset::Codex, true, Some(1), "Not logged in\n").fix, FixAction::RunLogin("codex login"));
        assert_eq!(d(AgentCliPreset::Grok, true, Some(1), "error: unauthorized (401)\n").fix, FixAction::RunLogin("grok login"));
    }

    #[test]
    fn any_other_early_exit_keeps_the_exit_code_and_offers_a_shell() {
        let f = d(AgentCliPreset::Grok, true, Some(3), "thread 'main' panicked at src/x.rs:1\n");
        assert_eq!(f.cause, StartFailureCause::ExitedEarly);
        assert_eq!(f.headline(), "grok exited with code 3");
        assert_eq!(f.fix, FixAction::OpenShell);
        assert_eq!(d(AgentCliPreset::Grok, true, None, "").headline(), "grok exited right after it started");
    }

    #[test]
    fn signed_out_output_beats_a_failed_install_probe() {
        let f = d(AgentCliPreset::Grok, false, Some(1), "Error: unauthorized\nrun grok login");
        assert_eq!(f.cause, StartFailureCause::NotLoggedIn);
        assert_eq!(f.fix, FixAction::RunLogin("grok login"));
    }

    #[test]
    fn unknown_flag_output_beats_a_failed_install_probe() {
        let f = d(AgentCliPreset::Antigravity, false, Some(2), "flags provided but not defined: -mcp-config");
        assert_eq!(f.cause, StartFailureCause::UnknownFlag);
        assert_eq!(f.fix, FixAction::OpenShell);
    }

    #[test]
    fn silent_exit_of_an_unfound_binary_is_not_installed() {
        let f = d(AgentCliPreset::Codex, false, Some(127), "");
        assert_eq!(f.cause, StartFailureCause::NotInstalled);
    }

    #[test]
    fn command_not_found_output_is_not_installed() {
        let f = d(AgentCliPreset::Claude, true, Some(127), "zsh: command not found: claude");
        assert_eq!(f.cause, StartFailureCause::NotInstalled);
    }

    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    /// `fg` (with its alpha) composited over the opaque `bg`, in sRGB like the renderer.
    fn over(fg: Color32, bg: Color32) -> Color32 {
        let [r, g, b, a] = fg.to_srgba_unmultiplied();
        let mix = |f: u8, back: u8| ((f as f32 * a as f32 + back as f32 * (255.0 - a as f32)) / 255.0).round() as u8;
        Color32::from_rgb(mix(r, bg.r()), mix(g, bg.g()), mix(b, bg.b()))
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn panel_text_meets_the_contrast_floor_on_its_surfaces() {
        let on_card = [("headline", theme::DANGER), ("explanation", over(theme::TEXT_SECONDARY, theme::BG_SECONDARY)), ("title", theme::TEXT)];
        for (what, fg) in on_card {
            let ratio = contrast(fg, theme::BG_SECONDARY);
            assert!(ratio >= 4.5, "{what} on the card is {ratio:.2}:1");
        }
        for state in [theme::BG_SELECTED, theme::BG_SELECTED_HOVER] {
            let ratio = contrast(theme::TEXT_BRAND, state);
            assert!(ratio >= 4.5, "fix button text is {ratio:.2}:1 on {state:?}");
        }
        let ratio = contrast(over(theme::TEXT_SECONDARY, theme::BG), theme::BG);
        assert!(ratio >= 4.5, "output block text is {ratio:.2}:1");
    }

    #[test]
    fn only_a_quick_unclean_exit_counts_as_a_failed_start() {
        let s = Duration::from_secs;
        assert!(counts_as_failed_start(s(1), Some(1)));
        assert!(counts_as_failed_start(s(5), Some(127)));
        assert!(counts_as_failed_start(s(2), None), "unknown exit status still failed fast");
        assert!(!counts_as_failed_start(s(6), Some(1)), "past the window it is an ordinary exit");
        assert!(!counts_as_failed_start(s(1), Some(0)), "a clean quit is not a failure");
    }

    #[test]
    fn the_error_line_stays_visible_above_a_long_usage_dump() {
        let mut raw = String::from("flags provided but not defined: -mcp-config\nUsage of agy:\n");
        for i in 1..=30 {
            raw.push_str(&format!("  --flag-{i}  does something\n"));
        }
        let f = d(AgentCliPreset::Antigravity, true, Some(2), &raw);
        assert_eq!(f.output_tail.len(), TAIL_LINES);
        assert_eq!(f.output_tail[0], "flags provided but not defined: -mcp-config");
        assert_eq!(f.output_tail[1], "…");
        assert_eq!(f.output_tail.last().map(String::as_str), Some("  --flag-30  does something"));
        // When the cause line is already within the tail nothing is added.
        let short = d(AgentCliPreset::Grok, true, Some(2), "a\nb\nc\nd\nerror: unexpected argument '--x' found\n");
        assert_eq!(short.output_tail.len(), 5);
        assert!(short.output_tail.iter().all(|l| l != "…"));
    }

    #[test]
    fn tail_is_clean_short_and_capped() {
        let raw = "\x1b[31mred line\x1b[0m\r\n\r\nprogress 10%\rprogress 90%\r\n";
        assert_eq!(output_excerpt(raw, &[]), vec!["red line".to_string(), "progress 90%".to_string()]);
        let many: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        let tail = output_excerpt(&many, &[]);
        assert_eq!(tail.len(), TAIL_LINES);
        assert_eq!(tail.last().map(String::as_str), Some("line 20"));
        let long = "x".repeat(400);
        let cut = output_excerpt(&long, &[]);
        assert_eq!(cut[0].chars().count(), 160);
        assert!(cut[0].ends_with('…'));
    }

    fn run_frame(ctx: &egui::Context, events: Vec<egui::Event>, rect: Rect, failure: &StartFailure, clicked: &mut Option<FixAction>) -> egui::FullOutput {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0))), events, ..Default::default() };
        ctx.run_ui(raw, |ui| {
            if let Some(a) = paint(ui, rect, failure, "start-failure-test") {
                *clicked = Some(a);
            }
        })
    }

    #[test]
    fn panel_paints_cause_output_and_one_working_fix_button() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let failure = d(AgentCliPreset::Antigravity, true, Some(2), "flags provided but not defined: -mcp-config\nUsage of agy:\n");
        let rect = Rect::from_min_size(Pos2::new(48.0, 36.0), Vec2::new(300.0, 500.0));

        crate::lint::take_icons();
        let mut clicked = None;
        let out = run_frame(&ctx, vec![], rect, &failure, &mut clicked);
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        let all: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        for expected in ["Antigravity failed to start", "agy rejected a launch flag", "Last output", "Open a shell"] {
            assert!(all.contains(&expected), "{expected:?} not painted: {all:?}");
        }
        assert!(all.iter().any(|t| t.contains("flags provided but not defined")), "stderr tail not painted: {all:?}");
        let findings = crate::lint::check_text(&texts, &boxes);
        assert!(findings.is_empty(), "failure panel text lint: {findings:?}");
        assert!(clicked.is_none());

        let btn: Rect =
            ctx.data(|d| d.get_temp(Id::new(("test-rect", "agents-start-failure-fix-start-failure-test".to_string())))).expect("fix button rect recorded");
        assert!(rect.contains_rect(btn), "fix button {btn:?} outside the panel {rect:?}");
        assert!(btn.height() >= 28.0 && btn.width() >= 24.0, "pointer floor: {btn:?}");

        let at = btn.center();
        run_frame(&ctx, vec![egui::Event::PointerMoved(at)], rect, &failure, &mut clicked);
        for pressed in [true, false] {
            let ev = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            run_frame(&ctx, vec![ev], rect, &failure, &mut clicked);
        }
        run_frame(&ctx, vec![], rect, &failure, &mut clicked);
        assert_eq!(clicked, Some(FixAction::OpenShell));
    }

    #[test]
    fn failure_classification_for_the_new_clis() {
        // 1. NotInstalled -> FixAction::OpenInstallPage(url)
        let gemini_missing = d(AgentCliPreset::Gemini, false, Some(127), "zsh: command not found: gemini\n");
        assert_eq!(gemini_missing.cause, StartFailureCause::NotInstalled);
        assert_eq!(gemini_missing.fix, FixAction::OpenInstallPage("https://github.com/google-gemini/gemini-cli"));
        assert_eq!(gemini_missing.headline(), "Gemini CLI is not installed");

        let opencode_missing = d(AgentCliPreset::OpenCode, false, Some(127), "zsh: command not found: opencode\n");
        assert_eq!(opencode_missing.cause, StartFailureCause::NotInstalled);
        assert_eq!(opencode_missing.fix, FixAction::OpenInstallPage("https://opencode.ai"));
        assert_eq!(opencode_missing.headline(), "OpenCode is not installed");

        // 2. UnknownFlag -> FixAction::OpenShell
        let gemini_flag = d(AgentCliPreset::Gemini, true, Some(1), "Unknown arguments: mcp-config, dangerously-skip-permissions\n");
        assert_eq!(gemini_flag.cause, StartFailureCause::UnknownFlag);
        assert_eq!(gemini_flag.fix, FixAction::OpenShell);
        assert_eq!(gemini_flag.headline(), "gemini rejected a launch flag");

        let opencode_flag = d(AgentCliPreset::OpenCode, true, Some(1), "Unknown argument: mcp-config\n");
        assert_eq!(opencode_flag.cause, StartFailureCause::UnknownFlag);
        assert_eq!(opencode_flag.fix, FixAction::OpenShell);
        assert_eq!(opencode_flag.headline(), "opencode rejected a launch flag");

        // 3. NotLoggedIn -> FixAction::RunLogin(cmd)
        let gemini_auth = d(AgentCliPreset::Gemini, true, Some(1), "Please set an Auth method or set GEMINI_API_KEY before continuing.\n");
        assert_eq!(gemini_auth.cause, StartFailureCause::NotLoggedIn);
        assert_eq!(gemini_auth.fix, FixAction::RunLogin("gemini"));
        assert_eq!(gemini_auth.headline(), "Gemini CLI is not signed in");

        let opencode_auth = d(AgentCliPreset::OpenCode, true, Some(1), "Error: No provider configured. Run `opencode auth login` to authenticate.\n");
        assert_eq!(opencode_auth.cause, StartFailureCause::NotLoggedIn);
        assert_eq!(opencode_auth.fix, FixAction::RunLogin("opencode auth login"));
        assert_eq!(opencode_auth.headline(), "OpenCode is not signed in");

        // 4. ExitedEarly -> FixAction::OpenShell
        let gemini_crash = d(AgentCliPreset::Gemini, true, Some(1), "Unhandled rejection in worker\n");
        assert_eq!(gemini_crash.cause, StartFailureCause::ExitedEarly);
        assert_eq!(gemini_crash.fix, FixAction::OpenShell);

        let opencode_crash = d(AgentCliPreset::OpenCode, true, Some(1), "panic: runtime error\n");
        assert_eq!(opencode_crash.cause, StartFailureCause::ExitedEarly);
        assert_eq!(opencode_crash.fix, FixAction::OpenShell);
    }
}
