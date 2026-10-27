"""Regression checks for the homepage's JavaScript-owned entrance animation.

The design gates run axe on docs/index.html while the entrance may still be playing.
A header control caught mid-fade fails colour contrast, so the header must never be
hidden or faded: it is readable and clickable from the first frame.
"""
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
STORY_CSS = ROOT / "docs" / "assets" / "v2-story.css"
CORE_JS = ROOT / "docs" / "assets" / "v2-core.js"


def preflight_body():
    css = STORY_CSS.read_text()
    match = re.search(
        r"@media \(prefers-reduced-motion: no-preference\) \{(?P<body>.*?)\n\}",
        css,
        re.DOTALL,
    )
    return match.group("body") if match else None


class HomepageMotionTests(unittest.TestCase):
    def test_header_is_not_hidden_by_the_static_preflight(self):
        body = preflight_body()
        self.assertIsNotNone(body)
        for selector in (".topbar .brand", ".topbar .navlinks li", ".topbar .btn"):
            self.assertNotIn("html.v2-pre " + selector, body)

    def test_hero_blocks_are_still_hidden_until_the_cascade(self):
        body = preflight_body()
        self.assertIn("html.v2-pre .hero h1", body)

    def test_gsap_entrance_still_moves_the_header(self):
        js = CORE_JS.read_text()
        self.assertIn(
            "pick('.topbar .brand, .topbar .navlinks li, .topbar .btn')",
            js,
        )

    def test_gsap_entrance_never_fades_the_header(self):
        js = CORE_JS.read_text()
        self.assertNotIn("gsap.set(all, { opacity: 0", js)
        header_set = re.search(r"gsap\.set\(headerEls, \{(?P<props>[^}]*)\}\)", js)
        self.assertIsNotNone(header_set)
        self.assertNotIn("opacity", header_set.group("props"))


if __name__ == "__main__":
    unittest.main()
