"""Primitive colour names: objective steps, never hex (controller ruling M4).

1. A colour that exactly equals a Tailwind v3.4.17 palette value takes that name: `neutral/950`, `yellow/50`, `orange/500`;
   white and black are `base/white` and `base/black`. When two families share a value (zinc-50 = neutral-50) the first
   of NEUTRALS then HUES wins.
2. Any other colour is `<hue>/<step>`:
   - hue: the nearest OKLCH hue among Tailwind's hue families (their 500 steps give the hue centres), or `neutral`
     when OKLCH chroma < 0.02;
   - step: round_half_up((1 - L_oklab) * 1000 / 50) * 50, clamped to 25..975.
3. A name is taken when another colour holds it, and that includes Tailwind's own names: an off-palette colour never
   takes a Tailwind v3.4.17 name that holds another value (`yellow/100`, `amber/200`, `lime/100`, `sky/200`, `teal/100`
   all hold other values in Tailwind). It takes the next free `-2`, `-3`, .... Exact palette colours are named first,
   then the rest in rgb order, so the result is deterministic.
4. An alpha colour is `<name of its opaque RGB>-a<alpha%>` (alpha% = round_half_up(alpha / 255 * 100)), so it shares the
   base of its opaque twin; two alphas that round to the same percent get `-2`, `-3`, ... after the suffix.

The step is absolute OKLab lightness, not a position in a Tailwind ramp, so a derived step can be darker than a
lower-numbered Tailwind step of the same hue (`orange/350-2`, Studio orange, is darker than Tailwind's `orange/500`). That is
accepted: the number is a lightness, the hue says which ramp it sits nearest to.

    names = build_names(colours)    # colours: iterable of (r, g, b, a) ints -> {(r, g, b, a): "hue/step[-N][-aNN]"}
"""
import json
import math
import pathlib

PALETTE = {k: v for k, v in json.loads((pathlib.Path(__file__).resolve().parent / "tailwind-v3-palette.json").read_text()).items()
           if not k.startswith("_")}
NEUTRALS = ("neutral", "slate", "gray", "zinc", "stone")
HUES = ("red", "orange", "amber", "yellow", "lime", "green", "emerald", "teal", "cyan", "sky", "blue", "indigo", "violet", "purple",
        "fuchsia", "pink", "rose")
BASE = {"#ffffff": "base/white", "#000000": "base/black"}


def _lin(c):
    c /= 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def oklch(r, g, b):
    r, g, b = _lin(r), _lin(g), _lin(b)
    l_ = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
    m_ = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
    s_ = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
    L = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_
    a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_
    bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_
    return L, math.hypot(a, bb), math.degrees(math.atan2(bb, a)) % 360


def _hex(r, g, b):
    return f"#{r:02x}{g:02x}{b:02x}"


def _half_up(x):
    return int(math.floor(x + 0.5))


EXACT = {}
for _fam in NEUTRALS + HUES:
    for _step, _hx in PALETTE[_fam].items():
        EXACT.setdefault(_hx, f"{_fam}/{_step}")
EXACT.update(BASE)
HUE_CENTRES = {fam: oklch(*(int(PALETTE[fam]["500"][i:i + 2], 16) for i in (1, 3, 5)))[2] for fam in HUES}


def derived_name(r, g, b):
    L, chroma, hue = oklch(r, g, b)
    step = min(975, max(25, _half_up((1 - L) * 1000 / 50) * 50))
    if chroma < 0.02:
        family = "neutral"
    else:
        family = min(HUES, key=lambda f: min(abs(hue - HUE_CENTRES[f]), 360 - abs(hue - HUE_CENTRES[f])))
    return f"{family}/{step}"


def opaque_name(r, g, b):
    return EXACT.get(_hex(r, g, b)) or derived_name(r, g, b)


TAILWIND_NAMES = {f"{fam}/{step}" for fam, steps in PALETTE.items() for step in steps} | set(BASE.values())


def build_names(colours):
    colours = set(colours)
    taken, rgb_name = set(), {}
    # Opaque RGB values: exact palette colours first, so a derived name can never displace a real palette name.
    for rgb in sorted({c[:3] for c in colours}, key=lambda t: (_hex(*t) not in EXACT, t)):
        base = opaque_name(*rgb)
        name, n = base, 1
        while _hex(*rgb) not in EXACT and (name in TAILWIND_NAMES or name in taken):
            n += 1
            name = f"{base}-{n}"
        taken.add(name)
        rgb_name[rgb] = name
    names, used = {}, set()
    for r, g, b, a in sorted(colours, key=lambda c: (c[3], c[:3])):
        base = rgb_name[(r, g, b)] + ("" if a == 255 else f"-a{_half_up(a / 255 * 100)}")
        name, n = base, 1
        while name in used:
            n += 1
            name = f"{base}-{n}"
        used.add(name)
        names[(r, g, b, a)] = name
    return names
