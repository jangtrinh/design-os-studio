#!/usr/bin/env node
/* ============================================================================
   test-tenant.mjs — Playwright assertions for the Tenant contract, per the
   plan's 6 assertion groups (plans/specs/embeddable-scrub-section/plan.md #5).

   Serves embed-section/ on a local port, drives headless chromium against
   demo/index.html, and shells out to tenant-lint.mjs for group 6.
   Run: node test/test-tenant.mjs
   ========================================================================== */
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { chromium } from 'playwright';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..'); // embed-section/
const PORT = 8934;
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.webp': 'image/webp', '.mjs': 'text/javascript' };

let passed = 0, failed = 0;
function ok(label, cond, detail) {
  if (cond) { passed++; console.log(`  PASS ${label}`); }
  else { failed++; console.log(`  FAIL ${label}${detail ? ' — ' + detail : ''}`); }
}

function startServer() {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const urlPath = decodeURIComponent(req.url.split('?')[0]);
      const filePath = path.join(ROOT, urlPath === '/' ? '/demo/index.html' : urlPath);
      fs.readFile(filePath, (err, data) => {
        if (err) { res.writeHead(404); res.end('not found: ' + urlPath); return; }
        res.writeHead(200, { 'Content-Type': MIME[path.extname(filePath)] || 'application/octet-stream' });
        res.end(data);
      });
    });
    server.listen(PORT, () => resolve(server));
  });
}

async function main() {
  const server = await startServer();
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });

  // ---- instrumentation, installed BEFORE any page script runs -----------
  await page.addInitScript(() => {
    window.__scrollToCalls = 0;
    const nativeScrollTo = window.scrollTo.bind(window);
    window.scrollTo = function (...args) { window.__scrollToCalls++; return nativeScrollTo(...args); };
    // rAF call-vs-frame ratio: proves a single shared driver (assertion group 5).
    window.__rafCalls = 0;
    window.__rafFrames = 0;
    const nativeRAF = window.requestAnimationFrame.bind(window);
    window.requestAnimationFrame = function (cb) { window.__rafCalls++; return nativeRAF(cb); };
    (function frameCounter() { window.__rafFrames++; nativeRAF(frameCounter); })();
  });

  const consoleErrors = [];
  page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + e.message));
  page.on('console', (m) => { if (m.type() === 'error') consoleErrors.push('console: ' + m.text()); });

  await page.goto(`http://localhost:${PORT}/demo/index.html`, { waitUntil: 'load' });
  await page.waitForTimeout(300);

  // ======================================================================
  console.log('\n[1/6] Tenant Law — pure bounding-box reader');
  const bodyHeightBefore = await page.evaluate(() => document.body.style.height);
  // scroll the whole page, then re-check the instrumented globals + statics
  await page.evaluate(() => window.scroll(0, document.body.scrollHeight));
  await page.waitForTimeout(200);
  await page.evaluate(() => window.scroll(0, 0));
  await page.waitForTimeout(200);
  const scrollToCalls = await page.evaluate(() => window.__scrollToCalls);
  const bodyHeightAfter = await page.evaluate(() => document.body.style.height);
  const fixedNodes = await page.evaluate(() =>
    Array.from(document.querySelectorAll('.scrub, .scrub *'))
      .filter((el) => getComputedStyle(el).position === 'fixed').length
  );
  const rootProgress = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--scrub-progress').trim());
  ok('window.scrollTo is never called by the section', scrollToCalls === 0, `calls=${scrollToCalls}`);
  ok('document.body.style.height never mutated', bodyHeightBefore === '' && bodyHeightAfter === '', `before="${bodyHeightBefore}" after="${bodyHeightAfter}"`);
  ok('no element inside .scrub has computed position:fixed', fixedNodes === 0, `count=${fixedNodes}`);
  ok('no --scrub-progress on documentElement', rootProgress === '', `value="${rootProgress}"`);
  ok('no console/page errors during full-page scroll', consoleErrors.length === 0, consoleErrors.join('; '));

  // ======================================================================
  console.log('\n[2/6] Sticky pin');
  const aTop = await page.evaluate(() => document.getElementById('scrub-a').offsetTop);
  const aHeight = await page.evaluate(() => document.getElementById('scrub-a').getBoundingClientRect().height);
  await page.evaluate((y) => window.scroll(0, y), aTop + 5);
  await page.waitForTimeout(150);
  const stagePosition = await page.evaluate(() => getComputedStyle(document.querySelector('#scrub-a .scrub__stage')).position);
  ok('.scrub__stage computed position is sticky', stagePosition === 'sticky', stagePosition);

  const travel = aHeight - 800;
  const stageTops = [];
  for (const frac of [0.1, 0.4, 0.7, 0.9]) {
    await page.evaluate((y) => window.scroll(0, y), aTop + travel * frac);
    await page.waitForTimeout(120);
    stageTops.push(await page.evaluate(() => document.querySelector('#scrub-a .scrub__stage').getBoundingClientRect().top));
  }
  ok('stage top stays ~0 while pinned within the section', stageTops.every((t) => Math.abs(t) < 2), JSON.stringify(stageTops));

  await page.evaluate((y) => window.scroll(0, y), aTop + aHeight + 300); // well past the section
  await page.waitForTimeout(150);
  const stageTopAfter = await page.evaluate(() => document.querySelector('#scrub-a .scrub__stage').getBoundingClientRect().top);
  ok('stage unpins once the section has scrolled past', stageTopAfter < -100 || stageTopAfter > 100, `top=${stageTopAfter}`);

  // canvas backing-store aspect must match its display box, else the frame is
  // squashed/stretched (regression guard: backing store was sized to the tall
  // section instead of the 100dvh stage → horizontal stretch).
  await page.evaluate((y) => window.scroll(0, y), aTop + 150);
  await page.waitForTimeout(200);
  const asp = await page.evaluate(() => {
    const c = document.querySelector('#scrub-a .scrub__canvas'); const r = c.getBoundingClientRect();
    return { back: c.width / c.height, disp: r.width / r.height };
  });
  ok('canvas backing-store aspect matches display box (no stretch)', Math.abs(asp.back - asp.disp) < 0.02, `back=${asp.back.toFixed(3)} disp=${asp.disp.toFixed(3)}`);

  // ======================================================================
  console.log('\n[3/6] Section-scoped progress');
  await page.evaluate((y) => window.scroll(0, y), aTop + travel / 2);
  await page.waitForTimeout(200);
  const progMid = parseFloat(await page.evaluate(() => getComputedStyle(document.getElementById('scrub-a')).getPropertyValue('--scrub-progress')));
  ok('progress at mid-section ~= 0.5 (+/-0.15)', Math.abs(progMid - 0.5) <= 0.15, `progress=${progMid}`);

  // ======================================================================
  console.log('\n[4/6] Coexistence');
  const canvasCount = await page.evaluate(() => document.querySelectorAll('.scrub__canvas').length);
  ok('both sections mount a canvas', canvasCount === 2, `count=${canvasCount}`);

  const bTop = await page.evaluate(() => document.getElementById('scrub-b').offsetTop);
  const bHeight = await page.evaluate(() => document.getElementById('scrub-b').getBoundingClientRect().height);
  await page.evaluate((y) => window.scroll(0, y), bTop + (bHeight - 800) / 2);
  await page.waitForTimeout(300);
  const armedA = await page.evaluate(() => document.getElementById('scrub-a').getAttribute('data-armed'));
  const armedB = await page.evaluate(() => document.getElementById('scrub-b').getAttribute('data-armed'));
  const onlyBArmed = armedB === 'true' && armedA !== 'true';
  ok('only one section is armed at the B midpoint', onlyBArmed, `armedA=${armedA} armedB=${armedB}`);

  const pricingBox = await page.evaluate(() => {
    const el = document.querySelector('.site-pricing');
    const r = el.getBoundingClientRect();
    return { display: getComputedStyle(el).display, position: getComputedStyle(el).position, height: r.height };
  });
  ok('sibling pricing section stays a normal-flow block', pricingBox.display === 'block' && pricingBox.position === 'static' && pricingBox.height > 0, JSON.stringify(pricingBox));

  // ======================================================================
  console.log('\n[5/6] Perf');
  await page.evaluate((y) => window.scroll(0, y), 0);
  await page.waitForTimeout(150);
  const perf = await page.evaluate(() => new Promise((resolve) => {
    const total = document.body.scrollHeight - window.innerHeight;
    const deltas = [];
    let last = performance.now(), y = 0;
    function step() {
      const now = performance.now();
      deltas.push(now - last);
      last = now;
      y += total / 100;
      window.scroll(0, Math.min(y, total));
      if (y < total) requestAnimationFrame(step);
      else resolve(deltas);
    }
    requestAnimationFrame(step);
  }));
  const avgDelta = perf.reduce((a, b) => a + b, 0) / perf.length;
  const fps = 1000 / avgDelta;
  ok(`fps >= 55 during top-to-bottom scroll (measured ${fps.toFixed(1)})`, fps >= 55, `avgDelta=${avgDelta.toFixed(2)}ms`);

  const tickerInfo = await page.evaluate(() => ({
    exists: !!window.__scrubTicker,
    subs: window.__scrubTicker ? window.__scrubTicker.subs.size : 0,
    rafCalls: window.__rafCalls,
    rafFrames: window.__rafFrames,
  }));
  ok('window.__scrubTicker exists with both sections subscribed', tickerInfo.exists && tickerInfo.subs === 2, JSON.stringify(tickerInfo));
  const rafRatio = tickerInfo.rafCalls / tickerInfo.rafFrames;
  ok('exactly one rAF driver (rAF-calls-per-frame ratio ~= 1)', rafRatio < 1.5, `ratio=${rafRatio.toFixed(2)} calls=${tickerInfo.rafCalls} frames=${tickerInfo.rafFrames}`);

  // ======================================================================
  // Regression for review FIX 2: a single-beat section's story naturally
  // sums to exactly 100dvh (== the stage), so travel (rect.height -
  // rootHeight) used to collapse to 0 and progress could never leave 0.
  // Mounted/unmounted on its own — must NOT change the [5/6] ticker-subs
  // count already asserted above, so this runs after that check, and is
  // fully torn down before group 6.
  console.log('\n[bonus] single-beat section still scrubs 0 -> 1 (FIX 2 regression)');
  const singleBeatHeight = await page.evaluate(() => new Promise((resolve) => {
    const el = document.createElement('section');
    el.id = 'scrub-single-beat-test';
    document.body.appendChild(el);
    window.__singleBeatUnmount = window.mountScrubSection(el, {
      frames: 'assets/seq/drone/',
      framesLQ: 'assets/seq/drone-lq/',
      frameCount: 116,
      beats: [{ range: [0, 115], eyebrow: 'Solo', title: 'Single beat', body: 'One chapter only.' }],
    });
    requestAnimationFrame(() => requestAnimationFrame(() => resolve(el.getBoundingClientRect().height)));
  }));
  const sbTop = await page.evaluate(() => document.getElementById('scrub-single-beat-test').offsetTop);
  const sbTravel = singleBeatHeight - 800;
  ok('single-beat section has real scroll travel (story min-height floor)', sbTravel > 0, `height=${singleBeatHeight}`);

  await page.evaluate((y) => window.scroll(0, y), sbTop + 2);
  await page.waitForTimeout(150);
  const pStart = parseFloat(await page.evaluate(() => getComputedStyle(document.getElementById('scrub-single-beat-test')).getPropertyValue('--scrub-progress')));
  await page.evaluate((y) => window.scroll(0, y), sbTop + Math.max(sbTravel - 2, 0));
  await page.waitForTimeout(150);
  const pEnd = parseFloat(await page.evaluate(() => getComputedStyle(document.getElementById('scrub-single-beat-test')).getPropertyValue('--scrub-progress')));
  ok('single-beat progress starts near 0', pStart < 0.1, `p=${pStart}`);
  ok('single-beat progress reaches near 1 at the end of its travel', pEnd > 0.9, `p=${pEnd}`);

  await page.evaluate(() => {
    if (window.__singleBeatUnmount) window.__singleBeatUnmount();
    const el = document.getElementById('scrub-single-beat-test');
    if (el) el.remove();
    delete window.__singleBeatUnmount;
  });

  await browser.close();
  server.close();

  // ======================================================================
  console.log('\n[6/6] tenant-lint gate (positive + negative fixture)');
  try {
    const out = execFileSync('node', ['tenant-lint.mjs', 'demo/index.html'], { cwd: ROOT, encoding: 'utf8' });
    ok('tenant-lint PASSes demo/index.html', /TENANT-LINT: PASS/.test(out), out.trim());
  } catch (e) {
    ok('tenant-lint PASSes demo/index.html', false, (e.stdout || e.message).trim());
  }
  try {
    execFileSync('node', ['tenant-lint.mjs', 'test/bad-host.html'], { cwd: ROOT, encoding: 'utf8' });
    ok('tenant-lint FAILs test/bad-host.html', false, 'lint exited 0 (should have failed)');
  } catch (e) {
    const out = (e.stdout || '').trim();
    ok('tenant-lint FAILs test/bad-host.html', e.status === 1 && /TENANT-LINT: FAIL/.test(out), out);
  }

  console.log(`\n${passed} passed, ${failed} failed`);
  console.log(failed === 0 ? 'VERDICT: PASS' : 'VERDICT: FAIL');
  process.exit(failed === 0 ? 0 : 1);
}

main().catch((e) => {
  console.error('TEST HARNESS ERROR:', e);
  console.log('VERDICT: FAIL');
  process.exit(1);
});
