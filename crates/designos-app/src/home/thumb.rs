//! File thumbnails: rendering the first frame to a PNG, and the texture cache the home screen draws from.

use designos_doc::{Document, DocumentKind, NodeId};
use designos_library::Library;
use std::collections::HashMap;
use std::time::SystemTime;

#[cfg(test)]
std::thread_local! {
    static BACKGROUND_REFRESH_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Longest edge of a stored thumbnail in pixels.
pub const THUMB_PX: f32 = 480.0;

/// The node a thumbnail shows: the first top-level frame of the first page, else its first layer.
pub fn thumbnail_node(doc: &Document) -> Option<NodeId> {
    let page = doc.get(*doc.pages.first()?)?;
    let kids: Vec<_> = page.children.iter().filter_map(|c| doc.get(*c)).filter(|n| n.visible).collect();
    kids.iter().find(|n| n.kind.frame_props().is_some() && !matches!(n.kind, designos_doc::NodeKind::Instance { .. })).or(kids.first()).map(|n| n.id)
}

/// Size of a board thumbnail: the whole content on the dotted canvas, 8:5 like the cards.
const BOARD_THUMB: (u32, u32) = (THUMB_PX as u32, (THUMB_PX * 0.625) as u32);

/// The whole first page of a board on its dotted canvas, fitted to [`BOARD_THUMB`] with some air
/// around the content; `None` for an empty board.
fn board_thumbnail_png(doc: &Document) -> Option<Vec<u8>> {
    use designos_render::tiny_skia as sk;
    let page = *doc.pages.first()?;
    let kids = &doc.get(page)?.children;
    let mut bounds: Option<designos_doc::Rect> = None;
    for k in kids {
        let n = doc.get(*k).filter(|n| n.visible)?;
        let w = doc.world_transform(*k);
        let corners = [
            w.apply(designos_doc::Vec2::ZERO),
            w.apply(designos_doc::Vec2::new(n.size.x, 0.0)),
            w.apply(n.size),
            w.apply(designos_doc::Vec2::new(0.0, n.size.y)),
        ];
        let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.x), a.1.max(c.x)));
        let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |a, c| (a.0.min(c.y), a.1.max(c.y)));
        let r = designos_doc::Rect::new(x0, y0, x1 - x0, y1 - y0);
        bounds = Some(bounds.map_or(r, |b| b.union(&r)));
    }
    let b = bounds?;
    let (tw, th) = (BOARD_THUMB.0 as f32, BOARD_THUMB.1 as f32);
    let pad = 24.0;
    let zoom = ((tw - 2.0 * pad) / b.w.max(1.0)).min((th - 2.0 * pad) / b.h.max(1.0)).clamp(0.001, 1.0);
    let offset = designos_doc::Vec2::new(b.x + b.w / 2.0 - tw / 2.0 / zoom, b.y + b.h / 2.0 - th / 2.0 / zoom);
    let mut content = sk::Pixmap::new(BOARD_THUMB.0, BOARD_THUMB.1)?;
    designos_render::render_page(doc, page, &designos_render::Viewport { offset, zoom, dpr: 1.0 }, &mut content);
    // the canvas colour and its dots go underneath (the page itself is transparent)
    let mut out = sk::Pixmap::new(BOARD_THUMB.0, BOARD_THUMB.1)?;
    let bg = crate::theme::BOARD_BG;
    out.fill(sk::Color::from_rgba8((bg >> 16) as u8, (bg >> 8) as u8, bg as u8, 255));
    let mut dot = sk::Paint::default();
    dot.set_color(sk::Color::from_rgba8(0, 0, 0, 46));
    let step = 16.0;
    let mut y = step / 2.0;
    while y < th {
        let mut x = step / 2.0;
        while x < tw {
            if let Some(r) = sk::Rect::from_xywh(x - 0.75, y - 0.75, 1.5, 1.5) {
                out.fill_rect(r, &dot, sk::Transform::identity(), None);
            }
            x += step;
        }
        y += step;
    }
    out.draw_pixmap(0, 0, content.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
    out.encode_png().ok()
}

/// PNG bytes of the document's thumbnail, scaled to fit [`THUMB_PX`]; `None` for an empty document.
/// A board shows its whole first page on the dotted canvas instead of one frame.
pub fn thumbnail_png(doc: &Document) -> Option<Vec<u8>> {
    if doc.kind == DocumentKind::Board {
        return board_thumbnail_png(doc);
    }
    let id = thumbnail_node(doc)?;
    let size = doc.get(id)?.size;
    let longest = size.x.max(size.y);
    if !longest.is_finite() || longest < 1.0 {
        return None;
    }
    let scale = (THUMB_PX / longest).clamp(0.01, 2.0);
    let pm = designos_render::render_node(doc, id, scale)?;
    pm.encode_png().ok()
}

/// Renders and stores the thumbnail now.
pub fn refresh(lib: &Library, file: &str, doc: &Document) -> bool {
    match thumbnail_png(doc) {
        Some(png) => lib.set_thumbnail(file, &png).is_ok(),
        None => false,
    }
}

/// Renders and stores the thumbnail on a background thread (saving must not stutter).
pub fn refresh_in_background(lib: Library, file: String, doc: Document) {
    #[cfg(test)]
    BACKGROUND_REFRESH_CALLS.with(|calls| calls.set(calls.get() + 1));
    std::thread::spawn(move || {
        refresh(&lib, &file, &doc);
    });
}

#[cfg(test)]
pub(crate) fn take_background_refresh_calls() -> usize {
    BACKGROUND_REFRESH_CALLS.with(|calls| calls.replace(0))
}

/// Which files are boards, read from the documents (each file is probed once, and again after it
/// was modified). A file not probed yet counts as a design file for the frame.
#[derive(Default)]
pub struct Kinds {
    map: HashMap<String, (u64, DocumentKind)>,
}

impl Kinds {
    /// The kind of `entry`; at most `budget` files are read per call.
    pub fn get(&mut self, lib: &Library, entry: &designos_library::FileEntry, budget: &mut u32) -> DocumentKind {
        if let Some((modified, kind)) = self.map.get(&entry.id) {
            if *modified == entry.modified {
                return *kind;
            }
        }
        if *budget == 0 {
            return self.map.get(&entry.id).map(|k| k.1).unwrap_or_default();
        }
        *budget -= 1;
        let kind = lib.file_kind(&entry.id).unwrap_or_default();
        self.map.insert(entry.id.clone(), (entry.modified, kind));
        kind
    }
}

/// Decoded thumbnails as GPU textures, reloaded when the PNG changes on disk.
#[derive(Default)]
pub struct Textures {
    map: HashMap<String, (Option<SystemTime>, Option<egui::TextureHandle>)>,
}

impl Textures {
    /// The texture for `file`'s thumbnail, loading it if new or changed. At most
    /// `budget` decodes happen per call so a big grid does not stall a frame.
    pub fn get(&mut self, ctx: &egui::Context, lib: &Library, entry: &designos_library::FileEntry, budget: &mut u32) -> Option<(egui::TextureId, egui::Vec2)> {
        let path = lib.thumbnail_path(entry)?;
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if let Some((seen, tex)) = self.map.get(&entry.id) {
            if *seen == mtime {
                return tex.as_ref().map(|t| (t.id(), t.size_vec2()));
            }
        }
        if *budget == 0 {
            ctx.request_repaint();
            return self.map.get(&entry.id).and_then(|(_, t)| t.as_ref().map(|t| (t.id(), t.size_vec2())));
        }
        *budget -= 1;
        let tex = std::fs::read(&path).ok().and_then(|bytes| image::load_from_memory(&bytes).ok()).map(|img| {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            ctx.load_texture(format!("thumb-{}", entry.id), egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), egui::TextureOptions::LINEAR)
        });
        let out = tex.as_ref().map(|t| (t.id(), t.size_vec2()));
        self.map.insert(entry.id.clone(), (mtime, tex));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, NodeKind, Rect, Vec2};

    fn doc_with_frame() -> Document {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let rect = d.alloc_id();
        d.insert(page, Node::rectangle(rect, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
        let fid = d.alloc_id();
        let mut f = Node::new(fid, NodeKind::Frame(Default::default()));
        f.size = Vec2::new(960.0, 540.0);
        d.insert(page, f).unwrap();
        d
    }

    #[test]
    fn picks_the_first_frame_over_loose_layers() {
        let d = doc_with_frame();
        let id = thumbnail_node(&d).unwrap();
        assert!(matches!(d.get(id).unwrap().kind, NodeKind::Frame(_)));
    }

    #[test]
    fn png_fits_the_thumbnail_size() {
        let d = doc_with_frame();
        let png = thumbnail_png(&d).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.width(), THUMB_PX as u32);
        assert!((img.height() as f32 - 270.0).abs() <= 1.0);
    }

    #[test]
    fn empty_documents_have_no_thumbnail() {
        let mut d = Document::new("e");
        d.add_page("P");
        assert!(thumbnail_node(&d).is_none());
        assert!(thumbnail_png(&d).is_none());
    }
}
