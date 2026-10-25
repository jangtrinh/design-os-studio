/* v2-narrator.js: the page's one narrator, a selection box.
   Blue hairline, four handles, and a pill that reports the box's live, measured size.
   It lives in an absolutely positioned layer inside <main> (never position:fixed), is placed
   from the bounding boxes of the things it selects, and glides between them on the shared ticker.
   A scene registers a stop: { id, order, test(vh), rect(), pad, key() }. The highest-order stop whose
   test passes is selected. A stop's rect() is a viewport DOMRect-like; key() names the sub-stop. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};
  var SVGNS = 'http://www.w3.org/2000/svg';
  var N = V2.narrator = { stops: [], intro: { draw: 1, handles: 1, pill: 1, count: 1 } };
  N.add = function (stop) {
    N.stops.push(stop);
    N.stops.sort(function (a, b) { return b.order - a.order; }); // highest order first
  };
  N.remove = function (id) { N.stops = N.stops.filter(function (s) { return s.id !== id; }); };

  var layer, frame, handles, pill, host;
  var cur = null, lastKey = null, glideUntil = 0, shown = false, pillW = 0, last = { d: '', off: '', txt: '' };

  function svgEl(name, attrs) {
    var el = document.createElementNS(SVGNS, name);
    Object.keys(attrs || {}).forEach(function (k) { el.setAttribute(k, attrs[k]); });
    return el;
  }
  function build() {
    host = document.querySelector('main');
    layer = document.createElement('div');
    layer.className = 'narrator';
    layer.setAttribute('aria-hidden', 'true');
    var svg = svgEl('svg', { focusable: 'false' });
    frame = svgEl('path', { 'class': 'narrator__frame', pathLength: '1', 'stroke-dasharray': '1 1', d: 'M0 0' });
    svg.appendChild(frame);
    handles = [0, 1, 2, 3].map(function () {
      var g = svgEl('g');
      g.appendChild(svgEl('rect', { 'class': 'narrator__handle-rim', width: 9, height: 9 }));
      g.appendChild(svgEl('rect', { 'class': 'narrator__handle-fill', x: 1, y: 1, width: 7, height: 7 }));
      svg.appendChild(g);
      return g;
    });
    pill = document.createElement('span');
    pill.className = 'narrator__pill';
    layer.appendChild(svg);
    layer.appendChild(pill);
    host.insertBefore(layer, host.firstChild);
  }

  function activeStop(vh) {
    for (var i = 0; i < N.stops.length; i++) {
      var s = N.stops[i];
      if (!s.test || s.test(vh)) return s;
    }
    return null;
  }

  function tick(time, deltaTime) {
    var vh = window.innerHeight;
    var stop = activeStop(vh);
    var r = stop ? stop.rect() : null;
    if (!r || r.width < 2 || r.height < 2 || r.bottom < 0 || r.top > vh || N.intro.draw <= 0) {
      if (shown) { layer.classList.remove('is-on'); shown = false; }
      if (r) cur = null; // snap on the way back in, never fly in from a stale spot
      return;
    }
    var m = host.getBoundingClientRect();
    var pad = stop.pad == null ? 6 : stop.pad;
    var t = { x: r.left - m.left - pad, y: r.top - m.top - pad, w: r.width + pad * 2, h: r.height + pad * 2 };
    var key = stop.id + ':' + (stop.key ? stop.key() : '');
    var now = performance.now();
    if (!cur) { cur = t; lastKey = key; }
    else {
      if (key !== lastKey) { lastKey = key; glideUntil = now + 800; }
      var k = now < glideUntil ? 9 : 40; // glide between stops, then stay locked to the target
      var a = 1 - Math.exp(-k * Math.min(0.1, (deltaTime || 16) / 1000));
      cur.x += (t.x - cur.x) * a; cur.y += (t.y - cur.y) * a;
      cur.w += (t.w - cur.w) * a; cur.h += (t.h - cur.h) * a;
    }
    if (!shown) { layer.classList.add('is-on'); shown = true; }
    paint(Math.round(cur.x), Math.round(cur.y), Math.round(cur.w), Math.round(cur.h));
  }

  function paint(x, y, w, h) {
    var I = N.intro;
    var half = 0.5;
    var d = 'M' + (x + half) + ' ' + (y + half) + 'H' + (x + w - half) + 'V' + (y + h - half) + 'H' + (x + half) + 'Z';
    if (d !== last.d) { frame.setAttribute('d', d); last.d = d; }
    var off = String(1 - I.draw);
    if (off !== last.off) { frame.setAttribute('stroke-dashoffset', off); last.off = off; }
    var corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    for (var i = 0; i < 4; i++) {
      var s = I.handles;
      handles[i].setAttribute('transform', 'translate(' + (corners[i][0] - 4.5 * s) + ' ' + (corners[i][1] - 4.5 * s) + ') scale(' + s + ')');
      handles[i].style.display = s > 0.001 ? '' : 'none';
    }
    var txt = Math.round(w * I.count) + ' × ' + Math.round(h * I.count);
    if (txt !== last.txt) { pill.textContent = txt; last.txt = txt; pillW = pill.offsetWidth; }
    pill.style.opacity = I.pill;
    pill.style.transform = 'translate(' + Math.round(x + w / 2 - pillW / 2) + 'px,' + (y + h + 14) + 'px)';
  }

  V2.scenes.push({
    mount: function () {
      if (!layer) build();
      cur = null; shown = false;
      window.__scrubTicker.add(tick);
    },
    unmount: function () {
      if (window.__scrubTicker) window.__scrubTicker.remove(tick);
      if (layer) layer.classList.remove('is-on');
      shown = false;
    }
  });

  // Hero stops: the headline, then the hero frame. Later scenes add their own with a higher order.
  var byId = function (id) { return document.getElementById(id); };
  N.add({ id: 'headline', order: 10, pad: 0, rect: function () { return byId('sel-box').getBoundingClientRect(); } });
  N.add({
    id: 'film', order: 20, pad: 6,
    test: function (vh) {
      var f = byId('film').getBoundingClientRect();
      return byId('top').getBoundingClientRect().top < -40 && f.top <= vh * 0.42 && f.bottom >= vh * 0.2;
    },
    rect: function () { return byId('film').querySelector('.film-stage').getBoundingClientRect(); }
  });
})();
