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
    /// Open Dev mode with this code language: `css`, `tailwind`, `swiftui`, `uikit`, `compose`, `android-xml`, `react` or `html`.
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
    /// With `--export-fig`: replace the output file when it already exists. Without it an existing file is refused. Only a
    /// regular file is ever replaced, never a symbolic link or a directory.
    pub overwrite: bool,
    /// Headless agent CLI: generate code for `--select` (or first screen/page) in the given target language and print to stdout.
    pub codegen: Option<String>,
    /// Import a `.webcapture` / `.capture.json` file into the document (`--import-capture <path>`).
    pub import_capture: Option<PathBuf>,
    /// Output `.designos` / `.fig` path for `--import-capture <path> --out <out.designos>` (`T8.27` §7).
    pub out: Option<PathBuf>,
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
    /// `--version` / `-V`: print the release version and exit.
    pub version: bool,
}

impl Options {
    /// Whether this run may create the file argument instead of reading it: the three agent commands that write a
    /// new document into a path that is not there yet.
    pub fn creates_missing_file(&self) -> bool {
        self.apply_spec.is_some() || self.import_capture.is_some() || self.import_variables.is_some()
    }

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
            || self.codegen.is_some()
            || (self.import_capture.is_some() && self.screenshot.is_none() && (self.out.is_some() || self.file.is_some() || self.open_id.is_some()))
            || self.import_variables.is_some()
            || self.export_variables.is_some()
    }

    /// Whether a launch with these options starts on the home screen: explicitly
    /// with `--home`, or interactively with nothing else to open.
    pub fn wants_home(&self) -> bool {
        !self.is_agent_cli()
            && self.open_id.is_none()
            && (self.home || (self.file.is_none() && self.import_capture.is_none() && self.screenshot.is_none() && self.fixture.is_none()))
    }
}

/// Values of `--panel`: `agents`, `agents-chat`, `specs`, `assets`, `variables` / `variables-floating` open the local views,
/// `libraries` the Libraries modal; the others are inspector popups opened on the selection for screenshots.
pub const PANELS: [&str; 57] = [
    "agents",
    "agents-chat",
    "agents-picker",
    "agents-skills",
    "agents-start-failure",
    "agents-approval-command",
    "agents-approval-queued",
    "agents-approval-open-tab",
    "agent-permissions",
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
    "ai-image-menu",
    "ai-image-expand",
    "ai-image-erase",
    "web-capture-report",
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
                let l = it.next().ok_or("--dev needs css, tailwind, swiftui, uikit, compose, android-xml, react or html")?;
                if designos_export::CodeLanguage::from_id(&l).is_none() {
                    return Err(format!("unknown language '{l}' (expected css, tailwind, swiftui, uikit, compose, android-xml, react or html)"));
                }
                o.dev = Some(l);
            }
            "--codegen" => {
                let l = it.next().ok_or("--codegen needs css, tailwind, swiftui, uikit, compose, android-xml, react or html")?;
                if designos_export::CodeLanguage::from_id(&l).is_none() {
                    return Err(format!("unknown language '{l}' (expected css, tailwind, swiftui, uikit, compose, android-xml, react or html)"));
                }
                o.codegen = Some(l);
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
            "--overwrite" => o.overwrite = true,
            "--import-capture" | "--import-web-capture" => o.import_capture = Some(it.next().ok_or("--import-capture needs a capture JSON path")?.into()),
            "--out" => o.out = Some(it.next().ok_or("--out needs an output path")?.into()),
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
            "-V" | "--version" => o.version = true,
            // Launch Services adds -psn_<serial> to some macOS launches; it names the process, not a file.
            s if s.starts_with("-psn_") => {}
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

/// What `--version` prints: the binary's name and the version of the release it was built from.
pub fn version_line() -> String {
    format!("{} {}", option_env!("CARGO_BIN_NAME").unwrap_or("designos"), env!("CARGO_PKG_VERSION"))
}

/// The line to print when the FILE argument names nothing on disk, or `None` when the run can go on.
pub fn missing_file_message(opts: &Options) -> Option<String> {
    let file = opts.file.as_ref()?;
    let gone = matches!(file.try_exists(), Ok(false));
    (gone && !opts.creates_missing_file()).then(|| format!("No file named {}. Run designos --help.", file.display()))
}

/// Parses a declarative `NodeSpec` JSON string for `--apply-spec`, returning a plain-language
/// error that names the field and what it must be instead of a Rust type name.
pub fn parse_spec_json(raw: &str) -> Result<designos_doc::NodeSpec, String> {
    let val: serde_json::Value = serde_json::from_str(raw).map_err(|e| format!("{e}. Check the JSON syntax, then try again."))?;
    match <designos_doc::NodeSpec as serde::Deserialize>::deserialize(&val) {
        Ok(spec) => {
            if let Some(msg) = describe_spec_value_error(&val) {
                return Err(msg);
            }
            Ok(spec)
        }
        Err(e) => {
            let msg = e.to_string();
            if designos_doc::extract_unknown_field_options(&msg).is_some() {
                return Err(designos_doc::format_spec_serde_error(&e));
            }
            if let Some(field_msg) = describe_spec_value_error(&val) {
                return Err(field_msg);
            }
            Err(humanize_spec_serde_error(&msg))
        }
    }
}

fn is_finite_f32(v: &serde_json::Value) -> bool {
    v.as_f64().is_some_and(|n| (n as f32).is_finite())
}

fn is_num_or_var(v: &serde_json::Value) -> bool {
    is_finite_f32(v) || v.is_string()
}

fn is_u16(v: &serde_json::Value) -> bool {
    v.as_u64().is_some_and(|n| u16::try_from(n).is_ok())
}

fn is_u32(v: &serde_json::Value) -> bool {
    v.as_u64().is_some_and(|n| u32::try_from(n).is_ok())
}

fn is_valid_comment_thread(v: &serde_json::Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    obj.get("id").is_some_and(serde_json::Value::is_string)
        && obj.get("resolved").is_some_and(serde_json::Value::is_boolean)
        && obj.get("page_id").and_then(serde_json::Value::as_u64).is_some()
        && obj.get("page_name").is_some_and(serde_json::Value::is_string)
        && obj.get("world_x").is_some_and(is_finite_f32)
        && obj.get("world_y").is_some_and(is_finite_f32)
        && obj.get("target_node_id").is_none_or(|x| x.is_null() || x.as_u64().is_some())
        && obj.get("target_node_name").is_none_or(|x| x.is_null() || x.is_string())
        && obj.get("target_node_kind").is_none_or(|x| x.is_null() || x.is_string())
        && obj.get("target_frame_id").is_none_or(|x| x.is_null() || x.as_u64().is_some())
        && obj.get("target_frame_name").is_none_or(|x| x.is_null() || x.is_string())
        && obj.get("messages").and_then(serde_json::Value::as_array).is_some_and(|msgs| {
            msgs.iter().all(|m| {
                m.as_object().is_some_and(|mo| {
                    mo.get("id").is_some_and(serde_json::Value::is_string)
                        && mo.get("author").is_some_and(serde_json::Value::is_string)
                        && mo.get("text").is_some_and(serde_json::Value::is_string)
                        && mo.get("created_at").is_none_or(serde_json::Value::is_string)
                })
            })
        })
}

fn describe_layout_spec_error(val: &serde_json::Value) -> Option<String> {
    let Some(obj) = val.as_object() else {
        return Some("layout must be a LayoutSpec object. Pass a JSON object such as {\"direction\": \"row\", \"gap\": 8}.".into());
    };
    for (k, v) in obj {
        match k.as_str() {
            "direction" | "dir" | "primary_align" | "justify" | "counter_align" | "align" => {
                if !v.is_string() {
                    return Some(format!("{k} must be a string. Pass a string value."));
                }
            }
            "wrap" => {
                if !v.is_boolean() {
                    return Some("wrap must be a boolean. Pass true or false.".into());
                }
            }
            "gap" | "counter_gap" => {
                if !is_num_or_var(v) {
                    return Some(format!("{k} must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."));
                }
            }
            "padding" => {
                if !v.as_array().is_some_and(|a| a.len() == 4 && a.iter().all(is_num_or_var)) {
                    return Some(
                        "padding must be an array of 4 numbers or \"var:...\" strings ([top, right, bottom, left]). Pass 4 values such as [16, 16, 16, 16]."
                            .into(),
                    );
                }
            }
            "grid_columns" | "grid_rows" => {
                if !v.is_null() && !is_u32(v) {
                    return Some(format!("{k} must be a positive integer. Pass a whole number of grid tracks."));
                }
            }
            _ => {}
        }
    }
    None
}

fn describe_slots_error(val: &serde_json::Value) -> Option<String> {
    let Some(map) = val.as_object() else {
        return Some("slots must be an object mapping slot names to false or an array of NodeSpec objects. Pass false or [NodeSpec] for each slot.".into());
    };
    for (slot_name, slot_val) in map {
        match slot_val {
            serde_json::Value::Bool(false) => {}
            serde_json::Value::Array(items) => {
                for item in items {
                    if !item.is_object() {
                        return Some(format!(
                            "slots.{slot_name} must be false or an array of NodeSpec objects. Pass false or each slot child as a JSON object."
                        ));
                    }
                    if let Some(err) = describe_spec_value_error(item) {
                        return Some(err);
                    }
                }
            }
            _ => {
                return Some(format!("slots.{slot_name} must be false or an array of NodeSpec objects. Pass false or [NodeSpec] for the slot."));
            }
        }
    }
    None
}

fn describe_override_spec_error(val: &serde_json::Value) -> Option<String> {
    let Some(obj) = val.as_object() else {
        return Some("overrides must be an array of OverrideSpec objects. Pass each override as a JSON object.".into());
    };
    for (k, v) in obj {
        match k.as_str() {
            "path" | "text" | "fill" | "stroke" | "swap" => {
                if !v.is_null() && !v.is_string() {
                    return Some(format!("{k} must be a string. Pass a string value."));
                }
            }
            "id" => {
                if !v.is_null() && v.as_u64().is_none() {
                    return Some("id must be a non-negative integer. Pass a numeric node ID.".into());
                }
            }
            "visible" => {
                if !v.is_null() && !v.is_boolean() {
                    return Some("visible must be a boolean. Pass true or false.".into());
                }
            }
            "opacity" => {
                if !v.is_null() && !is_finite_f32(v) {
                    return Some("opacity must be a number between 0 and 1. Pass a numeric opacity such as 0.8.".into());
                }
            }
            "variant" => {
                if !v.as_object().is_some_and(|m| m.values().all(serde_json::Value::is_string)) {
                    return Some(
                        "variant must be an object mapping variant property names to string values. Pass an object such as {\"Size\": \"md\"}.".into(),
                    );
                }
            }
            "props" => {
                if !v.is_object() {
                    return Some("props must be an object mapping component property names to values. Pass a JSON object.".into());
                }
            }
            "slots" => {
                if let Some(err) = describe_slots_error(v) {
                    return Some(err);
                }
            }
            _ => {}
        }
    }
    None
}

fn describe_in_progress_error(val: &serde_json::Value) -> Option<String> {
    if val.is_null() || val.is_boolean() {
        return None;
    }
    let Some(obj) = val.as_object() else {
        return Some("in_progress must be a boolean or an in_progress object. Pass true, false, or an object such as {\"agent\": \"...\"}.".into());
    };
    for (k, v) in obj {
        match k.as_str() {
            "run_id" | "agent" | "agent_name" | "status" => {
                if !v.is_null() && !v.is_string() {
                    return Some(format!("{k} in in_progress must be a string. Pass a string value."));
                }
            }
            "started_at_ms" | "since_ms" => {
                if !v.is_null() && v.as_u64().is_none() {
                    return Some(format!("{k} in in_progress must be a non-negative integer. Pass a whole number."));
                }
            }
            "stale_after_s" => {
                if !v.is_null() && !is_u32(v) {
                    return Some("stale_after_s in in_progress must be a non-negative integer. Pass a whole number.".into());
                }
            }
            _ => {}
        }
    }
    None
}

/// Inspects a raw `NodeSpec` JSON value and returns a plain-words error message naming the first
/// field whose value has the wrong JSON type.
pub fn describe_spec_value_error(val: &serde_json::Value) -> Option<String> {
    let Some(obj) = val.as_object() else {
        return Some("spec must be a NodeSpec object. Pass a JSON object with node fields such as kind, name, and children.".into());
    };
    for (k, v) in obj {
        match k.as_str() {
            "id" => {
                if !v.is_null() && v.as_u64().is_none() {
                    return Some("id must be a non-negative integer. Pass a numeric node ID.".into());
                }
            }
            "kind" | "name" => {
                if !v.is_string() {
                    return Some(format!("{k} must be a string. Pass a string value."));
                }
            }
            "component" | "preset" | "width_sizing" | "height_sizing" | "fill" | "stroke" | "text" | "text_color" | "color" | "text_style" | "effect_style"
            | "grid_style" | "dev_status" | "annotation" => {
                if !v.is_null() && !v.is_string() {
                    return Some(format!("{k} must be a string. Pass a string value."));
                }
            }
            "variant" => {
                if !v.as_object().is_some_and(|m| m.values().all(serde_json::Value::is_string)) {
                    return Some(
                        "variant must be an object mapping variant property names to string values. Pass an object such as {\"Size\": \"md\"}.".into(),
                    );
                }
            }
            "mode" => {
                if !v.as_object().is_some_and(|m| m.values().all(serde_json::Value::is_string)) {
                    return Some(
                        "mode must be an object mapping variable collection names to mode names. Pass an object such as {\"Theme\": \"Dark\"}.".into(),
                    );
                }
            }
            "props" => {
                if !v.is_object() {
                    return Some("props must be an object mapping component property names to values. Pass a JSON object.".into());
                }
            }
            "overrides" => {
                let Some(arr) = v.as_array() else {
                    return Some("overrides must be an array of OverrideSpec objects. Pass each override as a JSON object.".into());
                };
                for item in arr {
                    if let Some(err) = describe_override_spec_error(item) {
                        return Some(err);
                    }
                }
            }
            "slots" => {
                if let Some(err) = describe_slots_error(v) {
                    return Some(err);
                }
            }
            "x" | "y" => {
                if !is_finite_f32(v) {
                    return Some(format!("{k} must be a number. Pass a numeric coordinate in pixels."));
                }
            }
            "min_width" | "max_width" | "min_height" | "max_height" => {
                if !v.is_null() && !is_finite_f32(v) {
                    return Some(format!("{k} must be a number. Pass a numeric size in pixels."));
                }
            }
            "width" | "height" | "stroke_weight" | "stroke_width" | "corner_radius" | "radius" | "font_size" | "line_height" => {
                if !v.is_null() && !is_num_or_var(v) {
                    return Some(format!("{k} must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."));
                }
            }
            "opacity" => {
                if !is_num_or_var(v) {
                    return Some("opacity must be a number between 0 and 1 or a \"var:...\" string. Pass a number such as 0.8 or a variable reference.".into());
                }
            }
            "corner_radii" => {
                if !v.is_null() && !v.as_array().is_some_and(|a| a.len() == 4 && a.iter().all(is_num_or_var)) {
                    return Some(
                        "corner_radii must be an array of 4 numbers or \"var:...\" strings ([tl, tr, br, bl]). Pass 4 values such as [8, 8, 8, 8].".into(),
                    );
                }
            }
            "col_span" | "row_span" => {
                if !v.is_null() && !is_u16(v) {
                    return Some(format!("{k} must be a positive integer. Pass a whole number of grid tracks."));
                }
            }
            "font_weight" => {
                if !v.is_null() && !is_u16(v) {
                    return Some("font_weight must be a number between 100 and 900 (for example 400, 500, 600, or 700). Pass an integer weight.".into());
                }
            }
            "clips_content" => {
                if !v.is_null() && !v.is_boolean() {
                    return Some("clips_content must be a boolean. Pass true or false.".into());
                }
            }
            "visible" => {
                if !v.is_null() && !v.is_boolean() && !v.is_string() {
                    return Some("visible must be a boolean or a \"var:...\" string. Pass true, false, or a boolean variable reference.".into());
                }
            }
            "layout" => {
                if !v.is_null() {
                    if let Some(err) = describe_layout_spec_error(v) {
                        return Some(err);
                    }
                }
            }
            "annotations" => {
                if !v.as_array().is_some_and(|a| a.iter().all(serde_json::Value::is_string)) {
                    return Some("annotations must be an array of strings. Pass an array such as [\"Note\"].".into());
                }
            }
            "in_progress" => {
                if let Some(err) = describe_in_progress_error(v) {
                    return Some(err);
                }
            }
            "comments" => {
                if !v.as_array().is_some_and(|a| a.iter().all(is_valid_comment_thread)) {
                    return Some(
                        "comments must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects.".into(),
                    );
                }
            }
            "children" => {
                let Some(arr) = v.as_array() else {
                    return Some("children must be an array of NodeSpec objects. Pass each child as a JSON object.".into());
                };
                for child in arr {
                    if !child.is_object() {
                        return Some("children must be an array of NodeSpec objects. Pass each child as a JSON object.".into());
                    }
                    if let Some(err) = describe_spec_value_error(child) {
                        return Some(err);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Replaces Rust type names in a raw `serde_json` error string with plain spec wording.
pub fn humanize_spec_serde_error(msg: &str) -> String {
    if msg.contains("untagged enum NumberOrVar") {
        return "value must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference.".into();
    }
    if msg.contains("untagged enum BoolOrVar") {
        return "visible must be a boolean or a \"var:...\" string. Pass true, false, or a boolean variable reference.".into();
    }
    if msg.contains("untagged enum InProgressSpec") {
        return "in_progress must be a boolean or an in_progress object. Pass true, false, or an object such as {\"agent\": \"...\"}.".into();
    }
    msg.replace("struct NodeSpec", "a NodeSpec object")
        .replace("struct LayoutSpec", "a LayoutSpec object")
        .replace("struct OverrideSpec", "an OverrideSpec object")
        .replace("struct ResolvedCommentThread", "a comment thread object")
        .replace("a tuple of size 4", "an array of 4 numbers or \"var:...\" strings")
        .replace("expected u64", "expected a non-negative integer")
        .replace("expected u32", "expected a non-negative integer")
        .replace("expected u16", "expected a non-negative integer")
        .replace("expected f32", "expected a number")
}

pub const USAGE: &str = "usage: designos [FILE] [--screenshot OUT.png] [--size WxH] [--scale FACTOR] [--select LAYER|none] [--menu main|main/File|shape|palette] [--panel agents-start-failure|specs|assets|plugins|interaction|present-variables|variables|variables-floating|libraries|picker|picker-page|fill-d9|picker-gradient|effects-menu|effect-styles|grid-styles|edit-style-color|edit-style-text|edit-style-effect|edit-style-grid|stroke-settings|type-details|effect-noise|effect-texture|effect-glass|version-history|dev-annotate|dev-compare|web-capture-report]\n               [--zoom PERCENT] [--rulers] [--grid] [--gpu] [--home] [--library DIR] [--layout-grid grid|columns|rows] [--edit-text LAYER [--select-text START-END]]\n               [--inspect-json] [--comments-json] [--add-comment TEXT] [--reply-comment ID [--message TEXT] [--resolve]] [--resolve-comment ID] [--apply-spec SPEC.json] [--export-spec OUT.json] [--export-fig OUT.fig [--overwrite]] [--codegen LANG] [--import-capture CAP.json [--out OUT.designos]]\n  FILE  .fig, .json (Figma REST or native), .svg or .designos; without a file the projects home screen opens
  --version, -V  print the version and exit
  --scale    with --screenshot: device pixel ratio (2 writes a PNG twice --size, rendered crisp rather than upscaled)
  --home     show the home screen (also in --screenshot mode)
  --present  open presentation preview; --fullscreen starts it in native fullscreen (F11 toggles, Esc returns)
  --gpu      draw the canvas on the GPU; --screenshot uses the CPU renderer unless this is given
  --library  use DIR as the projects library instead of the default location
  --dev      open Dev mode with a code language (css|tailwind|swiftui|uikit|compose|android-xml|react|html); --dev-demo marks the selection Ready for dev with an annotation
  --codegen  headless Agent CLI: generate code for --select (or first screen/page) in the given language (css|tailwind|swiftui|uikit|compose|android-xml|react|html)
  --no-caption  with --panel micro-<slug>:seq:<N>: record frames without the top shortcut caption bar
  --no-toast    with --screenshot: leave transient toasts ('Opened <file>') out of the capture
  --seed-library  write the showcase sample files (a project with three files) into DIR and exit
  --find     open the Find and replace panel searching for TEXT (--replace-with TEXT shows the replace row)
  --fixture  open a sample document instead of the demo (showcase|proto-vars|no-selection|variables|comments|board|board-empty)
  --export-fig  headless Agent CLI: write the document to OUT.fig. The path must end in .fig and must not exist yet; --overwrite replaces an existing regular file. A symbolic link is never written through
  --import-variables  headless Agent CLI: read variables from FILE (.json or .zip) into the document
  --export-variables  headless Agent CLI: write the document's variables to PATH (a .json or .zip file, or a folder)
  --format   variable format for --import-variables and --export-variables (auto|figma-rest|figma-native|dtcg)
  --inspect-json / --comments-json / --apply-spec / --export-spec / --export-fig / --codegen / --import-capture  headless Agent CLI inspection and mutation";

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
        assert_eq!(p(&["--dev", "html"]).unwrap().dev.as_deref(), Some("html"));
        let cg = p(&["--codegen", "html"]).unwrap();
        assert_eq!(cg.codegen.as_deref(), Some("html"));
        assert!(cg.is_agent_cli());
        assert!(p(&["--dev", "cobol"]).is_err());
        assert!(p(&["--codegen", "cobol"]).is_err());
        assert!(p(&["--dev"]).is_err());
        assert!(p(&["--codegen"]).is_err());
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
        assert_eq!(p(&["--panel", "web-capture-report"]).unwrap().panel.as_deref(), Some("web-capture-report"));
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
    fn overwrite_is_off_unless_given() {
        assert!(!p(&["doc.designos", "--export-fig", "out.fig"]).unwrap().overwrite);
        let o = p(&["doc.designos", "--export-fig", "out.fig", "--overwrite"]).unwrap();
        assert!(o.overwrite && o.export_fig == Some("out.fig".into()));
        assert!(o.is_agent_cli());
    }

    #[test]
    fn the_help_text_names_the_export_fig_rules() {
        assert!(USAGE.contains("[--export-fig OUT.fig [--overwrite]]"));
        assert!(USAGE.contains("--overwrite replaces an existing regular file"), "the help says what --overwrite does");
        assert!(USAGE.contains("never written through"), "the help says a symbolic link is refused");
    }

    #[test]
    fn web_capture_cli_flags_parse() {
        let o = p(&["--import-capture", "card.capture.json", "--out", "out.designos"]).unwrap();
        assert!(o.is_agent_cli());
        assert!(!o.wants_home());
        assert_eq!(o.import_capture, Some("card.capture.json".into()));
        assert_eq!(o.out, Some("out.designos".into()));
    }

    #[test]
    fn parses_the_version_flags() {
        assert!(p(&["--version"]).unwrap().version);
        assert!(p(&["-V"]).unwrap().version);
        assert!(!p(&[]).unwrap().version);
        assert!(!p(&["--version"]).unwrap().is_agent_cli(), "printing the version opens no window and runs no command");
        let both = p(&["doctor", "--version"]).unwrap();
        assert!(both.version && both.file == Some("doctor".into()), "the file argument is kept, and --version wins over it in main");
    }

    #[test]
    fn the_version_line_names_the_binary_and_the_cargo_version() {
        let line = version_line();
        assert!(line.starts_with("designos"), "{line}");
        assert!(line.ends_with(env!("CARGO_PKG_VERSION")), "{line}");
        assert_eq!(line.split(' ').count(), 2, "one name, one version: {line}");
    }

    #[test]
    fn the_usage_lists_the_version_flag() {
        assert!(USAGE.contains("--version"), "{USAGE}");
        assert!(USAGE.contains("-V"), "{USAGE}");
    }

    #[test]
    fn the_usage_lists_the_fixture_and_variable_flags_the_parser_takes() {
        for flag in ["--fixture", "--import-variables", "--export-variables", "--format"] {
            let line = USAGE.lines().find(|l| l.trim_start().starts_with(flag)).unwrap_or_else(|| panic!("--help has no line for {flag}:\n{USAGE}"));
            assert!(line.split_whitespace().count() > 2, "{flag} says what it does: {line}");
            assert!(!line.contains("(1") && !line.contains("(2") && !line.contains("(3"), "no count in parentheses: {line}");
        }
        // The values named in --help are the ones the parser accepts.
        for fixture in ["showcase", "proto-vars", "no-selection", "variables", "comments", "board", "board-empty"] {
            assert!(USAGE.contains(fixture), "--fixture lists {fixture}");
        }
        for format in ["auto", "figma-rest", "figma-native", "dtcg"] {
            assert!(USAGE.contains(format), "--format lists {format}");
            assert!(p(&["--format", format]).is_ok(), "the parser takes {format}");
        }
    }

    #[test]
    fn a_file_that_is_not_there_is_named_in_one_plain_line() {
        let missing = p(&["doctor"]).unwrap();
        assert_eq!(missing_file_message(&missing).as_deref(), Some("No file named doctor. Run designos --help."));
        let nested = p(&["no/such/dir/a b.fig"]).unwrap();
        assert_eq!(missing_file_message(&nested).as_deref(), Some("No file named no/such/dir/a b.fig. Run designos --help."));
        assert_eq!(missing_file_message(&p(&["doctor", "--screenshot", "x.png"]).unwrap()).as_deref(), Some("No file named doctor. Run designos --help."));
        assert_eq!(missing_file_message(&p(&["doctor", "--inspect-json"]).unwrap()).as_deref(), Some("No file named doctor. Run designos --help."));
    }

    #[test]
    fn a_file_that_exists_or_is_not_asked_for_is_not_called_missing() {
        let here = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        assert_eq!(missing_file_message(&p(&[here]).unwrap()), None, "an existing file");
        assert_eq!(missing_file_message(&p(&[env!("CARGO_MANIFEST_DIR")]).unwrap()), None, "a directory is something, and the loader names its own problem");
        assert_eq!(missing_file_message(&p(&[]).unwrap()), None, "no file argument");
        assert_eq!(missing_file_message(&p(&["--home"]).unwrap()), None);
        assert_eq!(missing_file_message(&p(&["--open-id", "abc"]).unwrap()), None);
    }

    #[test]
    fn the_process_serial_number_macos_adds_on_some_launches_is_not_a_file() {
        let o = p(&["-psn_0_12345"]).unwrap();
        assert_eq!(o.file, None, "Finder and Launch Services may pass -psn_<serial>; it is not a file");
        assert_eq!(missing_file_message(&o), None, "the app must still start");
        assert!(o.wants_home());
        assert_eq!(p(&["-psn_0_12345", "a.designos"]).unwrap().file, Some("a.designos".into()), "a real file next to it still opens");
    }

    #[test]
    fn the_commands_that_write_a_new_file_may_name_one_that_is_not_there_yet() {
        for args in [
            &["new.designos", "--apply-spec", "spec.json"][..],
            &["new.designos", "--import-capture", "cap.json"][..],
            &["new.designos", "--import-variables", "vars.json"][..],
        ] {
            let o = p(args).unwrap();
            assert!(o.creates_missing_file(), "{args:?}");
            assert_eq!(missing_file_message(&o), None, "{args:?}");
        }
        assert!(!p(&["a.designos", "--inspect-json"]).unwrap().creates_missing_file());
    }

    #[test]
    fn empty_is_default() {
        assert_eq!(p(&[]).unwrap(), Options::default());
    }

    #[test]
    fn parse_spec_json_names_the_field_and_expected_type() {
        let err = parse_spec_json(r#"{"name":"Card","children":[[{"kind":"rectangle"}]]}"#).unwrap_err();
        assert!(err.contains("children must be an array of NodeSpec objects"), "{err}");
        assert!(!err.contains("expected u64"), "{err}");
    }
}
