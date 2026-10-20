// Live Playwright harness for `js/capture.js` (T8.25 §10).
//
// Usage: node run_capture_tests.mjs [--out-dir <dir>] [--write-fixtures]
//   --out-dir <dir>     write every live capture as `<name>.capture.json` (the Rust tests parse these)
//   --write-fixtures    refresh the checked-in `fixtures/card*.capture.json` samples
//
// Every case runs in its own try/catch and prints `PASS <name>` / `FAIL <name>`; the process exits
// 1 when any case fails, so a TypeError thrown by the script can never pass silently.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

// `require` honours NODE_PATH (the Rust tests and CI point it at the Playwright install); an ES
// `import` of a bare specifier does not.
const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const playwrightVersion = require("playwright/package.json").version;

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const crateRoot = path.resolve(__dirname, "../..");
const fixturesDir = path.join(__dirname, "fixtures");
const captureScript = fs.readFileSync(path.join(crateRoot, "js", "capture.js"), "utf8");

const argv = process.argv.slice(2);
const writeFixtures = argv.includes("--write-fixtures");
const outDirFlag = argv.indexOf("--out-dir");
const outDir = outDirFlag >= 0 ? path.resolve(argv[outDirFlag + 1]) : null;
if (outDirFlag >= 0 && !argv[outDirFlag + 1]) {
  console.error("--out-dir needs a directory argument");
  process.exit(2);
}
if (outDir) fs.mkdirSync(outDir, { recursive: true });

async function launchBrowser() {
  try {
    const browser = await chromium.launch({ headless: true });
    console.log(`browser: playwright ${playwrightVersion}, bundled chromium ${browser.version()}`);
    return browser;
  } catch (err) {
    console.log(`bundled chromium unavailable (${String(err.message).split("\n")[0]}); using channel chrome`);
    const browser = await chromium.launch({ channel: "chrome", headless: true });
    console.log(`browser: playwright ${playwrightVersion}, chrome ${browser.version()}`);
    return browser;
  }
}

function indexById(root, map = new Map()) {
  if (!root) return map;
  if (root.id) map.set(root.id, root);
  for (const child of root.children || []) indexById(child, map);
  return map;
}

function walk(root, visit) {
  visit(root);
  for (const child of root.children || []) walk(child, visit);
}

function need(byId, id) {
  const el = byId.get(id);
  assert.ok(el, `element #${id} must be captured`);
  return el;
}

/** Writes the capture for the Rust side; fixtures keep a machine-independent url. */
function saveCapture(name, capture, { fixture = false } = {}) {
  if (outDir) {
    fs.writeFileSync(path.join(outDir, `${name}.capture.json`), JSON.stringify(capture) + "\n");
  }
  if (fixture && writeFixtures) {
    const sample = { ...capture, url: `file:///fixtures/${name.split(".")[0]}.html` };
    fs.writeFileSync(
      path.join(fixturesDir, `${name}.capture.json`),
      JSON.stringify(sample, null, 2) + "\n"
    );
  }
}

const cases = [];
function defineCase(name, fn) {
  cases.push({ name, fn });
}

async function openFixture(context, fixtureName) {
  const page = await context.newPage();
  await page.goto(pathToFileURL(path.join(fixturesDir, fixtureName)).href, { waitUntil: "load" });
  await page.addScriptTag({ content: captureScript });
  return page;
}

function runCapture(page, options) {
  return page.evaluate((opts) => window.__designosCapture(opts), options || {});
}

// ---------------------------------------------------------------------------------------------
// T8.25 §10 core cases
// ---------------------------------------------------------------------------------------------

defineCase("boxes_styles_runs_images", async ({ context }) => {
  const page = await openFixture(context, "card.html");
  const capture = await runCapture(page);
  saveCapture("card", capture, { fixture: true });

  assert.equal(capture.schema, "designos-webcapture");
  assert.equal(capture.version, 1);
  assert.equal(capture.viewport.width, 1280);
  assert.equal(capture.viewport.height, 800);
  assert.equal(capture.truncated, false);
  assert.ok(capture.root, "capture.root must exist");

  const byId = indexById(capture.root);
  const card = need(byId, "pricing-card");

  // §10: the card box equals Playwright's boundingBox() within 0.5 px (the page does not scroll).
  const bb = await page.locator("#pricing-card").boundingBox();
  assert.ok(bb, "Playwright must report a boundingBox for #pricing-card");
  for (const [captured, expected, label] of [
    [card.box.x, bb.x, "x"],
    [card.box.y, bb.y, "y"],
    [card.box.width, bb.width, "width"],
    [card.box.height, bb.height, "height"],
  ]) {
    assert.ok(
      Math.abs(captured - expected) <= 0.5,
      `card box ${label} ${captured} must be within 0.5px of boundingBox ${expected}`
    );
  }

  assert.equal(card.styles["display"], "flex");
  assert.equal(card.styles["flex-direction"], "column");
  assert.equal(card.styles["border-top-left-radius"], "12px");
  assert.ok(card.pseudo && card.pseudo.before, "card ::before pseudo box must be captured");
  assert.equal(card.pseudo.before.content, "PRO");

  const desc = need(byId, "desc");
  assert.ok(desc.text_runs && desc.text_runs.length >= 1, "desc should have text_runs");
  assert.ok(desc.text_runs[0].lines.length >= 2, "wrapping paragraph needs >= 2 line rects");
  for (const line of desc.text_runs[0].lines) {
    assert.ok(line.width > 0 && line.height > 0, "line rect must have positive size");
  }

  const avatarImg = need(byId, "avatar-img");
  assert.ok(avatarImg.image && !avatarImg.image.blocked, "avatar-img must not be blocked");
  assert.ok(
    typeof avatarImg.image.data_url === "string" &&
      avatarImg.image.data_url.startsWith("data:image/png;base64,"),
    "avatar-img must be a data:image/png;base64 URL"
  );

  const badge = need(byId, "badge-svg");
  assert.ok(badge.image && badge.image.svg.includes("<svg"), "badge-svg must carry its svg");

  const spark = need(byId, "spark-canvas");
  assert.ok(spark.image.data_url.startsWith("data:image/png"), "canvas must be a PNG data_url");

  const banner = need(byId, "bg-banner");
  assert.ok(banner.image || (banner.images && banner.images.length > 0), "bg-banner needs an image");

  assert.equal(byId.has("secret-hidden"), false, "display:none must be skipped by default");
});

defineCase("css_var_refs", async ({ context }) => {
  const page = await openFixture(context, "card.html");
  const capture = await runCapture(page);
  assert.equal(capture.root.css_vars_declared["--brand"], "#0d99ff");
  const btn = need(indexById(capture.root), "cta-btn");
  assert.deepEqual(btn.css_var_refs["background-color"], ["--brand"]);
});

defineCase("component_hint", async ({ context }) => {
  const page = await openFixture(context, "card.html");
  const capture = await runCapture(page);
  assert.equal(need(indexById(capture.root), "pricing-card").component_hint, "PricingCard");
});

defineCase("cross_origin_image_blocked_recorded", async ({ context }) => {
  const page = await openFixture(context, "card.html");
  const capture = await runCapture(page);
  const blocked = need(indexById(capture.root), "blocked-img");
  assert.equal(blocked.image.blocked, true);
  assert.ok(blocked.image.src.includes("unreachable-cross-origin.png"));
});

defineCase("truncation", async ({ context }) => {
  const page = await openFixture(context, "card.html");
  const full = await runCapture(page);
  const truncated = await runCapture(page, { max_elements: 4 });
  saveCapture("card.truncated", truncated, { fixture: true });
  assert.equal(truncated.truncated, true);
  assert.equal(truncated.stats.elements, 4);
  // Every element the full walk captured and the budget stopped before is counted, not just the
  // first one past the limit.
  assert.equal(
    truncated.stats.skipped_elements,
    full.stats.elements - truncated.stats.elements,
    "skipped_elements must count every unvisited element"
  );
});

defineCase("performance_5000_elements", async ({ context }) => {
  const page = await context.newPage();
  await page.setContent(`<!DOCTYPE html><html><body><div id="root"></div><script>
    const root = document.getElementById("root");
    const frag = document.createDocumentFragment();
    for (let i = 0; i < 4995; i++) {
      const d = document.createElement("div");
      d.style.display = "inline-block";
      d.style.width = "16px";
      d.style.height = "16px";
      d.textContent = "x";
      frag.appendChild(d);
    }
    root.appendChild(frag);
  </script></body></html>`);
  await page.addScriptTag({ content: captureScript });
  const capture = await runCapture(page);
  assert.ok(capture.stats.elements >= 4995, `expected >= 4995 elements, got ${capture.stats.elements}`);
  assert.ok(
    capture.stats.elapsed_ms <= 1500,
    `5,000-element capture took ${capture.stats.elapsed_ms}ms (budget <= 1500ms)`
  );
  perfMs = capture.stats.elapsed_ms;
});
let perfMs = null;

// ---------------------------------------------------------------------------------------------
// Review fixes: paint order, scroll offsets, cascade, pseudo origin
// ---------------------------------------------------------------------------------------------

defineCase("paint_index_dense_and_stacking_aware", async ({ context }) => {
  const page = await openFixture(context, "stacking.html");
  const capture = await runCapture(page);
  saveCapture("stacking", capture);
  const byId = indexById(capture.root);
  const z = (id) => need(byId, id).z;

  // A raw z-index above 2,147,483 once overflowed the Rust i32 field.
  assert.equal(need(byId, "overlay").styles["z-index"], "2147483647");
  const all = [];
  walk(capture.root, (el) => all.push(el.z));
  for (const value of all) {
    assert.ok(Number.isInteger(value) && value >= 0 && value <= 2147483647, `z ${value} out of i32 range`);
  }
  assert.equal(new Set(all).size, all.length, "z must be a unique paint index per element");
  assert.equal(Math.max(...all), all.length - 1, "z must be a dense 0..n-1 index");

  // Paint order: negative layer, in-flow, positioned auto, then positive contexts. The child of
  // the z-index:1 parent paints with its parent, after the later positioned uncle.
  assert.ok(z("behind") < z("flow-first"), "z-index:-1 paints below in-flow content");
  assert.ok(z("flow-first") < z("opacity-ctx"), "in-flow opacity context follows earlier sibling");
  assert.ok(z("opacity-ctx") < z("opacity-child"), "opacity context paints its child atomically inside it");
  assert.ok(z("opacity-child") < z("flow-after-opacity"), "opacity sibling and its child stay below the following in-flow sibling");
  assert.ok(z("flow-after-opacity") < z("transform-ctx"), "transform context follows earlier in-flow sibling");
  assert.ok(z("transform-ctx") < z("flow-after-transform"), "translateZ(0) sibling stays below the following in-flow sibling");
  assert.ok(z("flex-opacity") < z("flex-plain"), "flex item with opacity only stays below following in-flow flex sibling");
  assert.ok(z("flex-plain") < z("flex-z0"), "flex item with explicit z-index:0 paints in layers.zero above in-flow siblings");
  assert.ok(z("flow-first") < z("uncle-auto"), "positioned auto paints above in-flow content");
  assert.ok(z("flow-after-transform") < z("uncle-auto"), "positioned auto paints above in-flow stacking contexts");
  assert.ok(z("uncle-auto") < z("parent-z1"), "z-index:1 context paints above positioned auto");
  assert.ok(z("parent-z1") < z("child-in-z1"), "a child paints after its stacking-context parent");
  assert.ok(z("child-in-z1") < z("overlay"), "the huge z-index context paints last");
});

defineCase("fixed_and_sticky_use_scroll_offsets", async ({ context }) => {
  const page = await openFixture(context, "scrolled.html");
  await page.evaluate(() => window.scrollTo(0, 500));
  const capture = await runCapture(page);
  saveCapture("scrolled", capture);
  const byId = indexById(capture.root);
  const sticky = need(byId, "sticky-bar");
  const fixed = need(byId, "fixed-bar");
  assert.equal(sticky.styles["position"], "sticky");
  assert.equal(fixed.styles["position"], "fixed");
  assert.ok(Math.abs(sticky.box.y - 500) <= 0.5, `sticky y ${sticky.box.y} must be 500`);
  assert.ok(Math.abs(fixed.box.y - 500) <= 0.5, `fixed y ${fixed.box.y} must be 500 (page coordinates)`);
  assert.ok(Math.abs(fixed.box.x - 300) <= 0.5, `fixed x ${fixed.box.x} must be 300`);
});

defineCase("css_var_refs_winning_declaration", async ({ context }) => {
  const page = await openFixture(context, "cascade.html");
  await page.waitForFunction(() => {
    const f = document.getElementById("frame");
    return f && f.contentDocument && f.contentDocument.getElementById("ibtn");
  });
  const capture = await runCapture(page);
  saveCapture("cascade", capture);
  const byId = indexById(capture.root);
  const bg = (id) => (need(byId, id).css_var_refs || {})["background-color"];
  const fail = [];
  const expect = (label, actual, expected) => {
    try {
      assert.deepEqual(actual, expected, label);
    } catch (err) {
      fail.push(err.message.split("\n")[0] + ` -> got ${JSON.stringify(actual)}`);
    }
  };

  expect("media rule that does not match at 1280px", bg("m"), ["--brand"]);
  expect("unlayered beats a layered rule", bg("u"), ["--brand"]);
  expect("later layer beats earlier layer", bg("lyr"), ["--theme"]);
  expect("!important reverses layer order", bg("lyr-imp"), ["--base"]);
  expect("later literal clears the earlier var()", bg("lit"), undefined);
  expect("higher specificity wins over later order", bg("spec"), ["--other"]);
  expect("inline literal clears the var()", bg("inline-lit"), undefined);
  expect(
    "custom property chain follows the winning definition by specificity",
    bg("cp-el"),
    ["--x", "--b"]
  );
  expect(
    "!important custom property definition wins",
    (need(byId, "imp-el").css_var_refs || {})["color"],
    ["--y", "--a"]
  );
  expect("@container override that cannot be evaluated emits no stale ref", bg("cq"), undefined);

  expect("@supports applies only when the condition holds", bg("sup"), ["--other"]);
  const border = need(byId, "bd").css_var_refs || {};
  expect("border shorthand token feeds the colour longhand", border["border-top-color"], ["--brand"]);
  expect("border shorthand token does not feed the width longhand", border["border-top-width"], undefined);

  // A shorthand that is one var() feeds every longhand it covers, even for rem/em tokens whose
  // computed value (8px) never equals the token text (0.5rem).
  const tok = need(byId, "tok").css_var_refs || {};
  for (const side of ["top", "right", "bottom", "left"]) {
    expect(`padding: var(--space) feeds padding-${side}`, tok[`padding-${side}`], ["--space"]);
  }
  for (const corner of ["top-left", "top-right", "bottom-right", "bottom-left"]) {
    expect(`border-radius: var(--radius) feeds ${corner}`, tok[`border-${corner}-radius`], ["--radius"]);
  }
  for (const prop of ["gap", "row-gap", "column-gap"]) {
    expect(`gap: var(--gap) feeds ${prop}`, tok[prop], ["--gap"]);
  }

  // Shadow roots and same-origin iframes read their own stylesheets.
  expect("shadow root uses its own stylesheet", bg("inner-btn"), ["--shadow-brand"]);
  expect("iframe uses its own stylesheet", bg("ibtn"), ["--frame-brand"]);
  expect("parent rule does not leak into the iframe", bg("im"), undefined);
  expect("@media evaluates against the iframe viewport", bg("imed"), ["--narrow"]);

  assert.deepEqual(fail, [], "cascade mismatches:\n" + fail.join("\n"));
});

defineCase("pseudo_box_origin", async ({ context }) => {
  const page = await openFixture(context, "pseudo.html");
  const capture = await runCapture(page);
  saveCapture("pseudo", capture);
  const byId = indexById(capture.root);
  const near = (a, b, label) => assert.ok(Math.abs(a - b) <= 0.5, `${label}: ${a} vs ${b}`);

  // Absolute: padding edge of the positioned host (inside its 4px border) + inset.
  const abs = need(byId, "abs-host");
  const absBefore = abs.pseudo.before.box;
  near(absBefore.x, abs.box.x + 4 + 100, "absolute ::before x");
  near(absBefore.y, abs.box.y + 4 + 8, "absolute ::before y");

  // In flow: ::before starts the content box (plus its margin); ::after follows the last child.
  const flow = need(byId, "flow-host");
  const kid = need(byId, "flow-kid");
  near(flow.pseudo.before.box.x, flow.content_box.x + 5, "in-flow ::before x");
  near(flow.pseudo.before.box.y, flow.content_box.y + 3, "in-flow ::before y");
  near(flow.pseudo.after.box.x, flow.content_box.x + 5, "in-flow ::after x");
  near(flow.pseudo.after.box.y, kid.box.y + kid.box.height + 3, "in-flow ::after y");
});

// ---------------------------------------------------------------------------------------------

async function main() {
  const browser = await launchBrowser();
  const results = [];
  try {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    for (const { name, fn } of cases) {
      try {
        await fn({ context });
        console.log(`PASS ${name}`);
        results.push({ name, ok: true });
      } catch (err) {
        console.log(`FAIL ${name}: ${err && err.message ? err.message : err}`);
        results.push({ name, ok: false });
      }
    }
  } finally {
    await browser.close();
  }
  const failed = results.filter((r) => !r.ok).map((r) => r.name);
  console.log(
    JSON.stringify({ ok: failed.length === 0, cases: results.length, failed, perf_5000_ms: perfMs })
  );
  process.exit(failed.length === 0 ? 0 : 1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
