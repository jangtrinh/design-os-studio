"""Parity of non-colour design tokens (radius, spacing, size) with `theme.rs`.

G01 in `design_os_self_evolve.py` already pins the colour constants. This module
extends it to every dimensional token that has a Rust constant. The mapping is
explicit: `font-size-title` is 12 while `FONT_SIZE_TITLE` is 20, so names are
never matched by similarity.

A token with no entry in `TOKEN_TO_RUST` is listed as unmapped (information
only). A mapped constant that is not a plain numeric literal fails, so an alias
or expression cannot silently escape the check.
"""

from __future__ import annotations

import re
from typing import Any, Dict, List, Optional

from rust_literal_scan import parse_theme_consts

# Token path -> Rust constants that must all equal the token value. Relationship aliases share the
# primitive they are built on (P3): the ladder rung, the inset and the control height each name a
# different relationship, so each is checked against its primitive separately.
TOKEN_TO_RUST: Dict[str, List[str]] = {
    "radius.radius-xs": ["RADIUS_MARK"],
    "radius.radius-sm": ["RADIUS_SEGMENT"],
    "radius.radius-md": ["RADIUS_CONTROL"],
    "typography.font-size-small": ["FONT_SIZE_CANVAS"],
    "typography.font-size-caption": ["FONT_SIZE_CAPTION"],
    "typography.font-size-body": ["FONT_SIZE_BODY"],
    "typography.font-size-heading": ["FONT_SIZE_LARGE"],
    # The token is named hero; the constant is still called TITLE (it is 20 px). The token
    # `font-size-title` is 12 and belongs to no constant, so names are never matched by similarity.
    "typography.font-size-hero": ["FONT_SIZE_TITLE"],
    "spacing.rail-width": ["RAIL_WIDTH"],
    "spacing.panel-width": ["LEFT_PANEL_WIDTH", "RIGHT_PANEL_WIDTH"],
    "spacing.space-1": ["SPACE_FACT", "SPACE_ROW"],
    "spacing.space-2": ["SPACE_INLINE", "INSET_FRAME", "INSET_CONTROL"],
    "spacing.space-3": ["SPACE_GROUP", "INSET_BUTTON"],
    "spacing.space-4": ["SPACE_BLOCK", "INSET_PANEL", "INSET_POPOVER"],
    "spacing.space-6": ["SPACE_SECTION", "INSET_MODAL"],
    "spacing.space-7": ["CONTROL_STANDARD"],
    "spacing.space-8": ["CONTROL_TOOL"],
    "spacing.space-10": ["CONTROL_ROW"],
    "spacing.space-12": ["SPACE_REGION"],
}

# Role constants with no token to compare against, and why. A new SPACE_/INSET_/CONTROL_/RADIUS_/
# FONT_SIZE_ constant must be mapped above or listed here, so no role value goes unguarded.
RUST_ONLY: Dict[str, str] = {
    "RADIUS_FRAME": "frame radius 14 has no token; the sealed store cannot add one (design/principles.md P1)",
    "INSET_SEGMENT": "2px tile inset inside a segmented control; no 2px spacing token exists",
    "TARGET_MIN": "24px pointer floor (WCAG 2.5.8), a floor rather than a spacing value",
    "CONTROL_DENSE": "24px dense control height, equal to the floor, not to a spacing step",
    "SWATCH_RADIUS": "alias of RADIUS_MARK",
    "CANVAS_BADGE_RADIUS": "alias of RADIUS_MARK",
    "RULER_FONT_SIZE": "alias of FONT_SIZE_CANVAS",
    "FONT_SIZE_LABEL_CANVAS": "11px canvas label, document scale",
}

# Drift that exists today and is fixed by a later, separately ruled change.
# An entry passes only while both sides still hold exactly the recorded values,
# and fails as stale once they agree, so the entry has to be deleted with the fix.
KNOWN_DRIFT: Dict[str, Dict[str, Any]] = {}

_ROLE_CONST = re.compile(r"^(SPACE|INSET|CONTROL|RADIUS|FONT_SIZE)_[A-Z0-9_]+$")

_PX = re.compile(r"^\s*(-?\d+(?:\.\d+)?)\s*(?:px)?\s*$")


def token_value(tokens: Dict[str, Any], path: str) -> Optional[float]:
    """Numeric value of a DTCG dimension token (`6px` -> 6.0); None if absent or not a number."""
    group, _, name = path.partition(".")
    leaf = tokens.get(group, {}).get(name)
    raw = leaf.get("$value") if isinstance(leaf, dict) else None
    m = _PX.match(str(raw)) if raw is not None else None
    return float(m.group(1)) if m else None


def _all_dimension_tokens(tokens: Dict[str, Any]) -> List[str]:
    out = []
    for group in ("radius", "spacing", "typography"):
        for name, leaf in tokens.get(group, {}).items():
            if isinstance(leaf, dict) and leaf.get("$type") == "dimension":
                out.append(f"{group}.{name}")
    return out


def check_non_colour_parity(
    theme_src: str,
    tokens: Dict[str, Any],
    mapping: Optional[Dict[str, List[str]]] = None,
    known_drift: Optional[Dict[str, Dict[str, Any]]] = None,
) -> Dict[str, Any]:
    """Compare mapped dimension tokens with `theme.rs`; pure, so tests can inject drift."""
    mapping = TOKEN_TO_RUST if mapping is None else mapping
    known_drift = KNOWN_DRIFT if known_drift is None else known_drift
    consts = parse_theme_consts(theme_src)
    verified: Dict[str, str] = {}
    drifts: List[str] = []
    known: List[str] = []
    stale: List[str] = []
    problems: List[str] = []

    for path, names in mapping.items():
        tok = token_value(tokens, path)
        if tok is None:
            problems.append(f"{path} missing or not numeric in design.tokens.json")
            continue
        for name in names:
            rust = consts.get(name)
            if rust is None:
                problems.append(f"{name} is not a plain numeric constant in theme.rs")
                continue
            exempt = known_drift.get(path)
            if exempt and exempt["constant"] == name:
                if rust == exempt["rust"] and tok == exempt["token"]:
                    known.append(f"{name} {rust:g} != {path} {tok:g} ({exempt['reason']})")
                elif rust == tok:
                    stale.append(f"{path}: {name} now matches the token; delete its KNOWN_DRIFT entry")
                else:
                    drifts.append(
                        f"{name} ({rust:g}) != {path} ({tok:g}); the pinned drift was {exempt['rust']:g} vs {exempt['token']:g}"
                    )
                continue
            if rust != tok:
                drifts.append(f"{name} ({rust:g}) != {path} ({tok:g})")
            else:
                verified[name] = f"{rust:g} == {path}"
    for path in known_drift:
        if path not in mapping:
            stale.append(f"{path}: KNOWN_DRIFT entry has no mapping row")

    guarded = {name for names in mapping.values() for name in names} | set(RUST_ONLY)
    unguarded = sorted(c for c in consts if _ROLE_CONST.match(c) and c not in guarded)
    for name in unguarded:
        problems.append(f"{name} is a role constant with no token and no RUST_ONLY entry")

    mapped = set(mapping)
    return {
        "ok": not (drifts or stale or problems),
        "unguarded_role_constants": unguarded,
        "verified": verified,
        "drifts": drifts,
        "known_drift": known,
        "stale_exemptions": stale,
        "problems": problems,
        "unmapped_tokens": sorted(set(_all_dimension_tokens(tokens)) - mapped),
    }
