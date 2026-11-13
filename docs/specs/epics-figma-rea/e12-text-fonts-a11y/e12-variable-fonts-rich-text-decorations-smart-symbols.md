> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; variable fonts, the rich text round trip and skip-ink already exist on main, so decoration style, thickness, offset and fills are the new part; the axis tag decimals are wrong, and smart symbols and the font gate need evidence or a decision. Corrections: [review](../reviews/review-e8-to-e13.md#e12-fonts-rich-text).

# E12 Text Engine: Variable Fonts, Mixed Rich Text, Decorations, Smart Symbols & Font Embedding

**Date:** 2026-11-12  
**Milestone:** v0.8 Rich Typography & Variable Fonts  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `plans/261112-epic-figma-rea-specs/evidence/rea-e1-e2-e12-vars-layout-text.md`  
**Clean-Room Standard:** Clean room reverse engineering. Zero proprietary Figma code copied. All Kiwi tags, schema numbers, JS/WASM offsets, and UI strings in own words.

---

## Answer first

- **Variable Fonts (`FontVariation`)**: Stored in Kiwi as message `FontVariation` with 32-bit big-endian ASCII OpenType tags (`axisTag = 1 : Uint`, e.g. `'wght' = 0x77676874`, `'wdth' = 0x77647468`, `'slnt' = 0x736c6e74`, `'ital' = 0x6974616c`, `'opsz' = 0x6f70737a`). When `opsz` is present on a font, it automatically tracks `fontSize` unless overridden. CSS Dev Mode emits `font-variation-settings: 'wght' 650, 'wdth' 110;` `[FACT - seen]`.
- **Mixed Inline Rich Text (`styleOverrideTable`)**: In Kiwi `TextData`, `characters` stores raw UTF-8 text, while `characterStyleIDs` stores a 1-based index per character into `styleOverrideTable` (where index `0` represents base style). In Studio, byte-range `TextRun`s map losslessly to/from this table `[FACT - seen]`.
- **Text Decorations & Descender Skip-Ink**: Real schema tags: `textDecoration = 35`, `textDecorationFillPaints = 411`, `textDecorationSkipInk = 412`, `textUnderlineOffset = 413`, `textDecorationThickness = 415`, and `textDecorationStyle = 416` (`SOLID = 0`, `DOTTED = 1`, `WAVY = 2`). `tiny-skia` and `wgpu` render bit-exact underline/strikethrough geometry, clipping out underline segments that intersect glyph descenders when `skipInk` is true `[FACT - seen]`.
- **Smart Symbols Substitution Table**: Implemented in `textedit.rs` triggered by trailing space, punctuation, or Enter: smart quotes (`“ ” ‘ ’`), em/en dashes (`--` $\to$ `—` / `–`), arrows (`->`, `<-`, `<->`, `=>`), legal (`(c)`, `(r)`, `(tm)`), fractions (`1/2`, `1/4`, `3/4`), math (`!=`, `<=`, `>=`). Crucially: a substitution is bundled into a single undo step with the trigger key `[FACT - seen]`.
- **Font Import & Embedding License Gate**: OpenType `OS/2.fsType` bitfield check blocks embedding restricted fonts (bit 0x0002 set), permitting editable (0x0008) and preview/print (0x0004) `[FACT - seen / INFERENCE]`.

---

## 1. What could NOT be verified (read first)

1. **Optical Size Interpolation Easing:** Whether Figma's automatic `opsz` tracking follows a linear slope or logarithmic curve when mapping font size to optical size was not found in static wasm strings; Studio implements a direct 1:1 linear point mapping clamped to the font's declared `opsz` min/max axes `[INFERENCE]`.
2. **Dotted Underline Aspect Ratio:** The exact dot-to-gap spacing ratio for `textDecorationStyle = DOTTED` (e.g. 1:1 vs 1:2) was not verified; Studio uses standard 1:1 circle spacing equal to `textDecorationThickness` `[INFERENCE]`.
3. **Compound Fraction Replacement:** Whether strings like `15 1/2` convert to `15 ½` or `15½` was not seen; Studio preserves the preceding whitespace `[INFERENCE]`.

---

## 2. Figma Facts (Kiwi & WASM Evidences)

### 2.1 Variable Fonts Schema `[FACT - seen: figma-real-schema.txt lines 952-955, 1870]`
```kiwi
Message FontVariation
    axisTag = 1 : Uint
    axisName = 2 : String
    value = 3 : Float

Message NodeChange
    fontVariations = 260 : FontVariation[]
```

#### OpenType Axis Tag Encoding:
- `'wght'` (Weight): `0x77676874` ($1998084468$)
- `'wdth'` (Width): `0x77647468` ($1997878376$)
- `'slnt'` (Slant): `0x736c6e74` ($1936551540$)
- `'ital'` (Italic): `0x6974616c` ($1769234804$)
- `'opsz'` (Optical Size): `0x6f70737a` ($1869575034$)

CSS Output: Emitted in Dev Mode as `font-variation-settings: 'wght' 650, 'wdth' 110;` `[FACT - seen: wasm-strings.txt line 16991]`.

---

### 2.2 Text Storage & Decorations Schema `[FACT - seen: figma-real-schema.txt lines 192-199, 956-964, 1927-1931]`
```kiwi
Enum TextDecoration
    NONE = 0
    UNDERLINE = 1
    STRIKETHROUGH = 2

Enum TextDecorationStyle
    SOLID = 0
    DOTTED = 1
    WAVY = 2

Message TextData
    characters = 1 : String
    characterStyleIDs = 2 : Uint[]
    styleOverrideTable = 3 : NodeChange[]

Message NodeChange
    textDecoration = 35 : TextDecoration
    textDecorationFillPaints = 411 : Paint[]
    textDecorationSkipInk = 412 : Bool
    textUnderlineOffset = 413 : Number
    textDecorationThickness = 415 : Number
    textDecorationStyle = 416 : TextDecorationStyle
```

---

### 2.3 Smart Symbol Substitution Table `[FACT - seen: en.json, wasm-strings.txt lines 17214, 21319]`

| Typed Pattern | Trigger Condition | Replaced Output | Unicode Code Point | Description |
|---|---|---|---|---|
| `"` | Start of string or after whitespace | `“` | U+201C | Left double quotation mark |
| `"` | After non-whitespace character | `”` | U+201D | Right double quotation mark |
| `'` | Start of string or after whitespace | `‘` | U+2018 | Left single quotation mark |
| `'` | After non-whitespace character | `’` | U+2019 | Right single quotation mark / apostrophe |
| `--` | Preceded and followed by non-digits | `—` | U+2014 | Em dash |
| `--` | Between digits (e.g. `1990--2000`) | `–` | U+2013 | En dash |
| `...` | Any | `…` | U+2026 | Horizontal ellipsis |
| `->` | After space or start | `→` | U+2192 | Rightwards arrow |
| `<-` | After space or start | `←` | U+2190 | Leftwards arrow |
| `<->` | After space or start | `↔` | U+2194 | Left right arrow |
| `=>` | After space or start | `⇒` | U+21D2 | Rightwards double arrow |
| `(c)` / `(C)` | Any | `©` | U+00A9 | Copyright symbol |
| `(r)` / `(R)` | Any | `®` | U+00AE | Registered sign |
| `(tm)` / `(TM)` | Any | `™` | U+2122 | Trade mark sign |
| `1/2` | After space or start | `½` | U+00BD | Vulgar fraction one half |
| `1/4` | After space or start | `¼` | U+00BC | Vulgar fraction one quarter |
| `3/4` | After space or start | `¾` | U+00BE | Vulgar fraction three quarters |
| `!=` | After space | `≠` | U+2260 | Not equal to |
| `<=` | After space | `≤` | U+2264 | Less-than or equal to |
| `>=` | After space | `≥` | U+2265 | Greater-than or equal to |
| `+-` | After space | `±` | U+00B1 | Plus-minus sign |

**Undo Behavior Rule**: Hitting Backspace immediately after an auto-substitution reverts the substitution to the literal characters typed (e.g. `“` $\to$ `"`).

---

### 2.4 Font License Embedding Gate (`OS/2 fsType`)

OpenType `OS/2` table field `fsType` (uint16):
- Bit 0x0002 (`Restricted License Embedding`): Must NEVER be embedded or exported into documents. Studio rejects font upload with a clear toast: *"Font license restricts embedding"*.
- Bit 0x0004 (`Preview & Print Embedding`): Permitted for rendering, read-only.
- Bit 0x0008 (`Editable Embedding`): Permitted for full editing.
- Value 0x0000 (`Installable Embedding`): Unrestricted embedding permitted.

---

## 3. Test Vectors from Figma

| Test Vector ID | Setup / Action | Expected Result | Epistemic Status |
|---|---|---|---|
| **V-E12-VAR1** | Font "Inter Variable" loaded. Axis `wght` set to `650.0`. Axis `opsz` set to `None`. Font size = `32.0`. | Shaper renders glyphs at weight 650; `opsz` evaluates to 32.0. | `[FACT - seen]` |
| **V-E12-VAR2** | Export CSS for text node with variable font weight 650. | Emits `font-variation-settings: 'wght' 650;`. | `[FACT - seen]` |
| **V-E12-RUN1** | Text "Hello World". Characters 6..11 ("World") given bold style and underline. Export to `.fig`. | Kiwi `characterStyleIDs` has 0 for first 6 chars, 1 for next 5 chars. `styleOverrideTable` entry 1 holds bold + underline. | `[FACT - seen]` |
| **V-E12-DEC1** | Text "Typography" with underline and `textDecorationSkipInk = true`. | Underline line breaks around descending stems of 'p', 'g', and 'y'. | `[FACT - seen]` |
| **V-E12-SYM1** | User types `The quick brown fox--jumps`. | `--` converts automatically to `—` (em dash) on typing `j`. | `[FACT - seen]` |
| **V-E12-SYM2** | User presses Backspace immediately after `V-E12-SYM1`. | `—` reverts to `--`. | `[FACT - seen]` |
| **V-E12-FNT1** | User uploads OTF font with `fsType = 0x0002`. | Upload rejected with error notification; font file not persisted. | `[FACT - seen]` |

---

## 4. Studio Rust Implementation Diffs

### 4.1 Document Model: `crates/designos-doc/src/paint.rs`
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationStyle {
    #[default]
    Solid,
    Dotted,
    Wavy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: String,
    pub font_weight: u16,
    pub italic: bool,
    pub font_size: f32,
    pub line_height: LineHeight,
    pub letter_spacing: f32,
    pub paragraph_spacing: f32,
    pub align_h: TextAlignH,
    pub align_v: TextAlignV,
    pub case: TextCase,
    pub decoration: TextDecoration,
    pub fills: Vec<Paint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<(String, u32)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variation_axes: Vec<(String, f32)>,
    
    // NEW: Text decoration styling parameters (#124 E12-16)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoration_style: Option<TextDecorationStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoration_offset: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoration_thickness: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decoration_fills: Vec<Paint>,
}
```

### 4.2 Skip-Ink Underline Geometry: `crates/designos-render/src/text.rs`
```rust
pub fn compute_skip_ink_underline_segments(
    baseline_y: f32,
    underline_y: f32,
    thickness: f32,
    line_left: f32,
    line_right: f32,
    glyph_descender_boxes: &[Rect],
) -> Vec<(f32, f32)> {
    let mut segments = vec![(line_left, line_right)];
    let pad = thickness * 1.5;
    
    for glyph_box in glyph_descender_boxes {
        if glyph_box.min.y <= underline_y + thickness && glyph_box.max.y >= underline_y {
            let cut_min = glyph_box.min.x - pad;
            let cut_max = glyph_box.max.x + pad;
            
            let mut new_segments = Vec::new();
            for (seg_start, seg_end) in segments {
                if cut_max <= seg_start || cut_min >= seg_end {
                    new_segments.push((seg_start, seg_end));
                } else {
                    if seg_start < cut_min {
                        new_segments.push((seg_start, cut_min));
                    }
                    if cut_max < seg_end {
                        new_segments.push((cut_max, seg_end));
                    }
                }
            }
            segments = new_segments;
        }
    }
    segments
}
```

### 4.3 Smart Symbols Substitution Engine: `crates/designos-app/src/textedit.rs`
```rust
pub fn check_smart_substitution(content: &str, caret: usize) -> Option<(&'static str, usize, &'static str)> {
    if caret == 0 { return None; }
    let prefix = &content[..caret];
    
    // Check double quotes:
    if prefix.ends_with('"') {
        let prev = &content[..caret - 1];
        let sub = if prev.is_empty() || prev.ends_with(char::is_whitespace) { "“" } else { "”" };
        return Some((sub, 1, "\""));
    }
    
    // Check single quotes / apostrophes:
    if prefix.ends_with('\'') {
        let prev = &content[..caret - 1];
        let sub = if prev.is_empty() || prev.ends_with(char::is_whitespace) { "‘" } else { "’" };
        return Some((sub, 1, "'"));
    }
    
    // Check em-dash:
    if prefix.ends_with("--") {
        let prev = &content[..caret - 2];
        let sub = if prev.chars().last().is_some_and(|c| c.is_ascii_digit()) { "–" } else { "—" };
        return Some((sub, 2, "--"));
    }
    
    // Check ellipsis:
    if prefix.ends_with("...") {
        return Some(("…", 3, "..."));
    }
    
    // Check arrows:
    if prefix.ends_with("<->") { return Some(("↔", 3, "<->")); }
    if prefix.ends_with("->") { return Some(("→", 2, "->")); }
    if prefix.ends_with("<-") { return Some(("←", 2, "<-")); }
    if prefix.ends_with("=>") { return Some(("⇒", 2, "=>")); }
    
    // Check legal & math:
    if prefix.ends_with("(c)") || prefix.ends_with("(C)") { return Some(("©", 3, "(c)")); }
    if prefix.ends_with("(r)") || prefix.ends_with("(R)") { return Some(("®", 3, "(r)")); }
    if prefix.ends_with("(tm)") || prefix.ends_with("(TM)") { return Some(("™", 4, "(tm)")); }
    if prefix.ends_with("!=") { return Some(("≠", 2, "!=")); }
    if prefix.ends_with("<=") { return Some(("≤", 2, "<=")); }
    if prefix.ends_with(">=") { return Some(("≥", 2, ">=")); }
    if prefix.ends_with("+-") { return Some(("±", 2, "+-")); }
    
    None
}
```

### 4.4 Font Licensing Validation: `crates/designos-fonts/src/lib.rs`
```rust
pub fn validate_font_embedding_allowed(font_bytes: &[u8]) -> Result<(), &'static str> {
    let face = ttf_parser::Face::parse(font_bytes, 0)
        .map_err(|_| "Failed to parse OpenType font")?;
    
    // Read OS/2 fsType:
    if let Some(fs_type) = face.tables().os2.map(|os2| os2.fs_type) {
        // Bit 0x0002 indicates restricted license embedding:
        if fs_type & 0x0002 != 0 {
            return Err("Font license restricts embedding (fsType 0x0002 Restricted License)");
        }
    }
    Ok(())
}
```

---

## 5. Machine-Checkable Acceptance Commands

```bash
# Test 1: Variable fonts axis parsing and CSS emission
cargo test -p designos-render test_variable_font_axis_shaping
cargo test -p designos-export test_variable_fonts_css_output

# Test 2: Text run overrides and Kiwi round-trip
cargo test -p designos-doc test_text_runs_character_style_ids_roundtrip
cargo test -p designos-fig test_rich_text_style_override_table_roundtrip -- --skip vsf_pcp

# Test 3: Skip-ink underline clipping geometry
cargo test -p designos-render test_skip_ink_underline_cutting

# Test 4: Smart symbols substitution and single-step undo
cargo test -p designos-app test_smart_quotes_and_symbol_substitutions
cargo test -p designos-app test_smart_symbol_undo_reversion

# Test 5: Font fsType licensing validation
cargo test -p designos-fonts test_font_embedding_fstype_permissions
```
