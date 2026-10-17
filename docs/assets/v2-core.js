/* v2-core.js: host conductor for the v2 scroll story.
   One rAF loop for the whole page: gsap.ticker clocks GSAP, Lenis, the Tenant engine
   (via window.__scrubTicker) and every scene. Scenes register { mount, unmount }.
   Motion mode = no reduced-motion preference and a viewport of 800px or more. Anywhere else
   the page stays the static, final-state layout (no pins, no scrub, no smooth scroll). */
(function () {
  'use strict';
  var html = document.documentElement;
  var V2 = window.V2 = window.V2 || {};
  V2.scenes = V2.scenes || [];
  var gsap = window.gsap;
  if (!gsap) { html.classList.remove('v2-pre'); return; } // CDN unreachable: the static page stands

  // Tenant clause 1: one page-level ticker, defined before any section mounts.
  window.__scrubTicker = {
    add: function (f) { gsap.ticker.add(f); },
    remove: function (f) { gsap.ticker.remove(f); }
  };
  gsap.ticker.lagSmoothing(0); // Lenis is clocked by this ticker: no catch-up jumps

  // Shared helpers for scenes.
  V2.clamp = function (x, a, b) { return Math.min(b == null ? 1 : b, Math.max(a == null ? 0 : a, x)); };
  V2.smooth = function (x) { x = V2.clamp(x); return x * x * (3 - 2 * x); };

  // Pin a scene with the Tenant engine (emitted verbatim by ui tenant-scaffold): the engine builds a
  // position:sticky stage inside `section`, subscribes to the shared ticker and arms/disarms on
  // IntersectionObserver. The pre-authored `content` (static, final-state markup) moves into its stage.
  // Progress is read from the section's own bounding box only.
  V2.pinScene = function (section, content, travelVh) {
    var unmountEngine = window.mountScrubSection(section, { beats: [] });
    var story = section.querySelector('.scrub__story');
    var stage = section.querySelector('.scrub__stage');
    story.style.minHeight = (100 + travelVh) + 'dvh'; // section height = stage + travel
    stage.appendChild(content);
    return {
      stage: stage,
      armed: function () { return section.getAttribute('data-armed') === 'true'; },
      progress: function () {
        var r = section.getBoundingClientRect(), travel = r.height - window.innerHeight;
        return travel > 0 ? V2.clamp(-r.top / travel) : 0;
      },
      unpin: function () {
        unmountEngine();
        section.insertBefore(content, section.firstChild);
        ['.scrub__stage', '.scrub__progress', '.scrub__story', '.scrub__route'].forEach(function (sel) {
          var el = section.querySelector(sel);
          if (el) el.parentNode.removeChild(el);
        });
        section.classList.remove('scrub', 'is-painted');
        section.removeAttribute('data-armed');
        section.style.removeProperty('--scrub-progress');
      }
    };
  };

  var mqReduce = matchMedia('(prefers-reduced-motion: reduce)');
  var mqWide = matchMedia('(min-width: 800px)');
  V2.motion = function () { return !mqReduce.matches && mqWide.matches; };

  // Smooth scroll: Lenis in lerp mode, on the shared ticker. Off below 800px and under reduced motion.
  var lenis = null, lenisTick = null;
  function lenisOn() {
    if (lenis || !window.Lenis) return;
    lenis = new window.Lenis({ lerp: 0.1, autoRaf: false, anchors: { offset: -72 } });
    lenisTick = function (time) { lenis.raf(time * 1000); };
    gsap.ticker.add(lenisTick);
    V2.lenis = lenis;
  }
  function lenisOff() {
    if (!lenis) return;
    gsap.ticker.remove(lenisTick);
    lenis.destroy();
    lenis = null; lenisTick = null; V2.lenis = null;
  }

  // Scene 1: header and hero cascade. y 24, blur 6px, power2.out, 1.1s, 0.16s offsets, props cleared after.
  var entered = false;
  function entrance() {
    if (entered) return;
    entered = true;
    var pick = function (s) { return Array.prototype.slice.call(document.querySelectorAll(s)); };
    var headerEls = pick('.topbar .brand, .topbar .navlinks li, .topbar .btn');
    var blocks = ['.hero h1', '.hero .sub', '.hero .cta', '.hero .cta-note', '.hero .film']
      .map(function (s) { return document.querySelector(s); }).filter(Boolean);
    var all = headerEls.concat(blocks);
    gsap.set(all, { opacity: 0, y: 24, filter: 'blur(6px)' });
    html.classList.remove('v2-pre');
    var shown = { opacity: 1, y: 0, filter: 'blur(0px)', duration: 1.1, ease: 'power2.out' };
    var tl = gsap.timeline({ onComplete: function () { gsap.set(all, { clearProps: 'opacity,transform,filter' }); } });
    tl.to(headerEls, Object.assign({ stagger: 0.07 }, shown), 0);
    blocks.forEach(function (el, i) { tl.to(el, shown, 0.16 * (i + 1)); });
    // The selection box draws itself around the headline once the headline is nearly settled.
    var n = V2.narrator;
    if (n && n.intro) {
      var intro = n.intro;
      intro.draw = intro.handles = intro.pill = intro.count = 0;
      tl.to(intro, { draw: 1, duration: 0.8, ease: 'power1.inOut' }, 0.35)
        .to(intro, { handles: 1, duration: 0.5, ease: 'back.out(2.2)' }, 1.05)
        .to(intro, { pill: 1, duration: 0.3, ease: 'power2.out' }, 1.25)
        .to(intro, { count: 1, duration: 0.8, ease: 'power2.out' }, 1.25);
    }
  }

  var mounted = false;
  function apply() {
    var on = V2.motion();
    if (on === mounted) return;
    mounted = on;
    html.classList.toggle('v2-motion', on);
    if (on) {
      lenisOn();
      V2.scenes.forEach(function (s) { if (s.mount) s.mount(); });
      entrance();
    } else {
      V2.scenes.slice().reverse().forEach(function (s) { if (s.unmount) s.unmount(); });
      lenisOff();
      html.classList.remove('v2-pre');
    }
  }
  function listen(mq) { if (mq.addEventListener) mq.addEventListener('change', apply); else mq.addListener(apply); }
  listen(mqReduce); listen(mqWide);
  // Deferred scripts run before DOMContentLoaded, so every scene has registered by then.
  if (document.readyState === 'complete') apply(); else document.addEventListener('DOMContentLoaded', apply);
})();
