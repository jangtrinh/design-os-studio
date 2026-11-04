//! Prototype-only video: what a "video fill" is, how its file is understood and how it plays in
//! Present.
//!
//! A video fill is an ordinary image paint (the poster frame, shown everywhere: editor, exports,
//! other renderers) plus a [`VideoFill`] sidecar on the layer's prototype settings; the video file
//! itself sits in `Document::images` under the sidecar's `source` key. Nothing here changes the
//! document: Present swaps the poster for the current frame on its scratch copy.
//!
//! Decoding is deliberately modest and pure Rust: MP4/MOV files whose video track is Motion JPEG
//! play (each sample is a JPEG, decoded with the `image` crate). Every other codec (H.264, HEVC,
//! VP8/VP9, AV1) is recognised from the container, keeps its poster and gets an "Unsupported
//! codec" badge. A native H.264 decoder lives behind the off-by-default `video` feature (see
//! `Cargo.toml`) so the default build, and CI, stay pure Rust.

use crate::proto::player::Player;
use designos_doc::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Container extensions the placement dialog accepts.
pub const VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "m4v", "mov", "webm"];

pub fn is_video_extension(ext: &str) -> bool {
    VIDEO_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// Video codecs told apart by container metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Codec {
    MotionJpeg,
    H264,
    Hevc,
    Vp8,
    Vp9,
    Av1,
    Other(String),
}

impl Codec {
    pub fn name(&self) -> String {
        match self {
            Codec::MotionJpeg => "Motion JPEG".into(),
            Codec::H264 => "H.264".into(),
            Codec::Hevc => "HEVC".into(),
            Codec::Vp8 => "VP8".into(),
            Codec::Vp9 => "VP9".into(),
            Codec::Av1 => "AV1".into(),
            Codec::Other(s) => s.clone(),
        }
    }

    fn from_fourcc(cc: &[u8]) -> Codec {
        match cc {
            b"jpeg" | b"mjpa" | b"mjpb" | b"MJPG" => Codec::MotionJpeg,
            b"avc1" | b"avc3" => Codec::H264,
            b"hvc1" | b"hev1" => Codec::Hevc,
            b"vp08" => Codec::Vp8,
            b"vp09" => Codec::Vp9,
            b"av01" => Codec::Av1,
            other => Codec::Other(String::from_utf8_lossy(other).trim().to_string()),
        }
    }
}

/// What a video file is, without decoding any pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub container: VideoContainer,
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    /// This build has a decoder for the video (and the file carries what it needs).
    pub decodable: bool,
}

impl VideoInfo {
    /// Whether this build can play the video.
    pub fn playable(&self) -> bool {
        self.decodable
    }

    /// Why the video does not play, for the badge.
    pub fn unsupported_reason(&self) -> Option<String> {
        if self.decodable {
            None
        } else if self.codec == Codec::H264 && self.container == VideoContainer::Mp4 && !cfg!(feature = "video") {
            Some("Unsupported codec: H.264 (needs the video feature)".into())
        } else {
            Some(format!("Unsupported codec: {}", self.codec.name()))
        }
    }
}

// ------------------------------------------------------------------ MP4 demux

struct Sample {
    offset: u64,
    size: u32,
    /// Presentation start, in track timescale units.
    start: u64,
}

struct Track {
    codec: Codec,
    width: u32,
    height: u32,
    timescale: u32,
    samples: Vec<Sample>,
    /// Sample numbers (0-based) a decoder can start at; empty means every sample. Only inter-frame
    /// codecs (H.264, behind the `video` feature) need it.
    #[cfg_attr(not(feature = "video"), allow(dead_code))]
    sync: Vec<usize>,
    /// The `avcC` configuration record of an H.264 track.
    avcc: Option<Vec<u8>>,
}

impl Track {
    /// Whether this build can decode the track.
    fn decodable(&self) -> bool {
        match self.codec {
            Codec::MotionJpeg => true,
            Codec::H264 => cfg!(feature = "video") && self.avcc.is_some(),
            _ => false,
        }
    }

    fn duration(&self) -> f64 {
        let last = self.samples.last().map(|s| s.start).unwrap_or(0);
        let per = if self.samples.len() > 1 { last / (self.samples.len() as u64 - 1) } else { 0 };
        (last + per) as f64 / self.timescale as f64
    }

    /// Index of the sample shown at `t` seconds.
    fn sample_at(&self, t: f64) -> usize {
        let ticks = (t.max(0.0) * self.timescale as f64) as u64;
        self.samples.partition_point(|s| s.start <= ticks).saturating_sub(1)
    }
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}
fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

/// Child boxes of `data` as (fourcc, payload).
fn boxes(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p + 8 <= data.len() && out.len() < 4096 {
        let Some(size32) = be32(data, p) else { break };
        let ty: [u8; 4] = data[p + 4..p + 8].try_into().unwrap_or([0; 4]);
        let (hdr, size) = match size32 {
            0 => (8, data.len() - p),
            1 => match be64(data, p + 8) {
                Some(s) => (16, s as usize),
                None => break,
            },
            s => (8, s as usize),
        };
        if size < hdr || p + size > data.len() {
            break;
        }
        out.push((ty, &data[p + hdr..p + size]));
        p += size;
    }
    out
}

fn child<'a>(data: &'a [u8], ty: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).into_iter().find(|(t, _)| t == ty).map(|(_, b)| b)
}

/// The first video track of an MP4/MOV file.
fn video_track(file: &[u8]) -> Option<Track> {
    let moov = child(file, b"moov")?;
    for (ty, trak) in boxes(moov) {
        if &ty != b"trak" {
            continue;
        }
        let Some(mdia) = child(trak, b"mdia") else { continue };
        // hdlr: version/flags(4) pre_defined(4) handler_type(4)
        let is_video = child(mdia, b"hdlr").and_then(|h| h.get(8..12)).is_some_and(|t| t == b"vide");
        if !is_video {
            continue;
        }
        let mdhd = child(mdia, b"mdhd")?;
        let timescale = if mdhd.first() == Some(&1) { be32(mdhd, 20)? } else { be32(mdhd, 12)? };
        let stbl = child(child(mdia, b"minf")?, b"stbl")?;
        // stsd: version/flags(4) entry_count(4) then the first sample entry
        let stsd = child(stbl, b"stsd")?;
        let entry = stsd.get(8..)?;
        let fourcc = entry.get(4..8)?;
        let codec = Codec::from_fourcc(fourcc);
        let (width, height) = (be16(entry, 32)? as u32, be16(entry, 34)? as u32);
        // A visual sample entry is 86 bytes (box header and fixed fields), then child boxes such as avcC.
        let entry_len = (be32(entry, 0)? as usize).min(entry.len());
        let avcc = entry.get(86..entry_len).and_then(|kids| child(kids, b"avcC")).map(|b| b.to_vec());
        // stss: sync samples (1-based); absent means all are
        let sync: Vec<usize> = match child(stbl, b"stss") {
            Some(s) => {
                let n = (be32(s, 4)? as usize).min(1 << 22);
                (0..n).filter_map(|i| be32(s, 8 + i * 4)).map(|v| (v as usize).saturating_sub(1)).collect()
            }
            None => Vec::new(),
        };
        // stts: durations
        let stts = child(stbl, b"stts")?;
        let n = be32(stts, 4)? as usize;
        let mut starts = Vec::new();
        let mut t = 0u64;
        for i in 0..n.min(1 << 20) {
            let (count, delta) = (be32(stts, 8 + i * 8)?, be32(stts, 12 + i * 8)?);
            for _ in 0..count.min(1 << 22) {
                if starts.len() >= (1 << 22) {
                    break;
                }
                starts.push(t);
                t += delta as u64;
            }
        }
        // stsz: sizes
        let stsz = child(stbl, b"stsz")?;
        let fixed = be32(stsz, 4)?;
        let count = (be32(stsz, 8)? as usize).min(1 << 22);
        let size_of = |i: usize| if fixed != 0 { Some(fixed) } else { be32(stsz, 12 + i * 4) };
        // stsc: samples per chunk
        let stsc = child(stbl, b"stsc")?;
        let stsc_n = be32(stsc, 4)? as usize;
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for i in 0..stsc_n.min(1 << 20) {
            runs.push((be32(stsc, 8 + i * 12)?, be32(stsc, 12 + i * 12)?));
        }
        // chunk offsets
        let (offs, wide) = match (child(stbl, b"stco"), child(stbl, b"co64")) {
            (Some(o), _) => (o, false),
            (None, Some(o)) => (o, true),
            _ => return None,
        };
        let chunks = (be32(offs, 4)? as usize).min(1 << 22);
        let chunk_off = |i: usize| if wide { be64(offs, 8 + i * 8) } else { be32(offs, 8 + i * 4).map(u64::from) };
        let mut samples = Vec::new();
        let mut idx = 0usize;
        'chunks: for c in 0..chunks {
            let per = runs.iter().rev().find(|(first, _)| (*first as usize) <= c + 1).map(|(_, p)| *p).unwrap_or(1) as usize;
            let mut off = chunk_off(c)?;
            for _ in 0..per {
                if idx >= count {
                    break 'chunks;
                }
                let size = size_of(idx)?;
                samples.push(Sample { offset: off, size, start: *starts.get(idx).unwrap_or(&t) });
                off += size as u64;
                idx += 1;
            }
        }
        if samples.is_empty() || timescale == 0 {
            return None;
        }
        return Some(Track { codec, width, height, timescale, samples, sync, avcc });
    }
    None
}

/// Identifies a video file from its container metadata (no pixel decoding).
pub fn probe(bytes: &[u8]) -> Option<VideoInfo> {
    match VideoContainer::sniff(bytes)? {
        VideoContainer::Mp4 => {
            let t = video_track(bytes);
            let t = match t {
                Some(t) => t,
                // an MP4 whose tables we cannot read (fragmented, moov at the end beyond the buffer...)
                None => {
                    return Some(VideoInfo {
                        container: VideoContainer::Mp4,
                        codec: Codec::Other("unreadable".into()),
                        width: 0,
                        height: 0,
                        duration: 0.0,
                        decodable: false,
                    })
                }
            };
            let decodable = t.decodable();
            let duration = t.duration();
            Some(VideoInfo { container: VideoContainer::Mp4, codec: t.codec, width: t.width, height: t.height, duration, decodable })
        }
        VideoContainer::WebM => {
            // Matroska names the codec in the Tracks header near the start of the file.
            let head = &bytes[..bytes.len().min(1 << 16)];
            let has = |s: &[u8]| head.windows(s.len()).any(|w| w == s);
            let codec = if has(b"V_VP9") {
                Codec::Vp9
            } else if has(b"V_VP8") {
                Codec::Vp8
            } else if has(b"V_AV1") {
                Codec::Av1
            } else if has(b"V_MPEG4/ISO/AVC") {
                Codec::H264
            } else if has(b"V_MJPEG") {
                Codec::MotionJpeg
            } else {
                Codec::Other("unknown".into())
            };
            Some(VideoInfo { container: VideoContainer::WebM, codec, width: 0, height: 0, duration: 0.0, decodable: false })
        }
    }
}

// ------------------------------------------------------------------- decoding

/// A decoded frame: straight RGBA.
#[derive(Clone)]
pub struct Frame {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Something frames can be pulled from by time.
pub trait VideoSource {
    fn duration(&self) -> f64;
    /// The frame shown at `t` seconds (clamped to the last one).
    fn frame_at(&mut self, t: f64) -> Result<Frame, String>;
}

struct MotionJpeg {
    data: Arc<Vec<u8>>,
    track: Track,
}

impl VideoSource for MotionJpeg {
    fn duration(&self) -> f64 {
        self.track.duration()
    }

    fn frame_at(&mut self, t: f64) -> Result<Frame, String> {
        let i = self.track.sample_at(t);
        let s = &self.track.samples[i];
        let (a, b) = (s.offset as usize, s.offset as usize + s.size as usize);
        let bytes = self.data.get(a..b).ok_or("Video sample is outside the file")?;
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).map_err(|e| format!("Cannot decode frame: {e}"))?.to_rgba8();
        Ok(Frame { index: i, width: img.width(), height: img.height(), rgba: img.into_raw() })
    }
}

/// Opens a playable video; the error is the reason shown on the layer.
pub fn open(data: Arc<Vec<u8>>) -> Result<(VideoInfo, Box<dyn VideoSource>), String> {
    let info = probe(&data).ok_or("Not a video file")?;
    if let Some(r) = info.unsupported_reason() {
        return Err(r);
    }
    let track = video_track(&data).ok_or("Unsupported codec: unreadable")?;
    match track.codec {
        Codec::MotionJpeg => Ok((info, Box::new(MotionJpeg { data, track }))),
        #[cfg(feature = "video")]
        Codec::H264 => {
            let src = h264::H264::new(data, track)?;
            Ok((info, Box::new(src)))
        }
        _ => Err(info.unsupported_reason().unwrap_or_else(|| "Unsupported codec".into())),
    }
}

/// H.264 through Cisco's OpenH264 (the `video` feature).
#[cfg(feature = "video")]
mod h264 {
    use super::{Frame, Track, VideoSource};
    use openh264::decoder::Decoder;
    use openh264::formats::YUVSource;
    use std::sync::Arc;

    pub struct H264 {
        data: Arc<Vec<u8>>,
        track: Track,
        decoder: Decoder,
        /// SPS and PPS as Annex B, sent before every keyframe.
        params: Vec<u8>,
        /// Bytes of the NAL length prefix (1..=4).
        nal_len: usize,
        /// The next sample the decoder expects.
        next: usize,
        last: Option<Frame>,
    }

    /// `avcC` -> (SPS+PPS in Annex B, NAL length size).
    pub fn parse_avcc(avcc: &[u8]) -> Option<(Vec<u8>, usize)> {
        let nal_len = (*avcc.get(4)? & 3) as usize + 1;
        let mut p = 5;
        let mut out = Vec::new();
        let n_sps = (*avcc.get(p)? & 0x1F) as usize;
        p += 1;
        for _ in 0..n_sps {
            let len = u16::from_be_bytes([*avcc.get(p)?, *avcc.get(p + 1)?]) as usize;
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(avcc.get(p + 2..p + 2 + len)?);
            p += 2 + len;
        }
        let n_pps = *avcc.get(p)? as usize;
        p += 1;
        for _ in 0..n_pps {
            let len = u16::from_be_bytes([*avcc.get(p)?, *avcc.get(p + 1)?]) as usize;
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(avcc.get(p + 2..p + 2 + len)?);
            p += 2 + len;
        }
        Some((out, nal_len))
    }

    /// Length-prefixed NAL units -> Annex B (start codes).
    pub fn to_annex_b(sample: &[u8], nal_len: usize, out: &mut Vec<u8>) -> bool {
        let mut p = 0;
        while p + nal_len <= sample.len() {
            let len = sample[p..p + nal_len].iter().fold(0usize, |a, b| (a << 8) | *b as usize);
            p += nal_len;
            let Some(nal) = sample.get(p..p + len) else { return false };
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(nal);
            p += len;
        }
        true
    }

    impl H264 {
        pub fn new(data: Arc<Vec<u8>>, track: Track) -> Result<Self, String> {
            let (params, nal_len) = track.avcc.as_deref().and_then(parse_avcc).ok_or("Unsupported codec: H.264 (bad avcC)")?;
            let decoder = Decoder::new().map_err(|e| format!("H.264 decoder: {e}"))?;
            Ok(H264 { data, track, decoder, params, nal_len, next: 0, last: None })
        }

        /// The sample to restart decoding from: the last keyframe at or before `i`.
        fn keyframe_before(&self, i: usize) -> usize {
            if self.track.sync.is_empty() {
                return i;
            }
            let at = self.track.sync.partition_point(|s| *s <= i);
            if at == 0 {
                0
            } else {
                self.track.sync[at - 1]
            }
        }

        fn is_sync(&self, i: usize) -> bool {
            self.track.sync.is_empty() || self.track.sync.binary_search(&i).is_ok() || i == 0
        }

        /// Feeds sample `i` to the decoder; the frame it produced, if any.
        fn feed(&mut self, i: usize) -> Result<Option<Frame>, String> {
            let s = &self.track.samples[i];
            let (a, b) = (s.offset as usize, s.offset as usize + s.size as usize);
            let bytes = self.data.get(a..b).ok_or("Video sample is outside the file")?;
            let mut packet = Vec::with_capacity(bytes.len() + self.params.len() + 16);
            if self.is_sync(i) {
                packet.extend_from_slice(&self.params);
            }
            if !to_annex_b(bytes, self.nal_len, &mut packet) {
                return Err("Corrupt H.264 sample".into());
            }
            let decoded = self.decoder.decode(&packet).map_err(|e| format!("Cannot decode frame: {e}"))?;
            let Some(yuv) = decoded else { return Ok(None) };
            let (w, h) = yuv.dimensions();
            let mut rgba = vec![255u8; w * h * 4];
            yuv.write_rgba8(&mut rgba);
            Ok(Some(Frame { index: i, width: w as u32, height: h as u32, rgba }))
        }
    }

    impl VideoSource for H264 {
        fn duration(&self) -> f64 {
            self.track.duration()
        }

        fn frame_at(&mut self, t: f64) -> Result<Frame, String> {
            let target = self.track.sample_at(t);
            if let Some(f) = self.last.as_ref().filter(|f| f.index == target) {
                return Ok(f.clone());
            }
            // continue from where the decoder is, unless that is behind the last keyframe before the
            // target (jumping to it decodes less) or ahead of the target (seeking back)
            let key = self.keyframe_before(target);
            let start = if self.last.is_some() && self.next > 0 && self.next <= target && self.next >= key {
                self.next
            } else {
                if let Ok(d) = Decoder::new() {
                    self.decoder = d;
                }
                key
            };
            let mut shown = None;
            for i in start..=target {
                if let Some(f) = self.feed(i)? {
                    shown = Some(f);
                }
                self.next = i + 1;
            }
            // a decoder with delay may hold the last frame back: show the newest one it gave us
            let frame = match shown {
                Some(mut f) => {
                    f.index = target;
                    f
                }
                None => {
                    let mut f = self.last.clone().ok_or("The decoder produced no frame yet")?;
                    f.index = target;
                    f
                }
            };
            self.last = Some(frame.clone());
            Ok(frame)
        }
    }
}

/// PNG bytes of an RGBA frame (fast compression: it is encoded every frame while playing).
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::NoFilter)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(out)
}

/// Poster for a newly placed video: its first frame when playable, else a dark card with a play
/// triangle sized like the video (or 640x360 when the size is unknown). Returns (png, width, height).
pub fn poster_png(video: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let info = probe(video);
    if let Some(i) = info.as_ref().filter(|i| i.playable()) {
        if let Ok((_, mut src)) = open(Arc::new(video.to_vec())) {
            if let Ok(f) = src.frame_at(0.0) {
                let _ = i;
                return encode_png(f.width, f.height, &f.rgba).map(|p| (p, f.width, f.height));
            }
        }
    }
    let (w, h) = info.filter(|i| i.width > 0 && i.height > 0).map(|i| (i.width, i.height)).unwrap_or((640, 360));
    placeholder_poster(w, h)
}

/// A dark card with a white play triangle.
pub fn placeholder_poster(w: u32, h: u32) -> Option<(Vec<u8>, u32, u32)> {
    let (w, h) = (w.clamp(16, 4096), h.clamp(16, 4096));
    let (cx, cy, r) = (w as f32 / 2.0, h as f32 / 2.0, (w.min(h) as f32) * 0.18);
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // triangle pointing right: x from cx - r/2 .. cx + r, |y - cy| < (cx + r - x) * 0.58
            let inside = fx >= cx - r * 0.5 && fx <= cx + r && (fy - cy).abs() <= (cx + r - fx) * 0.58;
            let v = if inside { 235 } else { 38 };
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    encode_png(w, h, &px).map(|p| (p, w, h))
}

// -------------------------------------------------------------------- Present

enum Clip {
    Ready { source: Box<dyn VideoSource>, shown: Option<usize>, poster: String },
    Unsupported(String),
}

/// Decoders of the videos shown in one Present session.
#[derive(Default)]
pub struct VideoRuntime {
    clips: HashMap<NodeId, Clip>,
    /// Frame images currently in the scratch document, per layer, so old ones can be dropped.
    frame_keys: HashMap<NodeId, String>,
}

impl VideoRuntime {
    /// Layers whose video cannot play, with the reason (for the badge).
    pub fn unsupported_nodes(&self) -> Vec<(NodeId, String)> {
        self.clips.iter().filter_map(|(n, c)| if let Clip::Unsupported(r) = c { Some((*n, r.clone())) } else { None }).collect()
    }

    /// Opens clips on first use and swaps the poster of every playing (or seeked) video for the
    /// frame at its position, on the player's scratch document. Returns whether anything changed.
    pub fn update(&mut self, player: &mut Player, now: f64) -> bool {
        let nodes: Vec<NodeId> = player.media.keys().copied().collect();
        let mut changed = false;
        for node in nodes {
            let Some(fill) = player.doc.get(node).and_then(|n| n.prototype.videos.first().cloned()) else { continue };
            if let std::collections::hash_map::Entry::Vacant(slot) = self.clips.entry(node) {
                let clip = match player.doc.images.get(&fill.source) {
                    None => Clip::Unsupported("Video file is missing".into()),
                    Some(bytes) => match open(Arc::new(bytes.clone())) {
                        Ok((_, source)) => {
                            let poster = match player.doc.get(node).and_then(|n| n.fills.get(fill.paint)).map(|p| &p.kind) {
                                Some(PaintKind::Image { image_ref, .. }) => image_ref.clone(),
                                _ => String::new(),
                            };
                            player.set_media_duration(node, source.duration());
                            Clip::Ready { source, shown: None, poster }
                        }
                        Err(r) => Clip::Unsupported(r),
                    },
                };
                slot.insert(clip);
            }
            let Some(Clip::Ready { source, shown, poster }) = self.clips.get_mut(&node) else { continue };
            let Some(state) = player.media.get(&node) else { continue };
            let t = state.position(now);
            let Ok(frame) = source.frame_at(t) else { continue };
            if *shown == Some(frame.index) {
                continue;
            }
            let Some(png) = encode_png(frame.width, frame.height, &frame.rgba) else { continue };
            let key = format!("{poster}~f{}", frame.index);
            if let Some(old) = self.frame_keys.insert(node, key.clone()) {
                player.doc.images.remove(&old);
            }
            player.doc.images.insert(key.clone(), png);
            if let Some(PaintKind::Image { image_ref, .. }) = player.doc.get_mut(node).and_then(|n| n.fills.get_mut(fill.paint)).map(|p| &mut p.kind) {
                *image_ref = key;
            }
            *shown = Some(frame.index);
            player.invalidate(node);
            changed = true;
        }
        changed
    }
}

/// Builds a minimal MP4 with one Motion JPEG track (used by tests and fixtures).
#[cfg(test)]
pub fn build_mjpeg_mp4(frames: &[Vec<u8>], width: u16, height: u16, fps: u32) -> Vec<u8> {
    build_mp4(b"jpeg", &[], frames, None, width, height, fps)
}

/// Builds a minimal MP4 with one video track: sample entry `fourcc` followed by `entry_kids`
/// (child boxes such as `avcC`), one sample per frame, optional sync sample list (1-based).
#[cfg(test)]
pub fn build_mp4(fourcc: &[u8; 4], entry_kids: &[u8], frames: &[Vec<u8>], sync: Option<&[u32]>, width: u16, height: u16, fps: u32) -> Vec<u8> {
    fn bx(ty: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(ty);
        v.extend_from_slice(payload);
        v
    }
    let u32b = |x: u32| x.to_be_bytes().to_vec();
    let ftyp = bx(b"ftyp", b"isom\0\0\0\0isom");
    let mdat_payload: Vec<u8> = frames.iter().flatten().copied().collect();
    let mdat = bx(b"mdat", &mdat_payload);
    let mdat_start = ftyp.len() + 8; // first frame
    let mut hdlr = vec![0; 8];
    hdlr.extend_from_slice(b"vide");
    hdlr.extend_from_slice(&[0; 13]);
    let mut mdhd = vec![0; 4];
    mdhd.extend([u32b(0), u32b(0), u32b(fps), u32b(frames.len() as u32), vec![0, 0, 0, 0]].concat());
    // a visual sample entry: 86 bytes of box header and fixed fields, then child boxes
    let mut entry = vec![0u8; 86];
    entry[4..8].copy_from_slice(fourcc);
    entry[32..34].copy_from_slice(&width.to_be_bytes());
    entry[34..36].copy_from_slice(&height.to_be_bytes());
    entry.extend_from_slice(entry_kids);
    let len = entry.len() as u32;
    entry[0..4].copy_from_slice(&len.to_be_bytes());
    let stsd = bx(b"stsd", &[vec![0; 4], u32b(1), entry].concat());
    let stts = bx(b"stts", &[vec![0; 4], u32b(1), u32b(frames.len() as u32), u32b(1)].concat());
    let stsc = bx(b"stsc", &[vec![0; 4], u32b(1), u32b(1), u32b(frames.len() as u32), u32b(1)].concat());
    let sizes: Vec<u8> = frames.iter().flat_map(|f| u32b(f.len() as u32)).collect();
    let stsz = bx(b"stsz", &[vec![0; 4], u32b(0), u32b(frames.len() as u32), sizes].concat());
    let stco = bx(b"stco", &[vec![0; 4], u32b(1), u32b(mdat_start as u32)].concat());
    let stss = sync.map(|s| bx(b"stss", &[vec![0; 4], u32b(s.len() as u32), s.iter().flat_map(|v| u32b(*v)).collect()].concat())).unwrap_or_default();
    let stbl = bx(b"stbl", &[stsd, stts, stsc, stsz, stco, stss].concat());
    let minf = bx(b"minf", &stbl);
    let mdia = bx(b"mdia", &[bx(b"mdhd", &mdhd), bx(b"hdlr", &hdlr), minf].concat());
    let moov = bx(b"moov", &bx(b"trak", &mdia));
    [ftyp, mdat, moov].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(16, 16, image::Rgb([r, g, b]));
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 95).encode_image(&img).unwrap();
        out
    }

    fn clip() -> Vec<u8> {
        build_mjpeg_mp4(&[jpeg(255, 0, 0), jpeg(0, 255, 0), jpeg(0, 0, 255)], 16, 16, 2)
    }

    #[test]
    fn probe_reads_codec_size_and_duration() {
        let info = probe(&clip()).unwrap();
        assert_eq!(info.container, VideoContainer::Mp4);
        assert_eq!(info.codec, Codec::MotionJpeg);
        assert_eq!((info.width, info.height), (16, 16));
        assert!((info.duration - 1.5).abs() < 1e-6, "3 frames at 2 fps: {}", info.duration);
        assert!(info.playable() && info.unsupported_reason().is_none());
    }

    #[test]
    fn frames_are_picked_by_time() {
        let (_, mut src) = open(Arc::new(clip())).unwrap();
        let px = |f: &Frame| (f.rgba[0], f.rgba[1], f.rgba[2]);
        let f0 = src.frame_at(0.0).unwrap();
        assert_eq!(f0.index, 0);
        assert!(px(&f0).0 > 200 && px(&f0).1 < 60);
        let f1 = src.frame_at(0.6).unwrap();
        assert_eq!(f1.index, 1);
        assert!(px(&f1).1 > 200);
        let f2 = src.frame_at(99.0).unwrap();
        assert_eq!(f2.index, 2, "clamped to the last frame");
        assert!(px(&f2).2 > 200);
        assert_eq!(src.frame_at(-3.0).unwrap().index, 0);
        assert!((src.duration() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn other_codecs_are_recognised_but_not_played() {
        let mut mp4 = clip();
        let at = mp4.windows(4).position(|w| w == b"jpeg").unwrap();
        mp4[at..at + 4].copy_from_slice(b"avc1");
        let info = probe(&mp4).unwrap();
        assert_eq!(info.codec, Codec::H264);
        assert!(info.unsupported_reason().unwrap().starts_with("Unsupported codec: H.264"));
        assert!(open(Arc::new(mp4)).err().unwrap().contains("H.264"));
        // WebM: codec id found in the header
        let mut webm = vec![0x1A, 0x45, 0xDF, 0xA3, 0x80];
        webm.extend_from_slice(b"....V_VP9....");
        let info = probe(&webm).unwrap();
        assert_eq!((info.container, info.codec.clone()), (VideoContainer::WebM, Codec::Vp9));
        assert!(!info.playable());
        assert!(open(Arc::new(webm)).err().unwrap().contains("VP9"));
    }

    #[test]
    fn garbage_is_not_a_video_and_never_panics() {
        assert!(probe(b"hello world").is_none());
        assert!(probe(&[]).is_none());
        let mut broken = clip();
        broken.truncate(broken.len() - 30);
        let info = probe(&broken).unwrap();
        assert!(!info.playable(), "moov cut off");
        for n in 0..clip().len().min(300) {
            let _ = probe(&clip()[..n]);
        }
        assert!(open(Arc::new(b"nope".to_vec())).is_err());
    }

    #[test]
    fn posters() {
        let (png, w, h) = poster_png(&clip()).unwrap();
        assert_eq!((w, h), (16, 16));
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert!(img.get_pixel(8, 8)[0] > 200, "first frame is red");
        // an unplayable video gets the placeholder at its size
        let mut mp4 = clip();
        let at = mp4.windows(4).position(|w| w == b"jpeg").unwrap();
        mp4[at..at + 4].copy_from_slice(b"hvc1");
        let (png, w, h) = poster_png(&mp4).unwrap();
        assert_eq!((w, h), (16, 16));
        assert!(image::load_from_memory(&png).is_ok());
        let (_, w, h) = poster_png(b"\x1a\x45\xdf\xa3 not really").unwrap();
        assert_eq!((w, h), (640, 360));
    }

    /// A frame with a video layer: poster image paint plus sidecar, the file in `images`.
    fn video_doc(mp4: Vec<u8>) -> (Document, NodeId, NodeId) {
        let mut d = Document::new("v");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let frame = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let (poster, _, _) = placeholder_poster(16, 16).unwrap();
        d.images.insert("poster".into(), poster);
        d.images.insert("video-1".into(), mp4);
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 64.0, 64.0));
        n.fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "poster".into(), scale_mode: ImageScaleMode::Fill, params: Default::default() },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        n.prototype.videos = vec![VideoFill { paint: 0, source: "video-1".into(), autoplay: true, looping: true, muted: true }];
        let clip = d.insert(frame, n).unwrap();
        (d, frame, clip)
    }

    fn image_ref(p: &Player, node: NodeId) -> String {
        match &p.doc.get(node).unwrap().fills[0].kind {
            PaintKind::Image { image_ref, .. } => image_ref.clone(),
            _ => panic!(),
        }
    }

    #[test]
    fn runtime_swaps_the_poster_for_the_frame_at_the_playhead() {
        let (d, frame, clip) = video_doc(clip());
        let mut p = Player::new(&d, frame);
        let mut rt = VideoRuntime::default();
        assert!(rt.update(&mut p, 0.0), "the first frame replaces the poster");
        let k0 = image_ref(&p, clip);
        assert_eq!(k0, "poster~f0");
        let rev = p.frame_rev(frame);
        assert!(!rt.update(&mut p, 0.1), "same frame: nothing to redraw");
        assert_eq!(p.frame_rev(frame), rev);
        assert!(rt.update(&mut p, 0.6), "2 fps: frame 1 from 0.5s");
        assert_eq!(image_ref(&p, clip), "poster~f1");
        assert!(p.frame_rev(frame) > rev, "the frame's texture is invalidated");
        assert!(!p.doc.images.contains_key(&k0), "old frame images are dropped");
        let png = image::load_from_memory(&p.doc.images["poster~f1"]).unwrap().to_rgba8();
        assert!(png.get_pixel(8, 8)[1] > 200, "the second frame is green");
        // looping wraps around: 1.6s is frame 0 again (1.5s long)
        assert!(rt.update(&mut p, 1.6));
        assert_eq!(image_ref(&p, clip), "poster~f0");
        // pausing freezes the picture
        p.media_control(clip, MediaOp::Pause);
        let at = image_ref(&p, clip);
        rt.update(&mut p, 50.0);
        assert_eq!(image_ref(&p, clip), at);
        // the user's document never saw any of it
        assert_eq!(
            match &d.get(clip).unwrap().fills[0].kind {
                PaintKind::Image { image_ref, .. } => image_ref.as_str(),
                _ => "",
            },
            "poster"
        );
        assert!(d.images.keys().all(|k| !k.contains('~')));
    }

    #[test]
    fn seeking_and_skipping_change_the_frame_even_while_paused() {
        let (mut d, frame, clip) = video_doc(clip());
        d.get_mut(clip).unwrap().prototype.videos[0].autoplay = false;
        let mut p = Player::new(&d, frame);
        let mut rt = VideoRuntime::default();
        rt.update(&mut p, 0.0);
        assert_eq!(image_ref(&p, clip), "poster~f0");
        p.media_control(clip, MediaOp::SeekTo(1.2));
        rt.update(&mut p, 0.0);
        assert_eq!(image_ref(&p, clip), "poster~f2");
        p.media_control(clip, MediaOp::SkipBy(-0.7));
        rt.update(&mut p, 0.0);
        assert_eq!(image_ref(&p, clip), "poster~f1");
        assert!((p.media[&clip].duration.unwrap() - 1.5).abs() < 1e-6, "the runtime reports the length to the player");
    }

    #[test]
    fn unsupported_videos_keep_their_poster_and_get_a_reason() {
        let mut mp4 = clip();
        let at = mp4.windows(4).position(|w| w == b"jpeg").unwrap();
        mp4[at..at + 4].copy_from_slice(b"hvc1");
        let (d, frame, clip_node) = video_doc(mp4);
        let mut p = Player::new(&d, frame);
        let mut rt = VideoRuntime::default();
        assert!(!rt.update(&mut p, 1.0));
        assert_eq!(image_ref(&p, clip_node), "poster");
        assert_eq!(rt.unsupported_nodes(), vec![(clip_node, "Unsupported codec: HEVC".to_string())]);
        // a missing file is reported the same way
        let (mut d, frame, clip_node) = video_doc(clip());
        d.images.remove("video-1");
        let mut p = Player::new(&d, frame);
        let mut rt = VideoRuntime::default();
        rt.update(&mut p, 0.0);
        assert_eq!(rt.unsupported_nodes(), vec![(clip_node, "Video file is missing".to_string())]);
    }

    /// Splits an Annex B stream into NAL units (start codes removed).
    #[cfg(feature = "video")]
    fn split_annex_b(s: &[u8]) -> Vec<&[u8]> {
        let mut starts = Vec::new();
        let mut i = 0;
        while i + 3 <= s.len() {
            if s[i..i + 3] == [0, 0, 1] {
                starts.push(i + 3);
                i += 3;
            } else {
                i += 1;
            }
        }
        let mut out = Vec::new();
        for (k, a) in starts.iter().enumerate() {
            let mut b = starts.get(k + 1).map(|n| n - 3).unwrap_or(s.len());
            while b > *a && s[b - 1] == 0 {
                b -= 1;
            }
            out.push(&s[*a..b]);
        }
        out
    }

    /// Encodes solid-colour frames with OpenH264 and wraps them in an MP4 (avc1 + avcC + stss).
    #[cfg(feature = "video")]
    fn h264_clip(colors: &[(u8, u8, u8)]) -> Vec<u8> {
        use openh264::encoder::Encoder;
        use openh264::formats::{RgbSliceU8, YUVBuffer};
        let (w, h) = (64usize, 64usize);
        let mut enc = Encoder::new().unwrap();
        let (mut sps, mut pps) = (Vec::new(), Vec::new());
        let (mut samples, mut sync) = (Vec::new(), Vec::new());
        for (n, c) in colors.iter().enumerate() {
            let rgb: Vec<u8> = (0..w * h).flat_map(|_| [c.0, c.1, c.2]).collect();
            let yuv = YUVBuffer::from_rgb8_source(RgbSliceU8::new(&rgb, (w, h)));
            let stream = enc.encode(&yuv).unwrap().to_vec();
            let mut sample = Vec::new();
            let mut is_idr = false;
            for nal in split_annex_b(&stream) {
                match nal[0] & 0x1F {
                    7 => sps = nal.to_vec(),
                    8 => pps = nal.to_vec(),
                    t => {
                        is_idr |= t == 5;
                        sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                        sample.extend_from_slice(nal);
                    }
                }
            }
            if is_idr {
                sync.push(n as u32 + 1);
            }
            samples.push(sample);
        }
        assert!(!sps.is_empty() && !pps.is_empty() && !sync.is_empty());
        let mut avcc = vec![1, sps[1], sps[2], sps[3], 0xFF, 0xE1];
        avcc.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&sps);
        avcc.push(1);
        avcc.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&pps);
        let mut avcc_box = ((avcc.len() + 8) as u32).to_be_bytes().to_vec();
        avcc_box.extend_from_slice(b"avcC");
        avcc_box.extend_from_slice(&avcc);
        build_mp4(b"avc1", &avcc_box, &samples, Some(&sync), w as u16, h as u16, 2)
    }

    #[cfg(feature = "video")]
    #[test]
    fn h264_plays_through_openh264() {
        let mp4 = h264_clip(&[(230, 20, 20), (20, 230, 20), (20, 20, 230)]);
        let info = probe(&mp4).unwrap();
        assert_eq!((info.codec.clone(), info.decodable), (Codec::H264, true));
        assert_eq!((info.width, info.height), (64, 64));
        assert!(info.unsupported_reason().is_none());
        let (_, mut src) = open(Arc::new(mp4.clone())).unwrap();
        let dominant = |f: &Frame| {
            let p = &f.rgba[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 3];
            (p[0] > 150, p[1] > 150, p[2] > 150)
        };
        assert_eq!(dominant(&src.frame_at(0.0).unwrap()), (true, false, false));
        assert_eq!(dominant(&src.frame_at(0.6).unwrap()), (false, true, false));
        assert_eq!(dominant(&src.frame_at(1.2).unwrap()), (false, false, true));
        // seeking back restarts at a keyframe
        let f = src.frame_at(0.0).unwrap();
        assert_eq!((f.index, dominant(&f)), (0, (true, false, false)));
        assert!((src.duration() - 1.5).abs() < 1e-6);
        // the poster is the first frame
        let (png, w, h) = poster_png(&mp4).unwrap();
        assert_eq!((w, h), (64, 64));
        assert!(image::load_from_memory(&png).unwrap().to_rgba8().get_pixel(32, 32)[0] > 150);
    }

    #[cfg(feature = "video")]
    #[test]
    fn h264_without_its_configuration_is_reported_not_played() {
        let mut mp4 = h264_clip(&[(230, 20, 20)]);
        let at = mp4.windows(4).position(|w| w == b"avcC").unwrap();
        mp4[at..at + 4].copy_from_slice(b"free");
        let info = probe(&mp4).unwrap();
        assert!(!info.decodable);
        assert!(open(Arc::new(mp4)).is_err());
    }

    #[cfg(not(feature = "video"))]
    #[test]
    fn h264_says_it_needs_the_video_feature_in_the_default_build() {
        let mut mp4 = clip();
        let at = mp4.windows(4).position(|w| w == b"jpeg").unwrap();
        mp4[at..at + 4].copy_from_slice(b"avc1");
        let info = probe(&mp4).unwrap();
        assert_eq!(info.unsupported_reason().as_deref(), Some("Unsupported codec: H.264 (needs the video feature)"));
    }

    #[cfg(feature = "video")]
    #[test]
    fn avcc_and_nal_conversion() {
        let avcc = [1, 66, 0, 30, 0xFF, 0xE1, 0, 3, 0x67, 1, 2, 1, 0, 2, 0x68, 9];
        let (params, n) = super::h264::parse_avcc(&avcc).unwrap();
        assert_eq!(n, 4);
        assert_eq!(params, [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 0, 1, 0x68, 9]);
        let mut out = Vec::new();
        assert!(super::h264::to_annex_b(&[0, 0, 0, 2, 0x65, 7, 0, 0, 0, 1, 0x41], 4, &mut out));
        assert_eq!(out, [0, 0, 0, 1, 0x65, 7, 0, 0, 0, 1, 0x41]);
        assert!(!super::h264::to_annex_b(&[0, 0, 0, 9, 1], 4, &mut Vec::new()), "a length past the end is corrupt");
        assert!(super::h264::parse_avcc(&avcc[..8]).is_none());
    }

    #[test]
    fn extensions() {
        assert!(is_video_extension("MP4") && is_video_extension("webm") && !is_video_extension("png"));
    }
}
