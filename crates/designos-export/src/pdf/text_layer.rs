//! The invisible text layer that makes exported text selectable and searchable. The glyphs a viewer
//! shows are the outlines drawn as paths, so the layer only has to carry the right characters:
//! text that fits WinAnsi goes into a standard Helvetica run; anything else (Vietnamese, CJK, the euro
//! sign, ...) goes into a two-byte font whose ToUnicode map names the real characters, so copying and
//! searching give back what the canvas shows.

use super::Writer;
use std::collections::HashMap;
use std::fmt::Write as _;

/// How one run of text is written into a content stream.
pub(super) enum Encoded {
    /// Escaped literal string for the WinAnsi font `/F1`.
    Latin(String),
    /// Hex string of two-byte character ids for the Unicode font `/F2`.
    Wide(String),
}

/// Characters of the document that need the Unicode font, numbered in order of first use.
#[derive(Default)]
pub(super) struct TextFonts {
    chars: Vec<char>,
    ids: HashMap<char, u16>,
}

/// Number of objects [`TextFonts::write_objects`] writes.
pub(super) const FONT_OBJECTS: usize = 4;

/// WinAnsi code of `c`, if the encoding has it.
fn win_ansi_byte(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
        _ => {
            const HIGH: [(char, u8); 27] = [
                ('\u{20AC}', 0x80),
                ('\u{201A}', 0x82),
                ('\u{0192}', 0x83),
                ('\u{201E}', 0x84),
                ('\u{2026}', 0x85),
                ('\u{2020}', 0x86),
                ('\u{2021}', 0x87),
                ('\u{02C6}', 0x88),
                ('\u{2030}', 0x89),
                ('\u{0160}', 0x8A),
                ('\u{2039}', 0x8B),
                ('\u{0152}', 0x8C),
                ('\u{017D}', 0x8E),
                ('\u{2018}', 0x91),
                ('\u{2019}', 0x92),
                ('\u{201C}', 0x93),
                ('\u{201D}', 0x94),
                ('\u{2022}', 0x95),
                ('\u{2013}', 0x96),
                ('\u{2014}', 0x97),
                ('\u{02DC}', 0x98),
                ('\u{2122}', 0x99),
                ('\u{0161}', 0x9A),
                ('\u{203A}', 0x9B),
                ('\u{0153}', 0x9C),
                ('\u{017E}', 0x9E),
                ('\u{0178}', 0x9F),
            ];
            HIGH.iter().find(|(ch, _)| *ch == c).map(|(_, b)| *b)
        }
    }
}

impl TextFonts {
    pub(super) fn is_used(&self) -> bool {
        !self.chars.is_empty()
    }

    /// The characters of `text` as the page writes them, `None` when nothing remains to show.
    /// Whitespace controls become spaces; other control characters are dropped.
    pub(super) fn encode(&mut self, text: &str) -> Option<Encoded> {
        let chars: Vec<char> = text
            .chars()
            .filter_map(|c| match c {
                '\t' | '\n' | '\r' => Some(' '),
                c if c.is_control() => None,
                c => Some(c),
            })
            .collect();
        if chars.is_empty() {
            return None;
        }
        if let Some(bytes) = chars.iter().map(|c| win_ansi_byte(*c)).collect::<Option<Vec<u8>>>() {
            let mut out = String::with_capacity(bytes.len());
            for b in bytes {
                match b {
                    b'\\' | b'(' | b')' => {
                        out.push('\\');
                        out.push(char::from(b));
                    }
                    0x20..=0x7E => out.push(char::from(b)),
                    _ => {
                        let _ = write!(out, "\\{b:03o}");
                    }
                }
            }
            return Some(Encoded::Latin(out));
        }
        let mut hex = String::with_capacity(chars.len() * 4);
        for c in chars {
            if let Some(id) = self.id_of(c) {
                let _ = write!(hex, "{id:04X}");
            }
        }
        (!hex.is_empty()).then_some(Encoded::Wide(hex))
    }

    /// Character id of `c`; `None` once the 65,535 ids of a two-byte font are used up.
    fn id_of(&mut self, c: char) -> Option<u16> {
        if let Some(id) = self.ids.get(&c) {
            return Some(*id);
        }
        let id = u16::try_from(self.chars.len() + 1).ok()?;
        self.chars.push(c);
        self.ids.insert(c, id);
        Some(id)
    }

    /// Writes the Unicode font as [`FONT_OBJECTS`] objects starting at `first`: the composite font, its
    /// descendant, the descriptor, and the ToUnicode map. No glyph program is embedded; the layer is
    /// never drawn.
    pub(super) fn write_objects(&self, writer: &mut Writer, first: usize) -> Result<(), String> {
        writer.object(
            first,
            format!(
                "<< /Type /Font /Subtype /Type0 /BaseFont /Helvetica /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
                first + 1,
                first + 3
            )
            .as_bytes(),
        )?;
        writer.object(
            first + 1,
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Helvetica /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 600 /CIDToGIDMap /Identity >>",
                first + 2
            )
            .as_bytes(),
        )?;
        writer.object(
            first + 2,
            b"<< /Type /FontDescriptor /FontName /Helvetica /Flags 4 /FontBBox [-166 -225 1000 931] /ItalicAngle 0 /Ascent 718 /Descent -207 /CapHeight 718 /StemV 88 >>",
        )?;
        writer.stream(first + 3, "", self.to_unicode_map().as_bytes())
    }

    fn to_unicode_map(&self) -> String {
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
        for (block, chars) in self.chars.chunks(100).enumerate() {
            let _ = writeln!(cmap, "{} beginbfchar", chars.len());
            for (i, c) in chars.iter().enumerate() {
                let id = block * 100 + i + 1;
                let mut units = [0u16; 2];
                let utf16: String = c.encode_utf16(&mut units).iter().map(|u| format!("{u:04X}")).collect();
                let _ = writeln!(cmap, "<{id:04X}> <{utf16}>");
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        cmap
    }
}
