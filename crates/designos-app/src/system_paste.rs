//! Pasting what another app put on the system clipboard: an image becomes an image layer and plain text becomes a
//! text layer.
//!
//! egui-winit hands the app the clipboard's text only, and nothing at all when the clipboard holds just an image,
//! so the image flavour is read here through [`App::system_image`]. Reading it, and encoding it as PNG, costs far
//! more than a frame on a large screenshot (5120x2880: about 20 ms to encode at best, more on real content, plus
//! the platform's own decode inside `arboard`), so both run on a worker thread while the window keeps drawing. The
//! paste is decided when the worker answers: [`App::poll_paste_jobs`] runs once per frame.

use crate::app::App;
use crate::paste_placement::{self, Landing};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// How long a paste waits for the clipboard worker before it gives up.
const PASTE_READ_TIMEOUT: Duration = Duration::from_secs(10);

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
    Image(Vec<u8>),
    /// An image that cannot become a layer (its pixel buffer does not match its size).
    Unreadable,
}

/// A paste waiting for the worker that reads the clipboard image.
pub struct PasteJob {
    /// The clipboard's plain text when the paste was started, `None` when the clipboard had none.
    text: Option<String>,
    /// The frame and spot the paste was aimed at when it was started.
    landing: Landing,
    /// The tab it was started in: a paste never lands in another document.
    tab: Option<u64>,
    deadline: Instant,
    probe: Receiver<ImageProbe>,
}

fn read_image(read: fn() -> Option<ClipImage>) -> ImageProbe {
    let Some(img) = read() else { return ImageProbe::Absent };
    match encode_png(&img) {
        Ok(png) => ImageProbe::Image(png),
        Err(_) => ImageProbe::Unreadable,
    }
}

impl App {
    /// The open tab, or `None` on the home screen and in a window without tabs.
    fn paste_tab(&self) -> Option<u64> {
        if self.home.is_some() || self.tabs.parked_in_home {
            return None;
        }
        self.tabs.entries.get(self.tabs.active_idx).map(|t| t.id)
    }

    /// Starts a paste that is neither Studio's own layers, SVG markup nor a Figma payload: the clipboard image, when
    /// there is one, wins over `text` (a browser puts the image's URL or HTML beside a copied image).
    pub(crate) fn start_system_paste(&mut self, text: Option<&str>) {
        let read = self.system_image;
        let (sender, probe) = mpsc::channel();
        // A failed spawn drops the sender with the closure, which `poll_paste_jobs` reads as "no image".
        let _ = std::thread::Builder::new().name("clipboard-image".into()).spawn(move || {
            let _ = sender.send(read_image(read));
        });
        let job = PasteJob {
            text: text.map(str::to_owned),
            landing: paste_placement::paste_landing(self),
            tab: self.paste_tab(),
            deadline: Instant::now() + PASTE_READ_TIMEOUT,
            probe,
        };
        self.paste_jobs.push_back(job);
    }

    /// Finishes the pastes whose worker has answered, in the order they started. A paste whose tab is no longer open
    /// is dropped. One whose worker is still silent after 10 s pastes the text it already had, or gives up with a
    /// toast when it had none, so a hung clipboard owner ends the redrawing; later pastes wait behind it until then.
    /// True while one is still waiting.
    pub fn poll_paste_jobs(&mut self) -> bool {
        while let Some(job) = self.paste_jobs.front() {
            let answer = if job.tab != self.paste_tab() {
                None
            } else {
                match job.probe.try_recv() {
                    Ok(probe) => Some(probe),
                    Err(TryRecvError::Disconnected) => Some(ImageProbe::Absent),
                    // The image never came. Text the paste already had is still worth pasting; the toast is for a
                    // clipboard with nothing else to offer.
                    Err(TryRecvError::Empty) if Instant::now() >= job.deadline => {
                        if job.text.as_deref().is_some_and(|t| !t.trim().is_empty()) {
                            Some(ImageProbe::Absent)
                        } else {
                            self.paste_jobs.pop_front();
                            self.show_toast("Clipboard did not answer. Try again.", true);
                            continue;
                        }
                    }
                    Err(TryRecvError::Empty) => return true,
                }
            };
            let Some(job) = self.paste_jobs.pop_front() else { break };
            if let Some(probe) = answer {
                self.finish_system_paste(job, probe);
            }
        }
        false
    }

    /// Blocks until every started paste is finished, so a test sees the layers right after the frame that pasted.
    #[cfg(test)]
    pub fn settle_paste_jobs(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.poll_paste_jobs() {
            assert!(Instant::now() < deadline, "the clipboard worker never answered");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Makes every waiting paste overdue, as if ten seconds had passed.
    #[cfg(test)]
    pub fn expire_paste_jobs(&mut self) {
        for job in &mut self.paste_jobs {
            job.deadline = Instant::now();
        }
    }

    fn finish_system_paste(&mut self, job: PasteJob, probe: ImageProbe) {
        // The frame it was aimed at may be gone by now.
        let landing = if self.doc.get(job.landing.parent).is_some() { job.landing } else { paste_placement::paste_landing(self) };
        match probe {
            ImageProbe::Image(png) => paste_placement::paste_image(self, &landing, png),
            ImageProbe::Unreadable => self.show_toast("Copied image is unreadable. Copy it again.", true),
            ImageProbe::Absent => match job.text.as_deref() {
                Some(text) => {
                    if !paste_placement::paste_text(self, &landing, text) {
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
}
