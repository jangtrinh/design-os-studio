/* v2-big-files.js: scene "Built for big files."
   A Tenant section: the engine pins it with position:sticky, progress comes from the section's own bounding
   box, and this module only writes inside the section. The engine scrubs the film's zoom-out of one page of
   the kit on a canvas; the node count climbs while the section rises into view and is final once the pin
   starts. Motion mode only (no reduced-motion preference, viewport of 800px or more);
   everywhere else the static markup stands: the real count and the last frame of the zoom. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};

  var TRAVEL = 40;                                     // pinned scroll distance, in percent of the viewport height
  var BASE = 'media/v2/scale/';
  var FRAMES = 39, SIZE = [1280, 720];                 // frame count and frame size, from manifest.json
  var RANGE = [1, 38];                                 // frame 0 still shows the code chapter of the film
  var FINAL = 208610;                                  // node_count of the kit file; the markup carries the same value
  var LAND = 0.9;                                      // a link to #scale lands this far into the pin, count final

  var section, scene, figure, canvas, num, pin = null, unlock = null;
  var settled = false, lastShown = -1;

  // the count climbs while the section rises into view and is final by the time the pin starts, so every
  // pinned position (the section at rest) shows the real count
  function share() {
    var r = section.getBoundingClientRect(), vh = window.innerHeight;
    var c = V2.clamp((vh * 0.7 - r.top) / (vh * 0.7));
    return 1 - (1 - c) * (1 - c);
  }
  function show(c) {
    var n = Math.round(FINAL * c);
    if (n === lastShown) return;
    lastShown = n;
    num.textContent = n.toLocaleString('en-US');
  }

  function tick() {
    if (!pin.armed()) { // offscreen: always the real count
      if (!settled) { show(1); settled = true; }
      return;
    }
    settled = false;
    show(share());
  }

  // The selection box wraps the canvas while it is in the middle of the viewport.
  var stop = {
    id: 'scale', order: 45, pad: 6,
    test: function (vh) {
      var r = canvas.getBoundingClientRect();
      return r.top <= vh * 0.62 && r.bottom >= vh * 0.2;
    },
    rect: function () { return figure.getBoundingClientRect(); } // the canvas and its caption
  };

  V2.scenes.push({
    mount: function () {
      section = document.getElementById('scale');
      scene = section && section.querySelector('.scale__scene');
      if (!scene || !window.mountScrubSection) return;
      figure = scene.querySelector('.scale__view');
      num = scene.querySelector('.scale__num');
      pin = V2.pinScene(section, scene, TRAVEL, {
        frames: BASE + 'frames/', framesLQ: BASE + 'frames-lq/', frameCount: FRAMES,
        beats: [{ range: RANGE }]
      });
      canvas = pin.canvas;
      if (!canvas) { pin.unpin(); pin = null; return; }
      canvas.setAttribute('role', 'img');
      canvas.setAttribute('aria-label', figure.querySelector('.scale__poster').getAttribute('alt'));
      figure.insertBefore(canvas, figure.firstChild);
      unlock = V2.lockCanvas(section, canvas, SIZE[0], SIZE[1]);
      settled = false; lastShown = -1;
      show(share());
      window.__scrubTicker.add(tick);
      if (V2.narrator) V2.narrator.add(stop);
      V2.anchors.scale = {
        el: section,
        offset: function () { return LAND * (section.getBoundingClientRect().height - window.innerHeight); }
      };
    },
    unmount: function () {
      if (!pin) return;
      window.__scrubTicker.remove(tick);
      if (V2.narrator) V2.narrator.remove('scale');
      delete V2.anchors.scale;
      unlock(); unlock = null;
      canvas.parentNode.removeChild(canvas);           // unpin() would carry it into the static markup
      pin.unpin(); pin = null;
      num.textContent = FINAL.toLocaleString('en-US');
      if (!section.getAttribute('style')) { section.setAttribute('style', ''); section.removeAttribute('style'); }
      canvas = null;
    }
  });
})();
