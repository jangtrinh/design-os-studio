"""THROWAWAY. Builds the side-by-side evidence sheets from run-all.sh output.

usage: python3 -I compose-evidence.py <run-all-out-dir> <reference-jpg> <dest-dir>
"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

out_dir, ref_jpg, dest = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
dest.mkdir(parents=True, exist_ok=True)
FONT = Path(__file__).resolve().parents[2] / "crates/designos-fonts/assets/inter/Inter[opsz,wght].ttf"
BG, PANEL, TEXT, MUTED = (10, 10, 10), (23, 23, 23), (212, 212, 212), (163, 163, 163)


def font(size):
    return ImageFont.truetype(str(FONT), size)


def panel(img, title, subtitle, width):
    """Scale `img` to `width`, add a caption strip above it."""
    scale = width / img.width
    img = img.resize((width, max(1, round(img.height * scale))), Image.LANCZOS)
    strip = 64
    p = Image.new("RGB", (width, img.height + strip), PANEL)
    d = ImageDraw.Draw(p)
    d.text((12, 10), title, font=font(18), fill=TEXT)
    d.text((12, 36), subtitle, font=font(13), fill=MUTED)
    p.paste(img, (0, strip))
    return p


def row(panels, gap=24):
    h = max(p.height for p in panels)
    w = sum(p.width for p in panels) + gap * (len(panels) + 1)
    sheet = Image.new("RGB", (w, h + gap * 2), BG)
    x = gap
    for p in panels:
        sheet.paste(p, (x, gap))
        x += p.width + gap
    return sheet


def load(engine, stem):
    return Image.open(out_dir / f"{engine}-{stem}.png").convert("RGB")


# 1. design-studio reference (only the card header of "Feedback review flow" is in the capture)
ref = Image.open(ref_jpg).convert("RGB").crop((210, 466, 606, 500))
ref = ref.resize((ref.width * 3, ref.height * 3), Image.LANCZOS)

stem = "flow-1-feedback-review-flow-reconstructed"
W = 760
sheet = row([
    panel(ref, "design-studio reference", "ds-project-context-doc.jpg, card header only (body is below the fold)", W),
    panel(load("mmdr", stem), "mermaid-rs-renderer 0.3.1", "Studio neutral tokens, Inter, reconstructed source", W),
    panel(load("merman", stem), "merman 0.9.3", "Studio neutral tokens, Inter, reconstructed source", W),
])
sheet.save(dest / "compare-feedback-review-flow-vs-design-studio.png", optimize=True)

# 2. per-diagram engine comparison sheets
groups = {
    "compare-flowcharts-mmdr-vs-merman.png": ["flow-2-checkout-payment", "flow-3-subgraphs-and-shapes"],
    "compare-sequence-mmdr-vs-merman.png": ["seq-1-open-doc", "seq-2-agent-coediting"],
}
for name, stems in groups.items():
    rows = []
    for s in stems:
        rows.append(row([
            panel(load("mmdr", s), f"mmdr: {s}", "mermaid-rs-renderer 0.3.1", 700),
            panel(load("merman", s), f"merman: {s}", "merman 0.9.3", 700),
        ]))
    w = max(r.width for r in rows)
    out = Image.new("RGB", (w, sum(r.height for r in rows)), BG)
    y = 0
    for r in rows:
        out.paste(r, (0, y))
        y += r.height
    out.save(dest / name, optimize=True)
print("ok", sorted(p.name for p in dest.glob("compare-*.png")))
