//! Pasting what another app put on the system clipboard: an image becomes an image layer and plain text becomes a
//! text layer.
//!
//! egui-winit hands the app the clipboard's text only, and nothing at all when the clipboard holds just an image,
//! so the image flavour is read here through [`App::system_image`]. Reading it, and encoding it as PNG, costs far
//! more than a frame on a large screenshot (5120x2880: about 20 ms to encode at best, more on real content, plus
//! the platform's own decode inside `arboard`), so both run on a worker thread while the window keeps drawing. The
//! paste is decided when the worker answers: [`App::poll_paste_jobs`] runs once per frame.

use crate::app::App;
use crate::canvas::{create, interact};
use crate::edit::Tx;
use crate::layout;
use designos_doc::{Command, NodeKind, TextAutoResize, Vec2};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Widest a pasted text layer grows before it wraps, in px; the longest side of a pasted image is capped at the
/// same 400 px by the image placement.
const PASTED_TEXT_MAX_WIDTH: f32 = 400.0;
/// The longest side of a placed image layer, as in `canvas::interact::place_image_bytes`.
const PLACED_IMAGE_MAX_SIDE: f32 = 400.0;
/// Longest layer name taken from pasted text.
const PASTED_NAME_CHARS: usize = 40;

/// Pixels read from the system clipboard: tightly packed 8-bit RGBA, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The system clipboard's plain text. The default of [`App::system_text`]; tests script the clipboard instead.
pub fn system_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get().text().ok().filter(|t| !t.is_empty())
}

/// The system clipboard's image, whatever format it was copied in. The default of [`App::system_image`].
pub fn system_image() -> Option<ClipImage> {
    let data = arboard::Clipboard::new().ok()?.get().image().ok()?;
    Some(ClipImage { width: u32::try_from(data.width).ok()?, height: u32::try_from(data.height).ok()?, rgba: data.bytes.into_owned() })
}

/// Encodes `img` as PNG: fast compression, because the bytes are embedded in the document and decoded at once.
pub fn encode_png(img: &ClipImage) -> Result<Vec<u8>, String> {
    let expected = (img.width as usize).checked_mul(img.height as usize).and_then(|px| px.checked_mul(4));
    if img.width == 0 || img.height == 0 || expected != Some(img.rgba.len()) {
        return Err(format!("{}x{} pixels do not fit {} bytes", img.width, img.height, img.rgba.len()));
    }
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Sub)
        .write_image(&img.rgba, img.width, img.height, ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

/// What the worker found on the clipboard.
enum ImageProbe {
    /// No image on the clipboard, or none the platform could read.
    Absent,
    Image {
        png: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// An image that cannot become a layer (its pixel buffer does not match its size).
    Unreadable,
}

/// A paste waiting for the worker that reads the clipboard image.
pub struct PasteJob {
    /// The clipboard's plain text when the paste was started, `None` when the clipboard had none.
    text: Option<String>,
    /// Viewport center in page space when the paste was started.
    center: Vec2,
    probe: Receiver<ImageProbe>,
}

fn read_image(read: fn() -> Option<ClipImage>) -> ImageProbe {
    let Some(img) = read() else { return ImageProbe::Absent };
    match encode_png(&img) {
        Ok(png) => ImageProbe::Image { png, width: img.width, height: img.height },
        Err(_) => ImageProbe::Unreadable,
    }
}

fn viewport_center(app: &App) -> Vec2 {
    app.view.to_page(app.canvas_rect.min, app.canvas_rect.center())
}

impl App {
    /// Starts a paste that is neither Studio's own layers, SVG markup nor a Figma payload: the clipboard image, when
    /// there is one, wins over `text` (a browser puts the image's URL or HTML beside a copied image).
    pub(crate) fn start_system_paste(&mut self, text: Option<&str>) {
        let read = self.system_image;
        let (sender, probe) = mpsc::channel();
        // A failed spawn drops the sender with the closure, which `poll_paste_jobs` reads as "no image".
        let _ = std::thread::Builder::new().name("clipboard-image".into()).spawn(move || {
            let _ = sender.send(read_image(read));
        });
        self.paste_jobs.push_back(PasteJob { text: text.map(str::to_owned), center: viewport_center(self), probe });
    }

    /// Finishes the pastes whose worker has answered, in the order they started. True while one is still waiting.
    pub fn poll_paste_jobs(&mut self) -> bool {
        while let Some(job) = self.paste_jobs.front() {
            let probe = match job.probe.try_recv() {
                Ok(probe) => probe,
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => ImageProbe::Absent,
            };
            let Some(job) = self.paste_jobs.pop_front() else { break };
            self.finish_system_paste(job, probe);
        }
        false
    }

    /// Blocks until every started paste is finished, so a test sees the layers right after the frame that pasted.
    #[cfg(test)]
    pub fn settle_paste_jobs(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.poll_paste_jobs() {
            assert!(std::time::Instant::now() < deadline, "the clipboard worker never answered");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    fn finish_system_paste(&mut self, job: PasteJob, probe: ImageProbe) {
        match probe {
            ImageProbe::Image { png, width, height } => place_pasted_image(self, job.center, png, width, height),
            ImageProbe::Unreadable => self.show_toast("Cannot paste image", true),
            ImageProbe::Absent => match job.text.as_deref() {
                Some(text) => {
                    if !paste_text(self, text, job.center) {
                        self.show_toast("Nothing to paste", false);
                    }
                }
                // Nothing on the system clipboard can be read, so Studio's own copy is the only thing to paste.
                None if self.clip.is_some() => self.paste(None),
                None => self.show_toast("Nothing to paste", false),
            },
        }
    }
}

/// Where `place_image_bytes`, which puts an image's top-left corner at the point it is given, must put a
/// `width` x `height` picture so that the layer (at most 400 px on its longest side) is centered on page point `center`.
pub(crate) fn centered_image_origin(center: Vec2, width: u32, height: u32) -> Vec2 {
    let (w, h) = (width as f32, height as f32);
    let scale = (PLACED_IMAGE_MAX_SIDE / w.max(h)).min(1.0);
    Vec2::new((center.x - w * scale / 2.0).round(), (center.y - h * scale / 2.0).round())
}

/// The pixel size of a PNG, JPEG or WebP picture, read from its header without decoding it.
pub(crate) fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

/// Inserts the clipboard image as an image layer, centered on page point `center` and at most 400 px on its
/// longest side like the Image tool, as one undo step.
fn place_pasted_image(app: &mut App, center: Vec2, png: Vec<u8>, width: u32, height: u32) {
    interact::place_image_bytes(app, centered_image_origin(center, width, height), "Pasted image", png);
}

/// Layer name of pasted text: its first line that has any text, cut to a short length.
fn pasted_name(content: &str) -> String {
    let line = content.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Text");
    line.chars().take(PASTED_NAME_CHARS).collect()
}

/// Pastes `text` as one text layer centered on page point `center`, selected, as one undo step. Text wider than
/// 400 px wraps at that width. Returns false, with the document untouched, when `text` is only whitespace.
fn paste_text(app: &mut App, text: &str, center: Vec2) -> bool {
    let content = text.replace("\r\n", "\n").trim_end().to_string();
    if content.trim().is_empty() {
        return false;
    }
    let parent = app.container_for_insert();
    let Some(inv) = app.doc.world_transform(parent).invert() else {
        app.show_toast("Cannot place into a collapsed layer", true);
        return true;
    };
    let at = inv.apply(center);
    let Some((mut node, _)) = create::new_text(app, parent, at) else { return false };
    node.name = pasted_name(&content);
    if let NodeKind::Text(t) = &mut node.kind {
        t.content = content;
        node.size = layout::estimate_text_size(t);
        if node.size.x > PASTED_TEXT_MAX_WIDTH {
            t.auto_resize = TextAutoResize::Height;
            node.size = layout::fit_text(t, Vec2::new(PASTED_TEXT_MAX_WIDTH, node.size.y));
        }
    }
    node.transform.tx = (at.x - node.size.x / 2.0).round();
    node.transform.ty = (at.y - node.size.y / 2.0).round();
    let id = node.id;
    if !app.slot_insert_allowed(parent, &[&node]) {
        return true;
    }
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    if let Err(e) = tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![node] }) {
        app.show_toast(e.to_string(), true);
        return true;
    }
    App::stretch_into_slot_if_configured(&mut app.doc, &mut tx.ed, parent, id);
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    layout::relayout_around(&mut app.doc, &mut tx.ed, parent);
    app.run_tx(tx);
    app.selection = vec![id];
    app.fonts.load_for(&app.doc);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(width: u32, height: u32) -> ClipImage {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&[(x * 255 / width.max(1)) as u8, (y * 255 / height.max(1)) as u8, 90, if (x + y) % 2 == 0 { 255 } else { 128 }]);
            }
        }
        ClipImage { width, height, rgba }
    }

    #[test]
    fn encoded_png_decodes_to_the_same_pixels() {
        let img = checker(37, 21);
        let png = encode_png(&img).expect("encodes");
        let back = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap().to_rgba8();
        assert_eq!((back.width(), back.height()), (37, 21));
        assert_eq!(back.into_raw(), img.rgba, "lossless, with the alpha channel");
    }

    #[test]
    fn a_pixel_buffer_that_does_not_match_its_size_is_refused() {
        let mut img = checker(8, 8);
        img.rgba.pop();
        assert!(encode_png(&img).is_err());
        assert!(encode_png(&ClipImage { width: 0, height: 4, rgba: Vec::new() }).is_err());
        assert!(encode_png(&ClipImage { width: u32::MAX, height: u32::MAX, rgba: vec![0; 16] }).is_err(), "a size that overflows is not an allocation");
    }

    #[test]
    fn pasted_text_names_its_layer_after_its_first_line() {
        assert_eq!(pasted_name("\n  Hello world \nsecond"), "Hello world");
        assert_eq!(pasted_name(&"x".repeat(100)).chars().count(), PASTED_NAME_CHARS);
        assert_eq!(pasted_name("   \n  "), "Text");
    }
}
