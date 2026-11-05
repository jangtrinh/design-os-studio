"""Nested instances for the lowerer (`rest_ir.X`): the instance of an already lowered variant, in REST form.

A childless main (a focus ring) is re-lowered into the instance, so the instance carries the main's bindings and allow-list entries. A main with children becomes
an INSTANCE with the main's frame fields and no children: the importer expands it from its main (`expand_instance`, designos-fig/src/rest_import.rs) and applies the
`componentProperties` the module gave. `Lowerer` supplies `main_ir`, `main_ids`, `set_props`, `icons`, `value`, `_frame_fields` and `lower`.

The texts inside such an instance exist only after the import (REST ids `I<instance>;<main node>`), so the layout pass learns them here: `main_texts[(set, variant)]` lists
`(REST id, content, characters property key)` for the texts of a lowered main, followed by the texts of the instances nested in it, derived the same way with the content
the module's `props` gave (a TEXT value replaces the main's content for the text that property drives). The layout pass sets each of them to a placeholder and back, so the
engine measures it: a copy is otherwise as wide as the estimate it was cloned at, and neither the default label's width nor a longer label's hug would be real.
"""
import rest_ir as ir


class NestedInstances:
    def default_sizing(self, node):
        """(sx, sy) a node takes in an auto-layout parent when the module gives none: an icon is fixed, an X that nests a variant with children is that
        variant's own (the importer expands it from its main, so its `layout_child` stays the main's until the module resizes it), everything else hugs."""
        if node["t"] == "I":
            return "fixed", "fixed"
        main = self.main_ir.get((node["set"], node["variant"])) if node["t"] == "X" else None
        if main is not None and main["kids"]:
            return main.get("sx", "hug"), main.get("sy", "hug")
        return "hug", "hug"

    def child_sizing(self, node):
        """(sx, sy) of a node in an auto-layout parent, as its `layoutSizing*` fields write them: the module's own, else `default_sizing`. For an `X` that nests a variant
        with children, `w=` / `h=` make the axis fixed, and a Hug that differs from the main's is written as Fixed (the editor's `set_sizing`: `layout_child` Fixed, the
        copy's own sizing Hug, see `own_sizing`)."""
        dx, dy = self.default_sizing(node)
        if node["t"] != "X":
            return node.get("sx", dx), node.get("sy", dy)
        main = self.main_ir.get((node["set"], node["variant"]))
        out = []
        for axis, default, size in (("sx", dx, "w"), ("sy", dy, "h")):
            mode = node.get(axis) or ("fixed" if node.get(size) else default)
            out.append("fixed" if mode == "hug" and main is not None and main["kids"] and mode != default else mode)
        return tuple(out)

    def own_sizing(self, node, main):
        """{"sx": .., "sy": ..} for the instance root's own frame sizing: only the axes where the copy asks for something other than its main's own, as `set_sizing` writes them: Fill and Fixed
        are own Fixed, Hug is own Hug (so the engine does not re-hug a Fill copy of a hugging main)."""
        out = {}
        for axis, size in (("sx", "w"), ("sy", "h")):
            want = node.get(axis) or ("fixed" if node.get(size) else None)
            if want and want != main.get(axis, "hug"):
                out[axis] = "hug" if want == "hug" else "fixed"
        return out

    def texts_of(self, rest, key):
        """[(REST id, content, characters property key or None)] of a lowered main `rest`: its own texts in document order, then those of its nested instances.
        Also fills `fill_texts[key]` (the texts whose width follows the main's root: Fill-width all the way up) and `fill_pairs[key]` ({copied text: (its instance, its main's root)}
        for the copies that keep their main's content and follow it: a resized copy's text is as wide as its main's plus the instance's growth, see library_layout.check_nested)."""
        own, follows, stack = [], {}, [(rest, True)]
        while stack:
            n, chain = stack.pop()
            follows[n["id"]] = chain
            if n["type"] == "TEXT":
                own.append((n["id"], n["characters"], (n.get("componentPropertyReferences") or {}).get("characters")))
            stack += [(c, chain and c.get("layoutSizingHorizontal") == "FILL") for c in reversed(n.get("children", []))]
        derived, fills, pairs = [], {t[0] for t in own if follows[t[0]]}, {}
        for sid, set_name, variant, props in self._nested_log:
            values = {k: props[name] for name, (k, _) in self.set_props.get(set_name, {}).items() if name in props}
            for ms, content, k in self.main_texts[(set_name, variant)]:
                derived.append((f"I{sid};{ms}", values.get(k, content) if k else content, None))
                follow = ms in self.fill_texts[(set_name, variant)]
                if follow and follows[sid]:
                    fills.add(derived[-1][0])
                if derived[-1][1] == content:  # the main's own content: its measured width must come out the same (library_layout.check_nested)
                    self._mirror_log.append((derived[-1][0], ms))
                    if follow:
                        pairs[derived[-1][0]] = (sid, self.main_ids[(set_name, variant)])
        self.fill_texts[key], self.fill_pairs[key] = fills, pairs
        return own + derived

    def _X(self, n, out, sid, x, y, w, h, ptype):
        """Nested INSTANCE of an already lowered variant. A childless main (a focus ring) is re-lowered into the instance: its own paints and radii mirror the
        main's, so it carries the main's bindings (and allow-list entries), except a `radius` the instance pins (a ring on a host that is not RADIUS_CONTROL).
        A main with children becomes an instance with no children: the importer expands it from its main and applies `props`."""
        main = self.main_ir[(n["set"], n["variant"])]
        if main["kids"]:
            return self._nested(n, main, out, sid)
        if n.get("props"):
            raise ValueError(f"{n['name']}: {n['set']} / {n['variant']} has no children, so no property to set")
        if not n.get("abs"):
            raise ValueError(f"{n['name']}: {n['set']} / {n['variant']} has no children, so it nests as an absolutely placed instance: give `abs`")
        inner = dict(ir.clone(main), name=n["name"], abs=n["abs"])
        if "radius" in n:  # the ring's own corner radius (host + SPACE_FACT): a literal with its own allow-list entries
            inner["radius"] = n["radius"]
        out.update(self.lower(inner, x, y, ptype="INSTANCE", forced_id=sid), componentId=self.main_ids[(n["set"], n["variant"])])

    def _nested(self, n, main, out, sid):
        """An instance root with the main's own frame fields (paints, radii, auto layout and its sizing modes: an instance is its main until it is resized), no
        children, and the property values the module gave. It records no bind intent or allow-list entry: the instance takes the main's bindings and
        waivers in the post-pass (mirror_sources), as the editor's sync does, so a child can never carry a binding its main does not."""
        self._nested_log.append((sid, n["set"], n["variant"], n.get("props") or {}))
        if n.get("w") or n.get("h"):
            self._resize_log.append((sid, n.get("w"), n.get("h")))
        root = dict(ir.clone(main), name=n["name"], kids=[], **self.own_sizing(n, main))
        marks = len(self.intents), len(self.allow)
        self._frame_fields(root, out, sid, "INSTANCE")
        del self.intents[marks[0]:], self.allow[marks[1]:]
        out["componentId"] = self.main_ids[(n["set"], n["variant"])]
        if n.get("props"):
            out["componentProperties"] = self.property_values(n["set"], n["props"])
        return out

    def property_values(self, set_name, props):
        """REST `componentProperties` for `props` ({property name: value}) of set `set_name`, keyed by the set's `Name#id` keys."""
        known = self.set_props.get(set_name, {})
        out = {}
        for name, value in props.items():
            if name not in known:
                raise KeyError(f"{set_name} has no component property {name!r} (has {sorted(known)})")
            key, kind = known[name]
            if kind == "TEXT" and isinstance(value, str):
                out[key] = {"type": "TEXT", "value": value}
            elif kind == "BOOLEAN" and isinstance(value, bool):
                out[key] = {"type": "BOOLEAN", "value": value}
            elif kind == "INSTANCE_SWAP" and isinstance(value, str):
                out[key] = {"type": "INSTANCE_SWAP", "value": self.icons.entry(value)["id"]}
            else:
                raise ValueError(f"{set_name}.{name} is a {kind} property, got {value!r}")
        return out
