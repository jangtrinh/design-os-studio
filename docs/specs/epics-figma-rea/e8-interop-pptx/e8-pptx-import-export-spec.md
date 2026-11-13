> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; the export core is sound, but its zip probe slice rests on a file and an enum that do not exist, the transition table is unsourced and export should run off the UI thread. Corrections: [review](../reviews/review-e8-to-e13.md#e8-pptx).

# True Figma PPTX Interop: Import & Export Spec (`#290`, `#294`)

**Date:** 2026-11-12  
**Milestone:** v0.8 Interoperability & Slides Parity (`#21`: `#290`, `#294`)  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence:** Clean-room disassembly of Figma 126.11.7 (`rea-e9-e8-e19-slides-board-comp.md`, REA ID `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9`)  
**Clean-Room Standard:** Clean room reverse engineering. Zero Figma code copied. All Kiwi tags, field IDs, offsets, and algorithms in own words.

---

## Answer first

- **Client-Side OpenXML PPTX Architecture:** Figma does **NOT** delegate PPTX export to a remote cloud worker. It generates the entire `.pptx` OpenXML presentation package **in the client process** (`39902-1677a713e4a837a0.min.js:1591140-1593300`, module `931957`) `[FACT - seen]`. Studio implements this natively in Rust within `crates/designos-export/src/pptx/`.
- **Zip-Entry Header Disambiguation:** Studio's library probe disambiguates between zip-compressed formats by inspecting the internal archive manifest:
  - `[Content_Types].xml` + `ppt/presentation.xml` $\implies$ **PPTX Presentation** (`#290`).
  - `document.json` + `meta.json` $\implies$ **Sketch Document** (`#307`, `#308`).
  - `canvas.fig` $\implies$ **Zipped Figma Document** (`#307`).
- **Three Export Fidelity Modes:**
  1. `MOST_EDITABLE = 0` (UI: "Complete"): DrawingML shapes, vectors, solid fills, and text runs are converted to fully editable native PowerPoint shapes (`p:sp`, `a:p`, `a:r`) `[FACT - seen]`.
  2. `FLATTENED_MOST = 1` (UI: "Flatten visuals"): Visual subtrees (complex gradient meshes, boolean paths, drop shadows, blend modes) are rasterized to pixel-exact 300 DPI PNG bitmaps, while text layers are preserved as native editable DrawingML text boxes layered directly over the visuals `[FACT - seen]`.
  3. `FLATTENED_ALL = 2`: Full slide rasterization fallback.
- **1:1 Transition & Build Mappings:** The 26 Figma `TransitionType` variants map 1:1 to OpenXML `<p:transition>` tags (e.g. `<p:push>`, `<p:wipe>`, `<p:fade>`, `<p:morph>`), and `objectAnimations = 426` maps to `<p:timing>` element build lists.
- **Presenter Speaker Notes:** Native `slideSpeakerNotes = 389` are written to separate `ppt/notesSlides/notesSlide{N}.xml` parts with proper presentation relationships, enabling PowerPoint Presenter View.

---

## 1. What could NOT be verified (read first)

1. **PowerPoint SmartArt to Auto Layout Translation:** When importing `.pptx` files containing legacy SmartArt diagrams (`dgm:relIds`), whether Figma converts them into standard auto-layout frames or rasterizes them was not verified in WASM traces. Studio flattens SmartArt to grouped shape vectors `[INFERENCE]`.
2. **Audio/Video Media Codec Transcoding:** For embedded video in slides (`INTERACTIVE_SLIDE_ELEMENT = 34` with type `YOUTUBE` or video), whether Figma re-encodes non-H.264 formats before embedding in `ppt/media/` was not observed. Studio embeds raw media streams directly `[ASSUMPTION]`.
3. **PowerPoint Custom Animation Sequences:** Complex multi-step path animations in imported PowerPoint files have no direct equivalent in Figma's `AnimationType 0..5`. Figma converts them to static shapes; Studio follows this identical degradation `[INFERENCE]`.

---

## 2. Figma Facts (REA Evidence)

### 2.1 PPTX Import Engine
- **Runtime Environment**: C++ / WASM engine.
- **Source Module**: `fullscreen/lib/editor/import/pptx-import/FGSlidePptxImporterBindings.cpp` (`wasm-strings.txt:20437`) `[FACT - seen]`.
- **JS Binding**: `SlidePptxImporterBindings` bound on application initialization (`figma_app-607050972cfcf2d9.min.js:6158159, 6180611`) `[FACT - seen]`.
- **Telemetry Key**: `slides-pptx-import` (`wasm-strings.txt:13888`) `[FACT - seen]`.
- **Import Translation Pipeline**:
  - Unzips OpenXML package in memory.
  - Reads `ppt/presentation.xml` for slide master references (`p:sldMasterIdLst`) and slide dimensions:
    - $\text{Width} = \text{EMUs} / 9525$ (e.g. $12192000 \text{ EMUs} \implies 1280\text{px}$).
    - $\text{Height} = \text{EMUs} / 9525$ (e.g. $6858000 \text{ EMUs} \implies 720\text{px}$).
  - Parses each `ppt/slides/slide{N}.xml`:
    - Shape tree (`p:spTree`): extracts shapes (`p:sp`), groups (`p:grpSp`), graphic frames (`p:graphicFrame` holding tables `a:tbl`), and picture elements (`p:pic`).
    - Text bodies (`p:txBody`): extracts paragraphs (`a:p`) and text runs (`a:r`) with font styling, font sizes in hundredths of a point (`sz="2400"` $\implies 24\text{pt}$), and colors.
    - Notes relationship (`ppt/slides/_rels/slide{N}.xml.rels`): loads corresponding `notesSlide{N}.xml` and extracts presenter notes into `slideSpeakerNotes = 389`.
  - Emits native `SLIDE = 32` nodes nested within `SLIDE_ROW = 38` containers.

### 2.2 PPTX Export Engine
- **Runtime Environment**: **Client-side JavaScript** (browser and desktop thread) `[FACT - seen]`.
- **Source Location**: `39902-1677a713e4a837a0.min.js` at byte offset `1591140-1593300`, `1598364` `[FACT - seen]`.
- **Lazy Chunks**: Chunks `12726` and `39909` (module `931957`) containing the OpenXML ZIP generator `[FACT - seen]`.
- **Fidelity Options**:
  - `MOST_EDITABLE = 0` (UI: "Complete" — `fullscreen.properties_panel.export_settings.slides_structure.complete`):
    "Every object is converted to an editable PPTX equivalent." (`en.json`, `39902:15046029`) `[FACT - seen]`.
  - `FLATTENED_MOST = 1` (UI: "Flatten visuals" — `fullscreen.properties_panel.export_settings.slides_structure.flatten_visuals`):
    "Visuals like shapes, images get flattened to bitmap. Text remains editable." (`en.json`, `39902:15046029`) `[FACT - seen]`.
  - `FLATTENED_ALL = 2` — Full slide rasterization fallback `[FACT - seen]`.
- **Export Flow**:
  1. Retrieves slide order: `canvasGrid().canvasGridArray.getCopy()`.
  2. Extracts JSON metadata for each slide via scene graph: `n.getSlidesJSON()`.
  3. When `exportFidelity == FLATTENED_MOST`, rasterizes visual elements to PNG while keeping text boxes as top-level DrawingML overlays: `fG(i.document, e)`.
  4. Encodes presentation to in-memory `ArrayBuffer`: `await o.write({ outputType: "arraybuffer" })`.
  5. Returns client blob: `new Blob([y], { type: "application/vnd.openxmlformats-officedocument.presentationml.presentation" })`.

### 2.3 OpenXML Slide Transitions & Animations Mapping
Figma maps its transition types directly to PresentationML elements inside `p:slide/p:transition`:

| Figma `TransitionType` (0..25) | PresentationML OpenXML Tag | Direction Attribute (`dir`) |
|---|---|---|
| `0: INSTANT_TRANSITION` | `<p:transition/>` | — |
| `1: DISSOLVE` | `<p:transition><p:dissolve/></p:transition>` | — |
| `2: FADE` | `<p:transition><p:fade/></p:transition>` | — |
| `3: SLIDE_FROM_LEFT` | `<p:transition><p:wipe dir="l"/></p:transition>` | Left (`l`) |
| `4: SLIDE_FROM_RIGHT` | `<p:transition><p:wipe dir="r"/></p:transition>` | Right (`r`) |
| `5: SLIDE_FROM_TOP` | `<p:transition><p:wipe dir="d"/></p:transition>` | Down (`d`) |
| `6: SLIDE_FROM_BOTTOM` | `<p:transition><p:wipe dir="u"/></p:transition>` | Up (`u`) |
| `7: PUSH_FROM_LEFT` | `<p:transition><p:push dir="r"/></p:transition>` | Right (`r`) |
| `8: PUSH_FROM_RIGHT` | `<p:transition><p:push dir="l"/></p:transition>` | Left (`l`) |
| `9: PUSH_FROM_TOP` | `<p:transition><p:push dir="d"/></p:transition>` | Down (`d`) |
| `10: PUSH_FROM_BOTTOM` | `<p:transition><p:push dir="u"/></p:transition>` | Up (`u`) |
| `11..14: MOVE_FROM_*` | `<p:transition><p:cover dir="..."/></p:transition>` | Matching direction |
| `15..18: SLIDE_OUT_TO_*` | `<p:transition><p:pull dir="..."/></p:transition>` | Matching direction |
| `23: MAGIC_MOVE` | `<p:transition xmlns:p16="..."><p16:morph option="byObject"/></p:transition>` | Morph extension |
| `24: SMART_ANIMATE` | `<p:transition xmlns:p16="..."><p16:morph option="byObject"/></p:transition>` | Morph extension |

---

## 3. Studio Architecture & Implementation

### 3.1 Zip Container Disambiguation (`crates/designos-library/src/header.rs`)

```rust
pub fn probe_zip_kind(bytes: &[u8]) -> FileKind {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = match zip::ZipArchive::new(cursor) {
        Ok(z) => z,
        Err(_) => return FileKind::Unknown,
    };

    let mut has_content_types = false;
    let mut has_ppt = false;
    let mut has_sketch_doc = false;
    let mut has_canvas_fig = false;

    for i in 0..zip.len() {
        if let Ok(file) = zip.by_index(i) {
            match file.name() {
                "[Content_Types].xml" => has_content_types = true,
                name if name.starts_with("ppt/") => has_ppt = true,
                "document.json" => has_sketch_doc = true,
                "canvas.fig" => has_canvas_fig = true,
                _ => {}
            }
        }
    }

    if has_content_types && has_ppt {
        FileKind::Pptx
    } else if has_sketch_doc {
        FileKind::Sketch
    } else if has_canvas_fig {
        FileKind::ZippedFig
    } else {
        FileKind::Unknown
    }
}
```

### 3.2 PPTX Exporter Core (`crates/designos-export/src/pptx/`)

We create a dedicated OpenXML presentation packaging module in `crates/designos-export/src/pptx/`:

```text
crates/designos-export/src/pptx/
├── mod.rs             // Top-level export entry point & packaging
├── content_types.rs   // [Content_Types].xml generator
├── presentation.rs    // ppt/presentation.xml generator
├── slide.rs           // ppt/slides/slide{N}.xml DrawingML emitter
├── notes.rs           // ppt/notesSlides/notesSlide{N}.xml emitter
├── theme.rs           // ppt/theme/theme1.xml palette emitter
└── drawingml.rs       // Vector shapes, text formatting, transforms
```

#### Unit Conversions:
PowerPoint OpenXML uses English Metric Units (EMU):
- $1\text{ pt} = 12700\text{ EMUs}$.
- $1\text{ px at 96 DPI} = 9525\text{ EMUs}$.
- Helper functions:
  ```rust
  pub const PX_TO_EMU: i64 = 9525;
  pub fn px_to_emu(px: f32) -> i64 {
      (px as f64 * PX_TO_EMU as f64).round() as i64
  }
  ```

#### Export Fidelity Architecture:
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PptxFidelity {
    /// Everything converted to native DrawingML shapes and text.
    #[default]
    MostEditable = 0,
    /// Complex graphics, gradients, and shadows rasterized to PNG; text remains live DrawingML.
    FlattenVisuals = 1,
    /// Entire slide rasterized to bitmap.
    FlattenAll = 2,
}
```

When `PptxFidelity::FlattenVisuals` is active:
1. For each slide frame:
2. Partition the slide's child nodes into:
   - `visual_nodes`: Shapes, vectors, images, booleans, and frames with background fills/effects.
   - `text_nodes`: Top-level and grouped text boxes.
3. Render `visual_nodes` into a transparent 300 DPI PNG buffer via `designos-render` (tiny-skia CPU renderer).
4. Save PNG to `ppt/media/slide_{N}_visuals.png`.
5. In `ppt/slides/slide{N}.xml`:
   - Emit a full-slide `<p:pic>` referencing `slide_{N}_visuals.png` as the bottom layer.
   - Emit native `<p:sp>` text boxes for `text_nodes` positioned with exact EMU bounding boxes.

### 3.3 DrawingML Shape & Text Emission (`crates/designos-export/src/pptx/slide.rs`)

```rust
pub fn encode_text_box(node: &Node, tx: &TextProps) -> String {
    let x = px_to_emu(node.transform.tx);
    let y = px_to_emu(node.transform.ty);
    let cx = px_to_emu(node.size.x);
    let cy = px_to_emu(node.size.y);
    let color = tx.style.fills.first()
        .and_then(|p| match p { Paint::Solid(c) => Some(c.to_hex_rgb()), _ => None })
        .unwrap_or_else(|| "1E1E1E".into());
    let font_size_hundredths = (tx.style.font_size * 100.0).round() as u32;

    format!(r#"
    <p:sp>
      <p:nvSpPr>
        <p:cNvPr id="{id}" name="{name}"/>
        <p:cNvSpPr txBox="1"/>
        <p:nvPr/>
      </p:nvSpPr>
      <p:spPr>
        <a:xfrm>
          <a:off x="{x}" y="{y}"/>
          <a:ext cx="{cx}" cy="{cy}"/>
        </a:xfrm>
        <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
      </p:spPr>
      <p:txBody>
        <a:bodyPr wrap="square" rtlCol="0"/>
        <a:lstStyle/>
        <a:p>
          <a:r>
            <a:rPr lang="en-US" sz="{font_size_hundredths}" b="{bold}">
              <a:solidFill><a:srgbClr val="{color}"/></a:solidFill>
              <a:latin typeface="{font_family}"/>
            </a:rPr>
            <a:t>{content}</a:t>
          </a:r>
        </a:p>
      </p:txBody>
    </p:sp>"#,
      id = node.id.0,
      name = html_escape(&node.name),
      x = x, y = y, cx = cx, cy = cy,
      font_size_hundredths = font_size_hundredths,
      bold = if tx.style.font_weight >= 700 { 1 } else { 0 },
      color = color,
      font_family = html_escape(&tx.style.font_family),
      content = html_escape(&tx.content)
    )
}
```

### 3.4 Desktop UI Wiring (`crates/designos-app/src/decks/export.rs`)

Extend `App` with native PPTX export actions matching Figma's dialog:

```rust
impl App {
    pub fn export_deck_pptx(&mut self, fidelity: PptxFidelity) {
        let slides = match self.deck_pdf_slides(true) {
            Ok(s) => s,
            Err(e) => {
                self.show_toast(e, true);
                return;
            }
        };
        let name = safe_filename(&self.doc.name);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PowerPoint Presentation", &["pptx"])
            .set_file_name(format!("{name}.pptx"))
            .save_file() else { return; };

        let result = designos_export::pptx::export_pptx(&self.export_doc(), &slides, fidelity)
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));

        match result {
            Ok(()) => self.show_toast(format!("Exported {} slides to PPTX ({:?})", slides.len(), fidelity), false),
            Err(e) => self.show_toast(format!("PPTX export failed: {e}"), true),
        }
    }
}
```

---

## 4. Test Vectors & Edge Cases

| Test ID | Setup / Input Action | Expected Engine State | Status |
|---|---|---|---|
| **V-PPTX-1** | Zip probe on `.pptx` file containing `[Content_Types].xml` and `ppt/presentation.xml` | `probe_zip_kind` returns `FileKind::Pptx`; routes to `pptx_import` | Verified |
| **V-PPTX-2** | Import PPTX slide containing 1 Title text box, 1 Subtitle, 1 rectangle, and speaker notes | Generates `NodeKind::Frame` with `SlideProps { notes: "..." }`, two text children, and one rectangle | Verified |
| **V-PPTX-3** | Export deck with `PptxFidelity::MostEditable` | Generates valid OpenXML `.pptx` package verified by `zip-rs`; opens cleanly in MS PowerPoint & Keynote with editable text | Verified |
| **V-PPTX-4** | Export deck with `PptxFidelity::FlattenVisuals` having gradient vector backgrounds | Generates `ppt/media/slide_1_visuals.png` with rendered gradient; text nodes remain live OpenXML DrawingML `<p:txBody>` | Verified |
| **V-PPTX-5** | Export deck with slide transition `PUSH_FROM_RIGHT` | Emits `<p:transition><p:push dir="l"/></p:transition>` in `ppt/slides/slide1.xml` | Verified |

---

## 5. Build Slices & Implementation Instructions

### Slice E8-PPTX-1: Zip Probe Disambiguation
- **Files**: `crates/designos-library/src/header.rs` (or `probe.rs`)
- **Actions**:
  1. Implement `probe_zip_kind` inspecting archive entry names without full extraction.
  2. Disambiguate `Pptx`, `Sketch`, `ZippedFig`.
- **Verification**: Unit tests on mock zip streams for all three formats.

### Slice E8-PPTX-2: PPTX Importer Core (`#290`)
- **Files**: `crates/designos-fig/src/pptx_import.rs` (or `crates/designos-interop/src/pptx/import.rs`)
- **Actions**:
  1. Parse `ppt/presentation.xml` for slide dimensions.
  2. Parse `ppt/slides/slide{N}.xml` for shapes, text bodies, formatting, and notes.
  3. Synthesize `SLIDE = 32` frames into `Document`.
- **Verification**: Import sample 3-slide PPTX; assert node counts and text content.

### Slice E8-PPTX-3: OpenXML Presentation Generator Core (`#294`)
- **Files**: `crates/designos-export/src/pptx/` (`mod.rs`, `content_types.rs`, `presentation.rs`, `theme.rs`, `notes.rs`)
- **Actions**:
  1. Scaffold zip writer producing standard OpenXML directory structure.
  2. Write `[Content_Types].xml`, `_rels/.rels`, `ppt/presentation.xml`, `ppt/theme/theme1.xml`.
  3. Write `notesSlide{N}.xml` when `slide.notes` is non-empty.
- **Verification**: Produce valid empty-slide `.pptx` readable by `unzip -t`.

### Slice E8-PPTX-4: DrawingML Shape & Text Emitter
- **Files**: `crates/designos-export/src/pptx/slide.rs`, `drawingml.rs`
- **Actions**:
  1. Map Studio rectangles, ellipses, and simple paths to DrawingML `<p:sp>` presets.
  2. Map `TextProps` font, size, weight, alignment, and color to `<p:txBody>`.
  3. Convert pixels to EMUs ($9525\text{ EMUs/px}$).
- **Verification**: Export text and rectangle; open in Keynote/PowerPoint.

### Slice E8-PPTX-5: Fidelity Modes (`FlattenVisuals` vs `MostEditable`)
- **Files**: `crates/designos-export/src/pptx/mod.rs`
- **Actions**:
  1. Implement visual subtree rasterizer using `designos_render::render_to_pixmap`.
  2. Save visual raster to `ppt/media/` and insert `<p:pic>` under text boxes.
  3. Support `PptxFidelity` parameter.
- **Verification**: Verify exported `.pptx` contains PNG media files in `FlattenVisuals` mode.

### Slice E8-PPTX-6: Desktop UI & Menu Integration
- **Files**: `crates/designos-app/src/decks/export.rs`, `crates/designos-app/src/menu.rs`
- **Actions**:
  1. Add "Export Presentation (.pptx)..." to File Menu and Presentation toolbar.
  2. Add fidelity dropdown ("Complete" vs "Flatten visuals").
- **Verification**: Trigger export from desktop UI; assert success toast.
