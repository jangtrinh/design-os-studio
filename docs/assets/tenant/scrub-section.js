/* ============================================================================
   scrub-section.js — the Tenant engine for an embeddable scroll-scrub section.
   UMD, zero dependencies, SSR-safe (all browser access guarded).

   THE TENANT LAW (plans/specs/embeddable-scrub-section/brainstorm.md): an
   embedded section is a GUEST — reads the host page only through its own
   bounding box, writes only inside its own subtree. This file must NEVER read
   the page's absolute scroll offset, move the page's scroll position, assign
   the body's inline block-size or the document's scrolling-element reference,
   write a document-root-scoped custom property, create a viewport-pinned node
   outside normal flow, or run a private per-section animation-frame loop.
   Progress comes ONLY from sectionEl.getBoundingClientRect() against the
   declared scroll root; all state (incl. the progress variable) lives on the
   SECTION ROOT element. tenant-lint.mjs enforces this mechanically.

   Frame-drawing core (cover-crop math, LQ->HQ tiering, the release window) is
   ported verbatim from the reference engine's imageSequenceRenderer
   (../drone-showcase/site/scrub-engine.js) — only mounting/scroll changed.
   ========================================================================== */
(function (root, factory) {
  if (typeof module !== 'undefined' && module.exports) module.exports = factory();
  else if (typeof root !== 'undefined') root.mountScrubSection = factory();
})(typeof window !== 'undefined' ? window : this, function () {
  'use strict';

  // Shared page ticker (singleton, NOT per-section): one requestAnimationFrame
  // loop for the whole page; every mount subscribes a tick() reader. Unmount
  // removes the subscriber; the loop cancels itself once the set is empty.
  function getTicker() {
    if (typeof window === 'undefined') return null;
    if (window.__scrubTicker) return window.__scrubTicker;
    const subs = new Set();
    let rafId = null;
    function loop() {
      subs.forEach(function (fn) { try { fn(); } catch (e) { /* one bad subscriber must not kill the ticker */ } });
      rafId = subs.size ? window.requestAnimationFrame(loop) : null;
    }
    const ticker = {
      subs: subs,
      add: function (fn) { subs.add(fn); if (rafId === null) rafId = window.requestAnimationFrame(loop); },
      remove: function (fn) { subs.delete(fn); if (!subs.size && rafId !== null) { window.cancelAnimationFrame(rafId); rafId = null; } },
    };
    window.__scrubTicker = ticker;
    return ticker;
  }

  // Cross-instance arm budget (module-local closure, not a global write) —
  // the "cap: if >2 armed at once, drop to LQ tier" rule from the plan.
  let armedCount = 0;
  const ARM_CAP = 2;

  const clamp = function (x, a, b) { a = a === undefined ? 0 : a; b = b === undefined ? 1 : b; return Math.min(b, Math.max(a, x)); };
  const pad4 = function (n) { return String(n).padStart(4, '0'); };
  const esc = function (s) { return String(s).replace(/[&<>"]/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]; }); };

  function mountScrubSection(sectionEl, config) {
    if (typeof window === 'undefined' || typeof document === 'undefined' || !sectionEl) {
      return function unmount() {}; // SSR guard — mounting is client-only
    }
    config = config || {};
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    const beats = config.beats || [];
    const N = beats.length;
    const frameCount = config.frameCount || 0;
    const dir = config.frames || '';
    const dirLQ = config.framesLQ || '';
    // Declared scroll root — default document.scrollingElement, READ only,
    // never assigned; that is what keeps this legal under the Tenant Law.
    const scrollRoot = config.scrollRoot || document.scrollingElement || document.documentElement;

    sectionEl.classList.add('scrub');

    // ---- build DOM: stage (canvas+poster), progress bar, route rail, story chapters ----
    const stage = document.createElement('div'); stage.className = 'scrub__stage';
    const poster = document.createElement('img'); poster.className = 'scrub__poster';
    poster.alt = ''; poster.decoding = 'async'; poster.loading = 'lazy';
    let canvas = null, ctx2d = null;
    if (!reduce && dir && frameCount) {
      canvas = document.createElement('canvas'); canvas.className = 'scrub__canvas';
      ctx2d = canvas.getContext('2d');
      stage.appendChild(canvas);
    }
    stage.appendChild(poster);

    const progressBar = document.createElement('div'); progressBar.className = 'scrub__progress';
    progressBar.appendChild(document.createElement('i'));

    const routeRail = document.createElement('div'); routeRail.className = 'scrub__route';
    const dots = beats.map(function () { const i = document.createElement('i'); routeRail.appendChild(i); return i; });

    const story = document.createElement('div'); story.className = 'scrub__story';
    const chapters = beats.map(function (beat) {
      const chapter = document.createElement('article'); chapter.className = 'scrub__chapter';
      const pin = document.createElement('div'); pin.className = 'scrub__chapter-pin';
      const copy = document.createElement('div'); copy.className = 'scrub__copy';
      copy.innerHTML =
        (beat.eyebrow ? '<span class="scrub__eyebrow">' + esc(beat.eyebrow) + '</span>' : '') +
        (beat.title ? '<h3 class="scrub__title">' + esc(beat.title) + '</h3>' : '') +
        (beat.body ? '<p class="scrub__body">' + esc(beat.body) + '</p>' : '') +
        (beat.tags && beat.tags.length ? '<ul class="scrub__tags">' + beat.tags.map(function (t) { return '<li>' + esc(t) + '</li>'; }).join('') + '</ul>' : '');
      pin.appendChild(copy); chapter.appendChild(pin); story.appendChild(chapter);
      return chapter;
    });
    // Chapter min-heights alone give the story N*100dvh of natural height —
    // at N=1 that equals the stage's own 100dvh, so travel = rect.height -
    // rootHeight collapses to 0 and the section can never scrub. Floor the
    // story to at least 2 chapter-heights' worth so there is always real
    // scroll travel, even for a single-beat section.
    story.style.minHeight = Math.max(N, 2) * 100 + 'dvh';

    sectionEl.appendChild(stage);
    sectionEl.appendChild(progressBar);
    if (N) sectionEl.appendChild(routeRail);
    sectionEl.appendChild(story);

    // ---- frame cache (ported verbatim from the reference engine) --------
    const frames = new Array(frameCount);
    const requested = new Array(frameCount).fill(false);
    const framesLQ = new Array(frameCount);
    const requestedLQ = new Array(frameCount).fill(false);
    const EAGER = Math.min(10, frameCount);
    const WINDOW = 5;
    const CROP_Y = 0.42; // matches poster's object-position: center 42%
    let firstPainted = false, lastIdx = 0;
    // Guards a late-arriving decode against reviving the cache after the
    // section has disarmed or unmounted: alive flips false on unmount; armed
    // is re-read live (closure) at resolve time so a request that was
    // in-flight when the section left the viewport is dropped, not cached.
    let alive = true;

    function frameUrl(i) { return dir + pad4(i + 1) + '.webp'; }
    function frameUrlLQ(i) { return dirLQ + pad4(i + 1) + '.webp'; }
    function bestImg(i) { return frames[i] || framesLQ[i] || null; }

    function requestFrameLQ(i) {
      if (!dirLQ || i < 0 || i >= frameCount || requestedLQ[i]) return;
      requestedLQ[i] = true;
      const img = new Image(); img.decoding = 'async';
      img.onload = function () {
        if (!alive || !armed) return; // disarmed/unmounted before this resolved
        framesLQ[i] = img;
        if (!firstPainted) paint(bestImg(lastIdx));
      };
      img.onerror = function () { requestedLQ[i] = false; };
      img.src = frameUrlLQ(i);
    }
    function requestFrame(i) {
      if (i < 0 || i >= frameCount || requested[i]) return;
      requested[i] = true;
      const img = new Image(); img.decoding = 'async';
      img.onload = function () {
        if (!alive || !armed) return; // disarmed/unmounted before this resolved
        frames[i] = img;
        if (!firstPainted) paint(bestImg(lastIdx));
        else if (i === lastIdx) drawFrame(img); // HQ upgrade for the on-screen frame
      };
      img.onerror = function () { requested[i] = false; };
      img.src = frameUrl(i);
    }
    function releaseFar(idx) {
      for (let i = EAGER; i < frameCount; i++) {
        if (Math.abs(i - idx) > WINDOW && frames[i]) { frames[i] = null; requested[i] = false; }
      }
    }
    // Manual object-fit:cover crop (canvas has no CSS object-fit), anchored at CROP_Y.
    function drawFrame(img) {
      if (!img || !ctx2d) return;
      const cw = canvas.width, ch = canvas.height; if (!cw || !ch) return;
      const ir = img.naturalWidth / img.naturalHeight, cr = cw / ch;
      let sx, sy, sw, sh;
      if (ir > cr) { sh = img.naturalHeight; sw = sh * cr; sx = (img.naturalWidth - sw) / 2; sy = 0; }
      else { sw = img.naturalWidth; sh = sw / cr; sx = 0; sy = (img.naturalHeight - sh) * CROP_Y; }
      ctx2d.drawImage(img, sx, sy, sw, sh, 0, 0, cw, ch);
    }
    function paint(img) {
      if (!img) return;
      drawFrame(img); firstPainted = true; sectionEl.classList.add('is-painted');
    }
    function resizeCanvas() {
      if (!canvas) return;
      // Size the backing store to the STAGE (the pinned 100dvh viewport box the
      // canvas actually displays in), NOT sectionEl — the section is N*100dvh
      // tall, so using its height made the backing store ~N× too tall and the
      // cover-crop draw squashed vertically (drone stretched horizontally).
      const rect = stage.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round((rect.width || window.innerWidth) * dpr);
      canvas.height = Math.round((rect.height || window.innerHeight) * dpr);
    }

    if (N) { // poster = first frame of the first beat's range
      const lo0 = beats[0].range ? beats[0].range[0] : 0;
      poster.src = dirLQ ? frameUrlLQ(lo0) : (dir ? frameUrl(lo0) : '');
    }

    // ---- progress + beat mapping ------------------------------------------
    // p=0 when the section's top hits the scroll root's top; p=1 once the
    // section's tall body (chapter min-heights, from CSS) has scrolled past
    // the pinned window. Beats partition [0,1] evenly.
    function readProgress() {
      const rect = sectionEl.getBoundingClientRect();
      const rootRect = (scrollRoot === document.scrollingElement || scrollRoot === document.documentElement)
        ? { top: 0, height: window.innerHeight }
        : scrollRoot.getBoundingClientRect();
      const travel = rect.height - rootRect.height;
      return travel > 0 ? clamp(-(rect.top - rootRect.top) / travel, 0, 1) : 0;
    }
    function beatFrameIndex(p) {
      if (!N) return { idx: 0, beatIdx: -1 };
      const beatW = 1 / N;
      const beatIdx = clamp(Math.floor(p / beatW), 0, N - 1);
      const localP = clamp((p - beatIdx * beatW) / beatW, 0, 1);
      const beat = beats[beatIdx];
      const lo = beat.range ? beat.range[0] : 0;
      const hi = beat.range ? beat.range[1] : frameCount - 1;
      const idx = clamp(Math.round(lo + localP * (hi - lo)), 0, Math.max(0, frameCount - 1));
      return { idx: idx, beatIdx: beatIdx };
    }

    let armed = false, activeBeat = -1;

    function tick() {
      const p = readProgress();
      sectionEl.style.setProperty('--scrub-progress', p.toFixed(4)); // section root only

      const mapped = beatFrameIndex(p);
      if (mapped.beatIdx !== activeBeat) {
        activeBeat = mapped.beatIdx;
        chapters.forEach(function (c, i) { c.classList.toggle('is-active', i === activeBeat); });
        dots.forEach(function (d, i) { d.classList.toggle('is-active', i === activeBeat); });
      }

      if (reduce || !canvas || !frameCount) return; // reduced-motion/no-frames: never fetch/decode
      if (!armed) return; // IO disarmed: only progress/chapter bookkeeping runs, no decode

      lastIdx = mapped.idx;
      // Re-evaluated every tick (not cached at arm time) so a 3rd concurrent
      // armed section drops to LQ only while the cap is actually breached,
      // and recovers full HQ the moment armedCount falls back to <= ARM_CAP.
      const forceLQNow = armedCount > ARM_CAP;
      const win = forceLQNow ? 2 : 5;
      for (let d = -win; d <= win; d++) {
        requestFrameLQ(mapped.idx + d);
        if (!forceLQNow) requestFrame(mapped.idx + d);
      }
      releaseFar(mapped.idx);
      const img = bestImg(mapped.idx);
      if (img) paint(img);
    }

    // Full cache wipe (frames + LQ + both requested flags) — used on disarm
    // and unmount alike so an offscreen section holds at most its poster,
    // and so a stale `requested[i]=true` doesn't block re-fetching on re-arm.
    function clearCache() {
      for (let i = 0; i < frameCount; i++) {
        frames[i] = null; framesLQ[i] = null; requested[i] = false; requestedLQ[i] = false;
      }
    }

    // ---- IntersectionObserver = ARM / DISARM -----------------------------
    const io = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting && !armed) {
          armed = true; armedCount++;
          sectionEl.setAttribute('data-armed', 'true');
          resizeCanvas();
        } else if (!entry.isIntersecting && armed) {
          armed = false; armedCount = Math.max(0, armedCount - 1);
          sectionEl.setAttribute('data-armed', 'false');
          clearCache(); // offscreen must not keep holding decoded frames
        }
      });
    }, { threshold: 0 });
    io.observe(sectionEl);

    const ticker = getTicker();
    if (ticker) ticker.add(tick);

    function onResize() { resizeCanvas(); }
    window.addEventListener('resize', onResize);

    return function unmount() {
      alive = false; // late in-flight decodes from before unmount must no-op
      io.disconnect();
      if (ticker) ticker.remove(tick);
      window.removeEventListener('resize', onResize);
      if (armed) armedCount = Math.max(0, armedCount - 1);
      armed = false;
      clearCache();
    };
  }

  return mountScrubSection;
});
