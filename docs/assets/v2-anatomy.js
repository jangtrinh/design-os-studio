/* v2-anatomy.js: scene 3, "Five layers, one window."
   A Tenant section: the engine pins it with position:sticky, progress comes from the section's own bounding
   box, and this module only writes inside the section. Six hairline drawings, traced from the real Studio
   0.7.5 render shown in the static state, draw on, tilt and lift apart with a leader line to each label,
   settle back flat, and the real render cross-fades in. One pure render(p) draws every frame.
   Motion mode only (no reduced-motion preference, viewport of 800px or more): the drawings and
   anatomy.json are fetched at mount, so the static state never downloads them. If they cannot be fetched
   the scene falls back to the static state. The narrator follows the drawing's on-screen box. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};
  V2.scenes = V2.scenes || [];

  var TRAVEL = 130;                                                    // pinned scroll distance, in percent of the viewport height
  var BASE = 'images/v2/anatomy/';
  var ORDER = ['file', 'canvas', 'layers', 'inspector', 'rail', 'chrome']; // bottom to top; chrome has no label
  var TILT_X = 52, TURN_Z = -24, END_SCALE = 0.82, LIFT = 70;          // the exploded pose
  var LIFT_REF_W = 760;                                                // frame width the 70 px lift is authored for
  var SHIFT_X = -0.045;                                                // the exploded stack moves left by this share of the frame width
  var LEAD = 18, GAP = 8, STACK_GAP = 16, STAGE_BOTTOM = 24;                                              // leader shoulder length; leader end to label text
  var DIM = 0.5;                                                       // word alpha before it is read (4.9:1 on --bg)
  var SVGNS = 'http://www.w3.org/2000/svg';
  var RAD = Math.PI / 180;

  var section, scene, frame, stack, img, lede, labelsList, labelEls;
  var pin = null, layerEls = [], anchorEls = [], leaders = [], overlay = null;
  var words = [], ledeHTML = '';
  var data = null, ready = false, token = 0;
  var p = 0, settled = false, lastT = -1, dirty = true, memo = new WeakMap();
  var k = 1, frameW = 0, watcher = null;

  // ---- write helpers: every style write skips an unchanged value ----
  function put(el, prop, val) {
    var m = memo.get(el);
    if (!m) { m = {}; memo.set(el, m); }
    if (m[prop] === val) return;
    m[prop] = val;
    if (prop.charAt(0) === '-') el.style.setProperty(prop, val); else el.style[prop] = val;
  }
  function putAttr(el, name, val) {
    var m = memo.get(el);
    if (!m) { m = {}; memo.set(el, m); }
    var key = '@' + name;
    if (m[key] === val) return;
    m[key] = val;
    el.setAttribute(name, val);
  }
  function clearStyle(e) { // after CSSOM writes a bare removeAttribute leaves style="" behind; setting it first does not
    if (!e) return;
    e.setAttribute('style', '');
    e.removeAttribute('style');
  }
  function r1(n) { return Math.round(n * 10) / 10; }
  function el(tag, cls) { var e = document.createElement(tag); if (cls) e.className = cls; return e; }
  function svgEl(name, attrs) {
    var e = document.createElementNS(SVGNS, name);
    Object.keys(attrs || {}).forEach(function (key) { e.setAttribute(key, attrs[key]); });
    return e;
  }

  // ---- the side paragraph reads word by word ----
  function wrapWords() {
    ledeHTML = lede.innerHTML;
    var text = lede.textContent;
    lede.textContent = '';
    text.split(/(\s+)/).forEach(function (tok) {
      if (!tok) return;
      if (/^\s+$/.test(tok)) { lede.appendChild(document.createTextNode(tok)); return; }
      var w = el('span', 'anatomy__w');
      w.textContent = tok;
      lede.appendChild(w);
      words.push(w);
    });
  }

  // ---- pure render: everything on screen is a function of p ----
  function render(t, force) {
    t = Math.round(t * 1000) / 1000;
    if (!force && !dirty && t === lastT) return;
    lastT = t; dirty = false;
    var sm = V2.smooth;
    var up = sm((t - 0.18) / 0.32);                  // 0.18-0.50 tilt and explode
    var down = sm((t - 0.72) / 0.20);                // 0.72-0.92 reassemble
    var a = up * (1 - down);                         // 0 flat, 1 exploded
    var cross = sm((t - 0.86) / 0.14);               // 0.86-1 the real render replaces the drawing

    // drawing: draw-on 0-0.18, layer by layer, bottom to top
    var i;
    for (i = 0; i < layerEls.length; i++) {
      put(layerEls[i], '--d', (1 - sm((t - i * 0.016) / 0.10)).toFixed(3));
      put(layerEls[i], 'opacity', (1 - cross).toFixed(3));
      put(layerEls[i], 'transform', a < 0.0005 ? 'none' : 'translateZ(' + (i * LIFT * k * a).toFixed(1) + 'px)');
    }
    put(img, 'opacity', cross.toFixed(3));

    // the stack: tilt, turn and shrink, with the pile centred on the frame
    var shiftY = LIFT * 2.5 * Math.sin(TILT_X * RAD) * k * a;
    var shiftX = SHIFT_X * frameW * a;
    put(stack, 'transform', a < 0.0005 ? 'none'
      : 'translate3d(' + shiftX.toFixed(1) + 'px,' + shiftY.toFixed(1) + 'px,0) rotateX(' + (TILT_X * a).toFixed(2) + 'deg) rotateZ(' + (TURN_Z * a).toFixed(2) + 'deg) scale(' + (1 - (1 - END_SCALE) * a).toFixed(3) + ')');

    // labels and leaders: in 0.30-0.55 (staggered), out 0.72-0.84
    var out = sm((t - 0.72) / 0.12);
    var any = false;
    for (i = 0; i < leaders.length; i++) {
      var L = leaders[i];
      var inn = sm((t - (0.30 + i * 0.04)) / 0.09);
      var v = inn * (1 - out);
      put(L.label, 'opacity', v.toFixed(3));
      put(L.dot, 'opacity', v.toFixed(3));
      put(L.path, 'opacity', v.toFixed(3));
      put(L.path, 'strokeDashoffset', (1 - inn).toFixed(3));
      L.on = v > 0.001;
      any = any || L.on;
    }
    if (any && ready) placeLeaders(); // measured after the transforms above, so the browser has done the 3D projection

    // the side paragraph: 0.05-0.75, word by word
    var n = words.length, read = V2.clamp((t - 0.05) / 0.70);
    for (i = 0; i < n; i++) {
      var wp = V2.clamp((read * (n + 3) - i) / 3);
      put(words[i], 'color', wp <= 0 ? '' : wp >= 1 ? 'var(--text)' : 'rgba(244,244,245,' + (Math.round((DIM + (1 - DIM) * wp) * 50) / 50) + ')');
    }
  }

  // leader from the layer's projected anchor to its label. An anchor beside the label gets a diagonal and a
  // short horizontal shoulder into the label's near edge; an anchor straight above or below it gets a vertical.
  function placeLeaders() {
    var sr = scene.getBoundingClientRect();
    for (var j = 0; j < leaders.length; j++) {
      var L = leaders[j];
      if (!L.on) continue;
      var ar = L.anchor.getBoundingClientRect();
      var ax = ar.left + ar.width / 2, ay = ar.top + ar.height / 2;
      var lr = L.label.getBoundingClientRect(), tr = L.title.getBoundingClientRect();
      var d, px = ax - sr.left, py = ay - sr.top;
      if (ax > lr.left && ax < lr.right) {
        var ey = (ay > lr.bottom ? lr.bottom + GAP : lr.top - GAP) - sr.top;
        d = 'M' + r1(px) + ' ' + r1(py) + 'L' + r1(px) + ' ' + r1(ey);
      } else {
        var right = lr.left + lr.width / 2 > ax;
        var ex = (right ? lr.left - GAP : lr.right + GAP) - sr.left, ty = tr.top + tr.height / 2 - sr.top;
        d = 'M' + r1(px) + ' ' + r1(py) + 'L' + r1(ex + (right ? -LEAD : LEAD)) + ' ' + r1(ty) + 'L' + r1(ex) + ' ' + r1(ty);
      }
      putAttr(L.path, 'd', d);
      putAttr(L.dot, 'cx', String(r1(px)));
      putAttr(L.dot, 'cy', String(r1(py)));
    }
  }

  function tick() {
    if (!pin) return;
    if (!pin.armed()) { // offscreen: rest at the nearer end so a fast scroll never leaves a half-state
      if (!settled) { render(p > 0.5 ? 1 : 0, true); settled = true; }
      return;
    }
    settled = false;
    p = pin.progress();
    render(p);
  }

  // Slots (--y in the css) are where a label wants to sit. Labels on one side of the frame are then pushed
  // down so none overlaps another, or the copy above them, and the group is lifted if it would pass the stage bottom.
  function layoutLabels() {
    var fr = frame.getBoundingClientRect(), sr = scene.getBoundingClientRect();
    var cx = fr.left + fr.width / 2, copy = lede.getBoundingClientRect();
    var sides = { left: [], right: [] };
    labelEls.forEach(function (label) {
      var want = parseFloat(getComputedStyle(label).getPropertyValue('--y')) * fr.height;
      var lr = label.getBoundingClientRect();
      sides[lr.left + lr.width / 2 < cx ? 'left' : 'right'].push({ label: label, want: want, h: label.offsetHeight, x0: lr.left, x1: lr.right });
    });
    ['left', 'right'].forEach(function (side) {
      var g = sides[side].sort(function (a, b) { return a.want - b.want; });
      if (!g.length) return;
      var overlapsCopy = g.some(function (e) { return e.x1 > copy.left && e.x0 < copy.right; });
      var min = overlapsCopy ? copy.bottom + STACK_GAP - fr.top : -Infinity;
      var top = min, i;
      for (i = 0; i < g.length; i++) { g[i].top = Math.max(g[i].want, i ? top : min); top = g[i].top + g[i].h + STACK_GAP; }
      var over = g[g.length - 1].top + g[g.length - 1].h - (sr.bottom - STAGE_BOTTOM - fr.top);
      if (over > 0) {
        var room = g[0].top - min;
        var lift = Math.min(over, isFinite(room) ? Math.max(0, room) : over);
        g.forEach(function (e) { e.top -= lift; });
      }
      g.forEach(function (e) { put(e.label, 'top', r1(e.top) + 'px'); });
    });
  }

  function onResize() {
    scene.style.setProperty('--vw', document.documentElement.clientWidth + 'px'); // viewport width without the scrollbar
    frameW = frame.offsetWidth;
    k = Math.min(1, frameW / LIFT_REF_W);
    layoutLabels();
    dirty = true;
  }

  // ---- narrator stop: the drawing's on-screen box ----
  var stop = {
    id: 'anatomy', order: 35, pad: 6,
    test: function (vh) {
      if (!pin || !pin.armed()) return false;
      var r = section.getBoundingClientRect();
      return r.top <= vh * 0.6 && r.bottom >= vh * 0.45;
    },
    rect: function () {
      var l = Infinity, t = Infinity, r = -Infinity, b = -Infinity;
      for (var i = 0; i < layerEls.length; i++) {
        var q = layerEls[i].getBoundingClientRect();
        if (q.left < l) l = q.left;
        if (q.top < t) t = q.top;
        if (q.right > r) r = q.right;
        if (q.bottom > b) b = q.bottom;
      }
      return { left: l, top: t, right: r, bottom: b, width: r - l, height: b - t };
    }
  };

  // ---- build and tear down (inside the section only) ----
  function build() {
    frame.appendChild(labelsList); // slots are shares of the frame; teardown puts the list back last in the grid
    layerEls = ORDER.map(function (id) {
      var layer = el('div', 'anatomy__layer');
      layer.setAttribute('data-layer', id);
      layer.setAttribute('aria-hidden', 'true');
      stack.appendChild(layer);
      return layer;
    });
    overlay = svgEl('svg', { 'class': 'anatomy__leaders', 'aria-hidden': 'true', focusable: 'false' });
    scene.appendChild(overlay);
    wrapWords();
  }

  function inject(d) {
    var byId = {};
    d.json.layers.forEach(function (l) { byId[l.id] = l; });
    var vb = d.json.viewBox;
    layerEls.forEach(function (layer, i) {
      layer.insertAdjacentHTML('afterbegin', d.svgs[i]);
      var svg = layer.querySelector('svg');
      svg.setAttribute('aria-hidden', 'true');
      svg.setAttribute('focusable', 'false');
      Array.prototype.forEach.call(svg.querySelectorAll('path'), function (path) { path.setAttribute('pathLength', '1'); });
      var meta = byId[ORDER[i]];
      if (!meta || !meta.anchor) return;
      var anchor = el('i', 'anatomy__anchor'); // the browser projects this point through the 3D transform
      anchor.style.left = (meta.anchor[0] / vb[2] * 100) + '%';
      anchor.style.top = (meta.anchor[1] / vb[3] * 100) + '%';
      layer.appendChild(anchor);
      anchorEls.push({ id: ORDER[i], el: anchor });
    });
    leaders = labelEls.map(function (label) {
      var a = anchorEls.filter(function (x) { return x.id === label.getAttribute('data-layer'); })[0];
      if (!a) return null;
      var path = svgEl('path', { 'class': 'anatomy__leader', pathLength: '1', d: 'M0 0' });
      var dot = svgEl('circle', { 'class': 'anatomy__dot', r: '2', cx: '0', cy: '0' });
      overlay.appendChild(path); overlay.appendChild(dot);
      return { label: label, title: label.querySelector('strong'), anchor: a.el, path: path, dot: dot, on: false };
    }).filter(Boolean);
  }

  function fetchAll() {
    var get = function (url, as) {
      return fetch(url).then(function (r) { if (!r.ok) throw new Error(url + ' ' + r.status); return as === 'json' ? r.json() : r.text(); });
    };
    return Promise.all([get(BASE + 'anatomy.json', 'json')].concat(ORDER.map(function (id) { return get(BASE + id + '.svg', 'text'); })))
      .then(function (all) { return { json: all[0], svgs: all.slice(1) }; });
  }

  function teardown() {
    token++;
    window.removeEventListener('resize', onResize);
    if (watcher) { watcher.disconnect(); watcher = null; }
    if (window.__scrubTicker) window.__scrubTicker.remove(tick);
    if (V2.narrator) V2.narrator.remove('anatomy');
    if (overlay && overlay.parentNode) overlay.parentNode.removeChild(overlay);
    layerEls.forEach(function (l) { if (l.parentNode) l.parentNode.removeChild(l); });
    if (labelsList) scene.appendChild(labelsList);
    if (lede) lede.innerHTML = ledeHTML;
    [scene, lede, stack, img].concat(labelEls || []).forEach(clearStyle);
    pin.unpin(); pin = null;
    if (!section.style.length) clearStyle(section); // unpin leaves an empty style attribute behind
    layerEls = []; anchorEls = []; leaders = []; words = []; overlay = null; ready = false;
  }

  V2.scenes.push({
    mount: function () {
      section = document.getElementById('anatomy');
      scene = section && section.querySelector('.anatomy-grid');
      if (!scene || !window.mountScrubSection || !window.fetch) return;
      frame = scene.querySelector('.anatomy__frame');
      stack = scene.querySelector('.anatomy__stack');
      img = scene.querySelector('.anatomy__render');
      lede = scene.querySelector('.anatomy__lede');
      labelsList = scene.querySelector('.anatomy__labels');
      labelEls = Array.prototype.slice.call(scene.querySelectorAll('.anatomy__label'));
      pin = V2.pinScene(section, scene, TRAVEL);
      memo = new WeakMap();
      build();
      p = 0; settled = false; lastT = -1; dirty = true; ready = false;
      onResize();
      window.addEventListener('resize', onResize);
      if (window.ResizeObserver) {
        watcher = new ResizeObserver(onResize); // text reflow after the web font loads changes label heights
        labelEls.forEach(function (l) { watcher.observe(l); });
        watcher.observe(lede);
      }
      window.__scrubTicker.add(tick);
      if (V2.narrator) V2.narrator.add(stop);
      var mine = ++token;
      (data ? Promise.resolve(data) : fetchAll().then(function (d) { data = d; return d; }))
        .then(function (d) {
          if (mine !== token || !pin) return;
          inject(d); ready = true; dirty = true;
        })
        .catch(function () { if (mine === token && pin) teardown(); }); // drawings unavailable: keep the static state
    },
    unmount: function () {
      if (!pin) return;
      teardown();
    }
  });
})();
