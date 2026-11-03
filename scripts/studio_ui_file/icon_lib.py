"""The Phosphor icon components of page 01 as seen by a component module: REST ids, keys and 16/20 px glyph paths.

    lib = IconLib(repo); lib.entry("Search") -> {"id": "icon:11", "key": "icon-Search", "glyph": "icon:11:v"}
    lib.path("Search", 20) -> an SVG path scaled from the 256 viewBox to 20 px; lib.keys() -> every preferred-value key
"""
import icons_rest


class IconLib:
    def __init__(self, repo):
        self.repo = repo
        variants, self.map = icons_rest.read_icon_map(repo)
        self.names = [v for v in variants if v != "Logo"]
        self.index = {v: i for i, v in enumerate(self.names)}
        self._paths = {}

    def entry(self, name):
        if name not in self.index:
            raise KeyError(f"no icon component Icon/{name}")
        k = self.index[name]
        return {"id": f"icon:{k}", "key": f"icon-{name}", "glyph": f"icon:{k}:v"}

    def path(self, name, size):
        if name not in self._paths:
            self._paths[name] = icons_rest.read_svg(self.repo, self.map[name])[0]
        return icons_rest.scale_path(self._paths[name], size / 256)

    def keys(self):
        return [self.entry(n)["key"] for n in self.names]
