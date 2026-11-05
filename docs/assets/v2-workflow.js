/* v2-workflow.js: scene "Workflow": Figma, agents, code.
   A Tenant section: the engine pins it with position:sticky, progress comes from the section's own bounding
   box, and this module only writes inside the section. The three 0.6.0 chapter stills crossfade on one stage,
   one third of the progress per chapter; the chapter copy and the sheet tint are pure functions of that same
   progress. Motion mode only (no reduced-motion preference, viewport of 800px or more); everywhere else the
   static markup stands: three chapters, one 0.7.5 capture each. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};

  var TRAVEL = 170;                                    // pinned scroll distance, in percent of the viewport height
  var TINTS = [[227, 99, 35], [13, 153, 255], [20, 174, 92]]; // --orange, --blue, --emerald
  var LAND = 0.05;                                     // a link lands this far into its chapter, copy fully in
  var IDS = ['workflow', 'figma', 'agents', 'code'];

  var section, scene, sheet, chapters = [], alts = [], view, stills = [];
  var pin = null, p = 0, settled = false, lastT = -1, lastIdx = -1, memo = new WeakMap();

  // every style write skips an unchanged value
  function put(el, prop, val) {
    var m = memo.get(el);
    if (!m) { m = {}; memo.set(el, m); }
    if (m[prop] === val) return;
    m[prop] = val;
    if (prop.charAt(0) === '-') el.style.setProperty(prop, val); else el.style[prop] = val;
  }
  function clearStyle(e) {
    e.setAttribute('style', '');
    e.removeAttribute('style');
  }
  function lerp(a, b, t) { return a + (b - a) * t; }

  // chapter i is fully in between its two crossings: out before the cut that ends it, in after the cut that starts it
  function weight(i, t) {
    var lo = i / 3, hi = (i + 1) / 3;
    var inn = i === 0 ? 1 : V2.smooth((t - lo - 0.012) / 0.03);
    var out = i === 2 ? 1 : 1 - V2.smooth((t - hi + 0.042) / 0.03);
    return Math.min(inn, out);
  }
  function tintAt(t) {
    var c = TINTS[0];
    for (var k = 1; k < 3; k++) {
      var s = V2.smooth((t - (k / 3 - 0.03)) / 0.06);
      c = [lerp(c[0], TINTS[k][0], s), lerp(c[1], TINTS[k][1], s), lerp(c[2], TINTS[k][2], s)];
    }
    return Math.round(c[0]) + ' ' + Math.round(c[1]) + ' ' + Math.round(c[2]);
  }

  // pure render: everything on screen is a function of t
  function render(t) {
    t = Math.round(t * 1000) / 1000;
    if (t === lastT) return;
    lastT = t;
    var idx = Math.min(2, Math.floor(t * 3));
    for (var i = 0; i < 3; i++) {
      var w = weight(i, t), side = t < (i + 0.5) / 3 ? 1 : -1;
      put(chapters[i], 'opacity', String(Math.round(w * 1000) / 1000));
      put(chapters[i], 'transform', 'translate3d(0,' + ((1 - w) * 14 * side).toFixed(1) + 'px,0)');
    }
    // Crossfade the 3 chapter stills: still 0 sits opaque underneath; still 1 and still 2 fade in across the 1/3 and 2/3 cuts.
    var s1 = V2.smooth((t - (1 / 3 - 0.05)) / 0.10);
    var s2 = V2.smooth((t - (2 / 3 - 0.05)) / 0.10);
    put(stills[0], 'opacity', '1');
    put(stills[1], 'opacity', s1.toFixed(3));
    put(stills[1], 'visibility', s1 > 0.001 ? 'visible' : 'hidden');
    put(stills[2], 'opacity', s2.toFixed(3));
    put(stills[2], 'visibility', s2 > 0.001 ? 'visible' : 'hidden');
    put(sheet, '--tint', tintAt(t));
    if (idx !== lastIdx) { lastIdx = idx; view.setAttribute('aria-label', alts[idx]); }
  }

  function tick() {
    if (!pin.armed()) {
      if (!settled) { render(p > 0.5 ? 1 : 0); settled = true; }
      return;
    }
    settled = false;
    p = pin.progress();
    render(p);
  }

  // The selection box wraps the stage while it is in the middle of the viewport.
  var stop = {
    id: 'workflow', order: 40, pad: 6,
    test: function (vh) {
      var r = view.getBoundingClientRect();
      return r.top <= vh * 0.7 && r.bottom >= vh * 0.3;
    },
    rect: function () { return view.getBoundingClientRect(); }
  };

  function target(share) {
    return {
      el: section,
      offset: function () { return share * (section.getBoundingClientRect().height - window.innerHeight); }
    };
  }

  V2.scenes.push({
    mount: function () {
      section = document.getElementById('workflow');
      scene = section && section.querySelector('.wf__scene');
      if (!scene || !window.mountScrubSection) return;
      sheet = scene.querySelector('.wf__sheet');
      chapters = Array.prototype.slice.call(scene.querySelectorAll('.wf__chapter'));
      alts = chapters.map(function (c) { return c.querySelector('.wf__poster').getAttribute('alt'); });
      pin = V2.pinScene(section, scene, TRAVEL);
      view = document.createElement('div');
      view.className = 'wf__view';
      view.setAttribute('role', 'img');
      stills = chapters.map(function (c, i) {
        var poster = c.querySelector('.wf__poster');
        var img = document.createElement('img');
        img.className = 'wf__still';
        img.src = poster.getAttribute('src');
        img.width = 2880;
        img.height = 1800;
        img.decoding = 'async';
        img.alt = '';
        img.setAttribute('aria-hidden', 'true');
        img.style.zIndex = String(i + 1);
        view.appendChild(img);
        return img;
      });
      sheet.appendChild(view);
      memo = new WeakMap();
      p = 0; settled = false; lastT = -1; lastIdx = -1;
      render(0);
      window.__scrubTicker.add(tick);
      if (V2.narrator) V2.narrator.add(stop);
      IDS.forEach(function (id, n) { V2.anchors[id] = target(n < 2 ? 0 : (n - 1) / 3 + LAND); });
    },
    unmount: function () {
      if (!pin) return;
      window.__scrubTicker.remove(tick);
      if (V2.narrator) V2.narrator.remove('workflow');
      IDS.forEach(function (id) { delete V2.anchors[id]; });
      view.parentNode.removeChild(view);
      pin.unpin(); pin = null;
      chapters.forEach(function (c) { c.style.removeProperty('opacity'); c.style.removeProperty('transform'); });
      sheet.style.removeProperty('--tint');
      if (!sheet.getAttribute('style')) clearStyle(sheet);
      if (!section.getAttribute('style')) clearStyle(section);
      view = null; stills = [];
    }
  });
})();
