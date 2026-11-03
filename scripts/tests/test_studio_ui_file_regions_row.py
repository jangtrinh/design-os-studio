"""Every registry `region` record sits in the `Regions` row of page `03 Studio`, the row the assembler lowers after the composites a region nests."""
import json
import unittest

import studio_ui_regions_fixtures as fx


class RegionsRowTest(unittest.TestCase):
    def setUp(self):
        registry = json.loads((fx.REPO / "design" / "component-registry.json").read_text())
        self.regions = sorted(r["name"] for r in registry["components"] if r["category"] == "region")

    def test_every_registry_region_is_built_in_the_regions_row(self):
        built = {n: fx.SPECS[n]["row"] for n in self.regions if n in fx.SPECS}
        self.assertEqual(len(built), len(self.regions), "a region record has no module")
        self.assertEqual({n: r for n, r in built.items() if r != "Regions"}, {})

    def test_the_regions_row_is_the_last_row_so_the_composites_a_region_nests_are_lowered_first(self):
        self.assertEqual(fx.components.STUDIO_ROWS[-1], "Regions")

    def test_a_composite_stays_out_of_the_regions_row(self):
        self.assertEqual(fx.SPECS["Input/Composer"]["row"], "Agents")


if __name__ == "__main__":
    unittest.main()
