/**
 * design:os studio — Self-contained ES2020 Web Capture Script (T8.25 / Issue #276)
 *
 * Exposes `window.__designosCapture(options) -> Promise<Capture>`.
 * Uses only standard DOM APIs, never mutates the page, and captures references to
 * native globals at load time so pages that override `Array.prototype` or `window.fetch`
 * cannot break capture (T8.25 §3.1, §8.6).
 */
(function () {
  "use strict";

  const g = typeof window !== "undefined" ? window : globalThis;
  const nativeFetch =
    typeof g.fetch === "function" ? Function.prototype.bind.call(g.fetch, g) : null;
  const nativeSlice = Array.prototype.slice;
  const nativeFrom = Array.from;
  const nativeKeys = Object.keys;
  const nativeToString = Object.prototype.toString;
  const nativeNow =
    typeof performance !== "undefined" && typeof performance.now === "function"
      ? () => performance.now()
      : () => Date.now();

  const SKIPPED_TAGS = new Set(["script", "style", "template", "noscript", "head"]);

  // Exact computed style subset specified in T8.25 §3.4.
  const CAPTURED_STYLE_PROPS = [
    "display",
    "position",
    "top",
    "right",
    "bottom",
    "left",
    "z-index",
    "flex-direction",
    "flex-wrap",
    "justify-content",
    "align-items",
    "align-content",
    "align-self",
    "flex-grow",
    "flex-shrink",
    "flex-basis",
    "gap",
    "row-gap",
    "column-gap",
    "grid-template-columns",
    "grid-template-rows",
    "grid-column",
    "grid-row",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-top-style",
    "border-right-style",
    "border-bottom-style",
    "border-left-style",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
    "background-color",
    "background-image",
    "background-size",
    "background-position",
    "background-repeat",
    "box-shadow",
    "opacity",
    "overflow-x",
    "overflow-y",
    "transform",
    "transform-origin",
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration-line",
    "text-transform",
    "white-space",
    "word-break",
    "text-overflow",
    "-webkit-line-clamp",
    "object-fit",
    "mix-blend-mode",
    "filter",
    "backdrop-filter",
    "visibility",
  ];

  const TEXT_RUN_STYLE_PROPS = [
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration-line",
    "text-transform",
    "white-space",
  ];

  // CSS shorthand -> longhand map. A shorthand declaration (with or without `var()`) resets every
  // longhand it covers, so it takes part in the cascade for each of them (T8.25 §3.7).
  const SIDES = ["top", "right", "bottom", "left"];
  const CORNERS = ["top-left", "top-right", "bottom-right", "bottom-left"];
  const BORDER_PARTS = ["width", "style", "color"];
  const SHORTHAND_EXPANSIONS = {
    background: [
      "background-color",
      "background-image",
      "background-size",
      "background-position",
      "background-repeat",
    ],
    "border-radius": CORNERS.map((c) => `border-${c}-radius`),
    padding: SIDES.map((s) => `padding-${s}`),
    margin: SIDES.map((s) => `margin-${s}`),
    inset: SIDES.slice(),
    "border-color": SIDES.map((s) => `border-${s}-color`),
    "border-width": SIDES.map((s) => `border-${s}-width`),
    "border-style": SIDES.map((s) => `border-${s}-style`),
    border: SIDES.flatMap((s) => BORDER_PARTS.map((p) => `border-${s}-${p}`)),
    gap: ["gap", "row-gap", "column-gap"],
    overflow: ["overflow-x", "overflow-y"],
    font: ["font-family", "font-size", "font-weight", "font-style", "line-height"],
    flex: ["flex-grow", "flex-shrink", "flex-basis"],
    "flex-flow": ["flex-direction", "flex-wrap"],
    "text-decoration": ["text-decoration-line"],
  };
  for (const side of SIDES) {
    SHORTHAND_EXPANSIONS[`border-${side}`] = BORDER_PARTS.map((p) => `border-${side}-${p}`);
  }
  const CAPTURED_SET = new Set(CAPTURED_STYLE_PROPS);
  const COLOR_PROPS = new Set([
    "color",
    "background-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
  ]);

  /** Captured longhands a declaration of `name` sets (`all` resets every captured property). */
  function expandProp(name) {
    if (name === "all") return CAPTURED_STYLE_PROPS;
    return (SHORTHAND_EXPANSIONS[name] || [name]).filter((p) => CAPTURED_SET.has(p));
  }

  function parsePx(val) {
    if (!val) return 0;
    const n = parseFloat(val);
    return Number.isFinite(n) ? n : 0;
  }

  function roundCoord(n) {
    return Math.round(n * 100) / 100;
  }

  /** Splits `text` at top-level `separator`, ignoring quotes and (), [], {} nesting. */
  function splitTopLevel(text, separator) {
    const parts = [];
    let depth = 0;
    let quote = "";
    let start = 0;
    for (let i = 0; i < text.length; i++) {
      const ch = text[i];
      if (quote) {
        if (ch === "\\") i++;
        else if (ch === quote) quote = "";
        continue;
      }
      if (ch === "\\") i++;
      else if (ch === '"' || ch === "'") quote = ch;
      else if (ch === "(" || ch === "[" || ch === "{") depth++;
      else if (ch === ")" || ch === "]" || ch === "}") depth = Math.max(0, depth - 1);
      else if (ch === separator && depth === 0) {
        parts.push(text.slice(start, i));
        start = i + 1;
      }
    }
    parts.push(text.slice(start));
    return parts;
  }

  /** Index of the `)` matching the `(` at `open`, or -1. */
  function matchParen(text, open) {
    let depth = 0;
    let quote = "";
    for (let i = open; i < text.length; i++) {
      const ch = text[i];
      if (quote) {
        if (ch === "\\") i++;
        else if (ch === quote) quote = "";
      } else if (ch === "\\") i++;
      else if (ch === '"' || ch === "'") quote = ch;
      else if (ch === "(") depth++;
      else if (ch === ")" && --depth === 0) return i;
    }
    return -1;
  }

  /** Parses a declaration block (`a: b; c: d !important`) keeping `var()` shorthands intact. */
  function parseDeclarations(cssText) {
    const out = [];
    for (const part of splitTopLevel(String(cssText || ""), ";")) {
      const colon = part.indexOf(":");
      if (colon <= 0) continue;
      let name = part.slice(0, colon).trim();
      let value = part.slice(colon + 1).trim();
      const important = /!\s*important\s*$/i.test(value);
      if (important) value = value.replace(/!\s*important\s*$/i, "").trim();
      if (!name.startsWith("--")) name = name.toLowerCase();
      if (name) out.push({ name, value, important });
    }
    return out;
  }

  function skipIdent(sel, i) {
    while (i < sel.length && /[\w-]|[^\x00-\x7f]/.test(sel[i])) i++;
    return i;
  }

  /** `[ids, classes/attributes/pseudo-classes, types/pseudo-elements]` of one complex selector. */
  function specificityTriple(sel) {
    let a = 0;
    let b = 0;
    let c = 0;
    let i = 0;
    const addMax = (list) => {
      let best = [0, 0, 0];
      for (const item of list) {
        const t = specificityTriple(item.trim());
        if (t[0] > best[0] || (t[0] === best[0] && (t[1] > best[1] || (t[1] === best[1] && t[2] > best[2])))) {
          best = t;
        }
      }
      a += best[0];
      b += best[1];
      c += best[2];
    };
    while (i < sel.length) {
      const ch = sel[i];
      if (ch === "\\") {
        i += 2;
      } else if (ch === "#") {
        a++;
        i = skipIdent(sel, i + 1);
      } else if (ch === ".") {
        b++;
        i = skipIdent(sel, i + 1);
      } else if (ch === "[") {
        b++;
        const end = sel.indexOf("]", i);
        i = end < 0 ? sel.length : end + 1;
      } else if (ch === ":") {
        const double = sel[i + 1] === ":";
        i += double ? 2 : 1;
        const start = i;
        i = skipIdent(sel, i);
        const name = sel.slice(start, i).toLowerCase();
        let arg = null;
        if (sel[i] === "(") {
          const end = matchParen(sel, i);
          arg = sel.slice(i + 1, end < 0 ? sel.length : end);
          i = end < 0 ? sel.length : end + 1;
        }
        if (double || name === "before" || name === "after" || name === "first-line" || name === "first-letter") {
          c++;
        } else if (name === "where") {
          // zero specificity
        } else if (["is", "matches", "not", "has", "-webkit-any", "-moz-any"].includes(name)) {
          addMax(splitTopLevel(arg || "", ","));
        } else if ((name === "nth-child" || name === "nth-last-child") && arg && / of /i.test(arg)) {
          b++;
          addMax(splitTopLevel(arg.split(/ of /i).slice(1).join(" of "), ","));
        } else {
          b++;
        }
      } else if (ch === "*") {
        i++;
      } else if (/[a-zA-Z_]|[^\x00-\x7f]/.test(ch)) {
        c++;
        i = skipIdent(sel, i);
      } else {
        i++;
      }
    }
    return [a, b, c];
  }

  function selectorSpecificity(selector) {
    const [a, b, c] = specificityTriple(String(selector || ""));
    return Math.min(a, 999) * 1e6 + Math.min(b, 999) * 1e3 + Math.min(c, 999);
  }

  /** Higher result = the `x` layer path wins a normal declaration; `[]` is the unlayered top. */
  function compareLayers(x, y) {
    for (let i = 0; ; i++) {
      if (i >= x.length && i >= y.length) return 0;
      if (i >= x.length) return 1;
      if (i >= y.length) return -1;
      if (x[i] !== y[i]) return x[i] > y[i] ? 1 : -1;
    }
  }

  /**
   * CSS cascade order of two author declarations: importance, element-attached (`style=""`),
   * layer (reversed for `!important`), specificity, then source order (css-cascade-5 §6.1).
   */
  function compareRank(a, b) {
    if (a.important !== b.important) return a.important - b.important;
    if (a.attached !== b.attached) return a.attached - b.attached;
    const layers = compareLayers(a.layer, b.layer);
    if (layers !== 0) return a.important ? -layers : layers;
    if (a.spec !== b.spec) return a.spec - b.spec;
    return a.order - b.order;
  }

  /** Substitutes `var()` with `lookup(name)` values; `null` when any reference cannot resolve. */
  function substituteVars(text, lookup, depth = 0) {
    if (depth > 8) return null;
    let out = "";
    let i = 0;
    while (i < text.length) {
      const at = text.indexOf("var(", i);
      if (at < 0) {
        out += text.slice(i);
        break;
      }
      out += text.slice(i, at);
      const end = matchParen(text, at + 3);
      if (end < 0) return null;
      const parts = splitTopLevel(text.slice(at + 4, end), ",");
      let value = lookup(parts[0].trim());
      if (!value) value = parts.slice(1).join(",").trim();
      if (!value) return null;
      if (value.includes("var(")) {
        value = substituteVars(value, lookup, depth + 1);
        if (value === null) return null;
      }
      out += value;
      i = end + 1;
    }
    return out.trim();
  }

  let colorProbeCtx;
  /**
   * Normalises a CSS colour through a detached 2D canvas (the page is never touched). `null` when
   * the text is not a colour: an invalid assignment keeps the sentinel, so two sentinels disagree.
   */
  function canonicalColor(text) {
    try {
      if (colorProbeCtx === undefined) {
        const canvas = g.document.createElement("canvas");
        colorProbeCtx = canvas.getContext("2d") || null;
      }
      if (!colorProbeCtx) return null;
      colorProbeCtx.fillStyle = "#010203";
      colorProbeCtx.fillStyle = text;
      const first = colorProbeCtx.fillStyle;
      colorProbeCtx.fillStyle = "#040506";
      colorProbeCtx.fillStyle = text;
      return first === colorProbeCtx.fillStyle ? first : null;
    } catch (_e) {
      return null;
    }
  }

  function sameValue(resolved, computed) {
    const colorA = canonicalColor(resolved);
    const colorB = canonicalColor(computed);
    if (colorA !== null && colorB !== null) return colorA === colorB;
    const norm = (s) => String(s).trim().toLowerCase().replace(/\s+/g, " ").replace(/^0$/, "0px");
    return norm(resolved) === norm(computed);
  }

  /**
   * Extracts all `--*` variable names referenced inside a CSS value string containing `var(...)`.
   */
  function extractVarNames(rawVal) {
    if (!rawVal || typeof rawVal !== "string" || !rawVal.includes("var(")) {
      return [];
    }
    const out = [];
    const re = /var\(\s*(--[a-zA-Z0-9_-]+)/g;
    let m;
    while ((m = re.exec(rawVal)) !== null) {
      if (!out.includes(m[1])) {
        out.push(m[1]);
      }
    }
    return out;
  }

  /**
   * Indexes the author style rules of one tree scope (a document or an open shadow root) in
   * cascade terms: every declaration (not only `var()` ones, so a later literal can clear an
   * earlier token), the layer path, specificity per selector, and source order. Rules inside
   * `@media`/`@supports`/`@import` conditions that do not apply to the scope's window are dropped.
   * `@container`, `@scope` and `@starting-style` groups cannot be evaluated here, so their rules
   * are left out rather than guessed (T8.25 §3.7). Unreadable cross-origin sheets go to
   * `unavailableSet`.
   */
  function collectScopeRules(scope, unavailableSet) {
    const ownerDoc = scope.nodeType === 9 ? scope : scope.ownerDocument;
    const scopeWin = ownerDoc.defaultView || g;
    const raw = [];
    const layerOrdinals = new Map();
    const layerCounts = new Map();
    let order = 0;
    let anonymousLayers = 0;

    function mediaApplies(media) {
      const text = media && media.mediaText;
      if (!text) return true;
      try {
        return Boolean(scopeWin.matchMedia(text).matches);
      } catch (_e) {
        return false;
      }
    }

    function supportsApplies(condition) {
      if (!condition) return true;
      try {
        return !scopeWin.CSS || typeof scopeWin.CSS.supports !== "function"
          ? true
          : Boolean(scopeWin.CSS.supports(condition));
      } catch (_e) {
        return false;
      }
    }

    /** Layer path (ordinals by first appearance per parent) for `name`, nested with dots. */
    function layerPathFor(parentPath, name) {
      let path = parentPath;
      const names = name ? String(name).split(".") : [`\u0002anonymous-${anonymousLayers++}`];
      for (const segment of names) {
        const parentKey = path.join(",");
        const key = `${parentKey}\u0000${segment}`;
        if (!layerOrdinals.has(key)) {
          const next = layerCounts.get(parentKey) || 0;
          layerCounts.set(parentKey, next + 1);
          layerOrdinals.set(key, next);
        }
        path = path.concat(layerOrdinals.get(key));
      }
      return path;
    }

    function addStyleRule(rule, layerPath) {
      if (!rule.selectorText || !rule.style) return;
      const decls = parseDeclarations(rule.style.cssText);
      if (decls.length === 0) return;
      const selectors = splitTopLevel(rule.selectorText, ",")
        .map((t) => t.trim())
        .filter(Boolean)
        .map((text) => ({ text, spec: selectorSpecificity(text) }));
      raw.push({ selectors, decls, layer: layerPath, order: order++ });
    }

    function visitImport(rule, layerPath) {
      const sheet = rule.styleSheet;
      if (!sheet || !mediaApplies(rule.media) || !supportsApplies(rule.supportsText)) return;
      const path =
        rule.layerName === null || rule.layerName === undefined
          ? layerPath
          : layerPathFor(layerPath, rule.layerName);
      let rules;
      try {
        rules = sheet.cssRules;
      } catch (_e) {
        if (sheet.href || rule.href) unavailableSet.add(sheet.href || rule.href);
        return;
      }
      if (rules) visit(rules, path);
    }

    function visit(ruleList, layerPath) {
      for (let i = 0; i < ruleList.length; i++) {
        const rule = ruleList[i];
        if (!rule) continue;
        // Realm-independent (rules inside an iframe fail `instanceof`).
        const kind = nativeToString.call(rule).slice(8, -1);
        if (kind === "CSSStyleRule") {
          addStyleRule(rule, layerPath);
        } else if (kind === "CSSMediaRule") {
          if (mediaApplies(rule.media)) visit(rule.cssRules, layerPath);
        } else if (kind === "CSSSupportsRule") {
          if (supportsApplies(rule.conditionText)) visit(rule.cssRules, layerPath);
        } else if (kind === "CSSLayerBlockRule") {
          visit(rule.cssRules, layerPathFor(layerPath, rule.name));
        } else if (kind === "CSSLayerStatementRule") {
          for (const name of rule.nameList || []) layerPathFor(layerPath, name);
        } else if (kind === "CSSImportRule") {
          visitImport(rule, layerPath);
        }
      }
    }

    function visitSheet(sheet) {
      if (!sheet || sheet.disabled || !mediaApplies(sheet.media)) return;
      let rules;
      try {
        rules = sheet.cssRules;
      } catch (_err) {
        if (sheet.href) unavailableSet.add(sheet.href);
        return;
      }
      if (rules) visit(rules, []);
    }

    const sheets = scope.styleSheets ? nativeSlice.call(scope.styleSheets) : [];
    if (scope.adoptedStyleSheets) sheets.push(...nativeSlice.call(scope.adoptedStyleSheets));
    for (const sheet of sheets) visitSheet(sheet);

    // Only a property that some declaration writes with `var()` can carry a ref, so only the rules
    // that write such a property (with any value) or a custom property can decide the winner.
    const varTargets = new Set();
    const noteVarTargets = (decl) => {
      if (!decl.name.startsWith("--") && decl.value.includes("var(")) {
        for (const target of expandProp(decl.name)) varTargets.add(target);
      }
    };
    for (const rule of raw) rule.decls.forEach(noteVarTargets);
    if (scope.querySelectorAll) {
      for (const node of nativeSlice.call(scope.querySelectorAll('[style*="var("]'))) {
        parseDeclarations(node.getAttribute("style")).forEach(noteVarTargets);
      }
    }
    const rules = [];
    for (const rule of raw) {
      const decls = rule.decls.filter(
        (d) => d.name.startsWith("--") || expandProp(d.name).some((t) => varTargets.has(t))
      );
      if (decls.length > 0) rules.push({ ...rule, decls });
    }
    return rules;
  }

  /**
   * Computes `css_vars_declared`, `css_var_refs`, and `declaration_sources` for `el` (T8.25 §3.7).
   * A ref is only emitted for a property whose cascade winner is a `var()` declaration, and a
   * colour winner is cross-checked against `getComputedStyle` so a stale winner is dropped.
   */
  function resolveElementCssVars(el, cs, parentCs, rules, inheritedCustomDefs) {
    const winners = new Map();
    const customWinners = new Map();

    function offer(decl, rank) {
      if (decl.name.startsWith("--")) {
        const prev = customWinners.get(decl.name);
        if (!prev || compareRank(rank, prev.rank) >= 0) {
          customWinners.set(decl.name, { value: decl.value, rank });
        }
        return;
      }
      const shorthand =
        decl.name === "all" || SHORTHAND_EXPANSIONS[decl.name] ? decl.name : null;
      for (const target of expandProp(decl.name)) {
        const prev = winners.get(target);
        if (!prev || compareRank(rank, prev.rank) >= 0) {
          winners.set(target, { value: decl.value, shorthand, rank });
        }
      }
    }

    for (const rule of rules) {
      let spec = -1;
      for (const sel of rule.selectors) {
        try {
          if (sel.spec > spec && el.matches(sel.text)) spec = sel.spec;
        } catch (_e) {
          // Pseudo-element or unsupported selectors never match an element.
        }
      }
      if (spec < 0) continue;
      for (const decl of rule.decls) {
        offer(decl, {
          important: decl.important ? 1 : 0,
          attached: 0,
          layer: rule.layer,
          spec,
          order: rule.order,
        });
      }
    }
    const inline = el.getAttribute("style");
    if (inline) {
      for (const decl of parseDeclarations(inline)) {
        offer(decl, { important: decl.important ? 1 : 0, attached: 1, layer: [], spec: 0, order: 0 });
      }
    }

    const localCustomDefs = Object.assign({}, inheritedCustomDefs);
    for (const [name, entry] of customWinners) localCustomDefs[name] = entry.value;

    // (a) `css_vars_declared`: custom properties whose computed value on `el` differs from parent's.
    const cssVarsDeclared = {};
    for (const varName of customWinners.keys()) {
      const val = cs.getPropertyValue(varName).trim();
      if (!val) continue;
      const parentVal = parentCs ? parentCs.getPropertyValue(varName).trim() : "";
      if (!parentCs || val !== parentVal) {
        cssVarsDeclared[varName] = val;
      }
    }

    // (b) `css_var_refs`: resolve chains `prop -> ["--brand", ...]` for var() winners only.
    const lookup = (name) => cs.getPropertyValue(name).trim();
    const cssVarRefs = {};
    const declarationSources = {};
    for (const [prop, winner] of winners) {
      const rawVal = winner.value;
      const directVars = extractVarNames(rawVal);
      if (directVars.length === 0) continue;
      const resolved = substituteVars(rawVal, lookup);
      const computed = cs.getPropertyValue(prop).trim();
      if (winner.shorthand) {
        // A shorthand feeds several longhands; a longhand only gets the ref when the value the
        // var() resolved to is provably what that longhand computes to.
        if (resolved === null) continue;
        if (winner.shorthand === "background") {
          const isImage = /url\(|gradient\(/i.test(resolved);
          if (prop === "background-image") {
            if (!isImage) continue;
          } else if (prop !== "background-color" || isImage || !sameValue(resolved, computed)) {
            continue;
          }
        } else {
          const words = splitTopLevel(rawVal.trim(), " ").filter(Boolean);
          // A lone var() supplies every longhand, whatever it resolves to (`0.5rem` computes to
          // `8px`). With several words, only a token equal to the longhand's value is attributed.
          const single = words.length === 1 && words[0].startsWith("var(");
          if (!single) {
            const tokens = words
              .filter((t) => t.includes("var("))
              .map((t) => substituteVars(t, lookup));
            if (!tokens.some((t) => t !== null && sameValue(t, computed))) continue;
          }
        }
      } else if (
        resolved !== null &&
        COLOR_PROPS.has(prop) &&
        /^(#[0-9a-f]{3,8}|(rgb|hsl)a?\([^()]*\)|[a-z]+)$/i.test(resolved) &&
        !/^(inherit|initial|unset|revert|revert-layer|currentcolor)$/i.test(resolved) &&
        !sameValue(resolved, computed)
      ) {
        // The winner we picked does not produce the painted colour (for example a rule we could
        // not evaluate overrode it). A wrong ref is worse than none.
        continue;
      }
      const chain = [];
      const visited = new Set();
      const queue = nativeFrom(directVars);
      while (queue.length > 0) {
        const vName = queue.shift();
        if (visited.has(vName)) continue;
        visited.add(vName);
        chain.push(vName);
        const defVal = localCustomDefs[vName];
        if (defVal && defVal.includes("var(")) {
          for (const nextVar of extractVarNames(defVal)) {
            if (!visited.has(nextVar)) {
              queue.push(nextVar);
            }
          }
        }
      }
      if (chain.length > 0) {
        cssVarRefs[prop] = chain;
        declarationSources[prop] = rawVal;
      }
    }

    return { cssVarsDeclared, cssVarRefs, declarationSources, localCustomDefs };
  }

  /**
   * Detects the nearest framework component name on `el` (T8.25 §3.8, E15-44 §3.1).
   */
  function extractComponentHint(el) {
    try {
      const keys = nativeKeys(el);
      // 1. React Fiber (`__reactFiber$*` or `__reactInternalInstance$*`)
      for (const k of keys) {
        if (k.startsWith("__reactFiber$") || k.startsWith("__reactInternalInstance$")) {
          let fiber = el[k];
          let depth = 0;
          while (fiber && depth < 32) {
            depth++;
            const t = fiber.type || fiber.elementType;
            if (t) {
              if (typeof t === "function") {
                const name = t.displayName || t.name;
                if (name) return String(name);
              } else if (typeof t === "object") {
                const name =
                  t.displayName ||
                  t.name ||
                  (t.render && (t.render.displayName || t.render.name)) ||
                  (t.type && (t.type.displayName || t.type.name));
                if (name) return String(name);
              }
            }
            fiber = fiber.return;
          }
        }
      }

      // 2. Vue 3 / Vue 2
      if (el.__vueParentComponent && el.__vueParentComponent.type) {
        const vt = el.__vueParentComponent.type;
        const vName = vt.name || vt.__name;
        if (vName) return String(vName);
      }
      if (el.__vue__ && el.__vue__.$options) {
        const v2Name = el.__vue__.$options.name || el.__vue__.$options._componentTag;
        if (v2Name) return String(v2Name);
      }

      // 3. Svelte dev metadata
      if (el.__svelte_meta) {
        if (el.__svelte_meta.componentName) {
          return String(el.__svelte_meta.componentName);
        }
        if (el.__svelte_meta.loc && el.__svelte_meta.loc.file) {
          const file = String(el.__svelte_meta.loc.file);
          const base = file.split("/").pop().replace(/\.svelte$/i, "");
          if (base) return base;
        }
      }

      // 4. Explicit `data-component` attribute
      const dataComp = el.getAttribute("data-component");
      if (dataComp && dataComp.trim()) {
        return dataComp.trim();
      }

      // 5. Angular (`ng-version` or `_nghost-*` custom element selector)
      const hasNgHost = nativeFrom(el.attributes || []).some(
        (a) => a.name === "ng-version" || a.name.startsWith("_nghost-")
      );
      if (hasNgHost && el.tagName && el.tagName.includes("-")) {
        return el.tagName.toLowerCase();
      }
    } catch (_e) {
      // Ignore property access errors on exotic host objects
    }
    return null;
  }

  function extractStyleSubset(cs, propList) {
    const out = {};
    for (const prop of propList) {
      const val = cs.getPropertyValue(prop);
      if (val !== undefined && val !== null && val !== "") {
        out[prop] = val;
      }
    }
    return out;
  }

  function isSetValue(cs, prop) {
    const v = cs.getPropertyValue(prop);
    return Boolean(v) && v !== "none" && v !== "normal";
  }

  const PX_VALUE = /^-?\d*\.?\d+(e-?\d+)?px$/;

  /**
   * Padding-box origin, in page coordinates, of the box an absolutely positioned child of `el`
   * is placed against: the nearest ancestor-or-self that positions, transforms, filters or
   * contains its descendants; the initial containing block when there is none.
   */
  function containingBlockOrigin(el, position, geo) {
    if (position === "fixed") return { x: geo.offX, y: geo.offY };
    for (let a = el; a; a = a.parentElement || (a.getRootNode && a.getRootNode().host) || null) {
      const acs = geo.elWin.getComputedStyle(a);
      const contains =
        acs.getPropertyValue("position") !== "static" ||
        isSetValue(acs, "transform") ||
        isSetValue(acs, "filter") ||
        isSetValue(acs, "perspective") ||
        /paint|layout|strict|content/.test(acs.getPropertyValue("contain"));
      if (contains) {
        const r = a.getBoundingClientRect();
        return {
          x: r.left + geo.offX + parsePx(acs.getPropertyValue("border-left-width")),
          y: r.top + geo.offY + parsePx(acs.getPropertyValue("border-top-width")),
        };
      }
    }
    return { x: geo.pageOffsetX, y: geo.pageOffsetY };
  }

  /**
   * Where in-flow content of `el` ends: the last in-flow child element or text line, starting from
   * `seed` (the in-flow `::before` box, when there is one).
   */
  function flowEnd(el, geo, seed) {
    let end = seed;
    const doc = el.ownerDocument;
    for (const node of nativeSlice.call(el.childNodes)) {
      if (node.nodeType === 1) {
        const ncs = geo.elWin.getComputedStyle(node);
        const pos = ncs.getPropertyValue("position");
        if (ncs.getPropertyValue("display") === "none" || pos === "absolute" || pos === "fixed") continue;
        const r = node.getBoundingClientRect();
        end = {
          right: r.right + geo.offX,
          top: r.top + geo.offY,
          bottom: r.bottom + geo.offY,
          marginBottom: parsePx(ncs.getPropertyValue("margin-bottom")),
        };
      } else if (node.nodeType === 3 && (node.nodeValue || "").trim()) {
        const range = doc.createRange();
        range.selectNodeContents(node);
        const rects = nativeSlice.call(range.getClientRects()).filter((r) => r.width > 0 || r.height > 0);
        const last = rects[rects.length - 1];
        if (last) {
          end = {
            right: last.right + geo.offX,
            top: last.top + geo.offY,
            bottom: last.bottom + geo.offY,
            marginBottom: 0,
          };
        }
      }
    }
    return end;
  }

  /**
   * Border-box origin of the `::before`/`::after` box of `el`. Absolutely positioned pseudos sit
   * at their used inset from the containing block's padding edge (plus their margin); in-flow
   * ones start the content box (`::before`) or follow the last in-flow item (`::after`).
   */
  function pseudoOrigin(el, which, pcs, geo, beforeEnd) {
    const ml = parsePx(pcs.getPropertyValue("margin-left"));
    const mt = parsePx(pcs.getPropertyValue("margin-top"));
    const position = pcs.getPropertyValue("position");
    if (position === "absolute" || position === "fixed") {
      const left = pcs.getPropertyValue("left");
      const top = pcs.getPropertyValue("top");
      if (PX_VALUE.test(left) && PX_VALUE.test(top)) {
        const cb = containingBlockOrigin(el, position, geo);
        return { x: cb.x + parsePx(left) + ml, y: cb.y + parsePx(top) + mt, inFlow: false };
      }
    }
    const start = geo.contentBox;
    if (which === "::before") return { x: start.x + ml, y: start.y + mt, inFlow: true };
    const end = flowEnd(el, geo, beforeEnd);
    if (!end) return { x: start.x + ml, y: start.y + mt, inFlow: true };
    if (/^inline/.test(pcs.getPropertyValue("display"))) {
      return { x: end.right + ml, y: end.top + mt, inFlow: true };
    }
    return {
      x: start.x + ml,
      y: end.bottom + Math.max(end.marginBottom, mt),
      inFlow: true,
    };
  }

  /**
   * Captures a rendered `::before`/`::after`. Returns `{ pseudo, end }` where `end` is the in-flow
   * extent used to place a following `::after`, or `null` when nothing renders.
   */
  function capturePseudo(el, which, geo, beforeEnd) {
    try {
      const pcs = geo.elWin.getComputedStyle(el, which);
      if (!pcs) return null;
      const content = pcs.getPropertyValue("content");
      const display = pcs.getPropertyValue("display");
      if (display === "none") return null;
      const bgColor = pcs.getPropertyValue("background-color");
      const bgImage = pcs.getPropertyValue("background-image");
      const hasContent =
        content &&
        content !== "none" &&
        content !== "normal" &&
        content !== '""' &&
        content !== "''";
      const hasBox =
        (bgColor && bgColor !== "rgba(0, 0, 0, 0)" && bgColor !== "transparent") ||
        (bgImage && bgImage !== "none");
      if (!hasContent && !hasBox) return null;
      const w = parsePx(pcs.getPropertyValue("width")) || geo.borderBox.width;
      const h = parsePx(pcs.getPropertyValue("height")) || geo.borderBox.height;
      const origin = pseudoOrigin(el, which, pcs, geo, beforeEnd);
      return {
        pseudo: {
          content: hasContent ? content.replace(/^["']|["']$/g, "") : "",
          box: {
            x: roundCoord(origin.x),
            y: roundCoord(origin.y),
            width: roundCoord(w),
            height: roundCoord(h),
          },
          styles: extractStyleSubset(pcs, CAPTURED_STYLE_PROPS),
        },
        end: origin.inFlow
          ? {
              right: origin.x + w,
              top: origin.y,
              bottom: origin.y + h,
              marginBottom: parsePx(pcs.getPropertyValue("margin-bottom")),
            }
          : null,
      };
    } catch (_e) {
      return null;
    }
  }

  function createsStackingContext(cs, position, zApplies, hasZ) {
    return (
      position === "fixed" ||
      position === "sticky" ||
      (zApplies && hasZ) ||
      parseFloat(cs.getPropertyValue("opacity")) < 1 ||
      ["transform", "translate", "rotate", "scale", "filter", "backdrop-filter", "perspective", "clip-path", "mask-image", "container-type"].some(
        (p) => isSetValue(cs, p)
      ) ||
      isSetValue(cs, "mix-blend-mode") ||
      cs.getPropertyValue("isolation") === "isolate" ||
      /layout|paint|strict|content/.test(cs.getPropertyValue("contain")) ||
      /transform|opacity|filter|perspective|clip-path|mask|isolation|mix-blend-mode/.test(
        cs.getPropertyValue("will-change")
      )
    );
  }

  /**
   * Stacking facts of one element used to order paint (T8.25 §3.2): whether it creates a stacking
   * context, its `z-index` level in it, and whether it is positioned.
   */
  function stackingInfo(cs, parentCs, isRoot, embedded) {
    const position = cs.getPropertyValue("position");
    const positioned = position !== "static" && position !== "";
    const parentDisplay = parentCs ? parentCs.getPropertyValue("display") : "";
    const zApplies = positioned || /flex|grid/.test(parentDisplay);
    const zNum = parseInt(cs.getPropertyValue("z-index"), 10);
    const hasZ = zApplies && Number.isFinite(zNum);
    const context = isRoot || createsStackingContext(cs, position, zApplies, hasZ);
    return { context, zi: hasZ ? zNum : 0, hasZ, positioned, embedded };
  }

  /**
   * Replaces every captured node's `z` with its dense paint index, walking stacking contexts in
   * paint order: the context root, negative `z-index` contexts, in-flow content in tree order,
   * positioned `auto`/`0` content in tree order, then positive `z-index` contexts ascending. A
   * context is painted as a unit, so descendants of a `z-index:1` parent stay with that parent.
   * The raw `z-index` value remains in `styles`.
   */
  function assignPaintOrder(root, metaOf) {
    let counter = 0;
    const byLevel = (a, b) => a.zi - b.zi;

    function gather(children, flow, layers) {
      for (const child of children) {
        const meta = metaOf(child);
        if (meta.context) {
          const item = { node: child, context: true, zi: meta.zi, flow: null };
          if (meta.embedded || (!meta.positioned && !meta.hasZ)) flow.push(item);
          else if (meta.zi < 0) layers.neg.push(item);
          else if (meta.zi > 0) layers.pos.push(item);
          else layers.zero.push(item);
        } else if (meta.positioned) {
          // Painted atomically in the parent context, but its in-flow content paints with it.
          const item = { node: child, context: false, zi: 0, flow: [] };
          layers.zero.push(item);
          gather(child.children || [], item.flow, layers);
        } else {
          flow.push({ node: child, context: false, zi: 0, flow: null });
          gather(child.children || [], flow, layers);
        }
      }
    }

    function emit(item) {
      if (item.context) {
        paintContext(item.node);
      } else {
        item.node.z = counter++;
        for (const inner of item.flow || []) emit(inner);
      }
    }

    function paintContext(node) {
      node.z = counter++;
      const layers = { neg: [], zero: [], pos: [] };
      const flow = [];
      gather(node.children || [], flow, layers);
      layers.neg.sort(byLevel);
      layers.pos.sort(byLevel);
      for (const item of layers.neg) emit(item);
      for (const item of flow) emit(item);
      for (const item of layers.zero) emit(item);
      for (const item of layers.pos) emit(item);
    }

    paintContext(root);
  }

  function captureTextRunsForElement(el, cs, offsetX, offsetY, stats) {
    const runs = [];
    const doc = el.ownerDocument;
    const childNodes = el.childNodes ? nativeSlice.call(el.childNodes) : [];
    for (const node of childNodes) {
      if (!node || node.nodeType !== 3) continue; // Node.TEXT_NODE === 3
      const rawText = node.nodeValue || "";
      if (!rawText.trim()) continue;
      const range = doc.createRange();
      range.selectNodeContents(node);
      const clientRects = range.getClientRects ? nativeSlice.call(range.getClientRects()) : [];
      const lines = [];
      let linesTruncated = false;
      for (let i = 0; i < clientRects.length; i++) {
        const r = clientRects[i];
        if (r.width <= 0 && r.height <= 0) continue;
        if (lines.length >= 2000) {
          linesTruncated = true;
          break;
        }
        lines.push({
          x: roundCoord(r.left + offsetX),
          y: roundCoord(r.top + offsetY),
          width: roundCoord(r.width),
          height: roundCoord(r.height),
        });
      }
      const run = {
        text: rawText,
        styles: extractStyleSubset(cs, TEXT_RUN_STYLE_PROPS),
        lines,
      };
      if (linesTruncated) {
        run.lines_truncated = true;
      }
      runs.push(run);
      stats.text_runs++;
    }
    return runs;
  }

  async function waitWithTimeout(promise, timeoutMs) {
    let timer = null;
    const timeoutPromise = new Promise((resolve) => {
      timer = setTimeout(() => resolve("timeout"), timeoutMs);
    });
    try {
      return await Promise.race([promise.then(() => "ok").catch(() => "error"), timeoutPromise]);
    } finally {
      if (timer !== null) clearTimeout(timer);
    }
  }

  async function fetchAsDataUrl(url, timeoutMs) {
    if (!nativeFetch) return null;
    const controller = typeof AbortController !== "undefined" ? new AbortController() : null;
    const timer = controller ? setTimeout(() => controller.abort(), timeoutMs) : null;
    try {
      const resp = await nativeFetch(url, {
        mode: "cors",
        signal: controller ? controller.signal : undefined,
      });
      if (!resp || !resp.ok) return null;
      const blob = await resp.blob();
      return await new Promise((resolve) => {
        const reader = new FileReader();
        reader.onloadend = () =>
          resolve(typeof reader.result === "string" ? reader.result : null);
        reader.onerror = () => resolve(null);
        reader.readAsDataURL(blob);
      });
    } catch (_e) {
      return null;
    } finally {
      if (timer !== null) clearTimeout(timer);
    }
  }

  function drawImageElementToPngDataUrl(img, doc) {
    try {
      const w = img.naturalWidth || img.width || 0;
      const h = img.naturalHeight || img.height || 0;
      if (w <= 0 || h <= 0) return null;
      const canvas = doc.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d");
      if (!ctx) return null;
      ctx.drawImage(img, 0, 0, w, h);
      return canvas.toDataURL("image/png");
    } catch (_e) {
      return null;
    }
  }

  async function captureImageFromUrl(src, imgEl, kind, doc, budgetState, options) {
    if (!src) return null;
    const timeoutMs = options.image_timeout_ms ?? 3000;

    if (src.startsWith("data:")) {
      const byteLen = src.length;
      if (budgetState.imageBytes + byteLen > budgetState.maxImageBytes) {
        return { src, skipped: "size limit", kind };
      }
      budgetState.imageBytes += byteLen;
      budgetState.images++;
      return { src, data_url: src, kind };
    }

    if (imgEl && !imgEl.complete && typeof imgEl.decode === "function") {
      const status = await waitWithTimeout(imgEl.decode(), timeoutMs);
      if (status === "timeout" && !imgEl.complete) {
        budgetState.images++;
        return { src, pending: true, kind };
      }
    }

    let dataUrl = imgEl ? drawImageElementToPngDataUrl(imgEl, doc) : null;
    if (!dataUrl) {
      dataUrl = await fetchAsDataUrl(src, Math.min(timeoutMs, 1500));
    }

    if (!dataUrl) {
      budgetState.images++;
      return { src, blocked: true, kind };
    }

    const byteLen = dataUrl.length;
    if (budgetState.imageBytes + byteLen > budgetState.maxImageBytes) {
      return { src, skipped: "size limit", kind };
    }
    budgetState.imageBytes += byteLen;
    budgetState.images++;
    return { src, data_url: dataUrl, kind };
  }

  function extractCssUrls(bgImageValue) {
    if (!bgImageValue || bgImageValue === "none") return [];
    const urls = [];
    const re = /url\(\s*(['"]?)(.*?)\1\s*\)/gi;
    let m;
    while ((m = re.exec(bgImageValue)) !== null) {
      if (m[2]) urls.push(m[2]);
    }
    return urls;
  }

  function collectFonts(doc) {
    const out = [];
    const seen = new Set();
    try {
      if (doc.fonts && typeof doc.fonts.forEach === "function") {
        doc.fonts.forEach((face) => {
          const family = String(face.family || "").replace(/^["']|["']$/g, "");
          const weight = String(face.weight || "400");
          const style = String(face.style || "normal");
          const key = `${family}|${weight}|${style}`;
          if (!family || seen.has(key)) return;
          seen.add(key);
          out.push({ family, weight, style });
        });
      }
    } catch (_e) {
      // Ignore font iteration failures
    }
    return out;
  }

  async function __designosCapture(options = {}) {
    const startTime = nativeNow();
    const doc = document;
    const win = g;
    const includeHidden = Boolean(options.include_hidden);
    const maxElements =
      typeof options.max_elements === "number" && options.max_elements > 0
        ? Math.floor(options.max_elements)
        : 20000;
    const maxImageBytes =
      typeof options.max_image_bytes === "number" && options.max_image_bytes >= 0
        ? options.max_image_bytes
        : 50 * 1024 * 1024;

    const rootEl =
      typeof options.root === "string"
        ? doc.querySelector(options.root) || doc.documentElement
        : options.root instanceof Element
        ? options.root
        : doc.documentElement;

    const unavailableSet = new Set();
    // Author rules are indexed per tree scope: the document, each same-origin iframe document and
    // each open shadow root read their own stylesheets.
    const scopeRuleCache = new Map();
    function rulesFor(el) {
      const scope = el.getRootNode ? el.getRootNode() : el.ownerDocument;
      const key = scope && (scope.nodeType === 9 || scope.nodeType === 11) ? scope : el.ownerDocument;
      if (!scopeRuleCache.has(key)) {
        scopeRuleCache.set(key, collectScopeRules(key, unavailableSet));
      }
      return scopeRuleCache.get(key);
    }

    const stats = {
      elements: 0,
      skipped_elements: 0,
      text_runs: 0,
      images: 0,
      image_bytes: 0,
      elapsed_ms: 0,
    };
    const budgetState = {
      maxElements,
      maxImageBytes,
      imageBytes: 0,
      images: 0,
      truncated: false,
    };

    // Stacking facts per captured node, consumed once by `assignPaintOrder` after the walk.
    const stackingOf = new Map();

    /** Elements `walkElement` would capture under `el`; counts what the budget leaves unvisited. */
    function countSubtree(el) {
      const tag = (el.tagName || "div").toLowerCase();
      if (SKIPPED_TAGS.has(tag)) return 0;
      const elWin = el.ownerDocument.defaultView || win;
      const cs = elWin.getComputedStyle(el);
      const isHidden =
        cs.getPropertyValue("display") === "none" || cs.getPropertyValue("visibility") === "hidden";
      if (isHidden && !includeHidden) return 0;
      let total = 1;
      if (tag === "svg") return total;
      if (el.shadowRoot) {
        for (const child of nativeSlice.call(el.shadowRoot.children || [])) {
          total += countSubtree(child);
        }
      }
      if (tag === "iframe") {
        let iframeDoc = null;
        try {
          iframeDoc = el.contentDocument;
        } catch (_e) {
          iframeDoc = null;
        }
        if (iframeDoc && iframeDoc.documentElement) total += countSubtree(iframeDoc.documentElement);
      } else {
        for (const child of nativeSlice.call(el.children || [])) total += countSubtree(child);
      }
      return total;
    }

    async function walkElement(
      el,
      parentCs,
      inheritedCustomDefs,
      pageOffsetX,
      pageOffsetY,
      embedded = false
    ) {
      const tag = (el.tagName || "div").toLowerCase();
      if (SKIPPED_TAGS.has(tag)) {
        return null;
      }

      const elWin = el.ownerDocument.defaultView || win;
      const cs = elWin.getComputedStyle(el);
      const isHidden =
        cs.getPropertyValue("display") === "none" ||
        cs.getPropertyValue("visibility") === "hidden";
      if (isHidden && !includeHidden) {
        return null;
      }

      if (stats.elements >= budgetState.maxElements) {
        budgetState.truncated = true;
        stats.skipped_elements += countSubtree(el);
        return null;
      }
      stats.elements++;

      // Boxes are `getBoundingClientRect` plus scroll offsets for every element, `position: fixed`
      // and `sticky` included (T8.25 §3.3, §8.3).
      const scrollX = elWin.scrollX || elWin.pageXOffset || 0;
      const scrollY = elWin.scrollY || elWin.pageYOffset || 0;
      const offX = pageOffsetX + scrollX;
      const offY = pageOffsetY + scrollY;

      const rect = el.getBoundingClientRect();
      const borderBox = {
        x: roundCoord(rect.left + offX),
        y: roundCoord(rect.top + offY),
        width: roundCoord(rect.width),
        height: roundCoord(rect.height),
      };

      const bt = parsePx(cs.getPropertyValue("border-top-width"));
      const br = parsePx(cs.getPropertyValue("border-right-width"));
      const bb = parsePx(cs.getPropertyValue("border-bottom-width"));
      const bl = parsePx(cs.getPropertyValue("border-left-width"));
      const pt = parsePx(cs.getPropertyValue("padding-top"));
      const pr = parsePx(cs.getPropertyValue("padding-right"));
      const pb = parsePx(cs.getPropertyValue("padding-bottom"));
      const pl = parsePx(cs.getPropertyValue("padding-left"));

      const contentBox = {
        x: roundCoord(borderBox.x + bl + pl),
        y: roundCoord(borderBox.y + bt + pt),
        width: roundCoord(Math.max(0, borderBox.width - bl - pl - br - pr)),
        height: roundCoord(Math.max(0, borderBox.height - bt - pt - bb - pb)),
      };

      // `z` is filled in by `assignPaintOrder` once the whole tree is known.
      const z = 0;
      const stacking = stackingInfo(cs, parentCs, el === rootEl || embedded, embedded);

      const styles = extractStyleSubset(cs, CAPTURED_STYLE_PROPS);
      const { cssVarsDeclared, cssVarRefs, declarationSources, localCustomDefs } =
        resolveElementCssVars(el, cs, parentCs, rulesFor(el), inheritedCustomDefs);
      const componentHint = extractComponentHint(el);

      const geometry = { elWin, borderBox, contentBox, offX, offY, pageOffsetX, pageOffsetY };
      const beforeResult = capturePseudo(el, "::before", geometry, null);
      const afterResult = capturePseudo(el, "::after", geometry, beforeResult && beforeResult.end);
      const pseudo =
        beforeResult || afterResult
          ? {
              ...(beforeResult ? { before: beforeResult.pseudo } : {}),
              ...(afterResult ? { after: afterResult.pseudo } : {}),
            }
          : undefined;

      const textRuns =
        tag === "svg" ? [] : captureTextRunsForElement(el, cs, offX, offY, stats);

      let primaryImage = null;
      const extraImages = [];

      if (tag === "img") {
        const src = el.currentSrc || el.getAttribute("src") || "";
        primaryImage = await captureImageFromUrl(
          src,
          el,
          "img",
          el.ownerDocument,
          budgetState,
          options
        );
      } else if (tag === "canvas" && typeof el.toDataURL === "function") {
        try {
          const dataUrl = el.toDataURL("image/png");
          if (budgetState.imageBytes + dataUrl.length <= budgetState.maxImageBytes) {
            budgetState.imageBytes += dataUrl.length;
            budgetState.images++;
            primaryImage = { src: "canvas", data_url: dataUrl, kind: "canvas" };
          } else {
            primaryImage = { src: "canvas", skipped: "size limit", kind: "canvas" };
          }
        } catch (_e) {
          primaryImage = { src: "canvas", blocked: true, kind: "canvas" };
        }
      } else if (tag === "svg") {
        const svgHtml = el.outerHTML || "";
        budgetState.images++;
        primaryImage = { src: "inline-svg", svg: svgHtml, kind: "svg" };
      } else if (tag === "video") {
        const poster = el.getAttribute("poster");
        if (poster) {
          primaryImage = await captureImageFromUrl(
            poster,
            null,
            "video-poster",
            el.ownerDocument,
            budgetState,
            options
          );
        }
      }

      const bgUrls = extractCssUrls(styles["background-image"]);
      for (const bgUrl of bgUrls) {
        const bgRef = await captureImageFromUrl(
          bgUrl,
          null,
          "background",
          el.ownerDocument,
          budgetState,
          options
        );
        if (bgRef) {
          if (!primaryImage) {
            primaryImage = bgRef;
          } else {
            extraImages.push(bgRef);
          }
        }
      }

      const children = [];
      let hasOpenShadow = false;
      const hasClosedShadow = Boolean(el.__designosClosedShadow);

      if (tag !== "svg") {
        if (el.shadowRoot) {
          hasOpenShadow = true;
          const shadowChildren = el.shadowRoot.children
            ? nativeSlice.call(el.shadowRoot.children)
            : [];
          for (const sc of shadowChildren) {
            const childCap = await walkElement(sc, cs, localCustomDefs, pageOffsetX, pageOffsetY);
            if (childCap) children.push(childCap);
          }
        }

        if (tag === "iframe") {
          let iframeDoc = null;
          try {
            iframeDoc = el.contentDocument;
          } catch (_e) {
            iframeDoc = null;
          }
          if (iframeDoc && iframeDoc.documentElement) {
            // A frame is its own browsing context: no parent style, no inherited custom
            // properties, and its content paints as part of the frame element.
            const subCap = await walkElement(
              iframeDoc.documentElement,
              null,
              {},
              borderBox.x,
              borderBox.y,
              true
            );
            if (subCap) children.push(subCap);
          } else if (el.getAttribute("src")) {
            const opaque = {
              tag,
              ...(el.id ? { id: el.id } : {}),
              classes: el.classList ? nativeFrom(el.classList) : [],
              box: borderBox,
              content_box: contentBox,
              z,
              visible: !isHidden,
              styles,
              cross_origin: true,
              children: [],
            };
            stackingOf.set(opaque, stacking);
            return opaque;
          }
        } else {
          const domChildren = el.children ? nativeSlice.call(el.children) : [];
          for (const child of domChildren) {
            const childCap = await walkElement(
              child,
              cs,
              localCustomDefs,
              pageOffsetX,
              pageOffsetY
            );
            if (childCap) children.push(childCap);
          }
        }
      }

      const out = {
        tag,
        ...(el.id ? { id: el.id } : {}),
        classes: el.classList ? nativeFrom(el.classList) : [],
        ...(el.getAttribute("role") ? { role: el.getAttribute("role") } : {}),
        ...(el.getAttribute("aria-label") ? { aria_label: el.getAttribute("aria-label") } : {}),
        ...(el.getAttribute("alt") ? { alt: el.getAttribute("alt") } : {}),
        box: borderBox,
        content_box: contentBox,
        z,
        visible: !isHidden,
        styles,
        ...(pseudo ? { pseudo } : {}),
        ...(textRuns.length > 0 ? { text_runs: textRuns } : {}),
        ...(primaryImage ? { image: primaryImage } : {}),
        ...(extraImages.length > 0 ? { images: extraImages } : {}),
        ...(nativeKeys(cssVarsDeclared).length > 0
          ? { css_vars_declared: cssVarsDeclared }
          : {}),
        ...(nativeKeys(cssVarRefs).length > 0 ? { css_var_refs: cssVarRefs } : {}),
        ...(nativeKeys(declarationSources).length > 0
          ? { declaration_sources: declarationSources }
          : {}),
        ...(componentHint
          ? { component_hint: componentHint, framework_name: componentHint }
          : {}),
        ...(hasOpenShadow ? { shadow: true } : {}),
        ...(hasClosedShadow ? { closed_shadow: true } : {}),
        children,
      };
      stackingOf.set(out, stacking);
      return out;
    }

    const rootCapture =
      (await walkElement(rootEl, null, {}, 0, 0)) || {
        tag: "html",
        classes: [],
        box: { x: 0, y: 0, width: 0, height: 0 },
        content_box: { x: 0, y: 0, width: 0, height: 0 },
        z: 0,
        visible: true,
        styles: {},
        children: [],
      };

    const plainFlow = { context: false, zi: 0, positioned: false, embedded: false };
    assignPaintOrder(rootCapture, (node) => stackingOf.get(node) || plainFlow);

    stats.images = budgetState.images;
    stats.image_bytes = budgetState.imageBytes;
    stats.elapsed_ms = Math.round((nativeNow() - startTime) * 100) / 100;

    return {
      schema: "designos-webcapture",
      version: 1,
      url: String(doc.location ? doc.location.href : ""),
      title: String(doc.title || ""),
      viewport: {
        width: win.innerWidth || doc.documentElement.clientWidth || 0,
        height: win.innerHeight || doc.documentElement.clientHeight || 0,
        dpr: win.devicePixelRatio || 1,
      },
      scroll: {
        width: Math.max(
          doc.documentElement ? doc.documentElement.scrollWidth : 0,
          doc.body ? doc.body.scrollWidth : 0
        ),
        height: Math.max(
          doc.documentElement ? doc.documentElement.scrollHeight : 0,
          doc.body ? doc.body.scrollHeight : 0
        ),
      },
      captured_at: new Date().toISOString(),
      fonts: collectFonts(doc),
      ...(unavailableSet.size > 0
        ? { css_vars_unavailable: nativeFrom(unavailableSet) }
        : {}),
      root: rootCapture,
      truncated: budgetState.truncated,
      stats,
    };
  }

  g.__designosCapture = __designosCapture;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = { __designosCapture };
  }
})();
