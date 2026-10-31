"""Headless Chrome vs layout_solver grid geometry harness package (Issue #484)."""

from .fixtures import find_chrome, inline_css, load_fixtures_from_rust
from .probe_page import MAX_DRIFT_PX, build_batch_probe_html, compare_boxes, measure_batch_in_chrome
from .screenshot import build_screenshot_html, render_comparison_png, run_comparison
from .tailwind_css_map import (
    TailwindChildExtractor,
    parse_tailwind_html,
    tailwind_classes_to_css,
    tailwind_token_to_decls,
)

__all__ = [
    "MAX_DRIFT_PX",
    "TailwindChildExtractor",
    "build_batch_probe_html",
    "build_screenshot_html",
    "compare_boxes",
    "find_chrome",
    "inline_css",
    "load_fixtures_from_rust",
    "measure_batch_in_chrome",
    "parse_tailwind_html",
    "render_comparison_png",
    "run_comparison",
    "tailwind_classes_to_css",
    "tailwind_token_to_decls",
]
