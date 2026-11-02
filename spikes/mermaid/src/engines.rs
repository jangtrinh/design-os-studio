//! Thin adapters: Mermaid source -> themed SVG string, offline, pure Rust.
//! Neither adapter touches the network; neither spawns a process.

use crate::studio_theme;

#[derive(Debug)]
pub struct EngineError {
    pub message: String,
    /// 1-based source line when the engine's error type carries one.
    pub line: Option<u32>,
}

pub fn render_svg(engine: &str, src: &str) -> Result<String, EngineError> {
    match engine {
        #[cfg(feature = "mmdr")]
        "mmdr" => mmdr(src),
        #[cfg(feature = "merman")]
        "merman" => merman(src),
        #[cfg(feature = "merman06")]
        "merman06" => merman06(src),
        other => Err(EngineError { message: format!("engine '{other}' not compiled in"), line: None }),
    }
}

#[cfg(feature = "mmdr")]
fn mmdr(src: &str) -> Result<String, EngineError> {
    use mermaid_rs_renderer::{render_strict, ParseError, RenderOptions};
    let mut opts = RenderOptions::modern();
    opts.theme = studio_theme::mmdr_theme();
    // Calibrated widths: skips fontdb::load_system_fonts() and the on-disk
    // metrics cache, so layout is deterministic and touches no files.
    opts.layout.fast_text_metrics = true;
    render_strict(src, opts).map_err(|e| {
        let line = match &e {
            ParseError::UnknownParticipant { line, .. } => Some(*line),
            ParseError::UnclosedSubgraph { opened_at } => Some(*opened_at),
            ParseError::UnexpectedToken { line, .. } => Some(*line),
            ParseError::InvalidDirective { line, .. } => Some(*line),
            _ => None,
        };
        EngineError { message: e.to_string(), line }
    })
}

#[cfg(feature = "merman")]
fn merman(src: &str) -> Result<String, EngineError> {
    use merman::svg::SvgPipeline;
    use merman::{
        Engine, MermaidConfig, OperationControl, ParseOptions, RenderOutput, RenderRequest,
        Renderer, SvgRequest,
    };
    let renderer = Renderer::new()
        .with_engine(Engine::new().with_site_config(MermaidConfig::from_value(
            studio_theme::merman_site_config(),
        )))
        .with_parse_options(ParseOptions::strict());
    use merman::svg::{CssOverridePolicy, ScopedCssPostprocessor};
    let request = SvgRequest {
        // foreignObject HTML labels -> plain SVG text (usvg/resvg ignore foreignObject),
        // plus host CSS for the spots where the browser-targeted SVG misrenders in resvg.
        pipeline: Some(
            SvgPipeline::resvg_safe().with_postprocessor(
                ScopedCssPostprocessor::new(studio_theme::MERMAN_HOST_CSS)
                    .with_override_policy(CssOverridePolicy::StripExistingImportant),
            ),
        ),
        ..Default::default()
    };
    let out = renderer
        .render(RenderRequest::svg(src, OperationControl::new(), request))
        .map_err(|e| match &e {
            // Structured path: the parse error carries a byte span, so the line
            // is derived from the source Studio already holds.
            merman::RenderError::Parse(d) => {
                let line = d
                    .terminal_diagnostic_details()
                    .span
                    .map(|s| 1 + src.as_bytes()[..s.start.min(src.len())].iter().filter(|b| **b == b'\n').count() as u32);
                EngineError { message: d.terminal_safe_message(), line }
            }
            other => EngineError { message: other.to_string(), line: parse_line(&other.to_string()) },
        })?;
    match out {
        RenderOutput::Svg(Some(svg)) => Ok(svg.svg().to_string()),
        _ => Err(EngineError { message: "no Mermaid diagram detected".into(), line: None }),
    }
}

/// merman 0.6.2: last release line whose MSRV (1.87) fits the repo's pinned
/// 1.93.1 toolchain. Older facade (`HeadlessRenderer`); errors are message-only
/// (no byte span), so the line number can only come from the message text.
#[cfg(feature = "merman06")]
fn merman06(src: &str) -> Result<String, EngineError> {
    use merman06::render::{HeadlessRenderer, SvgPipeline};
    let renderer = HeadlessRenderer::new()
        .with_site_config(merman06::MermaidConfig::from_value(studio_theme::merman_site_config()))
        .with_parse_options(merman06::ParseOptions::strict());
    match renderer.render_svg_with_pipeline_sync(src, &SvgPipeline::resvg_safe()) {
        Ok(Some(svg)) => Ok(svg),
        Ok(None) => Err(EngineError { message: "no Mermaid diagram detected".into(), line: None }),
        Err(e) => Err(EngineError { message: e.to_string(), line: parse_line(&e.to_string()) }),
    }
}

/// Last-resort line extraction from an error string ("... line 4 ...").
#[allow(dead_code)]
fn parse_line(msg: &str) -> Option<u32> {
    let i = msg.find("line ")? + 5;
    msg[i..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}
