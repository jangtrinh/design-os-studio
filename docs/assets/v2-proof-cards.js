/* v2-proof-cards.js: scene 2. Three cards, a fact on the front and a real Studio render on the back.
   A Tenant section: the engine pins it with position:sticky, progress comes from the section's own
   bounding box, and this module only writes inside the section. The cards rise, fan, then flip one by one.
   The narrator follows the card that is turning. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};
  var TRAVEL = 100;                 // pinned scroll distance, in percent of the viewport height; all pins together stay near 4,000 px at 900 px tall
  var TILT = [14, 0, -14], ROLL = [-4, 0, 4], DROP = [18, 0, 18], SPREAD = [-20, 0, 20];
  var FLIP_AT = [0.38, 0.54, 0.70]; // each card turns over a 0.30 window of progress

  var section, scene, pin, cards = [], slots = [], p = 0, settled = false, last = [];

  function render(t) {
    for (var i = 0; i < cards.length; i++) {
      var rise = V2.smooth((t - 0.03 * i) / 0.22);
      var fan = V2.smooth((t - 0.16) / 0.24);
      var flip = V2.smooth((t - FLIP_AT[i]) / 0.30);
      var keep = 1 - flip;
      var y = (1 - rise) * 520 + fan * DROP[i] * keep - Math.sin(Math.PI * flip) * 28;
      var x = fan * SPREAD[i] * keep;
      var ry = fan * TILT[i] * keep + 180 * flip;
      var rz = fan * ROLL[i] * keep;
      var s = 1 + 0.025 * Math.sin(Math.PI * flip);
      var css = 'translate3d(' + x.toFixed(1) + 'px,' + y.toFixed(1) + 'px,0) rotateZ(' + rz.toFixed(2) + 'deg) rotateY(' + ry.toFixed(2) + 'deg) scale(' + s.toFixed(3) + ')';
      if (css !== last[i]) { cards[i].style.transform = css; last[i] = css; } // skip unchanged writes
    }
  }

  function tick() {
    if (!pin.armed()) { // offscreen: rest at the nearer end so a fast scroll never leaves a half-state
      if (!settled) { render(p > 0.5 ? 1 : 0); settled = true; }
      return;
    }
    settled = false;
    p = pin.progress();
    render(p);
  }

  function activeCard() { var i = 0; for (var k = 1; k < 3; k++) if (p >= FLIP_AT[k] - 0.04) i = k; return i; }

  var stop = {
    id: 'cards', order: 30, pad: 6,
    test: function (vh) {
      var r = section.getBoundingClientRect();
      var c = cards[activeCard()].getBoundingClientRect();
      return r.top <= vh * 0.62 && r.bottom >= vh * 0.45 && c.top < pin.stage.getBoundingClientRect().bottom - 80;
    },
    key: function () { return String(activeCard()); },
    // Position follows the card (it rises into place); size and left edge stay on the slot so the box
    // does not collapse while the card is edge-on mid-flip.
    rect: function () {
      var i = activeCard(), slot = slots[i].getBoundingClientRect(), c = cards[i].getBoundingClientRect();
      return { left: slot.left, right: slot.right, width: slot.width, top: c.top, bottom: c.top + slot.height, height: slot.height };
    }
  };

  V2.scenes.push({
    mount: function () {
      section = document.getElementById('proof');
      scene = section && section.querySelector('.proof__scene');
      if (!scene || !window.mountScrubSection) return;
      pin = V2.pinScene(section, scene, TRAVEL);
      cards = Array.prototype.slice.call(scene.querySelectorAll('.proof-card'));
      slots = Array.prototype.slice.call(scene.querySelectorAll('.proof-slot'));
      last = cards.map(function () { return ''; });
      p = 0; settled = false;
      window.__scrubTicker.add(tick);
      if (V2.narrator) V2.narrator.add(stop);
    },
    unmount: function () {
      if (!pin) return;
      window.__scrubTicker.remove(tick);
      if (V2.narrator) V2.narrator.remove('cards');
      pin.unpin(); pin = null;
      cards.forEach(function (c) { c.style.removeProperty('transform'); });
    }
  });
})();
