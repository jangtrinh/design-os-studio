//! Figma's glyph command blobs: the outline of one glyph in font-em space (y up), as the bytes a `.fig` stores.

use crate::{PathCmd, Vec2};

/// Decodes a command blob: 0 = close, 1 = move, 2 = line, 3 = quad, 4 = cubic, each followed by little-endian `f32`
/// coordinates. Decoding stops at the first unknown opcode or truncated command and keeps what came before it.
pub fn decode_glyph_commands(b: &[u8]) -> Vec<PathCmd> {
    let mut cmds = Vec::new();
    let mut i = 0usize;
    let f = |i: usize| -> Option<f32> { b.get(i..i + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]])) };
    let pt = |i: usize| -> Option<Vec2> { Some(Vec2::new(f(i)?, f(i + 4)?)) };
    while i < b.len() {
        let op = b[i];
        i += 1;
        let ok = match op {
            0 => {
                cmds.push(PathCmd::Close);
                Some(())
            }
            1 => pt(i).map(|p| {
                cmds.push(PathCmd::MoveTo(p));
                i += 8;
            }),
            2 => pt(i).map(|p| {
                cmds.push(PathCmd::LineTo(p));
                i += 8;
            }),
            3 => pt(i).zip(pt(i + 8)).map(|(c, p)| {
                cmds.push(PathCmd::QuadTo(c, p));
                i += 16;
            }),
            4 => pt(i).zip(pt(i + 8)).zip(pt(i + 16)).map(|((c1, c2), p)| {
                cmds.push(PathCmd::CubicTo(c1, c2, p));
                i += 24;
            }),
            _ => None,
        };
        if ok.is_none() {
            break;
        }
    }
    cmds
}

/// What the importer needs to know about a glyph blob without keeping its commands: how many there are and the box
/// around every point (control points included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphBlobExtent {
    pub commands: usize,
    pub min: Vec2,
    pub max: Vec2,
}

/// The extent of the commands in `blob`, or `None` when a coordinate is not finite. A blob without commands has an
/// extent of zero commands at the origin.
pub fn glyph_blob_extent(blob: &[u8]) -> Option<GlyphBlobExtent> {
    let cmds = decode_glyph_commands(blob);
    let (mut min, mut max) = (Vec2::new(f32::INFINITY, f32::INFINITY), Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY));
    let mut seen = |p: Vec2| -> bool {
        min = Vec2::new(min.x.min(p.x), min.y.min(p.y));
        max = Vec2::new(max.x.max(p.x), max.y.max(p.y));
        p.x.is_finite() && p.y.is_finite()
    };
    let finite = cmds.iter().all(|cmd| match *cmd {
        PathCmd::MoveTo(p) | PathCmd::LineTo(p) => seen(p),
        PathCmd::QuadTo(a, b) => seen(a) & seen(b),
        PathCmd::CubicTo(a, b, c) => seen(a) & seen(b) & seen(c),
        PathCmd::Close => true,
    });
    if !finite {
        return None;
    }
    let zero = Vec2::new(0.0, 0.0);
    let has_points = cmds.iter().any(|cmd| !matches!(cmd, PathCmd::Close));
    Some(GlyphBlobExtent { commands: cmds.len(), min: if has_points { min } else { zero }, max: if has_points { max } else { zero } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(cmds: &[(u8, &[f32])]) -> Vec<u8> {
        let mut b = Vec::new();
        for (op, pts) in cmds {
            b.push(*op);
            b.extend(pts.iter().flat_map(|v| v.to_le_bytes()));
        }
        b
    }

    #[test]
    fn decoding_stops_at_a_truncated_or_unknown_command() {
        let mut b = blob(&[(1, &[1.0, 2.0])]);
        b.push(2);
        b.extend(3.0f32.to_le_bytes());
        assert_eq!(decode_glyph_commands(&b), vec![PathCmd::MoveTo(Vec2::new(1.0, 2.0))]);
        assert!(decode_glyph_commands(&[9, 9, 9]).is_empty());
    }

    #[test]
    fn the_extent_boxes_every_point_and_refuses_a_non_finite_one() {
        let b = blob(&[(1, &[-1.0, 0.5]), (3, &[0.0, 4.0, 2.0, 1.0]), (0, &[])]);
        assert_eq!(glyph_blob_extent(&b), Some(GlyphBlobExtent { commands: 3, min: Vec2::new(-1.0, 0.5), max: Vec2::new(2.0, 4.0) }));
        assert_eq!(glyph_blob_extent(&blob(&[(1, &[f32::NAN, 0.0])])), None);
        assert_eq!(glyph_blob_extent(&blob(&[(1, &[0.0, 0.0]), (2, &[f32::INFINITY, 0.0])])), None);
        assert_eq!(glyph_blob_extent(&[9]), Some(GlyphBlobExtent { commands: 0, min: Vec2::new(0.0, 0.0), max: Vec2::new(0.0, 0.0) }));
    }
}
