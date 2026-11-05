"""A small evaluator for the Rust constant expressions theme.rs uses: numbers (integers stay integers, so `/` on two integers
is integer division), strings, aliases of other constants, `+ - * /`, arrays, tuples and the `rgb` / `rgba` / `Color32::*` calls.
Anything else raises `ThemeError`. Constants are duck-typed objects with `.expr`, `.value` and `.resolved`.
"""
from __future__ import annotations

import re
from typing import Any, Dict, List


class ThemeError(Exception):
    pass


_TOK = re.compile(r'\s*(0x[0-9A-Fa-f_]+|\d[\d_]*(?:\.\d*)?(?:[eE][+-]?\d+)?(?:f32|f64|u8|u16|u32|usize)?|"[^"]*"|[A-Za-z_]\w*(?:::\w+)*|[()\[\],*+\-/])')


def tokens(expr: str) -> List[str]:
    out, pos = [], 0
    while pos < len(expr.rstrip()):
        m = _TOK.match(expr, pos)
        if not m:
            raise ThemeError(f"cannot tokenize `{expr[pos:pos + 20]}`")
        out.append(m.group(1))
        pos = m.end()
    return out


def number(tok: str):
    t = tok.replace("_", "")
    if t.lower().startswith("0x"):
        return int(t, 16)
    is_float = "." in t or "e" in t.lower() or t.endswith(("f32", "f64"))
    t = re.sub(r"(f32|f64|u8|u16|u32|usize)$", "", t)
    return float(t) if is_float else int(t)


def call(fn: str, a: list):
    fn = fn.split("::")[-1]
    if fn == "rgb" and len(a) == 1:
        v = int(a[0])
        return {"r": v >> 16 & 255, "g": v >> 8 & 255, "b": v & 255, "a": 255}
    if fn == "rgba" and len(a) == 2:
        v = int(a[0])
        return {"r": v >> 16 & 255, "g": v >> 8 & 255, "b": v & 255, "a": int(a[1])}
    if fn == "from_rgb" and len(a) == 3:
        return {"r": int(a[0]), "g": int(a[1]), "b": int(a[2]), "a": 255}
    if fn == "from_rgba_premultiplied" and len(a) == 4:
        r, g, b, al = (int(x) for x in a)
        un = (lambda c: min(255, round(c * 255 / al))) if al else (lambda c: 0)
        return {"r": un(r), "g": un(g), "b": un(b), "a": al}
    raise ThemeError(f"unsupported call {fn}/{len(a)}")


class Evaluator:
    def __init__(self, consts: Dict[str, Any]):
        self.consts, self.stack, self.t, self.p = consts, [], [], 0

    def const(self, name: str) -> Any:
        c = self.consts.get(name)
        if c is None:
            raise ThemeError(f"unknown constant {name}")
        if not c.resolved:
            if name in self.stack:
                raise ThemeError(f"cycle {' -> '.join(self.stack + [name])}")
            self.stack.append(name)
            c.value = self.expr(c.expr)
            c.resolved = True
            self.stack.pop()
        return c.value

    def expr(self, text: str) -> Any:
        self.t, self.p = tokens(text), 0
        v = self._sum()
        if self.p != len(self.t):
            raise ThemeError(f"trailing tokens in `{text}`")
        return v

    def _peek(self):
        return self.t[self.p] if self.p < len(self.t) else None

    def _eat(self, want=None):
        tok = self._peek()
        if tok is None or (want and tok != want):
            raise ThemeError(f"expected {want or 'token'}, got {tok}")
        self.p += 1
        return tok

    def _sum(self):
        v = self._prod()
        while self._peek() in ("+", "-"):
            op, r = self._eat(), self._prod()
            v = v + r if op == "+" else v - r
        return v

    def _prod(self):
        v = self._unary()
        while self._peek() in ("*", "/"):
            op, r = self._eat(), self._unary()
            v = v * r if op == "*" else (v // r if isinstance(v, int) and isinstance(r, int) else v / r)
        return v

    def _unary(self):
        if self._peek() == "-":
            self._eat()
            return -self._unary()
        return self._primary()

    def _items(self, close: str):
        items = []
        while self._peek() != close:
            items.append(self._sum())
            if self._peek() == ",":
                self._eat()
        self._eat(close)
        return items

    def _primary(self):
        tok = self._eat()
        if tok[0].isdigit():
            return number(tok)
        if tok[0] == '"':
            return tok[1:-1]
        if tok == "[":
            return self._items("]")
        if tok == "(":
            items = self._items(")")
            return items[0] if len(items) == 1 else tuple(items)
        if self._peek() == "(":
            self._eat("(")
            return call(tok, self._items(")"))
        saved = (self.t, self.p)
        value = self.const(tok.split("::")[-1])
        self.t, self.p = saved
        return value
