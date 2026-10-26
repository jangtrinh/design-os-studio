/* v2-big-files.js: scene "Built for big files."
   A Tenant section: the engine pins it with position:sticky, progress comes from the section's own bounding
   box, and this module only writes inside the section. In motion mode the view crossfades from the close-up
   0.6.0 hero still to the full-page kit overview still while the node count climbs to 207,601.
   Motion mode only (no reduced-motion preference, viewport of 800px or more); everywhere else the static
   markup stands: the real count and the kit overview capture. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};

  var TRAVEL = 40;                                     // pinned scroll distance, in percent of the viewport height
  var FROM_SRC = 'images/v0.6/studio-hero.png';        // close-up capture on the same Page headers page
  var FINAL = 207601;                                  // node_count of the kit file; the markup carries the same value
  var LAND = 0.9;                                      // a link to #scale lands this far into the pin, count final

  var section, scene, figure, stage, poster, fromImg, num, pin = null;
  var settled = false, lastShown = -1, lastFade = '';

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
  function fade(p) {
    var w = 1 - V2.smooth((p - 0.08) / 0.78);
    var v = w.toFixed(3);
    if (v === lastFade) return;
    lastFade = v;
    fromImg.style.opacity = v;
    fromImg.style.visibility = w > 0.001 ? 'visible' : 'hidden';
  }

  function tick() {
    if (!pin.armed()) { // offscreen: always the real count, and settle the crossfade at the nearer end
      if (!settled) {
        show(1);
        fade(pin.progress() > 0.5 ? 1 : 0);
        settled = true;
      }
      return;
    }
    settled = false;
    show(share());
    fade(pin.progress());
  }

  // The selection box wraps the stage and its caption while in the middle of the viewport.
  var stop = {
    id: 'scale', order: 45, pad: 6,
    test: function (vh) {
      var r = stage.getBoundingClientRect();
      return r.top <= vh * 0.62 && r.bottom >= vh * 0.2;
    },
    rect: function () { return figure.getBoundingClientRect(); }
  };

  V2.scenes.push({
    mount: function () {
      section = document.getElementById('scale');
      scene = section && section.querySelector('.scale__scene');
      if (!scene || !window.mountScrubSection) return;
      figure = scene.querySelector('.scale__view');
      stage = figure.querySelector('.scale__stage');
      poster = stage.querySelector('.scale__poster');
      num = scene.querySelector('.scale__num');
      pin = V2.pinScene(section, scene, TRAVEL);
      fromImg = document.createElement('img');
      fromImg.className = 'scale__from';
      fromImg.src = FROM_SRC;
      fromImg.width = 2880;
      fromImg.height = 1800;
      fromImg.decoding = 'async';
      fromImg.alt = '';
      fromImg.setAttribute('aria-hidden', 'true');
      stage.insertBefore(fromImg, poster);
      settled = false; lastShown = -1; lastFade = '';
      show(share());
      fade(pin.progress());
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
      if (fromImg && fromImg.parentNode) fromImg.parentNode.removeChild(fromImg);
      poster.style.removeProperty('opacity');
      if (!poster.getAttribute('style')) { poster.setAttribute('style', ''); poster.removeAttribute('style'); }
      pin.unpin(); pin = null;
      num.textContent = FINAL.toLocaleString('en-US');
      if (!section.getAttribute('style')) { section.setAttribute('style', ''); section.removeAttribute('style'); }
      fromImg = null;
    }
  });
})();
