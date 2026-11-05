"""A gradient crop has no surface and no mark, so it is compared pixel by pixel (crop_gradient): the mean of the largest channel difference per pixel and its 99th percentile over the two crops' common
size, with sizes within 1 px. A wrong stop, a flat colour, a hue off by a step or a ramp drawn 4 px to the side each fail; the rounded cap pixels that differ by anti-aliasing do not. No Pillow:
an image is a flat list of RGB tuples.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_gradient
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_gradient as cg  # noqa: E402
import crop_metrics as cm  # noqa: E402

W, H = 172, 12


def ramp(shift=0, w=W, h=H, lo=0, hi=255):
    """A horizontal grey ramp lo..hi over `w` px, drawn `shift` px to the right."""
    row = [min(w - 1, max(0, x - shift)) * (hi - lo) // (w - 1) + lo for x in range(w)]
    return [(v, v, v) for _ in range(h) for v in row], w, h


def hue(shift=0):
    stops = [(255, 0, 0), (255, 255, 0), (0, 255, 0), (0, 255, 255), (0, 0, 255), (255, 0, 255), (255, 0, 0)]
    row = []
    for x in range(W):
        t = min(W - 1, max(0, x - shift)) / (W - 1) * 6
        i = min(5, int(t))
        a, b, f = stops[i], stops[i + 1], t - i
        row.append(tuple(round(a[k] + (b[k] - a[k]) * f) for k in range(3)))
    return row * H, W, H


class FitTest(unittest.TestCase):
    def test_identical_ramps_fit_exactly(self):
        fit = cg.gradient_fit(ramp(), ramp())
        self.assertEqual((fit["fit_mean"], fit["fit_p99"], fit["fit_ok"]), (0.0, 0, True))

    def test_a_one_pixel_shift_of_a_ramp_is_within_the_anti_aliasing_tolerance(self):
        fit = cg.gradient_fit(ramp(1), ramp())
        self.assertTrue(fit["fit_ok"], fit)
        self.assertLessEqual(fit["fit_mean"], cg.MEAN_TOL)

    def test_a_four_pixel_shift_fails(self):
        fit = cg.gradient_fit(ramp(4), ramp())
        self.assertFalse(fit["fit_ok"], fit)
        self.assertGreater(fit["fit_mean"], cg.MEAN_TOL)

    def test_a_flat_colour_in_place_of_the_ramp_fails(self):
        flat = ([(38, 38, 38)] * (W * H), W, H)
        self.assertFalse(cg.gradient_fit(flat, ramp())["fit_ok"])

    def test_a_hue_strip_one_step_off_fails_and_the_same_hue_fits(self):
        self.assertTrue(cg.gradient_fit(hue(), hue())["fit_ok"])
        self.assertFalse(cg.gradient_fit(hue(10), hue())["fit_ok"])

    def test_a_few_cap_pixels_far_off_do_not_fail_it(self):
        """18 of 1848 px (under 1 %) differ by 154, as the hue strip's rounded right cap does against egui's."""
        render, prod = ramp(), ramp()
        px = list(render[0])
        for i in range(18):
            px[W * 5 + 10 + i] = (255 - px[W * 5 + 10 + i][0],) * 3
        fit = cg.gradient_fit((px, W, H), prod)
        self.assertGreater(max(cg.pixel_diffs((px, W, H), prod)[0]), 100)
        self.assertTrue(fit["fit_ok"], fit)

    def test_a_wrong_drawing_over_more_than_one_percent_of_the_pixels_fails_the_percentile(self):
        px = list(ramp()[0])
        for i in range(60):  # 60 of 2064 px = 2.9 %, each 20 levels off
            px[100 + i] = (min(255, px[100 + i][0] + 20),) * 3
        fit = cg.gradient_fit((px, W, H), ramp())
        self.assertGreater(fit["fit_p99"], cg.P99_TOL)
        self.assertFalse(fit["fit_ok"])

    def test_a_small_mean_with_a_wider_spread_still_fails_on_the_percentile(self):
        px = list(ramp()[0])
        for i in range(30):  # 30 of 2064 px = 1.45 %, 30 levels off
            px[i] = (px[i][0] + 30,) * 3
        fit = cg.gradient_fit((px, W, H), ramp())
        self.assertLessEqual(fit["fit_mean"], cg.MEAN_TOL, "the mean alone would have let it through")
        self.assertEqual(fit["fit_p99"], 30)
        self.assertFalse(fit["fit_ok"])

    def test_sizes_that_differ_by_more_than_one_pixel_fail_even_when_the_common_area_matches(self):
        fit = cg.gradient_fit(ramp(), ramp(w=W - 3))
        self.assertFalse(fit["fit_size_ok"])
        self.assertFalse(fit["fit_ok"])
        self.assertTrue(cg.gradient_fit(ramp(), ramp(w=W - 1))["fit_size_ok"])

    def test_a_render_three_px_wider_over_an_identical_common_area_does_not_fit(self):
        """The mean and the percentile see only the common top-left area: two identical ramps, the render 3 px wider, differ by 0 there, so only the size check can fail it."""
        prod = ramp()
        wide = [px for y in range(H) for px in prod[0][y * W:(y + 1) * W] + [(255, 255, 255)] * 3]
        fit = cg.gradient_fit((wide, W + 3, H), prod)
        self.assertEqual((fit["fit_mean"], fit["fit_p99"]), (0.0, 0))
        self.assertEqual((fit["fit_size_ok"], fit["fit_ok"]), (False, False))
        self.assertTrue(cg.gradient_fit((wide, W + 3, H), ramp(w=W + 1))["fit_size_ok"] is False)

    def test_an_empty_common_area_never_fits(self):
        fit = cg.gradient_fit(([], 0, 0), ramp())
        self.assertFalse(fit["fit_ok"])
        self.assertIsNone(fit["fit_mean"])

    def test_the_percentile_index_is_the_ceiling_of_99_percent(self):
        px = [(0, 0, 0)] * 100
        px[99] = (50, 50, 50)  # the 100th pixel alone: the 99th percentile of 100 values is the 99th value, 0
        self.assertEqual(cg.gradient_fit((px, 100, 1), ([(0, 0, 0)] * 100, 100, 1))["fit_p99"], 0)
        px[98] = (50, 50, 50)  # two pixels: the 99th value is now 50
        self.assertEqual(cg.gradient_fit((px, 100, 1), ([(0, 0, 0)] * 100, 100, 1))["fit_p99"], 50)


class VerdictTest(unittest.TestCase):
    ENTRY = {"gradient": True}

    def test_a_fitting_gradient_row_is_not_bad_and_has_no_box_surface_or_peak_check(self):
        m = cm.measure_pixels(ramp(), ramp(1), self.ENTRY)
        self.assertEqual((m["bad"], m["fit_fails"], m["gradient"]), (False, False, True))
        self.assertEqual((m["fg_fails"], m["surface_fails"], m["peak_fails"], m["ambiguous_fails"]), (False, False, False, False))
        self.assertIsNone(m["peak_diff"])

    def test_a_gradient_row_that_does_not_fit_is_bad_and_not_ambiguous(self):
        m = cm.measure_pixels(([(38, 38, 38)] * (W * H), W, H), ramp(), self.ENTRY)
        self.assertEqual((m["bad"], m["fit_fails"], m["ambiguous_fails"]), (True, True, False))

    def test_without_the_key_the_same_images_take_the_old_checks_and_are_ambiguous(self):
        m = cm.measure_pixels(ramp(), ramp(1), {})
        self.assertTrue(m["ambiguous_fails"] and m["bad"], "a ramp has no surface: that is why a gradient row exists")

    def test_the_fail_line_names_the_fit_numbers(self):
        row = {"set": "Popover/ColorPicker", "variant": "", "capture": "f-picker/hue", "gradient": True, "fit_ok": False, "fit_mean": 9.5, "fit_p99": 40, "fit_size_ok": True, "finding": None}
        line = cm.fail_line(row)
        self.assertIn("gradient fit FAIL", line)
        self.assertIn("mean difference 9.5 of at most 2.0", line)
        self.assertIn("99th percentile 40 of at most 8", line)

    def test_a_failing_gradient_row_is_accepted_only_with_a_pin_on_its_fit_numbers(self):
        row = {"set": "S", "variant": "", "capture": "c", "bad": True, "pending": None, "gradient": True, "fit_ok": False, "fit_mean": 9.5, "fit_p99": 40, "fit_size_ok": True,
               "background_ok": True, "finding": "why", "finding_pin": None}
        self.assertEqual(cm.exit_code([row]), 1)
        self.assertEqual(cm.exit_code([dict(row, finding_pin={"fit_mean": 9.5})]), 1, "the pin holds one of the two fit fields")
        self.assertEqual(cm.exit_code([dict(row, finding_pin={"fit_mean": 9.5, "fit_p99": 40})]), 1, "the pin leaves the size check out")
        self.assertEqual(cm.exit_code([dict(row, finding_pin={"fit_mean": 9.5, "fit_p99": 40, "fit_size_ok": True})]), 0)
        self.assertEqual(cm.exit_code([dict(row, finding_pin={"fit_mean": 9.4, "fit_p99": 40, "fit_size_ok": True})]), 1)

    def test_a_gradient_pin_that_leaves_the_size_check_out_does_not_accept_a_strip_that_grew(self):
        """A strip 3 px too wide that still overlaps its production crop: mean 0.6, p99 1, size not ok. The pin on the two fit numbers alone must not accept it."""
        row = {"set": "S", "variant": "", "capture": "c", "bad": True, "pending": None, "gradient": True, "fit_ok": False, "fit_mean": 0.6, "fit_p99": 1, "fit_size_ok": False,
               "background_ok": True, "finding": "why", "finding_pin": {"fit_mean": 0.6, "fit_p99": 1}}
        self.assertEqual(cm.exit_code([row]), 1)
        self.assertIn("fit_size_ok", "\n".join(cm.report_lines([row])))
        self.assertEqual(cm.exit_code([dict(row, finding_pin={"fit_mean": 0.6, "fit_p99": 1, "fit_size_ok": False})]), 0)
        self.assertEqual(cm.exit_code([dict(row, finding_pin={"fit_mean": 0.6, "fit_p99": 1, "fit_size_ok": True})]), 1, "a pin that says the sizes agree no longer describes the row")


if __name__ == "__main__":
    unittest.main()
