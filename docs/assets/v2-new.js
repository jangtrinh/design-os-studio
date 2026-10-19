/* v2-new.js: scene 7, "New in 0.3.0". An active list on the left, one real Studio render on the right.
   The pre-authored markup is the no-JS state (every title and line visible, first render shown, a link per
   render). This module upgrades it to an ARIA tablist and swaps the render. It does not pin.
   Tablist behaviour works in every mode. Only the narrator stop depends on the scene being mounted
   (motion mode: no reduced-motion preference and a viewport of 800px or more). There is no
   auto-advance: the visitor drives the list (hover, click, arrow keys), so nothing moves on its own. */
(function () {
  'use strict';
  var V2 = window.V2 = window.V2 || {};
  V2.scenes = V2.scenes || [];

  var section = document.getElementById('new');
  var list = document.getElementById('new-list');
  var frame = document.getElementById('new-panel');
  var img = frame && frame.querySelector('.new-img');
  if (!section || !list || !frame || !img) return;

  var HOVER_MS = 120;      // hover intent before a hover selects
  var JITTER_PX = 12;      // pointer travel (|dx|+|dy|) that counts as deliberate after a selection
  var GLIDE_MS = 300;      // list and bar glide
  var EASE = 'cubic-bezier(.33,1,.68,1)';

  var calm = matchMedia('(prefers-reduced-motion: reduce)');
  var animates = function () { return !calm.matches && typeof img.animate === 'function'; };

  // Read the authored markup, then rebuild each row as a single tab button.
  var rows = Array.prototype.slice.call(list.querySelectorAll('.new-item'));
  var N = rows.length;
  var items = rows.map(function (li) {
    return {
      title: li.querySelector('.new-item__title').textContent,
      line: li.querySelector('.new-item__line').textContent,
      src: li.querySelector('.new-item__full').getAttribute('href'),
      alt: li.getAttribute('data-alt'),
      w: li.getAttribute('data-w'),
      h: li.getAttribute('data-h')
    };
  });

  section.classList.add('is-live');
  list.setAttribute('role', 'tablist');
  list.setAttribute('aria-orientation', 'vertical');
  frame.setAttribute('role', 'tabpanel');

  var bar = document.createElement('div');
  bar.className = 'new-bar';
  bar.setAttribute('aria-hidden', 'true');
  list.appendChild(bar);

  var tabs = items.map(function (it, i) {
    var tab = document.createElement('button');
    tab.type = 'button';
    tab.className = 'new-tab';
    tab.id = 'new-tab-' + i;
    tab.setAttribute('role', 'tab');
    tab.setAttribute('aria-controls', 'new-panel');
    tab.setAttribute('aria-selected', 'false');
    tab.tabIndex = -1;
    var t = document.createElement('span');
    t.className = 'new-tab__t';
    t.textContent = it.title;
    var l = document.createElement('span');
    l.className = 'new-tab__l';
    l.textContent = it.line;
    tab.appendChild(t);
    tab.appendChild(l);
    rows[i].textContent = '';
    rows[i].setAttribute('role', 'presentation');
    rows[i].appendChild(tab);
    return tab;
  });

  var cur = -1, swapToken = 0, busyUntil = 0, cache = [];
  var px = null, py = null, anchor = null;   // last pointer position over the list; where it was at the last selection

  function cancelAnimations(els) {
    els.forEach(function (el) {
      if (el.getAnimations) el.getAnimations().forEach(function (a) { a.cancel(); });
    });
  }

  function preload(i) {
    if (i < 0 || i >= N) return null;
    if (!cache[i]) { cache[i] = new Image(); cache[i].src = items[i].src; }
    return cache[i];
  }

  // The bar sits at the active row's measured box. Height is set to the final value once; only transform is animated.
  function placeBar() {
    var lr = list.getBoundingClientRect(), r = rows[cur].getBoundingClientRect();
    var box = { y: r.top - lr.top, h: r.height, listTop: lr.top };
    bar.style.height = box.h + 'px';
    bar.style.transform = 'translateY(' + box.y + 'px)';
    return box;
  }

  // First-Last-Invert-Play for the rows (their lines open and collapse instantly) and for the bar.
  function glide(firstTops, firstBar, box) {
    rows.forEach(function (li, i) {
      var d = firstTops[i] - li.getBoundingClientRect().top;
      if (Math.abs(d) > 0.5) li.animate([{ transform: 'translateY(' + d + 'px)' }, { transform: 'translateY(0)' }], { duration: GLIDE_MS, easing: EASE });
    });
    var fy = firstBar.top - box.listTop, fs = firstBar.height / box.h;
    bar.animate([
      { transform: 'translateY(' + fy + 'px) scaleY(' + fs + ')' },
      { transform: 'translateY(' + box.y + 'px) scaleY(1)' }
    ], { duration: GLIDE_MS, easing: EASE });
    busyUntil = performance.now() + GLIDE_MS + 40;
  }

  // Swap the one real <img>. A clone of the outgoing render sits under it and fades out, so the
  // cross-fade never dips toward the panel colour.
  function swapRender(n, play) {
    var it = items[n], token = ++swapToken, pre = preload(n);
    var apply = function () {
      if (token !== swapToken) return;
      var ghost = null;
      if (play) {
        ghost = img.cloneNode(false);
        ghost.removeAttribute('id');
        ghost.removeAttribute('loading');
        ghost.setAttribute('alt', '');
        ghost.setAttribute('aria-hidden', 'true');
        ghost.className = 'new-img new-img--ghost';
        frame.insertBefore(ghost, img);
      }
      img.removeAttribute('loading');
      img.src = it.src;
      img.alt = it.alt;
      img.setAttribute('width', it.w);
      img.setAttribute('height', it.h);
      if (ghost) {
        img.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 150, easing: 'ease-out' });
        img.animate([{ transform: 'scale(.985)' }, { transform: 'scale(1)' }], { duration: GLIDE_MS, easing: EASE });
        var out = ghost.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 150, easing: 'ease-in', fill: 'forwards' });
        out.onfinish = function () { if (ghost.parentNode) ghost.parentNode.removeChild(ghost); };
      }
    };
    if (!play) { apply(); return; }
    if (pre.complete && pre.naturalWidth) apply();
    else if (pre.decode) pre.decode().then(apply, apply);
    else { pre.addEventListener('load', apply); pre.addEventListener('error', apply); }
  }

  function setSelected(n) {
    tabs.forEach(function (tab, i) { tab.setAttribute('aria-selected', i === n ? 'true' : 'false'); });
  }

  function select(n) {
    if (n === cur) return;
    var first = cur < 0;          // the authored markup already shows item 0
    var play = animates() && !first;
    var firstTops = null, firstBar = null;
    if (play) {
      firstTops = rows.map(function (li) { return li.getBoundingClientRect().top; });
      firstBar = bar.getBoundingClientRect();
      cancelAnimations(rows.concat([bar]));
    }
    cur = n;
    anchor = px === null ? null : { x: px, y: py };
    setSelected(n);
    tabs.forEach(function (tab, i) { tab.tabIndex = i === n ? 0 : -1; });
    frame.setAttribute('aria-labelledby', tabs[n].id);
    var box = placeBar();
    if (play) {
      glide(firstTops, firstBar, box);
      tabs[n].querySelector('.new-tab__l').animate([{ opacity: 0 }, { opacity: 1 }], { duration: 200, easing: 'ease-out' });
    }
    if (first) return;
    swapRender(n, play);
    preload((n + 1) % N);
  }

  // The section keeps one height whichever item is active: the list reserves its tallest state.
  function lockHeight() {
    var keep = cur, best = 0;
    list.style.minHeight = '';
    for (var s = 0; s < N; s++) {
      setSelected(s);
      best = Math.max(best, Math.ceil(list.getBoundingClientRect().height));
    }
    setSelected(keep);
    list.style.minHeight = best + 'px';
  }

  function relayout() {
    cancelAnimations(rows.concat([bar]));
    lockHeight();
    placeBar();
  }

  // Events
  tabs.forEach(function (tab, i) {
    tab.addEventListener('click', function () { select(i); });
    tab.addEventListener('focus', function () { select(i); });
    tab.addEventListener('keydown', function (e) {
      var n = null;
      if (e.key === 'ArrowDown') n = (i + 1) % N;
      else if (e.key === 'ArrowUp') n = (i + N - 1) % N;
      else if (e.key === 'Home') n = 0;
      else if (e.key === 'End') n = N - 1;
      if (n === null) return;
      e.preventDefault();
      select(n);
      tabs[n].focus();
    });
  });

  // Hover selects after HOVER_MS. Only deliberate pointer movement counts: rows slide under a
  // resting pointer when lines open and collapse, so jitter within JITTER_PX of where the pointer was
  // at the last selection selects nothing.
  var hoverTimer = 0, hoverIdx = -1;
  list.addEventListener('mousemove', function (e) {
    if (px === null) { px = e.clientX; py = e.clientY; return; }
    if (Math.abs(e.clientX - px) + Math.abs(e.clientY - py) < 2) return;
    px = e.clientX; py = e.clientY;
    if (anchor) {
      if (Math.abs(px - anchor.x) + Math.abs(py - anchor.y) < JITTER_PX) return;
      anchor = null;
    }
    var tab = e.target.closest ? e.target.closest('.new-tab') : null;
    var i = tab ? tabs.indexOf(tab) : -1;
    if (i === hoverIdx) return;
    hoverIdx = i;
    window.clearTimeout(hoverTimer);
    if (i >= 0 && i !== cur) {
      preload(i);
      hoverTimer = window.setTimeout(function () { select(i); }, HOVER_MS);
    }
  });
  list.addEventListener('mouseleave', function () {
    window.clearTimeout(hoverTimer);
    hoverIdx = -1; px = py = null; anchor = null;
  });

  // Keep the bar and the reserved height right when the layout reflows (resize, web font).
  var resizeTimer = 0;
  window.addEventListener('resize', function () {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(relayout, 120);
  });
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(function () {
    if (performance.now() > busyUntil) relayout();
  });

  // The next render is fetched once the section is near the viewport.
  if ('IntersectionObserver' in window) {
    var near = new IntersectionObserver(function (entries) {
      if (entries[entries.length - 1].isIntersecting) { preload(1); near.disconnect(); }
    }, { rootMargin: '800px 0px' });
    near.observe(section);
  }

  lockHeight();
  select(0);

  // Narrator: the selection box wraps the render while its frame is in the 20%-70% band of the viewport.
  var stop = {
    id: 'new', order: 50, pad: 6,
    test: function (vh) {
      var r = frame.getBoundingClientRect();
      return r.top <= vh * 0.7 && r.bottom >= vh * 0.2;
    },
    rect: function () { return frame.getBoundingClientRect(); }
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
