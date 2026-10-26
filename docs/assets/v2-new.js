/* v2-new.js: scene "What's new".
   The section mirrors README.md's What's new section: 0.6.1 and 0.6.0 (with its two captures) open at the top,
   and 0.5.0, 0.4.0 and 0.3.0 in native <details> disclosures. In motion mode, the narrator's selection box
   frames the 0.6.0 captures container, or the individual capture figure currently hovered or focused. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};
  V2.scenes = V2.scenes || [];

  var section = document.getElementById('new');
  var panel = document.getElementById('new-panel');
  if (!section || !panel) return;

  var shots = Array.prototype.slice.call(panel.querySelectorAll('.new-shot'));
  var activeIdx = -1;

  shots.forEach(function (shot, i) {
    shot.addEventListener('mouseenter', function () { activeIdx = i; });
    shot.addEventListener('mouseleave', function () { if (activeIdx === i) activeIdx = -1; });
    shot.addEventListener('focusin', function () { activeIdx = i; });
    shot.addEventListener('focusout', function () { if (activeIdx === i) activeIdx = -1; });
  });

  var stop = {
    id: 'new', order: 50, pad: 6,
    test: function (vh) {
      var r = panel.getBoundingClientRect();
      return r.top <= vh * 0.72 && r.bottom >= vh * 0.18;
    },
    key: function () { return String(activeIdx); },
    rect: function () {
      var target = activeIdx >= 0 && shots[activeIdx] ? shots[activeIdx] : panel;
      return target.getBoundingClientRect();
    }
  };

  V2.scenes.push({
    mount: function () {
      if (V2.narrator) V2.narrator.add(stop);
    },
    unmount: function () {
      if (V2.narrator) V2.narrator.remove('new');
    }
  });
})();
