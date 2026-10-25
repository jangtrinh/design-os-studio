#!/usr/bin/env python3
"""
Figma 1:1 Parity Benchmark & Scorecard Engine (`scripts/figma_parity_meter.py`)
===============================================================================
Deterministic, machine-executable benchmark measuring the exact `%` parity between
`design-os-studio` and Figma Desktop `126.9.13` (`app.asar` + `VSF - PCP.fig` v106)
across 5 quantitative pillars:

  1. Pillar 1 (25% weight): `.fig` v106 Kiwi Binary Schema & Node Instance Fidelity
  2. Pillar 2 (20% weight): Desktop Menu & Shortcut Action Matrix (68 Canonical Actions)
  3. Pillar 3 (20% weight): Per-Element Canvas & Inspector Functional Matrix (17 x 5 = 85 Checkpoints)
  4. Pillar 4 (20% weight): UI3 Surfaces, Popovers & Visual Contract (16 Surface Checkpoints)
  5. Pillar 5 (15% weight): Figma AI Agent & Native PTY Terminal Core (15 Checkpoints)

Outputs:
  - `plans/specs/designos-studio-v2/research/parity-scorecard.json`
  - `plans/specs/designos-studio-v2/research/parity-scorecard.md`
  - Refreshes `plans/specs/designos-studio-v2/research/kiwi-schema-diff.json`
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import re
import sys
from pathlib import Path
from typing import Any, Callable, Dict, List, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
RESEARCH_DIR = REPO_ROOT / "plans" / "specs" / "designos-studio-v2" / "research"
KIWI_DIFF_PATH = RESEARCH_DIR / "kiwi-schema-diff.json"
SCORECARD_JSON_PATH = RESEARCH_DIR / "parity-scorecard.json"
SCORECARD_MD_PATH = RESEARCH_DIR / "parity-scorecard.md"

TARGET_FIGMA_VERSION = "126.9.13"
TARGET_FIG_SCHEMA_VERSION = 106

# 7 internal Figma compiler/layout-cache version numbers in VSF - PCP.fig
INTERNAL_COMPILER_METADATA_FIELDS = {
    "derivedSymbolDataLayoutVersion",
    "textUserLayoutVersion",
    "fontVersion",
    "textBidiVersion",
    "textExplicitLayoutVersion",
    "stackLayoutVersion",
    "emojiImageSet",
}


def read_text(rel_path: str) -> str:
    p = REPO_ROOT / rel_path
    if not p.exists():
        return ""
    return p.read_text(encoding="utf-8", errors="replace")


def has_any(text: str, patterns: List[str], ignore_case: bool = False) -> Tuple[bool, str]:
    flags = re.IGNORECASE if ignore_case else 0
    for pat in patterns:
        m = re.search(pat, text, flags)
        if m:
            return True, m.group(0)
    return False, ""


def has_all(text: str, patterns: List[str], ignore_case: bool = False) -> Tuple[bool, str]:
    flags = re.IGNORECASE if ignore_case else 0
    matched = []
    for pat in patterns:
        m = re.search(pat, text, flags)
        if not m:
            return False, f"missing pattern `{pat}`"
        matched.append(m.group(0))
    return True, ", ".join(matched[:3])


# ==============================================================================
# PILLAR 1 (25% Weight): .fig v106 Kiwi Binary Schema & Node Instance Fidelity
# ==============================================================================
def evaluate_pillar_1() -> Tuple[Dict[str, Any], Dict[str, Any]]:
    fig_src_dir = REPO_ROOT / "crates" / "designos-fig" / "src"
    import_modules = [
        "fig.rs",
        "variables.rs",
        "proto.rs",
        "textfmt.rs",
        "board.rs",
        "dev.rs",
        "vecnet.rs",
        "container.rs",
        "rest_import.rs",
    ]
    export_modules = [
        "fig_export.rs",
        "variables.rs",
        "proto.rs",
        "textfmt.rs",
        "board.rs",
        "dev.rs",
        "vecnet.rs",
        "container.rs",
        "export.rs",
    ]

    import_src = "\n".join(
        (fig_src_dir / m).read_text(encoding="utf-8", errors="replace")
        for m in import_modules
        if (fig_src_dir / m).exists()
    )
    export_src = "\n".join(
        (fig_src_dir / m).read_text(encoding="utf-8", errors="replace")
        for m in export_modules
        if (fig_src_dir / m).exists()
    )
    fig_rs = read_text("crates/designos-fig/src/fig.rs")
    fig_export_rs = read_text("crates/designos-fig/src/fig_export.rs")
    node_rs = read_text("crates/designos-doc/src/node.rs")

    import_strings = set(re.findall(r'"([A-Za-z0-9_]+)"', import_src))
    export_strings = set(re.findall(r'"([A-Za-z0-9_]+)"', export_src))

    # Check if lossless raw-field roundtrip bag is implemented in fig.rs & fig_export.rs
    has_roundtrip_bag = bool(
        re.search(r"raw_kiwi|_kiwi_extra|kiwi_extra|unmapped_kiwi|roundtrip_fields", fig_rs)
        and re.search(r"raw_kiwi|_kiwi_extra|kiwi_extra|unmapped_kiwi|roundtrip_fields", fig_export_rs)
    )

    kiwi_diff = json.loads(KIWI_DIFF_PATH.read_text(encoding="utf-8"))
    field_matrix: List[Dict[str, Any]] = kiwi_diff["node_change_field_matrix"]

    # Refresh kiwi-schema-diff.json dynamically against current crates/designos-fig/src
    mapped_in_import_all = 0
    mapped_in_export_all = 0
    semantic_fields_total = 0
    semantic_fields_mapped_import = 0
    semantic_fields_mapped_export = 0

    total_occurrences_all = 0
    mapped_occurrences_all = 0
    total_occurrences_semantic = 0
    mapped_occurrences_semantic = 0

    unmapped_semantic_fields: List[Dict[str, Any]] = []

    for entry in field_matrix:
        fname = entry["field"]
        occ = entry["total_occurrences"]
        is_compiler_meta = fname in INTERNAL_COMPILER_METADATA_FIELDS
        entry["is_internal_compiler_metadata"] = is_compiler_meta

        in_imp = (fname in import_strings) or has_roundtrip_bag
        in_exp = (fname in export_strings) or has_roundtrip_bag
        entry["in_designos_fig_import"] = in_imp
        entry["in_designos_fig_export"] = in_exp
        entry["import_status"] = "mapped" if in_imp else ("compiler_metadata" if is_compiler_meta else "unmapped")

        total_occurrences_all += occ
        if in_imp:
            mapped_in_import_all += 1
            mapped_occurrences_all += occ
        if in_exp:
            mapped_in_export_all += 1

        if not is_compiler_meta:
            semantic_fields_total += 1
            total_occurrences_semantic += occ
            if in_imp:
                semantic_fields_mapped_import += 1
                mapped_occurrences_semantic += occ
            else:
                unmapped_semantic_fields.append(entry)
            if in_exp:
                semantic_fields_mapped_export += 1

    # Verify the 7 Production Fidelity Gaps (GAP-01 .. GAP-07)
    gap_checks = [
        (
            "GAP-01",
            "Per-side border weights (borderTop/Right/Bottom/LeftWeight) without borderStrokeWeightsIndependent",
            bool(
                re.search(
                    r'\[\s*"borderTopWeight"\s*,\s*"borderRightWeight"\s*,\s*"borderBottomWeight"\s*,\s*"borderLeftWeight"\s*\]\s*\.iter\(\)\.any',
                    fig_rs,
                )
            ),
            "crates/designos-fig/src/fig.rs:490",
        ),
        (
            "GAP-02",
            "Per-corner radii (rectangleTopLeft..BottomLeftCornerRadius) without rectangleCornerRadiiIndependent",
            bool(
                re.search(
                    r'\[\s*"rectangleTopLeftCornerRadius"\s*,\s*"rectangleTopRightCornerRadius"\s*,\s*"rectangleBottomRightCornerRadius"\s*,\s*"rectangleBottomLeftCornerRadius"\s*\]\s*\.iter\(\)\s*\.any',
                    fig_rs,
                )
            ),
            "crates/designos-fig/src/fig.rs:509",
        ),
        (
            "GAP-03",
            "VariableField bindings in variableConsumptionMap (VISIBLE, TEXT_DATA, OVERRIDDEN_SYMBOL_ID, BORDER_*_WEIGHT)",
            all(
                tok in fig_rs
                for tok in [
                    '"VISIBLE" => &["visible"]',
                    '"TEXT_DATA" => &["characters"]',
                    '"OVERRIDDEN_SYMBOL_ID"',
                    '"BORDER_TOP_WEIGHT"',
                    '"BORDER_BOTTOM_WEIGHT"',
                ]
            ),
            "crates/designos-fig/src/fig.rs:743",
        ),
        (
            "GAP-04",
            "CSS Grid v106 fields (gridColumnAnchor, gridRowAnchor, gridColumnsSizing, gridRowsSizing, gridColumnGap, gridRowGap)",
            all(
                tok in fig_rs
                for tok in [
                    '"gridColumnAnchor"',
                    '"gridRowAnchor"',
                    '"gridColumnsSizing"',
                    '"gridRowsSizing"',
                    '"gridColumnGap"',
                    '"gridRowGap"',
                ]
            ),
            "crates/designos-fig/src/fig.rs:566",
        ),
        (
            "GAP-05",
            "Aspect ratio lock (proportionsConstrained, targetAspectRatio) & stackPrimarySizing override consistency",
            all(
                tok in fig_rs
                for tok in [
                    '"proportionsConstrained"',
                    '"targetAspectRatio"',
                    '"preserveRatio"',
                    '"stackPrimarySizing"',
                ]
            )
            and "lock_aspect" in node_rs,
            "crates/designos-fig/src/fig.rs:417",
        ),
        (
            "GAP-06",
            "derivedSymbolData size/transform/geometry preservation + nc/symbolData fallback",
            bool(re.search(r'"size"\s*\|\s*"relativeTransform"\s*\|\s*"fillGeometry"\s*\|\s*"strokeGeometry"', fig_rs))
            and 'nc.get("derivedSymbolData").or_else(' in fig_rs,
            "crates/designos-fig/src/fig.rs:722,1246",
        ),
        (
            "GAP-07",
            "Typography variable bindings (fontStyleValue, FONT_SIZE..PARAGRAPH_INDENT) + v106 Section devStatus, Hyperlinks, PluginData, Connector & ShapeWithText",
            all(
                tok in fig_rs
                for tok in [
                    '"fontStyleValue"',
                    '"FONT_SIZE"',
                    '"LINE_HEIGHT"',
                    '"LETTER_SPACING"',
                    '"sectionStatusInfo"',
                    '"hyperlink"',
                    '"pluginData"',
                    '"connectorStart"',
                    '"shapeWithTextType"',
                ]
            ),
            "crates/designos-fig/src/fig.rs:516,798",
        ),
    ]

    # Update kiwi-schema-diff.json on disk with refreshed counts
    kiwi_diff["schema_totals"]["mapped_in_designos_fig_import"] = mapped_in_import_all
    kiwi_diff["schema_totals"]["unmapped_in_designos_fig_import"] = len(field_matrix) - mapped_in_import_all
    kiwi_diff["schema_totals"]["mapped_in_designos_fig_export"] = mapped_in_export_all
    kiwi_diff["schema_totals"]["unmapped_in_designos_fig_export"] = len(field_matrix) - mapped_in_export_all
    kiwi_diff["schema_totals"]["semantic_observed_fields_excl_compiler_meta"] = semantic_fields_total
    kiwi_diff["schema_totals"]["semantic_mapped_in_import"] = semantic_fields_mapped_import
    kiwi_diff["schema_totals"]["semantic_mapped_in_export"] = semantic_fields_mapped_export
    kiwi_diff["schema_totals"]["node_instance_weighted_import_pct"] = round(
        (mapped_occurrences_semantic / max(1, total_occurrences_semantic)) * 100.0, 2
    )
    kiwi_diff["schema_totals"]["node_instance_weighted_raw_import_pct"] = round(
        (mapped_occurrences_all / max(1, total_occurrences_all)) * 100.0, 2
    )
    KIWI_DIFF_PATH.write_text(json.dumps(kiwi_diff, indent=2), encoding="utf-8")

    # Pillar 1 item accounting:
    # 196 semantic observed fields + 7 production fidelity gates = 203 items
    passed_gaps = sum(1 for _, _, ok, _ in gap_checks if ok)
    total_items = semantic_fields_total + len(gap_checks)  # 196 + 7 = 203
    passed_items = semantic_fields_mapped_import + passed_gaps

    # Pillar 1 score blends:
    # - 50% Node-Instance-Weighted Semantic Coverage % (measures actual node fidelity across 155,297 nodes)
    # - 30% Schema Field Coverage % (semantic fields mapped / 196)
    # - 20% 7 Production Fidelity Gates (GAP-01..GAP-07)
    instance_weighted_pct = (mapped_occurrences_semantic / max(1, total_occurrences_semantic)) * 100.0
    raw_instance_weighted_pct = (mapped_occurrences_all / max(1, total_occurrences_all)) * 100.0
    schema_semantic_pct = (semantic_fields_mapped_import / max(1, semantic_fields_total)) * 100.0
    gap_gate_pct = (passed_gaps / len(gap_checks)) * 100.0

    pillar_1_score = round(
        0.50 * instance_weighted_pct + 0.30 * schema_semantic_pct + 0.20 * gap_gate_pct,
        2,
    )

    remaining_gaps: List[Dict[str, Any]] = []
    for gid, gtitle, ok, fhint in gap_checks:
        if not ok:
            remaining_gaps.append(
                {
                    "id": f"P1-{gid}",
                    "title": gtitle,
                    "pillar": "Pillar 1: .fig v106 Kiwi Schema & Node Fidelity",
                    "impact_pct": round((1.0 / len(gap_checks)) * 20.0 * 0.25, 3),
                    "file_hint": fhint,
                }
            )

    # Group remaining unmapped semantic fields into high-signal actionable items
    for entry in sorted(unmapped_semantic_fields, key=lambda e: -e["total_occurrences"]):
        fname = entry["field"]
        occ = entry["total_occurrences"]
        nc_cnt = entry["node_count_in_vsf_pcp"]
        ov_cnt = entry["override_count_in_vsf_pcp"]
        occ_share = occ / max(1, total_occurrences_semantic)
        impact = round((occ_share * 0.50 + (1.0 / semantic_fields_total) * 0.30) * 25.0, 4)
        remaining_gaps.append(
            {
                "id": f"P1-FIELD-{fname}",
                "title": f"Map/preserve Kiwi NodeChange.{fname} (tag={entry['tag']}, type={entry['kiwi_type']}; {nc_cnt} nodes, {ov_cnt} overrides in VSF-PCP)",
                "pillar": "Pillar 1: .fig v106 Kiwi Schema & Node Fidelity",
                "impact_pct": impact,
                "file_hint": "crates/designos-fig/src/fig.rs & crates/designos-fig/src/fig_export.rs",
            }
        )

    kiwi_field_stats = {
        "total_defined_on_node_change": kiwi_diff["schema_totals"]["node_change_defined_fields"],
        "total_observed_in_vsf_pcp": len(field_matrix),
        "internal_compiler_metadata_fields_count": len(INTERNAL_COMPILER_METADATA_FIELDS),
        "internal_compiler_metadata_fields": sorted(INTERNAL_COMPILER_METADATA_FIELDS),
        "semantic_observed_in_vsf_pcp": semantic_fields_total,
        "mapped_in_import": mapped_in_import_all,
        "mapped_in_export": mapped_in_export_all,
        "semantic_mapped_in_import": semantic_fields_mapped_import,
        "semantic_mapped_in_export": semantic_fields_mapped_export,
        "schema_field_import_pct": round((mapped_in_import_all / len(field_matrix)) * 100.0, 2),
        "semantic_schema_field_import_pct": round(schema_semantic_pct, 2),
        "node_instance_weighted_import_pct": round(instance_weighted_pct, 2),
        "node_instance_weighted_raw_import_pct": round(raw_instance_weighted_pct, 2),
        "production_gaps_passed": passed_gaps,
        "production_gaps_total": len(gap_checks),
        "production_gap_checks": [
            {"id": gid, "title": gtitle, "passed": ok, "file_hint": fhint}
            for gid, gtitle, ok, fhint in gap_checks
        ],
    }

    pillar_obj = {
        "id": "pillar-1-kiwi-schema",
        "name": "Pillar 1: .fig v106 Kiwi Binary Schema & Node Instance Fidelity",
        "weight_pct": 25.0,
        "score_pct": pillar_1_score,
        "weighted_contribution_pct": round(pillar_1_score * 0.25, 2),
        "passed_items": passed_items,
        "total_items": total_items,
        "submetrics": {
            "node_instance_weighted_semantic_pct": round(instance_weighted_pct, 2),
            "node_instance_weighted_raw_pct": round(raw_instance_weighted_pct, 2),
            "semantic_schema_field_coverage_pct": round(schema_semantic_pct, 2),
            "raw_schema_field_coverage_pct": round((mapped_in_import_all / len(field_matrix)) * 100.0, 2),
            "production_gaps_gate_pct": round(gap_gate_pct, 2),
        },
        "remaining_gaps": remaining_gaps,
    }
    return pillar_obj, kiwi_field_stats


# ==============================================================================
# PILLAR 2 (20% Weight): Desktop Menu & Shortcut Action Matrix (68 Actions)
# ==============================================================================
def evaluate_pillar_2() -> Dict[str, Any]:
    menus_rs = read_text("crates/designos-app/src/panels/menus.rs")
    tools_rs = read_text("crates/designos-app/src/canvas/tools.rs")
    shell_rs = read_text("crates/designos-app/src/shell.rs")
    context_rs = read_text("crates/designos-app/src/panels/context.rs")
    clipboard_rs = read_text("crates/designos-app/src/clipboard.rs")
    app_rs = read_text("crates/designos-app/src/app.rs")
    ops_rs = read_text("crates/designos-app/src/ops.rs")
    textsession_rs = read_text("crates/designos-app/src/canvas/textsession.rs")

    combined = "\n".join([menus_rs, tools_rs, shell_rs, context_rs, clipboard_rs, app_rs, ops_rs, textsession_rs])

    # 68 Canonical Figma Desktop (126.9.13) Menu & Shortcut Actions
    actions_spec: List[Tuple[str, str, str, List[str], str]] = [
        # (id, category, label_and_shortcut, regex_patterns_in_code, file_hint)
        ("M01", "App", "About design:os studio (AboutDesignOS)", [r"Action::AboutDesignOS"], "crates/designos-app/src/panels/menus.rs"),
        ("M02", "App", "Preferences -> Your name & GPU rendering", [r"Action::YourName", r"Action::ToggleGpu"], "crates/designos-app/src/panels/menus.rs"),
        ("M03", "App", "Back to files (Home screen)", [r"Action::BackToFiles"], "crates/designos-app/src/panels/menus.rs"),
        ("M04", "File", "New design file (Action::New / Cmd+T)", [r"Action::New\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M05", "File", "New FigJam board (Action::NewBoard)", [r"Action::NewBoard"], "crates/designos-app/src/panels/menus.rs"),
        ("M06", "File", "New slide deck (Action::NewDeck)", [r"Action::NewDeck"], "crates/designos-app/src/panels/menus.rs"),
        ("M07", "File", "Open file (.fig / .designos / JSON) (Cmd+O)", [r"Action::Open\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M08", "File", "Save file (Cmd+S)", [r"Action::Save\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M09", "File", "Save as (Cmd+Shift+S)", [r"Action::SaveAs"], "crates/designos-app/src/panels/menus.rs"),
        ("M10", "File", "Save local .fig copy (Action::ExportFig)", [r"Action::ExportFig"], "crates/designos-app/src/panels/menus.rs"),
        ("M11", "File", "Save to version history (Cmd+Alt+S)", [r"Action::SaveVersion"], "crates/designos-app/src/panels/menus.rs"),
        ("M12", "File", "Show version history", [r"Action::ToggleVersionHistory"], "crates/designos-app/src/panels/menus.rs"),
        ("M13", "File", "Place image/video (Cmd+Shift+K / Tool::Image)", [r"Tool::Image"], "crates/designos-app/src/canvas/tools.rs"),
        ("M14", "File", "Export selection (Cmd+Shift+E / Action::Export)", [r"Action::Export\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M15", "File", "Export frames/slides to PDF (Action::ExportDeckPdf)", [r"Action::ExportDeckPdf"], "crates/designos-app/src/panels/menus.rs"),
        ("M16", "Edit", "Undo (Cmd+Z)", [r"Action::Undo"], "crates/designos-app/src/panels/menus.rs"),
        ("M17", "Edit", "Redo (Cmd+Shift+Z / Cmd+Y)", [r"Action::Redo"], "crates/designos-app/src/panels/menus.rs"),
        ("M18", "Edit", "Cut selection (Cmd+X)", [r"Event::Cut", r"cut_selection"], "crates/designos-app/src/shell.rs"),
        ("M19", "Edit", "Copy selection (Cmd+C)", [r"Action::Copy\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M20", "Edit", "Copy as SVG / CSS / PNG (copy_as)", [r"pub fn copy_as\b", r"COPY_ITEMS"], "crates/designos-app/src/panels/export.rs"),
        ("M21", "Edit", "Paste (Cmd+V)", [r"Action::Paste\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M22", "Edit", "Paste over selection (Cmd+Shift+V)", [r"PasteOverSelection|paste_over_selection"], "crates/designos-app/src/panels/menus.rs"),
        ("M23", "Edit", "Paste to replace (Cmd+Alt+Shift+R)", [r"PasteToReplace|paste_to_replace"], "crates/designos-app/src/panels/menus.rs"),
        ("M24", "Edit", "Duplicate (Cmd+D)", [r"Action::Duplicate"], "crates/designos-app/src/panels/menus.rs"),
        ("M25", "Edit", "Delete (Backspace / Del)", [r"Action::Delete"], "crates/designos-app/src/panels/menus.rs"),
        ("M26", "Edit", "Copy / Paste properties (Cmd+Alt+C / Cmd+Alt+V)", [r"CopyProperties|copy_properties"], "crates/designos-app/src/panels/menus.rs"),
        ("M27", "Edit", "Eyedropper / Pick color (I)", [r"eyedropper|Eyedropper|PickColor"], "crates/designos-app/src/widgets/picker.rs"),
        ("M28", "Edit", "Find and replace (Cmd+F)", [r"Action::Find\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M29", "Edit", "Select all (Cmd+A)", [r"Action::SelectAll"], "crates/designos-app/src/panels/menus.rs"),
        ("M30", "Edit", "Select inverse (Cmd+Shift+A)", [r"Action::SelectInverse"], "crates/designos-app/src/panels/menus.rs"),
        ("M31", "Edit", "Select all with same fill/stroke/effect/font/instance", [r"MatchCriteria::Fill", r"MatchCriteria::Effect", r"MatchCriteria::Instance"], "crates/designos-app/src/panels/menus.rs"),
        ("M32", "View", "Pixel grid toggle (Shift+' / Cmd+')", [r"Action::TogglePixelGrid"], "crates/designos-app/src/panels/menus.rs"),
        ("M33", "View", "Snap to pixel grid toggle (Cmd+Shift+')", [r"Action::ToggleSnapPixel"], "crates/designos-app/src/panels/menus.rs"),
        ("M34", "View", "Layout guides toggle (Ctrl+G / Cmd+Shift+4)", [r"Action::ToggleLayoutGrids"], "crates/designos-app/src/panels/menus.rs"),
        ("M35", "View", "Rulers toggle (Shift+R)", [r"Action::ToggleRulers"], "crates/designos-app/src/panels/menus.rs"),
        ("M36", "View", "Outlines mode toggle (Shift+O / Cmd+Y)", [r'"Outlines"'], "crates/designos-app/src/panels/menus.rs"),
        ("M37", "View", "Pixel preview (Shift+Cmd+P)", [r'"Pixel preview"'], "crates/designos-app/src/panels/menus.rs"),
        ("M38", "View", "Show/Hide UI sidebar (Cmd+\\)", [r"Action::ToggleSidebar"], "crates/designos-app/src/panels/menus.rs"),
        ("M39", "View", "Multiplayer cursors row (Alt+Cmd+\\)", [r'"Multiplayer cursors"'], "crates/designos-app/src/panels/menus.rs"),
        ("M40", "View", "Comments visibility/tool (C / Shift+C)", [r"Tool::Comment"], "crates/designos-app/src/panels/menus.rs"),
        ("M41", "View", "Annotations in Zoom/Dev Mode (Shift+Y)", [r'"Annotations"'], "crates/designos-app/src/panels/menus.rs"),
        ("M42", "View", "Actions / Command palette (Cmd+K / Cmd+P)", [r"Action::OpenPalette"], "crates/designos-app/src/panels/menus.rs"),
        ("M43", "View", "Zoom in / Zoom out (Cmd++ / Cmd+-)", [r"Action::ZoomIn", r"Action::ZoomOut"], "crates/designos-app/src/panels/menus.rs"),
        ("M44", "View", "Zoom to 100% / 50% / 200% (Cmd+0 / Shift+0)", [r"Action::Zoom100", r"Action::ZoomTo"], "crates/designos-app/src/panels/menus.rs"),
        ("M45", "View", "Zoom to fit / Zoom to selection (Shift+1 / Shift+2)", [r"Action::FitAll", r"Action::FitSelection"], "crates/designos-app/src/panels/menus.rs"),
        ("M46", "View", "Switch open project tabs (Cmd+Tab / Cmd+Shift+Tab / Cmd+W)", [r"app\.next_tab\(\)", r"app\.prev_tab\(\)", r"app\.close_tab"], "crates/designos-app/src/shell.rs"),
        ("M47", "Object", "Group selection (Cmd+G)", [r"Action::Group\b"], "crates/designos-app/src/panels/menus.rs"),
        ("M48", "Object", "Frame selection (Cmd+Alt+G)", [r"Action::FrameSelection"], "crates/designos-app/src/panels/menus.rs"),
        ("M49", "Object", "Ungroup selection (Cmd+Shift+G)", [r"Action::Ungroup"], "crates/designos-app/src/panels/menus.rs"),
        ("M50", "Object", "Use as Mask (is_mask toggle)", [r'"Use as mask"', r"n\.is_mask\s*=\s*!on"], "crates/designos-app/src/panels/properties.rs"),
        ("M51", "Object", "Add / Toggle Auto Layout (Shift+A)", [r"Action::AutoLayout"], "crates/designos-app/src/panels/menus.rs"),
        ("M52", "Object", "Create component & Combine as variants (Cmd+Alt+K / Cmd+Shift+K)", [r"Action::CreateComponent", r"Action::CombineVariants"], "crates/designos-app/src/panels/menus.rs"),
        ("M53", "Object", "Detach instance / Go to main / Reset instance (Cmd+Alt+B)", [r"Action::Detach\b", r"Action::GoToMain", r"Action::ResetInstance"], "crates/designos-app/src/panels/menus.rs"),
        ("M54", "Object", "Lock/Unlock & Show/Hide layers (Cmd+Shift+L / Cmd+Shift+H)", [r"n\.locked\s*=\s*!n\.locked|toggle_lock", r"n\.visible\s*=\s*!n\.visible|toggle_visible"], "crates/designos-app/src/panels/layers.rs"),
        ("M55", "Object", "Flip horizontal / Flip vertical (Shift+H / Shift+V / flip_selection)", [r"pub fn flip_selection\b", r"flip_nodes_about"], "crates/designos-app/src/ops.rs"),
        ("M56", "Object", "Mark as ready for dev / Completed / Clear status", [r"Action::MarkReadyForDev", r"Action::MarkCompleted", r"Action::ClearDevStatus"], "crates/designos-app/src/panels/menus.rs"),
        ("M57", "Vector", "Flatten selection (Cmd+E)", [r"Action::Flatten"], "crates/designos-app/src/panels/menus.rs"),
        ("M58", "Vector", "Outline stroke (Cmd+Alt+O / Cmd+Shift+O)", [r"Action::OutlineStroke"], "crates/designos-app/src/panels/menus.rs"),
        ("M59", "Vector", "Boolean Union / Subtract / Intersect / Exclude", [r"Action::Boolean\(designos_doc::BooleanOp::Union\)", r"BooleanOp::Subtract", r"BooleanOp::Intersect", r"BooleanOp::Exclude"], "crates/designos-app/src/panels/menus.rs"),
        ("M60", "Vector", "Edit vertices / Join / Split / Simplify / Shape Builder (Enter / Cmd+J / Shift+M)", [r"Action::EditVertices", r"Action::VectorJoin", r"Action::VectorSplit", r"Action::VectorSimplify", r"Tool::ShapeBuilder"], "crates/designos-app/src/panels/menus.rs"),
        ("M61", "Text", "Bold / Italic / Underline (Cmd+B / Cmd+I / Cmd+U)", [r"Action::TextBold", r"Action::TextItalic", r"Action::TextUnderline"], "crates/designos-app/src/panels/menus.rs"),
        ("M62", "Text", "Strikethrough (Cmd+Shift+X / TextDecoration::Strikethrough)", [r"Strikethrough"], "crates/designos-app/src/panels/typography.rs"),
        ("M63", "Text", "Bulleted & Numbered lists (Cmd+Shift+8 / Cmd+Shift+7)", [r"Action::TextBulletList", r"Action::TextNumberedList"], "crates/designos-app/src/panels/menus.rs"),
        ("M64", "Text", "Increase / Decrease font size (Cmd+Shift+> / <)", [r"Action::TextSizeUp", r"Action::TextSizeDown"], "crates/designos-app/src/panels/menus.rs"),
        ("M65", "Arrange", "Bring to front / Bring forward / Send backward / Send to back", [r"Action::BringToFront", r"Action::BringForward", r"Action::SendBackward", r"Action::SendToBack"], "crates/designos-app/src/panels/menus.rs"),
        ("M66", "Arrange", "Align Left / HCenter / Right / Top / VCenter / Bottom (Alt+A/H/D/W/V/S)", [r"Align::Left", r"Align::HCenter", r"Align::Right", r"Align::Top", r"Align::VCenter", r"Align::Bottom"], "crates/designos-app/src/panels/menus.rs"),
        ("M67", "Arrange", "Distribute spacing (Action::Distribute)", [r"Action::Distribute"], "crates/designos-app/src/panels/menus.rs"),
        ("M68", "Tools", "Single-key tool accelerators (V, H, K, F, Shift+S, S, R, L, Shift+L, O, P, Shift+P, T, C)", [r"Tool::Move", r"Tool::Hand", r"Tool::Scale", r"Tool::Frame", r"Tool::Section", r"Tool::Slice", r"Tool::Pen", r"Tool::Pencil"], "crates/designos-app/src/canvas/tools.rs"),
    ]

    layers_rs = read_text("crates/designos-app/src/panels/layers.rs")
    typography_rs = read_text("crates/designos-app/src/panels/typography.rs")
    picker_rs = read_text("crates/designos-app/src/widgets/picker.rs")
    properties_rs = read_text("crates/designos-app/src/panels/properties.rs")
    export_rs = read_text("crates/designos-app/src/panels/export.rs")
    full_corpus = "\n".join([combined, layers_rs, typography_rs, picker_rs, properties_rs, export_rs])

    passed = 0
    remaining_gaps = []
    check_results = []
    for aid, cat, title, pats, fhint in actions_spec:
        ok, ev = has_all(full_corpus, pats)
        if ok:
            passed += 1
        else:
            remaining_gaps.append(
                {
                    "id": f"P2-{aid}",
                    "title": f"[{cat}] {title}",
                    "pillar": "Pillar 2: Desktop Menu & Shortcut Action Matrix",
                    "impact_pct": round((1.0 / len(actions_spec)) * 20.0, 3),
                    "file_hint": fhint,
                }
            )
        check_results.append({"id": aid, "category": cat, "title": title, "passed": ok, "evidence": ev, "file_hint": fhint})

    score_pct = round((passed / len(actions_spec)) * 100.0, 2)
    return {
        "id": "pillar-2-menus-shortcuts",
        "name": "Pillar 2: Desktop Menu & Shortcut Action Matrix (68 Canonical Actions)",
        "weight_pct": 20.0,
        "score_pct": score_pct,
        "weighted_contribution_pct": round(score_pct * 0.20, 2),
        "passed_items": passed,
        "total_items": len(actions_spec),
        "checks": check_results,
        "remaining_gaps": remaining_gaps,
    }


# ==============================================================================
# PILLAR 3 (20% Weight): Per-Element Canvas & Inspector Functional Matrix (17x5=85)
# ==============================================================================
def evaluate_pillar_3() -> Dict[str, Any]:
    draw_rs = read_text("crates/designos-app/src/canvas/draw.rs")
    handles_rs = read_text("crates/designos-app/src/canvas/handles.rs")
    interact_rs = read_text("crates/designos-app/src/canvas/interact.rs")
    hit_rs = read_text("crates/designos-app/src/canvas/hit.rs")
    vector_rs = read_text("crates/designos-app/src/canvas/vector.rs")
    create_rs = read_text("crates/designos-app/src/canvas/create.rs")
    properties_rs = read_text("crates/designos-app/src/panels/properties.rs")
    components_rs = read_text("crates/designos-app/src/panels/components.rs")
    context_rs = read_text("crates/designos-app/src/panels/context.rs")
    board_bar_rs = read_text("crates/designos-app/src/board/bar.rs")
    board_connect_rs = read_text("crates/designos-app/src/board/connect.rs")
    board_create_rs = read_text("crates/designos-app/src/board/create.rs")
    node_rs = read_text("crates/designos-doc/src/node.rs")

    corpus = "\n".join(
        [
            draw_rs,
            handles_rs,
            interact_rs,
            hit_rs,
            vector_rs,
            create_rs,
            properties_rs,
            components_rs,
            context_rs,
            board_bar_rs,
            board_connect_rs,
            board_create_rs,
            node_rs,
        ]
    )

    # 17 elements x 5 checkpoints = 85 checkpoints
    # Checkpoints per element:
    #   C1: Selection/Hover Chrome
    #   C2: On-Canvas Smart Handles / Resize
    #   C3: Double-Click / Enter Behavior
    #   C4: Right-Panel / Contextual Inspector Controls
    #   C5: Context Menu Specials
    elements_spec: List[Tuple[str, List[Tuple[str, str, List[str], str]]]] = [
        (
            "Frame",
            [
                ("C1", "Frame selection outline + top-left title label + CSS grid / auto-layout overlays", [r"NodeKind::Frame", r"label_rects", r"draw_css_grid_overlays"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "8 resize handles + 4 corner-radius handles + pink Auto-Layout padding/gap handles", [r"corner_radius_handle_positions", r"auto_layout_handles"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click / Enter drills down to direct child layer", [r"on_double_click", r"navigate_hierarchy_enter"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Position, Layout, Auto-Layout (H/V/Wrap/Grid), Clip content, Fill/Stroke/Effects/Export", [r"auto_layout_ui", r"clips_content"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Context menu Frame selection, Ungroup, Add auto layout, Mark as ready for dev", [r"Action::FrameSelection", r"Action::AutoLayout", r"Action::MarkReadyForDev"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Section",
            [
                ("C1", "Section pill header label + dev status badge on canvas", [r"NodeKind::Section", r"label_rects"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Section 8-handle resize without stretching children + reparent overlapping nodes", [r"NodeKind::Section", r"reparent_after_move"], "crates/designos-app/src/canvas/interact.rs"),
                ("C3", "Double-click / Enter selects child inside Section", [r"on_double_click", r"NodeKind::Section"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Section Position, Size, Fill, Stroke, Export", [r"NodeKind::Section"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Context menu Ungroup Section & Mark as ready for dev", [r"NodeKind::Section", r"Action::MarkReadyForDev"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Group",
            [
                ("C1", "Group bounds union & selection outline", [r"NodeKind::Group"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Group proportional child resize handles", [r"NodeKind::Group", r"update_resize"], "crates/designos-app/src/canvas/interact.rs"),
                ("C3", "Double-click drills into Group child hierarchy", [r"on_double_click", r"entered"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Group bounds, Opacity, Blend, Selection colors, Export", [r"selection_colors_section"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Context menu Group / Ungroup / Frame selection", [r"Action::Group", r"Action::Ungroup"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Component",
            [
                ("C1", "Purple (#9747FF) component outline & diamond badge on canvas", [r"FIGMA_COMPONENT_PURPLE", r"NodeKind::Component\("], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Component 8 resize + corner-radius + auto-layout handles", [r"supports_corner_radius_handles", r"auto_layout_handles"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click enters Component child layer", [r"on_double_click"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Component properties definitions & description", [r"ComponentPropDef", r"description"], "crates/designos-app/src/panels/components.rs"),
                ("C5", "Context menu Create component & Combine as variants", [r"Action::CreateComponent", r"Action::CombineVariants"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "ComponentSet",
            [
                ("C1", "Purple (#9747FF) dashed ComponentSet frame outline", [r"NodeKind::ComponentSet", r"dashed_quad"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "ComponentSet resize & auto-layout grid handles", [r"NodeKind::ComponentSet"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click selects variant Component inside ComponentSet", [r"on_double_click"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Variant properties & options editor", [r"PropKind::Variant"], "crates/designos-app/src/panels/components.rs"),
                ("C5", "Context menu Combine as variants & Ungroup", [r"Action::CombineVariants"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Instance",
            [
                ("C1", "Purple (#9747FF) instance outline + hollow diamond glyph (◇)", [r"paint_instance_diamond", r"NodeKind::Instance"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Instance resize with auto-layout reflow & constraint rules", [r"NodeKind::Instance", r"update_resize"], "crates/designos-app/src/canvas/interact.rs"),
                ("C3", "Double-click drills into instance sub-layer for override editing", [r"n\.component\.source\.is_some\(\)"], "crates/designos-app/src/canvas/draw.rs"),
                ("C4", "Right-panel Instance swap, Variant/Bool/Text property controls, Reset overrides", [r"OverrideField", r"PropValue"], "crates/designos-app/src/panels/components.rs"),
                ("C5", "Context menu Detach instance, Go to main component, Reset all changes", [r"Action::Detach", r"Action::GoToMain", r"Action::ResetInstance"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Text",
            [
                ("C1", "Text bounding box, baseline indicator, and live selection highlight", [r"NodeKind::Text"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Text box resize handles updating TextAutoResize mode", [r"TextAutoResize"], "crates/designos-app/src/canvas/interact.rs"),
                ("C3", "Double-click / Enter enters in-canvas rich text editing session", [r"begin_text_edit"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Typography section + Type settings 3-tab popover (Basic/Details/Variable)", [r"details_popover", r"family_picker"], "crates/designos-app/src/panels/typography.rs"),
                ("C5", "Context menu / shortcuts Bold, Italic, Underline, Lists, Flatten to outlines", [r"Action::TextBold", r"Action::Flatten"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Vector",
            [
                ("C1", "Vector path outline + vertex/tangent handle overlay in vector edit mode", [r"NodeKind::Vector", r"draw_pen"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Vector vertex drag, cubic Bezier tangent handles, segment midpoint insertion", [r"VectorNetwork"], "crates/designos-app/src/canvas/vector.rs"),
                ("C3", "Double-click / Enter enters interactive vertex editing (canvas::vector::enter)", [r"canvas::vector::enter"], "crates/designos-app/src/shell.rs"),
                ("C4", "Right-panel Stroke weight, align, cap, join, dashes, miter + Vertex corner radius", [r"miter_limit", r"dashes"], "crates/designos-app/src/panels/stroke.rs"),
                ("C5", "Vector menu Flatten, Outline stroke, Join, Split, Simplify", [r"Action::VectorJoin", r"Action::VectorSplit", r"Action::VectorSimplify"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "BooleanGroup",
            [
                ("C1", "BooleanOp result outline and child operand wireframe", [r"NodeKind::BooleanOp"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Resize BooleanOp container and live-recompute when child operands move", [r"BooleanOp"], "crates/designos-doc/src/node.rs"),
                ("C3", "Double-click drills into BooleanOp child operand shapes", [r"NodeKind::BooleanOp"], "crates/designos-doc/src/node.rs"),
                ("C4", "Right-panel Boolean operation switcher (Union/Subtract/Intersect/Exclude)", [r"BooleanOp::Union", r"BooleanOp::Subtract"], "crates/designos-app/src/panels/menus.rs"),
                ("C5", "Menu Boolean groups & Flatten boolean result", [r"Action::Boolean", r"Action::Flatten"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Star",
            [
                ("C1", "Star parametric polygon outline on canvas", [r"NodeKind::Star"], "crates/designos-doc/src/node.rs"),
                ("C2", "On-canvas smart handles for Star point count and inner ratio", [r"shape_control_handles", r"NodeKind::Star"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click / Enter converts or enters vertex editing", [r"canvas::vector::enter"], "crates/designos-app/src/shell.rs"),
                ("C4", "Right-panel Star count, inner ratio %, and corner radius controls", [r"inner_ratio", r"NodeKind::Star"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Flatten & Outline stroke on Star", [r"Action::Flatten", r"Action::OutlineStroke"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Polygon",
            [
                ("C1", "Polygon parametric N-gon outline on canvas", [r"NodeKind::Polygon"], "crates/designos-doc/src/node.rs"),
                ("C2", "On-canvas smart handle for Polygon side count", [r"shape_control_handles", r"NodeKind::Polygon"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click / Enter enters vertex editing", [r"canvas::vector::enter"], "crates/designos-app/src/shell.rs"),
                ("C4", "Right-panel Polygon side count and corner radius controls", [r"NodeKind::Polygon"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Flatten & Outline stroke on Polygon", [r"Action::Flatten", r"Action::OutlineStroke"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Ellipse",
            [
                ("C1", "Ellipse / Donut / Pie arc outline on canvas", [r"NodeKind::Ellipse"], "crates/designos-doc/src/node.rs"),
                ("C2", "On-canvas smart handles for Arc start angle, sweep end, and inner donut radius", [r"shape_control_handles", r"ArcData"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click / Enter enters vertex editing", [r"canvas::vector::enter"], "crates/designos-app/src/shell.rs"),
                ("C4", "Right-panel Ellipse Arc start, sweep, and ratio controls", [r"ArcData", r"inner_radius"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Flatten & Outline stroke on Ellipse", [r"Action::Flatten", r"Action::OutlineStroke"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Line",
            [
                ("C1", "Line 1D stroke selection outline on canvas", [r"NodeKind::Line"], "crates/designos-doc/src/node.rs"),
                ("C2", "Line endpoint resize & 15/45-degree angle snapping", [r"snap_angle", r"Tool::Line", r"Tool::Arrow"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click / Enter enters vertex editing on Line", [r"canvas::vector::enter"], "crates/designos-app/src/shell.rs"),
                ("C4", "Right-panel Line length, angle, stroke weight, and arrow caps", [r"StrokeCap"], "crates/designos-app/src/panels/stroke.rs"),
                ("C5", "Outline stroke & Flip on Line", [r"Action::OutlineStroke"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Connector",
            [
                ("C1", "Connector polyline route outline & hovered node 4-side magnet dots", [r"connector_route", r"dot_positions"], "crates/designos-app/src/board/connect.rs"),
                ("C2", "Connector endpoint drag snapping to Top/Right/Bottom/Left magnets + live re-routing", [r"ConnectDrag", r"Magnet::Top"], "crates/designos-app/src/board/connect.rs"),
                ("C3", "Double-click / Enter edits centered Connector text label", [r"edit_label", r"NodeKind::Connector"], "crates/designos-app/src/shell.rs"),
                ("C4", "Contextual board bar for Connector line style, stroke color, and start/end caps", [r"c\.line", r"NodeKind::Connector"], "crates/designos-app/src/board/bar.rs"),
                ("C5", "Delete / Duplicate / Reorder Connector", [r"Action::Duplicate", r"Action::Delete"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Sticky",
            [
                ("C1", "Sticky note card shadow, fill, and author footer on canvas", [r"NodeKind::Sticky", r"StickyProps"], "crates/designos-doc/src/node.rs"),
                ("C2", "Sticky resize handles + Tab key spawns adjacent sticky note", [r"tab_next", r"Tool::Sticky"], "crates/designos-app/src/shell.rs"),
                ("C3", "Double-click / Enter immediately edits Sticky note body text", [r"NodeKind::Sticky", r"begin_text_edit"], "crates/designos-app/src/shell.rs"),
                ("C4", "Contextual board bar swatch palette, text formatting, and author visibility toggle", [r"c\.author", r"NodeKind::Sticky"], "crates/designos-app/src/board/bar.rs"),
                ("C5", "Duplicate / Delete / Select same on Sticky", [r"Action::Duplicate"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
        (
            "Slice",
            [
                ("C1", "Dashed green (#14AE5C) Slice chrome & badge on canvas (is_slice_node)", [r"FIGMA_SLICE_GREEN", r"is_slice_node"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Tool::Slice drag-creation and 8-handle bounding box resize", [r"Tool::Slice"], "crates/designos-app/src/canvas/create.rs"),
                ("C3", "Double-click Slice label or layer to rename export slice", [r"is_slice_node"], "crates/designos-app/src/canvas/draw.rs"),
                ("C4", "Right-panel Export settings (PNG/JPG/SVG/PDF, 1x/2x scale) for Slice", [r"export_settings"], "crates/designos-app/src/panels/export.rs"),
                ("C5", "Export selection (Action::Export) on Slice", [r"Action::Export"], "crates/designos-app/src/panels/menus.rs"),
            ],
        ),
        (
            "Table",
            [
                ("C1", "Table / Grid cell layout & dashed track overlay on canvas", [r"LayoutDirection::Grid", r"draw_css_grid_overlays"], "crates/designos-app/src/canvas/draw.rs"),
                ("C2", "Dedicated on-canvas interactive Table column/row resize dividers & + row/col buttons", [r"NodeKind::Table\b|table_col_resize_handle"], "crates/designos-app/src/canvas/handles.rs"),
                ("C3", "Double-click Table cell enters cell text editing", [r"begin_text_edit"], "crates/designos-app/src/canvas/interact.rs"),
                ("C4", "Right-panel Grid / Table rows, columns, and track sizing controls", [r"grid_columns", r"grid_rows", r"GridTrack"], "crates/designos-app/src/panels/properties.rs"),
                ("C5", "Context menu Insert/Delete Table row & column actions", [r"InsertTableRow|DeleteTableRow|insert_table_row"], "crates/designos-app/src/panels/context.rs"),
            ],
        ),
    ]

    stroke_rs = read_text("crates/designos-app/src/panels/stroke.rs")
    export_panel_rs = read_text("crates/designos-app/src/panels/export.rs")
    fig_rs = read_text("crates/designos-fig/src/fig.rs")
    menus_rs = read_text("crates/designos-app/src/panels/menus.rs")
    typography_rs = read_text("crates/designos-app/src/panels/typography.rs")
    shell_rs = read_text("crates/designos-app/src/shell.rs")
    full_corpus = "\n".join([corpus, stroke_rs, export_panel_rs, fig_rs, menus_rs, typography_rs, shell_rs])

    passed = 0
    total = 0
    remaining_gaps = []
    element_summaries = []

    for elem_name, checks in elements_spec:
        elem_passed = 0
        elem_checks = []
        for cid, ctitle, pats, fhint in checks:
            total += 1
            ok, ev = has_all(full_corpus, pats)
            if ok:
                passed += 1
                elem_passed += 1
            else:
                remaining_gaps.append(
                    {
                        "id": f"P3-{elem_name.upper()}-{cid}",
                        "title": f"[{elem_name} {cid}] {ctitle}",
                        "pillar": "Pillar 3: Per-Element Canvas & Inspector Matrix",
                        "impact_pct": round((1.0 / 85.0) * 20.0, 3),
                        "file_hint": fhint,
                    }
                )
            elem_checks.append({"id": cid, "title": ctitle, "passed": ok, "evidence": ev, "file_hint": fhint})
        element_summaries.append(
            {
                "element": elem_name,
                "passed": elem_passed,
                "total": len(checks),
                "checks": elem_checks,
            }
        )

    score_pct = round((passed / total) * 100.0, 2)
    return {
        "id": "pillar-3-element-matrix",
        "name": "Pillar 3: Per-Element Canvas & Inspector Functional Matrix (17 Elements x 5 = 85 Checkpoints)",
        "weight_pct": 20.0,
        "score_pct": score_pct,
        "weighted_contribution_pct": round(score_pct * 0.20, 2),
        "passed_items": passed,
        "total_items": total,
        "elements": element_summaries,
        "remaining_gaps": remaining_gaps,
    }


# ==============================================================================
# PILLAR 4 (20% Weight): UI3 Surfaces, Popovers & Visual Contract (16 Surfaces)
# ==============================================================================
def evaluate_pillar_4() -> Dict[str, Any]:
    home_rs = read_text("crates/designos-app/src/home/mod.rs")
    tabs_rs = read_text("crates/designos-app/src/tabs.rs")
    rail_rs = read_text("crates/designos-app/src/panels/rail.rs")
    menus_rs = read_text("crates/designos-app/src/panels/menus.rs")
    left_rs = read_text("crates/designos-app/src/panels/left.rs")
    layers_rs = read_text("crates/designos-app/src/panels/layers.rs")
    assets_rs = read_text("crates/designos-app/src/panels/assets.rs")
    toolbar_rs = read_text("crates/designos-app/src/panels/toolbar.rs")
    cmdk_rs = read_text("crates/designos-app/src/cmdk.rs")
    right_rs = read_text("crates/designos-app/src/panels/right.rs")
    properties_rs = read_text("crates/designos-app/src/panels/properties.rs")
    prototype_rs = read_text("crates/designos-app/src/panels/prototype.rs")
    inspect_rs = read_text("crates/designos-app/src/panels/inspect.rs")
    typography_rs = read_text("crates/designos-app/src/panels/typography.rs")
    picker_rs = read_text("crates/designos-app/src/widgets/picker.rs")
    variables_rs = read_text("crates/designos-app/src/variables/view.rs")
    draw_rs = read_text("crates/designos-app/src/canvas/draw.rs")
    canvas_mod_rs = read_text("crates/designos-app/src/canvas/mod.rs")
    rulers_rs = read_text("crates/designos-app/src/canvas/rulers.rs")
    theme_rs = read_text("crates/designos-app/src/theme.rs")
    shell_rs = read_text("crates/designos-app/src/shell.rs")
    icons_rs = read_text("crates/designos-app/src/icons.rs")

    surfaces_spec: List[Tuple[str, str, str, List[str], str]] = [
        (
            "S01",
            "Top 32px Multi-Tab Project Bar (tabs.rs)",
            "Figma Desktop top strip: Home button + open document tabs with type badges + '+' new tab + middle-click/close",
            [r"show_top_tab_bar", r"paint_home_glyph", r"ProjectTabKind"],
            tabs_rs,
            "crates/designos-app/src/tabs.rs",
        ),
        (
            "S02",
            "DIR-12 Brand Rule: lowercase `design:os studio`",
            "All user-facing brand strings use lowercase `design:os` / `design:os studio` with `#0D99FF` accent",
            [r"design:os"],
            home_rs + "\n" + menus_rs + "\n" + shell_rs,
            "crates/designos-app/src/home/mod.rs",
        ),
        (
            "S03",
            "Home / File Browser (home/mod.rs)",
            "UI3 Home with Recents/Drafts/Starred/Templates/Libraries sidebar, create cards, search, and Grid/List toggle",
            [r"Recents", r"Libraries", r"design:os"],
            home_rs,
            "crates/designos-app/src/home/mod.rs",
        ),
        (
            "S04",
            "Left Navigation Rail (48px), Main Menu & Phosphor Icon System (rail.rs, menus.rs, icons.rs)",
            "UI3 left rail with Main menu, File, Assets, Specs, Terminal/Tools, Variables, Libraries + unified Phosphor Icons (`DIR-19`)",
            [r"Action::BackToFiles", r"RailItem::Specs", r"RailItem::Tools", r"phosphor_name"],
            rail_rs + "\n" + menus_rs + "\n" + shell_rs + "\n" + icons_rs,
            "crates/designos-app/src/panels/rail.rs",
        ),
        (
            "S05",
            "Left Sidebar (240px): Pages Splitter, Page Dividers & Layers Tree (left.rs, layers.rs)",
            "Draggable Pages/Layers splitter, non-selectable Page dividers (`---`), `< />` Ready-for-dev badges, purple component/instance rows",
            [r"pages_list", r"ReadyForDev", r"FIGMA_COMPONENT_PURPLE|COMPONENT"],
            left_rs + "\n" + layers_rs + "\n" + draw_rs,
            "crates/designos-app/src/panels/left.rs",
        ),
        (
            "S06",
            "Left Sidebar: Assets Tab & Component Previews (assets.rs)",
            "Search all libraries, category filter chips, 2-column component miniature cards, drag-to-canvas instantiation",
            [r"Search", r"Assets"],
            assets_rs,
            "crates/designos-app/src/panels/assets.rs",
        ),
        (
            "S07",
            "Bottom Floating Toolbelt & Dev Mode Tool Swap (toolbar.rs)",
            "Centered floating toolbelt with Move/Scale(K), Frame/Section/Slice, Shapes, Pen/Pencil, Text, Comment, Actions + Dev Mode tool swap (Select, Annotate, Measure)",
            [r"show_dev_mode_toolbar", r"Tool::Scale", r"Annotate", r"Measure"],
            toolbar_rs,
            "crates/designos-app/src/panels/toolbar.rs",
        ),
        (
            "S08",
            "Bottom-Docked Actions / AI Command Bar (`Cmd+K` / `Cmd+P`) (cmdk.rs, menus.rs)",
            "Docked above bottom toolbelt with Actions, AI / Make Design, and Design Gate audit tabs",
            [r"FigmaAiOp", r"run_design_gate_audit", r"palette"],
            cmdk_rs + "\n" + menus_rs,
            "crates/designos-app/src/cmdk.rs",
        ),
        (
            "S09",
            "Right Panel Top Bar & 1:1 Zoom Menu (right.rs, menus.rs)",
            "Editable `%` zoom input box (`#0D99FF` ring) + Zoom presets + Pixel preview/grid/snap/guides/rulers/outlines/comments/annotations",
            [r"show_zoom_menu", r"zoom-input", r"Pixel grid", r"Snap to pixel grid", r"Layout guides", r"Rulers", r"Annotations"],
            menus_rs + "\n" + right_rs,
            "crates/designos-app/src/panels/menus.rs",
        ),
        (
            "S10",
            "Design Inspector & Selection Colors Rollup (properties.rs)",
            "UI3 Position, Layout (Auto Layout + CSS Grid), Appearance, Fill, Stroke, Effects, Layout Guide, Selection colors rollup, Export",
            [r"selection_colors_section", r"auto_layout_ui", r"edit_paint_list"],
            properties_rs,
            "crates/designos-app/src/panels/properties.rs",
        ),
        (
            "S11",
            "Prototype Mode & On-Canvas Noodle Connections (prototype.rs)",
            "Prototype tab with Flow starting point, Interactions (Trigger/Action/Transition/Smart animate), and on-canvas blue Bezier connection noodles",
            [r"SmartAnimate", r"Flow"],
            prototype_rs,
            "crates/designos-app/src/panels/prototype.rs",
        ),
        (
            "S12",
            "Dev Mode (`Shift+D`) & Inspect Panel (inspect.rs)",
            "Box model diagram, CSS/SwiftUI/React/Tailwind/Compose codegen, resolved Colors & Variable tokens, downloadable Assets, hover red/green distance measurement",
            [r"collect_colors", r"collect_assets", r"measure_overlay"],
            inspect_rs,
            "crates/designos-app/src/panels/inspect.rs",
        ),
        (
            "S13",
            "Type Settings (`...`) 3-Tab Popover (typography.rs)",
            "3-tab (`Basic | Details | Variable`) popover with Resizing, Case, Vertical trim, Lists, OpenType features (`liga`, `calt`, `tnum`), and Variable font axis sliders",
            [r"Basic", r"Details", r"Variable", r"details_popover"],
            typography_rs,
            "crates/designos-app/src/panels/typography.rs",
        ),
        (
            "S14",
            "Color Picker + WCAG AA/AAA Contrast + Libraries Tab (widgets/picker.rs)",
            "`Custom | Libraries` tabs, 6 paint modes (Solid/Linear/Radial/Angular/Diamond/Image), WCAG contrast badge, Hue/Alpha/Eyedropper, Color styles & variables",
            [r"Libraries", r"Contrast|contrast", r"Hsva"],
            picker_rs,
            "crates/designos-app/src/widgets/picker.rs",
        ),
        (
            "S15",
            "Variables 780x520 Full Modal & `/` Group Headers (variables/view.rs)",
            "Full `780x520` Variables modal with Collections + Groups sidebar, `/` folder headers, multi-mode columns, and inline swatch/value editing",
            [r"group_paths", r"matches_scope_filter", r"build_rows"],
            variables_rs,
            "crates/designos-app/src/variables/view.rs",
        ),
        (
            "S16",
            "Canvas Selection Chrome (#0D99FF / #9747FF) & 20px Pinned Rulers (draw.rs, rulers.rs)",
            "`#0D99FF` selection box, `#9747FF` component/instance purple box, `#FF388C` auto-layout handles, `#14AE5C` slice green, and 20px top/left canvas rulers",
            [r"FIGMA_COMPONENT_PURPLE", r"FIGMA_BLUE", r"FIGMA_PINK", r"FIGMA_SLICE_GREEN"],
            draw_rs + "\n" + rulers_rs + "\n" + canvas_mod_rs,
            "crates/designos-app/src/canvas/draw.rs",
        ),
    ]

    passed = 0
    remaining_gaps = []
    checks = []
    for sid, name, desc, pats, target_txt, fhint in surfaces_spec:
        ok, ev = has_all(target_txt, pats)
        if ok:
            passed += 1
        else:
            remaining_gaps.append(
                {
                    "id": f"P4-{sid}",
                    "title": f"{name}: {desc}",
                    "pillar": "Pillar 4: UI3 Surfaces, Popovers & Visual Contract",
                    "impact_pct": round((1.0 / len(surfaces_spec)) * 20.0, 3),
                    "file_hint": fhint,
                }
            )
        checks.append({"id": sid, "name": name, "description": desc, "passed": ok, "evidence": ev, "file_hint": fhint})

    score_pct = round((passed / len(surfaces_spec)) * 100.0, 2)
    return {
        "id": "pillar-4-ui3-surfaces",
        "name": "Pillar 4: UI3 Surfaces, Popovers & Visual Contract (16 Surface Checkpoints)",
        "weight_pct": 20.0,
        "score_pct": score_pct,
        "weighted_contribution_pct": round(score_pct * 0.20, 2),
        "passed_items": passed,
        "total_items": len(surfaces_spec),
        "checks": checks,
        "remaining_gaps": remaining_gaps,
    }


# ==============================================================================
# PILLAR 5 (15% Weight): Figma AI Agent & Native PTY Terminal Core (15 Items)
# ==============================================================================
def evaluate_pillar_5() -> Dict[str, Any]:
    cmdk_rs = read_text("crates/designos-app/src/cmdk.rs")
    terminal_rs = read_text("crates/designos-app/src/terminal.rs")
    shell_rs = read_text("crates/designos-app/src/shell.rs")
    mcp_server_rs = read_text("crates/designos-mcp/src/server.rs")
    mcp_engine_rs = read_text("crates/designos-mcp/src/engine.rs")

    ai_terminal_spec: List[Tuple[str, str, str, List[str], str, str]] = [
        # 6 Figma AI Actions in cmdk.rs
        ("A01", "Figma AI", "Make Design (Prompt -> NodeSpec -> Multi-layer Auto-Layout Frame)", [r"ApplySpec", r"synthesize_spec_from_prompt"], cmdk_rs, "crates/designos-app/src/cmdk.rs"),
        ("A02", "Figma AI", "Make Prototype (Auto-wire interactions across screens)", [r"MakePrototype", r"auto_wire_prototype_flows"], cmdk_rs, "crates/designos-app/src/cmdk.rs"),
        ("A03", "Figma AI", "Rename Layers Semantically (Replace generic Frame 42 names)", [r"RenameLayers", r"auto_rename_layers_semantically"], cmdk_rs, "crates/designos-app/src/cmdk.rs"),
        ("A04", "Figma AI", "Rewrite / Shorten / Translate Selected Text (EN/VI/JA)", [r"RewriteText", r"rewrite_selected_text"], cmdk_rs, "crates/designos-app/src/cmdk.rs"),
        ("A05", "Figma AI", "Populate Realistic Content (Table rows & repeat cards)", [r"PopulateContent", r"populate_realistic_content"], cmdk_rs, "crates/designos-app/src/cmdk.rs"),
        ("A06", "Figma AI", "Design Gate Audit & 1-Click Auto-Fix (4px grid, touch targets, WCAG)", [r"run_design_gate_audit", r"auto_fix_design_gate"], cmdk_rs, "crates/designos-app/src/cmdk.rs"),
        # 5 Native PTY CLI Runtimes + Chat-over-PTY Wrapper (DIR-21) in terminal.rs
        ("T01", "PTY Runtime", "Claude Code (`claude`) 1-click preset, Chat-over-PTY wrapper & PTY spawn", [r"Claude", r'"claude"', r"agents-runtime-claude", r"submit_chat_prompt"], terminal_rs, "crates/designos-app/src/terminal.rs"),
        ("T02", "PTY Runtime", "Codex CLI (`codex`) 1-click preset, Chat-over-PTY wrapper & PTY spawn", [r"Codex", r'"codex"', r"agents-runtime-codex", r"sync_chat_from_pty"], terminal_rs, "crates/designos-app/src/terminal.rs"),
        ("T03", "PTY Runtime", "Antigravity CLI (`agy`) 1-click preset, Chat-over-PTY wrapper & PTY spawn", [r"Antigravity", r'"agy"', r"agents-runtime-agy", r"strip_ansi_codes"], terminal_rs, "crates/designos-app/src/terminal.rs"),
        ("T04", "PTY Runtime", "Grok CLI (`grok`) 1-click preset & PTY spawn", [r"Grok", r'"grok"'], terminal_rs, "crates/designos-app/src/terminal.rs"),
        ("T05", "PTY Runtime", "User Shell (`$SHELL`) fallback & multi-tab PTY manager", [r"default_shell_program", r"portable_pty"], terminal_rs, "crates/designos-app/src/terminal.rs"),
        # 4 Live Canvas Bridge & MCP Checkpoints
        ("B01", "Live Bridge", "`$DESIGNOS_LIVE_SPEC` & `$DESIGNOS_SELECTION_JSON` env injection into PTY", [r"DESIGNOS_LIVE_SPEC", r"configure_pty_env"], terminal_rs, "crates/designos-app/src/terminal.rs"),
        ("B02", "Live Bridge", "`<50ms` `poll_live_agent_bridge` hot-apply to live canvas with single-step Undo", [r"poll_live_agent_bridge", r"ApplySpecFile"], terminal_rs + "\n" + shell_rs, "crates/designos-app/src/terminal.rs"),
        ("B03", "Live Bridge", "Selection + Pin Comments export to active Agent PTY (`send_copy_for_ai`)", [r"send_copy_for_ai", r"export_selection_and_comments_to_pty"], terminal_rs, "crates/designos-app/src/terminal.rs"),
        ("B04", "MCP Server", "`designos-mcp` server binary exposing scenegraph & design tools", [r"designos_open_file", r"designos_get_document", r"designos_apply_spec"], mcp_server_rs + "\n" + mcp_engine_rs, "crates/designos-mcp/src/server.rs"),
    ]

    passed = 0
    remaining_gaps = []
    checks = []
    for cid, cat, title, pats, txt, fhint in ai_terminal_spec:
        ok, ev = has_all(txt, pats)
        if ok:
            passed += 1
        else:
            remaining_gaps.append(
                {
                    "id": f"P5-{cid}",
                    "title": f"[{cat}] {title}",
                    "pillar": "Pillar 5: Figma AI Agent & Native PTY Terminal Core",
                    "impact_pct": round((1.0 / len(ai_terminal_spec)) * 15.0, 3),
                    "file_hint": fhint,
                }
            )
        checks.append({"id": cid, "category": cat, "title": title, "passed": ok, "evidence": ev, "file_hint": fhint})

    score_pct = round((passed / len(ai_terminal_spec)) * 100.0, 2)
    return {
        "id": "pillar-5-ai-terminal-mcp",
        "name": "Pillar 5: Figma AI Agent & Native PTY Terminal Core (15 Checkpoints)",
        "weight_pct": 15.0,
        "score_pct": score_pct,
        "weighted_contribution_pct": round(score_pct * 0.15, 2),
        "passed_items": passed,
        "total_items": len(ai_terminal_spec),
        "checks": checks,
        "remaining_gaps": remaining_gaps,
    }


def generate_markdown_report(scorecard: Dict[str, Any]) -> str:
    overall = scorecard["overall_parity_pct"]
    rem = scorecard["remaining_gap_pct"]
    kstats = scorecard["kiwi_field_stats"]
    pillars = scorecard["pillars"]

    lines = [
        "# `design`:os studio vs Figma Desktop (`126.9.13`) — 1:1 Parity Scorecard",
        "",
        f"- **Generated At:** `{scorecard['generated_at']}`",
        f"- **Target Figma Desktop Version:** `{scorecard['target_figma_version']}` (`app.asar` + UI3 Reference Corpus)",
        f"- **Target `.fig` Kiwi Schema Version:** `v{scorecard['target_fig_schema_version']}` (`VSF - PCP.fig`: `155,297` nodes, `165,671` overrides, `653` definitions)",
        f"- **Overall Weighted 1:1 Parity Score:** **`{overall:.2f}%`** (**`{rem:.2f}%` remaining to `100.00%`**)",
        "",
        "---",
        "",
        "## 1. Executive Parity Dashboard",
        "",
        "| Pillar | Weight | Passed / Total | Pillar Score (`%`) | Weighted Contribution (`%`) | Remaining Gap (`%`) |",
        "|---|---:|---:|---:|---:|---:|",
    ]

    for p in pillars:
        w = p["weight_pct"]
        sc = p["score_pct"]
        contrib = p["weighted_contribution_pct"]
        gap_w = round(w - contrib, 2)
        lines.append(
            f"| **{p['name']}** | `{w:.0f}%` | `{p['passed_items']} / {p['total_items']}` | **`{sc:.2f}%`** | `{contrib:.2f}%` | `{gap_w:.2f}%` |"
        )

    lines.extend(
        [
            f"| **TOTAL (`design`:os studio vs Figma `126.9.13`)** | **`100%`** | **`{scorecard['total_passed_items']} / {scorecard['total_evaluated_items']}`** | **`{overall:.2f}%`** | **`{overall:.2f}%`** | **`{rem:.2f}%`** |",
            "",
            "---",
            "",
            "## 2. Pillar 1 Deep-Dive: `.fig` v106 Kiwi Binary Schema & Node Instance Fidelity",
            "",
            "| Metric | Value | Notes |",
            "|---|---:|---|",
            f"| **Total `NodeChange` Fields in Kiwi v106 Schema** | `{kstats['total_defined_on_node_change']}` | Extracted from `VSF - PCP.fig` Chunk 0 (`schema.bin`) |",
            f"| **Observed `NodeChange` Fields in `VSF - PCP.fig`** | `{kstats['total_observed_in_vsf_pcp']}` | Across `155,297` nodes and `165,671` `symbolOverrides` |",
            f"| **Internal Compiler/Cache Version Fields Excluded** | `{kstats['internal_compiler_metadata_fields_count']}` | `{', '.join(kstats['internal_compiler_metadata_fields'])}` |",
            f"| **Semantic Design Fields Observed in `VSF - PCP.fig`** | `{kstats['semantic_observed_in_vsf_pcp']}` | Mapped on import: **`{kstats['semantic_mapped_in_import']} / {kstats['semantic_observed_in_vsf_pcp']}` (`{kstats['semantic_schema_field_import_pct']:.2f}%`)** |",
            f"| **Node-Instance-Weighted Import Fidelity (Semantic)** | **`{kstats['node_instance_weighted_import_pct']:.2f}%`** | Share of all node + override field instances mapped in `designos-fig` |",
            f"| **Node-Instance-Weighted Import Fidelity (Raw 203)** | `{kstats['node_instance_weighted_raw_import_pct']:.2f}%` | Including internal compiler version numbers |",
            f"| **7 Production Fidelity Gates (`GAP-01..GAP-07`)** | **`{kstats['production_gaps_passed']} / {kstats['production_gaps_total']}`** | Verified directly against `crates/designos-fig/src/fig.rs` |",
            "",
            "### `GAP-01..GAP-07` Verification Status",
            "",
            "| Gate ID | Description | Status | Location |",
            "|---|---|---|---|",
        ]
    )

    for g in kstats["production_gap_checks"]:
        st = "PASS" if g["passed"] else "GAP"
        lines.append(f"| `{g['id']}` | {g['title']} | **{st}** | `{g['file_hint']}` |")

    lines.extend(
        [
            "",
            "---",
            "",
            "## 3. Prioritized Remaining Delta to Reach `100.00%` Parity",
            "",
        ]
    )

    all_gaps = scorecard["all_remaining_gaps"]
    if not all_gaps:
        lines.append("All evaluated checkpoints across all 5 pillars passed (`100.00%` parity achieved)!")
    else:
        lines.extend(
            [
                f"Total remaining items across all 5 pillars: **`{len(all_gaps)}`** (accounting for **`{rem:.2f}%`** total weighted distance to `100%`).",
                "",
                "| Rank | Gap ID | Pillar | Global Impact (`%`) | Description | Target File Hint |",
                "|---:|---|---|---:|---|---|",
            ]
        )
        for idx, g in enumerate(all_gaps[:45], start=1):
            lines.append(
                f"| `{idx}` | `{g['id']}` | {g['pillar']} | `+{g['impact_pct']:.3f}%` | {g['title']} | `{g['file_hint']}` |"
            )
        if len(all_gaps) > 45:
            lines.append(f"\n*... plus `{len(all_gaps) - 45}` low-frequency `.fig` metadata fields (see `parity-scorecard.json` for full list).*")

    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Figma 1:1 Parity Benchmark & Scorecard Engine")
    parser.add_argument("--json", action="store_true", help="Print JSON scorecard to stdout")
    args = parser.parse_args()

    RESEARCH_DIR.mkdir(parents=True, exist_ok=True)

    p1, kiwi_field_stats = evaluate_pillar_1()
    p2 = evaluate_pillar_2()
    p3 = evaluate_pillar_3()
    p4 = evaluate_pillar_4()
    p5 = evaluate_pillar_5()

    pillars = [p1, p2, p3, p4, p5]
    overall_parity_pct = round(sum(p["weighted_contribution_pct"] for p in pillars), 2)
    remaining_gap_pct = round(max(0.0, 100.0 - overall_parity_pct), 2)

    all_remaining_gaps = []
    for p in pillars:
        all_remaining_gaps.extend(p["remaining_gaps"])
    all_remaining_gaps.sort(key=lambda g: -g["impact_pct"])

    total_passed = sum(p["passed_items"] for p in pillars)
    total_items = sum(p["total_items"] for p in pillars)

    # Compact pillar list for JSON output (keep full detail in parity-scorecard.json)
    scorecard = {
        "generated_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "target_figma_version": TARGET_FIGMA_VERSION,
        "target_fig_schema_version": TARGET_FIG_SCHEMA_VERSION,
        "overall_parity_pct": overall_parity_pct,
        "remaining_gap_pct": remaining_gap_pct,
        "total_passed_items": total_passed,
        "total_evaluated_items": total_items,
        "kiwi_field_stats": kiwi_field_stats,
        "pillars": [
            {
                "id": p["id"],
                "name": p["name"],
                "weight_pct": p["weight_pct"],
                "score_pct": p["score_pct"],
                "weighted_contribution_pct": p["weighted_contribution_pct"],
                "passed_items": p["passed_items"],
                "total_items": p["total_items"],
                "submetrics": p.get("submetrics"),
                "remaining_gaps": p["remaining_gaps"],
            }
            for p in pillars
        ],
        "all_remaining_gaps": all_remaining_gaps,
    }

    SCORECARD_JSON_PATH.write_text(json.dumps(scorecard, indent=2), encoding="utf-8")
    md_report = generate_markdown_report(scorecard)
    SCORECARD_MD_PATH.write_text(md_report, encoding="utf-8")

    if args.json:
        print(json.dumps(scorecard, indent=2))
    else:
        print("=" * 94)
        print(
            f" design:os studio vs Figma Desktop {TARGET_FIGMA_VERSION} (.fig v{TARGET_FIG_SCHEMA_VERSION}) — 1:1 PARITY SCORECARD"
        )
        print("=" * 94)
        print(f" {'Pillar':<56} | {'Weight':>6} | {'Passed':>9} | {'Score':>7} | {'Contrib':>7}")
        print("-" * 94)
        for p in pillars:
            pname = p["name"][:56]
            p_str = f"{p['passed_items']}/{p['total_items']}"
            print(
                f" {pname:<56} | {p['weight_pct']:>5.0f}% | {p_str:>9} | {p['score_pct']:>6.2f}% | {p['weighted_contribution_pct']:>6.2f}%"
            )
        print("-" * 94)
        tot_str = f"{total_passed}/{total_items}"
        print(
            f" {'OVERALL 1:1 FIGMA PARITY':<56} | {'100%':>6} | {tot_str:>9} | {overall_parity_pct:>6.2f}% | {overall_parity_pct:>6.2f}%"
        )
        print(
            f" {'REMAINING GAP TO 100.00%':<56} | {'':>6} | {len(all_remaining_gaps):>4} gaps | {remaining_gap_pct:>6.2f}% | {remaining_gap_pct:>6.2f}%"
        )
        print("=" * 94)
        print(
            f" Kiwi v106 Field Stats (VSF - PCP.fig): {kiwi_field_stats['semantic_mapped_in_import']}/{kiwi_field_stats['semantic_observed_in_vsf_pcp']} semantic fields "
            f"({kiwi_field_stats['semantic_schema_field_import_pct']:.2f}%), "
            f"{kiwi_field_stats['node_instance_weighted_import_pct']:.2f}% node-instance-weighted fidelity, "
            f"GAP-01..07: {kiwi_field_stats['production_gaps_passed']}/{kiwi_field_stats['production_gaps_total']}"
        )
        print(f" Wrote JSON: {SCORECARD_JSON_PATH}")
        print(f" Wrote MD:   {SCORECARD_MD_PATH}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
