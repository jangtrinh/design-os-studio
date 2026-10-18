//! Command line: `designos [file] [--screenshot out.png] [--size WxH] [--select name|none] [--menu name]`.

use std::path::PathBuf;

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub file: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    /// Device pixel ratio of `--screenshot` (`--scale 2` writes a PNG twice the `--size`, with crisp UI and text).
    pub scale: Option<f32>,
    /// Select the first layer with this name after loading (handy for screenshots).
    pub select: Option<String>,
    /// Open a menu after loading: `main`, `main/File`, `shape`, `palette` (for screenshots).
    pub menu: Option<String>,
    /// Built-in test document instead of the demo (`showcase`: the Marlow product-design file the README screenshots use; `proto-vars`: a shop prototype with variables; `no-selection`: the Figma page-level reference;
    /// `variables`; `comments`: Figma comments imported, one thread open in comment mode;
    /// `board`: a sample whiteboard; `board-empty`: an empty one).
    pub fixture: Option<String>,
    /// Right panel tab to show: `design` or `prototype`.
    pub tab: Option<String>,
    /// Open the Present view on start.
    pub present: bool,
    /// Start presentation in native fullscreen.
    pub fullscreen: bool,
    /// Open a panel on start: `variables` (full view) or `variables-floating`.
    pub panel: Option<String>,
    /// Initial zoom in percent (`--zoom 800`), centred on the selection or the page.
    pub zoom: Option<f32>,
    /// Show the rulers.
    pub rulers: bool,
    /// Draw the canvas with the GPU renderer (`--screenshot` otherwise always uses the
    /// deterministic CPU renderer; normal launches use the GPU unless it is turned off).
    pub gpu: bool,
    /// Force the pixel grid on (it appears from 800% zoom).
    pub grid: bool,
    /// Enter text editing on the first text layer with this name (select it with `--select-text`).
    pub edit_text: Option<String>,
    /// Character range selected while editing: `START-END` (in chars); default is everything.
    pub select_text: Option<(usize, usize)>,
    /// Add a layout grid to the selected frame: `grid`, `columns` or `rows`.
    pub layout_grid: Option<String>,
    /// Show the projects home screen (default when launched without a file).
    pub home: bool,
    /// Library root directory (`--library DIR`, default `$DESIGNOS_LIBRARY` or the user data dir).
    pub library: Option<PathBuf>,
    /// Open this library file id on start (used by "Open in new window").
    pub open_id: Option<String>,
    /// Open the Find and replace panel with this query (`--find TEXT`).
    pub find: Option<String>,
    /// Replacement text for `--find`; also shows the replace row.
    pub replace_with: Option<String>,
    /// Open Dev mode with this code language: `css`, `tailwind`, `swiftui`, `uikit`, `compose`, `android-xml` or `react`.
    pub dev: Option<String>,
    /// With `--dev`: mark the selected layer Ready for dev and give it a demo annotation and link.
    pub dev_demo: bool,
    /// Write the showcase sample files into this library directory and exit (screenshot aid).
    pub seed_library: Option<PathBuf>,
    /// Headless agent CLI: dump document/pages/screens summary as JSON and exit.
    pub inspect_json: bool,
    /// Headless agent CLI: dump resolved canvas comment threads and their target nodes/frames as JSON and exit.
    pub comments_json: bool,
    /// Headless agent CLI: pin a new comment thread (anchored to `--select` or first page) and exit.
    pub add_comment: Option<String>,
    /// Headless agent CLI: reply to an existing comment thread id and exit.
    pub reply_comment: Option<String>,
    /// Headless agent CLI: reply message text used with `--reply-comment`.
    pub message: Option<String>,
    /// Headless agent CLI: mark a comment thread id resolved and exit.
    pub resolve_comment: Option<String>,
    /// Headless agent CLI: with `--reply-comment`, also mark the thread resolved.
    pub resolve: bool,
    /// Headless agent CLI: apply a declarative `NodeSpec` JSON file into the document and save in-place.
    pub apply_spec: Option<PathBuf>,
    /// Headless agent CLI: export `--select` (or first screen/page) to a declarative `NodeSpec` JSON file.
    pub export_spec: Option<PathBuf>,
    /// Headless agent CLI: export the document to a native `.fig` binary file (`fig-kiwi` + ZIP).
    pub export_fig: Option<PathBuf>,
    /// Headless CLI: import variables from a file (`.json` or `.zip`) into the document (`E1-9` §5).
    pub import_variables: Option<PathBuf>,
    /// Headless CLI: export variables from the document to a directory or file (`E1-9` §5).
    pub export_variables: Option<PathBuf>,
    /// Variable format for `--import-variables` / `--export-variables`: `auto`, `figma-rest`, `figma-native`, or `dtcg`.
    pub format: Option<String>,
    /// With `--panel micro-<slug>:seq:<N>`: drop the top shortcut caption bar from recorded frames
    /// (pointer, drag ring and live metric pill stay), for footage that carries its own titles.
    pub no_caption: bool,
    /// With `--screenshot`: never paint transient toasts (e.g. "Opened <file>") into the capture.
    pub no_toast: bool,
    pub help: bool,
}

impl Options {
    /// Whether this invocation runs a headless Agent CLI command and exits without opening a window.
    pub fn is_agent_cli(&self) -> bool {
        self.inspect_json
            || self.comments_json
            || self.add_comment.is_some()
            || self.reply_comment.is_some()
            || self.resolve_comment.is_some()
            || self.apply_spec.is_some()
            || self.export_spec.is_some()
            || self.export_fig.is_some()
            || self.import_variables.is_some()
            || self.export_variables.is_some()
    }

    /// Whether a launch with these options starts on the home screen: explicitly
    /// with `--home`, or interactively with nothing else to open.
    pub fn wants_home(&self) -> bool {
        !self.is_agent_cli() && self.open_id.is_none() && (self.home || (self.file.is_none() && self.screenshot.is_none() && self.fixture.is_none()))
    }
}

/// Values of `--panel`: `agents`, `agents-chat`, `specs`, `assets`, `variables` / `variables-floating` open the local views,
/// `libraries` the Libraries modal; the others are inspector popups opened on the selection for screenshots.
pub const PANELS: [&str; 47] = [
    "agents",
    "agents-chat",
    "agents-start-failure",
    "specs",
    "terminal",
    "cmdk",
    "check-designs",
    "check-designs-fixed",
    "assets",
    "plugins",
    "plugin-install",
    "interaction",
    "present-variables",
    "variables",
    "variables-floating",
    "libraries",
    "picker",
    "picker-page",
    "fill-d9",
    "picker-gradient",
    "effects-menu",
    "effect-styles",
    "grid-styles",
    "edit-style-color",
    "edit-style-text",
    "edit-style-effect",
    "edit-style-grid",
    "stroke-settings",
    "type-details",
    "effect-noise",
    "effect-texture",
    "effect-glass",
    "version-history",
    "dev-annotate",
    "dev-compare",
    "micro-autolayout",
    "micro-css-grid",
    "micro-smart-selection",
    "micro-shape-controls",
    "micro-vector-edit",
    "micro-layer-picker",
    "micro-dev-boxmodel",
    "micro-scale-tool",
    "micro-inspector-math",
    "micro-selection-corner",
    "micro-typography-multiedit",
    "micro-components-variables",
];

pub fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let (w, h) = (w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?);
    (w >= 200 && h >= 200 && w <= 8192 && h <= 8192).then_some((w, h))
}

/// Parses a screenshot scale like `2` or `1.5` (device pixel ratio, 1..=4).
pub fn parse_scale(s: &str) -> Option<f32> {
    let v: f32 = s.trim().trim_end_matches(['x', 'X']).trim().parse().ok()?;
    (v.is_finite() && (1.0..=4.0).contains(&v)).then_some(v)
}

/// Parses a zoom like `800`, `800%` or `12.5%` (percent, 1..=25600).
pub fn parse_zoom(s: &str) -> Option<f32> {
    let v: f32 = s.trim().trim_end_matches('%').trim().parse().ok()?;
    (v.is_finite() && (1.0..=25600.0).contains(&v)).then_some(v)
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
    let (a, b) = s.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut o = Options::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--screenshot" => o.screenshot = Some(it.next().ok_or("--screenshot needs an output path")?.into()),
            "--size" => {
                let v = it.next().ok_or("--size needs WxH")?;
                o.size = Some(parse_size(&v).ok_or_else(|| format!("invalid size '{v}' (expected WxH, e.g. 1600x1088)"))?);
            }
            "--scale" => {
                let v = it.next().ok_or("--scale needs a factor (e.g. 2)")?;
                o.scale = Some(parse_scale(&v).ok_or_else(|| format!("invalid scale '{v}' (expected 1..4, e.g. 2)"))?);
            }
            "--select" => o.select = Some(it.next().ok_or("--select needs a layer name")?),
            "--menu" => o.menu = Some(it.next().ok_or("--menu needs a name (main, main/File, shape, palette)")?),
            "--fixture" => o.fixture = Some(it.next().ok_or("--fixture needs a name (no-selection)")?),
            "--tab" => {
                let t = it.next().ok_or("--tab needs design or prototype")?;
                if !matches!(t.as_str(), "design" | "prototype") {
                    return Err(format!("unknown tab '{t}' (expected design or prototype)"));
                }
                o.tab = Some(t);
            }
            "--present" => o.present = true,
            "--fullscreen" => {
                o.present = true;
                o.fullscreen = true;
            }
            "--agents-chat" => {
                o.panel = Some("agents-chat".into());
            }
            "--panel" => {
                let t = it.next().ok_or("--panel needs a name (see --help)")?;
                let base = t.split_once(':').map(|(b, _)| b).unwrap_or(t.as_str());
                if !PANELS.contains(&base) {
                    return Err(format!("unknown panel '{t}' (expected {})", PANELS.join(", ")));
                }
                o.panel = Some(t);
            }
            "--rulers" => o.rulers = true,
            "--no-caption" => o.no_caption = true,
            "--no-toast" => o.no_toast = true,
            "--gpu" => o.gpu = true,
            "--grid" => o.grid = true,
            "--zoom" => {
                let v = it.next().ok_or("--zoom needs a percentage (e.g. 800)")?;
                o.zoom = Some(parse_zoom(&v).ok_or_else(|| format!("invalid zoom '{v}' (expected 1..25600, e.g. 800 or 800%)"))?);
            }
            "--edit-text" => o.edit_text = Some(it.next().ok_or("--edit-text needs a text layer name")?),
            "--select-text" => {
                let v = it.next().ok_or("--select-text needs START-END")?;
                o.select_text = Some(parse_range(&v).ok_or_else(|| format!("invalid range '{v}' (expected START-END in characters)"))?);
            }
            "--layout-grid" => {
                let k = it.next().ok_or("--layout-grid needs grid, columns or rows")?;
                if !matches!(k.as_str(), "grid" | "columns" | "rows") {
                    return Err(format!("unknown layout grid '{k}' (expected grid, columns or rows)"));
                }
                o.layout_grid = Some(k);
            }
            "--home" => o.home = true,
            "--library" => o.library = Some(it.next().ok_or("--library needs a directory")?.into()),
            "--open-id" => o.open_id = Some(it.next().ok_or("--open-id needs a library file id")?),
            "--find" => o.find = Some(it.next().ok_or("--find needs the text to search for")?),
            "--replace-with" => o.replace_with = Some(it.next().ok_or("--replace-with needs the replacement text")?),
            "--dev" => {
                let l = it.next().ok_or("--dev needs css, tailwind, swiftui, uikit, compose, android-xml or react")?;
                if !matches!(l.as_str(), "css" | "tailwind" | "swiftui" | "uikit" | "compose" | "android-xml" | "react") {
                    return Err(format!("unknown language '{l}' (expected css, tailwind, swiftui, uikit, compose, android-xml or react)"));
                }
                o.dev = Some(l);
            }
            "--dev-demo" => o.dev_demo = true,
            "--seed-library" => o.seed_library = Some(it.next().ok_or("--seed-library needs a directory")?.into()),
            "--inspect-json" => o.inspect_json = true,
            "--comments-json" => o.comments_json = true,
            "--add-comment" => o.add_comment = Some(it.next().ok_or("--add-comment needs comment text")?),
            "--reply-comment" => o.reply_comment = Some(it.next().ok_or("--reply-comment needs a thread id")?),
            "--message" => o.message = Some(it.next().ok_or("--message needs reply text")?),
            "--resolve-comment" => o.resolve_comment = Some(it.next().ok_or("--resolve-comment needs a thread id")?),
            "--resolve" => o.resolve = true,
            "--apply-spec" => o.apply_spec = Some(it.next().ok_or("--apply-spec needs a spec JSON path")?.into()),
            "--export-spec" => o.export_spec = Some(it.next().ok_or("--export-spec needs an output JSON path")?.into()),
            "--export-fig" => o.export_fig = Some(it.next().ok_or("--export-fig needs an output .fig path")?.into()),
            "--import-variables" => o.import_variables = Some(it.next().ok_or("--import-variables needs a file path")?.into()),
            "--export-variables" => o.export_variables = Some(it.next().ok_or("--export-variables needs a directory or file path")?.into()),
            "--format" => {
                let f = it.next().ok_or("--format needs auto, figma-rest, figma-native or dtcg")?;
                if designos_fig::VariablesFormat::parse_label(&f).is_none() {
                    return Err(format!("unknown format '{f}' (expected auto, figma-rest, figma-native or dtcg)"));
                }
                o.format = Some(f);
            }
            "-h" | "--help" => o.help = true,
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => {
                if o.file.is_some() {
                    return Err("only one file can be opened".into());
                }
                o.file = Some(a.into());
            }
        }
    }
    Ok(o)
}

pub const USAGE: &str = "usage: designos [FILE] [--screenshot OUT.png] [--size WxH] [--scale FACTOR] [--select LAYER|none] [--menu main|main/File|shape|palette] [--panel agents-start-failure|specs|assets|plugins|interaction|present-variables|variables|variables-floating|libraries|picker|picker-page|fill-d9|picker-gradient|effects-menu|effect-styles|grid-styles|edit-style-color|edit-style-text|edit-style-effect|edit-style-grid|stroke-settings|type-details|effect-noise|effect-texture|effect-glass|version-history|dev-annotate|dev-compare]\n               [--zoom PERCENT] [--rulers] [--grid] [--gpu] [--home] [--library DIR] [--layout-grid grid|columns|rows] [--edit-text LAYER [--select-text START-END]]\n               [--inspect-json] [--comments-json] [--add-comment TEXT] [--reply-comment ID [--message TEXT] [--resolve]] [--resolve-comment ID] [--apply-spec SPEC.json] [--export-spec OUT.json] [--export-fig OUT.fig]\n  FILE  .fig, .json (Figma REST or native), .svg or .designos; without a file the projects home screen opens
  --scale    with --screenshot: device pixel ratio (2 writes a PNG twice --size, rendered crisp rather than upscaled)
  --home     show the home screen (also in --screenshot mode)
  --present  open presentation preview; --fullscreen starts it in native fullscreen (F11 toggles, Esc returns)
  --gpu      draw the canvas on the GPU; --screenshot uses the CPU renderer unless this is given
  --library  use DIR as the projects library instead of the default location
  --dev      open Dev mode with a code language (css|tailwind|swiftui|uikit|compose|android-xml|react); --dev-demo marks the selection Ready for dev with an annotation
  --no-caption  with --panel micro-<slug>:seq:<N>: record frames without the top shortcut caption bar
  --no-toast    with --screenshot: leave transient toasts ('Opened <file>') out of the capture
  --seed-library  write the showcase sample files (a project with three files) into DIR and exit
  --find     open the Find and replace panel searching for TEXT (--replace-with TEXT shows the replace row)
  --inspect-json / --comments-json / --apply-spec / --export-spec / --export-fig  headless Agent CLI inspection and mutation";

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Options, String> {
        parse(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn gpu_flag_is_off_unless_given() {
        assert!(!p(&["--screenshot", "x.png"]).unwrap().gpu);
        let o = p(&["--gpu", "--screenshot", "x.png"]).unwrap();
        assert!(o.gpu && o.screenshot.is_some());
    }

    #[test]
    fn parses_file_and_screenshot() {
        let o = p(&["a.fig", "--screenshot", "out/app.png", "--size", "1600x1088", "--select", "Card"]).unwrap();
        assert_eq!(o.file, Some("a.fig".into()));
        assert_eq!(o.screenshot, Some("out/app.png".into()));
        assert_eq!(o.size, Some((1600, 1088)));
        assert_eq!(o.select.as_deref(), Some("Card"));
    }

    #[test]
    fn parses_dev_options() {
        let o = p(&["--dev", "swiftui", "--dev-demo", "--select", "Card"]).unwrap();
        assert_eq!(o.dev.as_deref(), Some("swiftui"));
        assert!(o.dev_demo);
        assert!(p(&["--dev", "cobol"]).is_err());
        assert!(p(&["--dev"]).is_err());
        assert!(p(&["--panel", "dev-compare"]).is_ok());
    }

    #[test]
    fn parses_scale() {
        assert_eq!(p(&["--scale", "2"]).unwrap().scale, Some(2.0));
        assert_eq!(p(&["--scale", "1.5x"]).unwrap().scale, Some(1.5));
        assert_eq!(p(&[]).unwrap().scale, None);
        assert!(p(&["--scale", "0.5"]).is_err() && p(&["--scale", "9"]).is_err() && p(&["--scale"]).is_err());
    }

    #[test]
    fn parses_menu_and_select_none() {
        let o = p(&["--menu", "main/File", "--select", "none"]).unwrap();
        assert_eq!(o.menu.as_deref(), Some("main/File"));
        assert_eq!(o.select.as_deref(), Some("none"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(p(&["--size", "huge"]).is_err());
        assert!(p(&["--screenshot"]).is_err());
        assert!(p(&["--bogus"]).is_err());
        assert!(p(&["--menu"]).is_err());
        assert!(p(&["--tab", "nope"]).is_err());
        assert!(p(&["--tab"]).is_err());
        assert!(p(&["a", "b"]).is_err());
        assert_eq!(parse_size("1440X900"), Some((1440, 900)));
        assert_eq!(parse_size("10x10"), None);
    }

    #[test]
    fn parses_tab_and_present() {
        let o = p(&["--tab", "prototype", "--present"]).unwrap();
        assert_eq!(o.tab.as_deref(), Some("prototype"));
        assert!(o.present);
    }

    #[test]
    fn parses_panel() {
        assert_eq!(p(&["--panel", "agents"]).unwrap().panel.as_deref(), Some("agents"));
        assert_eq!(p(&["--panel", "agents-chat"]).unwrap().panel.as_deref(), Some("agents-chat"));
        assert_eq!(p(&["--agents-chat"]).unwrap().panel.as_deref(), Some("agents-chat"));
        assert_eq!(p(&["--panel", "specs"]).unwrap().panel.as_deref(), Some("specs"));
        assert_eq!(p(&["--panel", "assets"]).unwrap().panel.as_deref(), Some("assets"));
        assert_eq!(p(&["--panel", "variables"]).unwrap().panel.as_deref(), Some("variables"));
        assert_eq!(p(&["--panel", "variables-floating"]).unwrap().panel.as_deref(), Some("variables-floating"));
        assert!(p(&["--panel", "nope"]).is_err());
        assert!(p(&["--panel"]).is_err());
    }

    #[test]
    fn parses_canvas_aid_flags() {
        let o = p(&["--zoom", "800%", "--rulers", "--grid", "--layout-grid", "columns", "--edit-text", "Title", "--select-text", "2-7"]).unwrap();
        assert_eq!(o.zoom, Some(800.0));
        assert!(o.rulers && o.grid);
        assert_eq!(o.layout_grid.as_deref(), Some("columns"));
        assert_eq!(o.edit_text.as_deref(), Some("Title"));
        assert_eq!(o.select_text, Some((2, 7)));
        assert_eq!(parse_zoom("50"), Some(50.0));
        assert!(p(&["--zoom", "0"]).is_err() && p(&["--zoom", "x"]).is_err() && p(&["--zoom"]).is_err());
        assert!(!p(&["--panel", "micro-autolayout:seq:24"]).unwrap().no_caption, "recordings keep the caption bar by default");
        assert!(p(&["--panel", "micro-autolayout:seq:24", "--no-caption"]).unwrap().no_caption);
        assert!(!p(&["--screenshot", "x.png"]).unwrap().no_toast, "screenshots keep toasts by default");
        assert!(p(&["--screenshot", "x.png", "--no-toast"]).unwrap().no_toast);
        assert!(p(&["--layout-grid", "diagonal"]).is_err());
        assert!(p(&["--select-text", "5"]).is_err());
    }

    #[test]
    fn parses_home_and_library() {
        let o = p(&["--home", "--library", "/tmp/lib", "--screenshot", "h.png"]).unwrap();
        assert!(o.home && o.wants_home());
        assert_eq!(o.library, Some("/tmp/lib".into()));
        assert!(p(&["--library"]).is_err());
        assert_eq!(p(&["--open-id", "abc"]).unwrap().open_id.as_deref(), Some("abc"));
        assert!(p(&["--open-id"]).is_err());
    }

    #[test]
    fn home_is_the_default_only_for_a_bare_interactive_launch() {
        assert!(p(&[]).unwrap().wants_home());
        assert!(!p(&["a.designos"]).unwrap().wants_home(), "a file argument opens the editor");
        assert!(!p(&["--screenshot", "x.png"]).unwrap().wants_home(), "existing screenshot flows keep the editor");
        assert!(!p(&["--fixture", "no-selection"]).unwrap().wants_home());
        assert!(!p(&["--home", "--open-id", "x"]).unwrap().wants_home());
    }

    #[test]
    fn parses_find_and_replace_with() {
        let o = p(&["--find", "quarter", "--replace-with", "period"]).unwrap();
        assert_eq!((o.find.as_deref(), o.replace_with.as_deref()), (Some("quarter"), Some("period")));
        assert!(p(&["--find"]).is_err() && p(&["--replace-with"]).is_err());
    }

    #[test]
    fn parses_agent_cli_flags() {
        let o = p(&[
            "doc.designos",
            "--inspect-json",
            "--comments-json",
            "--apply-spec",
            "in.json",
            "--export-spec",
            "out.json",
            "--export-fig",
            "out.fig",
            "--reply-comment",
            "t-1",
            "--message",
            "Fixed spacing",
            "--resolve",
        ])
        .unwrap();
        assert!(o.is_agent_cli());
        assert!(!o.wants_home());
        assert!(o.inspect_json && o.comments_json && o.resolve);
        assert_eq!(o.apply_spec, Some("in.json".into()));
        assert_eq!(o.export_spec, Some("out.json".into()));
        assert_eq!(o.export_fig, Some("out.fig".into()));
        assert_eq!(o.reply_comment.as_deref(), Some("t-1"));
        assert_eq!(o.message.as_deref(), Some("Fixed spacing"));
    }

    #[test]
    fn empty_is_default() {
        assert_eq!(p(&[]).unwrap(), Options::default());
    }
}
