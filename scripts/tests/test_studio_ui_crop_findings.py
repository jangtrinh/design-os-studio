"""A compare row that fails on a known difference carries a `finding` (the why) and a `finding_pin` (the measured score fields). The run accepts it while the pin still equals the measurement
and covers every check the row fails, and exits 1 otherwise: a bad row with no finding, a finding that pins nothing, a pin whose numbers moved (even on a passing row), a pin that
leaves a failing check out, an ambiguous background. Rows are plain dicts (no Pillow, no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_findings
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402

CROPS = HERE.parent / "studio_ui_file" / "evidence" / "crops.json"
BOX_PIN = {"fg_render_px": [230, 16], "fg_production_px": [234, 16]}
PEAK_PIN = {"peak_colour_render": [255, 255, 255], "peak_colour_production": [198, 198, 198]}


def row(**kw):
    base = {"set": "Table/Data", "variant": "", "capture": "k-variables/x", "bad": False, "pending": None, "fg_ok": True, "surface_ok": True, "peak_ok": True, "background_ok": True,
            "fg_render_px": (230, 16), "fg_production_px": (234, 16), "surface_render": (10, 10, 10), "surface_production": (10, 10, 10),
            "peak_colour_render": (255, 255, 255), "peak_colour_production": (198, 198, 198), "finding": None, "finding_pin": None}
    return {**base, **kw}


def box_row(**kw):
    return row(bad=True, fg_ok=False, finding="the last F is under the half-peak threshold", finding_pin=BOX_PIN, **kw)


class AcceptedFindingTest(unittest.TestCase):
    def test_a_bad_row_whose_pin_still_matches_does_not_fail_the_run(self):
        self.assertEqual(cm.exit_code([row(), box_row()]), 0)

    def test_the_accepted_row_is_printed_with_its_pinned_numbers(self):
        lines = cm.report_lines([row(), box_row()])
        self.assertEqual(len(lines), 1)
        self.assertTrue(lines[0].startswith("ACCEPTED Table/Data  k-variables/x:"), lines[0])
        self.assertIn("fg_render_px [230, 16], fg_production_px [234, 16];", lines[0])
        self.assertIn("fails box", lines[0])
        self.assertIn("the last F is under", lines[0])

    def test_a_pin_from_json_lists_equals_the_measured_tuples(self):
        pin = json.loads(json.dumps(PEAK_PIN))
        self.assertEqual(cm.exit_code([row(bad=True, peak_ok=False, finding="aa", finding_pin=pin)]), 0)

    def test_a_passing_row_with_a_matching_finding_is_accepted_and_listed(self):
        r = row(finding="the glyph is thin", finding_pin={"fg_render_px": [230, 16]})
        self.assertEqual(cm.exit_code([r]), 0)
        self.assertIn("the row passes", cm.report_lines([r])[0])

    def test_a_row_with_no_finding_that_passes_prints_nothing(self):
        self.assertEqual(cm.report_lines([row()]), [])


class FindingThatFailsTheRunTest(unittest.TestCase):
    def assertFails(self, rows, text):
        self.assertEqual(cm.exit_code(rows), 1)
        self.assertIn(text, "\n".join(cm.report_lines(rows)))

    def test_a_bad_row_with_no_finding_exits_1(self):
        self.assertFails([row(bad=True, fg_ok=False)], "no recorded finding")

    def test_a_finding_that_pins_nothing_exits_1_bad_row_or_not(self):
        self.assertFails([row(bad=True, fg_ok=False, finding="why")], "pins no numbers")
        self.assertFails([row(finding="why")], "pins no numbers")
        self.assertFails([row(finding="why", finding_pin={})], "pins no numbers")

    def test_a_pin_without_a_finding_exits_1(self):
        self.assertFails([row(finding_pin=BOX_PIN)], "without a finding")

    def test_a_moved_number_exits_1_and_names_pinned_and_measured(self):
        self.assertFails([box_row(fg_render_px=(229, 16))], "pinned fg_render_px [230, 16], measured [229, 16]")

    def test_the_production_number_moving_alone_exits_1(self):
        self.assertFails([box_row(fg_production_px=(235, 16))], "pinned fg_production_px [234, 16], measured [235, 16]")

    def test_the_render_number_moving_alone_exits_1_even_when_the_row_passes(self):
        self.assertFails([row(finding="thin", finding_pin={"fg_render_px": [230, 16]}, fg_render_px=(231, 16))], "pinned fg_render_px")

    def test_a_missing_measurement_is_a_moved_number(self):
        self.assertFails([box_row(fg_render_px=None)], "measured None")

    def test_a_pin_that_leaves_a_failing_check_out_exits_1(self):
        r = row(bad=True, fg_ok=False, peak_ok=False, finding="two things", finding_pin=BOX_PIN)
        self.assertFails([r], "fails the strongest pixel check but the pin does not hold peak_colour_render, peak_colour_production")

    def test_a_pin_that_holds_one_of_two_fields_of_a_failing_check_exits_1(self):
        r = row(bad=True, fg_ok=False, finding="why", finding_pin={"fg_render_px": [230, 16]})
        self.assertFails([r], "does not hold fg_production_px")

    def test_both_failing_checks_pinned_is_accepted(self):
        r = row(bad=True, fg_ok=False, peak_ok=False, finding="two things", finding_pin={**BOX_PIN, **PEAK_PIN})
        self.assertEqual(cm.exit_code([r]), 0)

    def test_an_ambiguous_background_is_never_accepted(self):
        self.assertFails([box_row(background_ok=False)], "ambiguous background is never accepted")

    def test_one_bad_row_among_accepted_ones_exits_1(self):
        self.assertEqual(cm.exit_code([box_row(), row(bad=True, surface_ok=False)]), 1)

    def test_nothing_compared_is_still_2(self):
        self.assertEqual(cm.exit_code([]), 2)


class RecordedFindingsTest(unittest.TestCase):
    def test_every_finding_in_crops_json_pins_known_score_fields(self):
        known = {f for _, _, fields in cm.CHECKS for f in fields} | {"mean_abs_rgb_diff", "max_abs_rgb_diff"}  # the figures of a check, and the whole-crop differences (which see where a drawing sits)
        rows = [e for e in json.loads(CROPS.read_text())["entries"] if e.get("finding")]
        self.assertGreaterEqual(len(rows), 4)
        for e in rows:
            with self.subTest(e["case"] + str(e.get("named") or e.get("rect"))):
                self.assertTrue(e.get("finding_pin"), "a finding pins its measured numbers")
                self.assertTrue(set(e["finding_pin"]) <= known, e["finding_pin"])

    def test_the_two_failing_table_rows_pin_the_numbers_the_brief_names(self):
        by = {tuple(e["rect"]): e for e in json.loads(CROPS.read_text())["entries"] if e["set"] == "Table/Data" and e.get("finding")}
        self.assertEqual(by[(322, 204, 449, 41)]["finding_pin"], {"fg_render_px": [230, 16], "fg_production_px": [234, 16], "mean_abs_rgb_diff": 1.42})
        self.assertEqual(by[(772, 81, 39, 41)]["finding_pin"], {"peak_colour_render": [255, 255, 255], "peak_colour_production": [198, 198, 198]})

    def test_a_finding_on_a_row_that_fails_the_box_pins_the_mean_difference_too(self):
        """A box check sees only the extreme columns and rows of the ink: a recolour or a mid-row shift that keeps both leaves it untouched, but moves the mean over the crop (merge review LOW 3)."""
        rows = [e for e in json.loads(CROPS.read_text())["entries"] if e.get("finding") and "fg_render_px" in e["finding_pin"] and not e.get("colour_only")]
        self.assertGreaterEqual(len(rows), 2)
        for e in rows:
            with self.subTest(e["case"] + str(e.get("named") or e.get("rect"))):
                self.assertIn("mean_abs_rgb_diff", e["finding_pin"])

    def test_the_table_kind_glyph_row_carries_no_exemption_and_no_finding(self):
        """Both boxes measure 14 x 14 and both strongest pixels 255: the row passes the full box and colour checks, so an exemption or a finding on it only loosens it (merge review MINOR 3)."""
        rows = [e for e in json.loads(CROPS.read_text())["entries"] if e["set"] == "Table/Data" and e.get("rect") == [302, 212, 24, 24]]
        self.assertEqual(len(rows), 1)
        self.assertFalse({"colour_only", "finding", "finding_pin"} & set(rows[0]), sorted(rows[0]))


if __name__ == "__main__":
    unittest.main()
