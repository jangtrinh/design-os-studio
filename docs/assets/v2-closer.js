/* v2-closer.js: the closing call to action and the typographic closer.
   - Download burst: a click on the closing Download button lets the browser start the download as usual
      (no preventDefault, nothing awaited) and, in the same event, four hairline handles appear at the
      button's corners and burst outward into a loose grid of hairline squares. The squares live in an
      aria-hidden SVG inside #get and are removed when the timeline ends. Nothing runs under reduced motion.
   - Wordmark: the footer SVG is complete markup, so every mode shows it drawn. In motion mode the outline
      draws on once, when it first scrolls into view, with one GSAP timeline on the shared ticker.
   - Narrator: the last stop closes the selection box on the wordmark. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};
  var SVGNS = 'http://www.w3.org/2000/svg';
  var calm = matchMedia('(prefers-reduced-motion: reduce)');

  // ---- download burst ----
  var CELL = 72;          // grid pitch of the burst, px
  var SIZES = [16, 24, 32, 40];
  var HANDLE = 6;         // the handle square at rest, like the narrator's
  var SPREAD = 0.15;      // stagger window across the grid, seconds
  var burstRun = null;

  function rnd(i, k) { var s = Math.sin(i * 127.1 + k * 311.7) * 43758.5453; return s - Math.floor(s); }

  function square(layer, size) {
    var r = document.createElementNS(SVGNS, 'rect');
    r.setAttribute('class', 'get__sq');
    r.setAttribute('x', -size / 2); r.setAttribute('y', -size / 2);
    r.setAttribute('width', size); r.setAttribute('height', size);
    layer.appendChild(r);
    return r;
  }

  // loose grid: one square per cell, jittered, skipping the cells the button covers
  function plan(host, b) {
    var w = host.clientWidth, h = host.clientHeight, cols = Math.ceil(w / CELL), rows = Math.ceil(h / CELL);
    var cx = (b.l + b.r) / 2, cy = (b.t + b.b) / 2, out = [], far = 1;
    for (var r = 0; r < rows; r++) {
      for (var c = 0; c < cols; c++) {
        var i = r * cols + c, s = SIZES[Math.floor(rnd(i, 1) * SIZES.length)];
        var x = (c + 0.5) * CELL + (rnd(i, 2) - 0.5) * 24, y = (r + 0.5) * CELL + (rnd(i, 3) - 0.5) * 24;
        if (x + s / 2 > b.l - 12 && x - s / 2 < b.r + 12 && y + s / 2 > b.t - 12 && y - s / 2 < b.b + 12) continue;
        var d = Math.hypot(x - cx, y - cy);
        far = Math.max(far, d);
        out.push({ x: x, y: y, s: s, d: d });
      }
    }
    out.forEach(function (q) { q.at = (q.d / far) * SPREAD; });
    return out;
  }

  function burst(button) {
    var gsap = window.gsap, host = document.getElementById('get-burst');
    if (!gsap || !host || calm.matches) return;
    if (burstRun) { burstRun.kill(); burstRun = null; }
    host.textContent = '';
    var layer = document.createElementNS(SVGNS, 'svg');
    layer.setAttribute('focusable', 'false');
    host.appendChild(layer);

    var hr = host.getBoundingClientRect(), br = button.getBoundingClientRect();
    var b = { l: br.left - hr.left, t: br.top - hr.top, r: br.right - hr.left, b: br.bottom - hr.top };
    var corners = [[b.l, b.t], [b.r, b.t], [b.r, b.b], [b.l, b.b]];
    var tl = gsap.timeline({ onComplete: function () { host.textContent = ''; burstRun = null; } });
    burstRun = tl;

    var handles = corners.map(function (p) {
      var el = square(layer, HANDLE);
      gsap.set(el, { x: p[0], y: p[1], transformOrigin: '0 0' });
      return el;
    });
    tl.from(handles, { scale: 0, duration: 0.12, ease: 'back.out(3)', stagger: 0.02 }, 0)
      .to(handles, { opacity: 0, duration: 0.25, ease: 'power1.in' }, 0.4);

    plan(host, b).forEach(function (q) {
      var from = corners.reduce(function (a, p) {
        return Math.hypot(p[0] - q.x, p[1] - q.y) < Math.hypot(a[0] - q.x, a[1] - q.y) ? p : a;
      });
      var el = square(layer, q.s);
      gsap.set(el, { x: from[0], y: from[1], scale: HANDLE / q.s, transformOrigin: '0 0' });
      tl.to(el, { x: q.x, y: q.y, scale: 1, duration: 0.45, ease: 'power3.out' }, q.at)
        .to(el, { fillOpacity: 0, duration: 0.2, ease: 'power1.out' }, q.at)
        .to(el, { opacity: 0, duration: 0.25, ease: 'power1.in' }, q.at + 0.3);
    });
  }

  var download = document.getElementById('get-download');
  if (download) download.addEventListener('click', function () { burst(download); });

  // ---- wordmark draw-on (motion mode only) ----
  var DASH = 1000;        // a little longer than the longest glyph contour (about 900px), so each one draws in full
  var mark, outline, rows, entering = null, drawing = null;

  function reveal() {
    var gsap = window.gsap;
    gsap.set(outline, { strokeDasharray: DASH, strokeDashoffset: DASH });
    gsap.set(rows, { opacity: 0 });
    if (!('IntersectionObserver' in window)) { settle(); return; }
    entering = new IntersectionObserver(function (list) {
      if (!list[0].isIntersecting) return;
      entering.disconnect(); entering = null;
      drawing = gsap.timeline({ onComplete: settle })
        .to(outline, { strokeDashoffset: 0, duration: 1.6, ease: 'power2.inOut' }, 0)
        .to(rows, { opacity: 1, duration: 0.9, ease: 'power1.out' }, 0.9);
    }, { threshold: 0.35 });
    entering.observe(mark);
  }
  function settle() { window.gsap.set([outline, rows], { clearProps: 'strokeDasharray,strokeDashoffset,opacity' }); }
  function restore() {
    if (entering) { entering.disconnect(); entering = null; }
    if (drawing) { drawing.kill(); drawing = null; }
    if (window.gsap && outline) settle();
  }

  // ---- narrator: the selection box closes on the wordmark ----
  var stop = {
    id: 'closer', order: 90, pad: 6,
    test: function (vh) {
      var r = mark.getBoundingClientRect();
      return r.top <= vh * 0.82 && r.bottom >= vh * 0.1;
    },
    rect: function () { return mark.getBoundingClientRect(); }
  };

  V2.scenes.push({
    mount: function () {
      mark = document.querySelector('.closer__mark');
      if (!mark) return;
      outline = mark.querySelector('.closer__outline');
      rows = mark.querySelector('.closer__rows');
      reveal();
      if (V2.narrator) V2.narrator.add(stop);
    },
    unmount: function () {
      if (!mark) return;
      if (V2.narrator) V2.narrator.remove('closer');
      restore();
    }
  });
})();
