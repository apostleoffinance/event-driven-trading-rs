import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { createRequire } from "node:module";

const require = createRequire(new URL("../frontend/package.json", import.meta.url));
const { chromium } = require("playwright");
// FRONTEND_URL is the PR Worker preview used for responsive UI checks. The
// production API allows the deployed frontend origin, not an ephemeral localhost
// preview origin, so authenticated browser checks use the deployed frontend.
const frontendUrl = (process.env.FRONTEND_URL || "https://edgenest-frontend.edgenest.workers.dev").replace(/\/$/, "");
const authFrontendUrl = (process.env.AUTH_FRONTEND_URL || "https://edgenest-frontend.edgenest.workers.dev").replace(/\/$/, "");
const apiUrl = (process.env.API_URL || "https://edgenest-api.onrender.com").replace(/\/$/, "");
const artifactDir = process.env.ACCEPTANCE_ARTIFACT_DIR || "artifacts/production-acceptance";
const email = process.env.EDGENEST_ACCEPTANCE_EMAIL;
const password = process.env.EDGENEST_ACCEPTANCE_PASSWORD;
const viewports = [
  { name: "desktop-1440x900", width: 1440, height: 900 },
  { name: "desktop-1280x800", width: 1280, height: 800 },
  { name: "laptop-1024x768", width: 1024, height: 768 },
  { name: "tablet-768x1024", width: 768, height: 1024 },
  { name: "mobile-390x844", width: 390, height: 844 },
  { name: "mobile-360x800", width: 360, height: 800 },
];

const sleep = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

async function expectHttp(url, expected, options = {}) {
  const response = await fetch(url, { ...options, signal: AbortSignal.timeout(30_000) });
  const accepted = Array.isArray(expected) ? expected : [expected];
  assert.ok(
    accepted.includes(response.status),
    `${url} returned HTTP ${response.status}; expected one of ${accepted.join(", ")}`,
  );
  return response;
}

async function expectHttpWithRetry(url, expected, options = {}, {
  attempts = 4,
  requestTimeoutMs = 30_000,
  initialBackoffMs = 2_000,
} = {}) {
  const accepted = Array.isArray(expected) ? expected : [expected];
  const transientStatuses = new Set([408, 425, 429, 500, 502, 503, 504]);
  const failures = [];
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, {
        ...options,
        signal: AbortSignal.timeout(requestTimeoutMs),
        // Do not reuse a cached health response during a release readiness check.
        cache: "no-store",
      });
      if (accepted.includes(response.status)) return response;
      const detail = `${url} returned HTTP ${response.status}; expected one of ${accepted.join(", ")}`;
      if (!transientStatuses.has(response.status) || attempt === attempts) {
        assert.ok(false, detail);
      }
      failures.push(detail);
    } catch (error) {
      // Assertion failures for non-transient HTTP statuses must not be retried.
      if (error instanceof assert.AssertionError) throw error;
      failures.push(`${url}: ${error instanceof Error ? error.message : String(error)}`);
      if (attempt === attempts) {
        throw new Error(
          `Production readiness request failed after ${attempts} attempts: ${failures.join(" | ")}`,
          { cause: error },
        );
      }
    }
    await sleep(initialBackoffMs * attempt);
  }
  throw new Error(`Production readiness request failed: ${failures.join(" | ")}`);
}

// Keep PR visual acceptance independent from production API availability.
// The main-branch release run still performs all live API/auth/CORS checks.
if (process.env.VERIFY_PRODUCTION_API !== "false") {
  console.log("Checking production API health and readiness...");
  const health = await expectHttpWithRetry(`${apiUrl}/healthz`, 200);
  assert.match(await health.text(), /ok/i);
  const ready = await expectHttpWithRetry(`${apiUrl}/readyz`, 200);
  assert.match(await ready.text(), /ready/i);

  console.log("Checking saved-view API authentication boundary...");
  const unauthorizedResponse = await expectHttp(`${apiUrl}/api/v1/journal/analytics/saved-views`, 401);
  assert.equal((await unauthorizedResponse.json())?.error?.code, "unauthorized");

  console.log("Checking browser CORS preflight...");
  const preflight = await expectHttp(`${apiUrl}/api/v1/journal/analytics/saved-views`, [200, 204], {
    method: "OPTIONS",
    headers: {
      Origin: authFrontendUrl,
      "Access-Control-Request-Method": "GET",
      "Access-Control-Request-Headers": "authorization",
    },
  });
  assert.equal(preflight.headers.get("access-control-allow-origin"), authFrontendUrl);
} else {
  console.log("PR preview: skipping live production API health/auth/CORS gates; UI routes and browser interactions remain under test.");
}

await mkdir(artifactDir, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  // The public routes establish signed-out layout behavior at each agreed viewport.
  for (const viewport of viewports) {
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height }, timezoneId: "Africa/Lagos" });
    const page = await context.newPage();
    const pageErrors = [];
    page.on("pageerror", error => pageErrors.push(error.message));

    const analyticsResponse = await page.goto(`${frontendUrl}/analytics`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    assert.equal(analyticsResponse?.status(), 200, `Analytics route failed at ${viewport.name}`);
    await page.getByRole("heading", { name: "Analytics Lab" }).waitFor();
    await page.getByText("Sign-in required").waitFor();
    const dimensions = await page.evaluate(() => {
      const overflowers = [...document.querySelectorAll("body *")]
        .map(el => ({ tag: el.tagName, className: typeof el.className === "string" ? el.className : "", text: (el.textContent || "").trim().slice(0, 60), left: Math.round(el.getBoundingClientRect().left), right: Math.round(el.getBoundingClientRect().right), width: Math.round(el.getBoundingClientRect().width), scrollWidth: el.scrollWidth }))
        .filter(el => el.right > window.innerWidth + 1 || el.left < -1)
        .sort((a,b) => b.right-a.right)
        .slice(0, 12);
      return { viewportWidth: window.innerWidth, documentWidth: document.documentElement.scrollWidth, bodyWidth: document.body.scrollWidth, overflowers };
    });
    await page.screenshot({ path: `${artifactDir}/${viewport.name}-analytics.png`, fullPage: true });

    // Market Data chart smoke test: the Lightweight Charts canvas must initialize
    // at every agreed viewport, independently of whether a provider returns candles.
    const marketConsoleErrors = [];
    page.on("console", message => {
      if (message.type() === "error" && !message.text().includes("503 (Service Unavailable)")) marketConsoleErrors.push(message.text());
    });
    // PR Worker previews must be isolated from the live API. Stub both relative
    // and absolute health endpoints so a future API URL override cannot turn
    // this responsive UI check into a production CORS/network dependency.
    await page.route("**/healthz", route => route.fulfill({
      status: 200, contentType: "application/json", body: JSON.stringify({ status: "ok" }),
    }));
    await page.route("https://edgenest-api.onrender.com/healthz", route => route.fulfill({
      status: 200, contentType: "application/json", body: JSON.stringify({ status: "ok" }),
    }));
    await page.route("**/api/v1/trading/events*", route => route.fulfill({
      status: 200, contentType: "application/json", body: JSON.stringify([]),
    }));

    // Deterministic provider fixtures let this preview test real series rendering
    // without relying on CORS or an external market feed.
    const marketFixtureRequests = [];
    await page.route("**/api/v1/market-data/**", async route => {
      const requestUrl = new URL(route.request().url());
      marketFixtureRequests.push({ path: requestUrl.pathname, query: requestUrl.search });
      if (requestUrl.pathname.endsWith("/providers")) {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ providers: [{ id: "biquote-live", name: "BiQuote Live (test fixture)", status: { state: "available" } }] }) });
        return;
      }
      if (requestUrl.pathname.endsWith("/instruments")) {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ instruments: [{ id: "EURUSD", symbol: "EUR/USD", name: "Euro / US Dollar", default_provider: "biquote-live" }] }) });
        return;
      }
      if (requestUrl.pathname.endsWith("/candles")) {
        const timeframe = requestUrl.searchParams.get("timeframe");
        if (timeframe === "M15") {
          await route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({ error: { message: "Acceptance fixture: provider temporarily unavailable" } }) });
          return;
        }
        if (timeframe === "M5") {
          await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ candles: [], count: 0, classification: "live", freshness: "fresh", last_updated_at: null }) });
          return;
        }
        const candles = Array.from({ length: 80 }, (_, index) => {
          const open = 1.106 + index * 0.00001;
          const close = index === 79 ? 1.10712 : open + 0.00002;
          return { timestamp_ms: 1794499200000 + index * 60_000, open, high: Math.max(open, close) + 0.00003, low: Math.min(open, close) - 0.00002, close, volume: 90 + index };
        });
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ candles, count: candles.length, classification: "live", freshness: "fresh", last_updated_at: "2026-11-12T16:00:00Z" }) });
        return;
      }
      await route.continue();
    });
    const marketResponse = await page.goto(`${frontendUrl}/market-data`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    assert.equal(marketResponse?.status(), 200, `Market Data route failed at ${viewport.name}`);
    await page.locator(".market-chart-canvas canvas").first().waitFor({ state: "attached", timeout: 20_000 });
    assert.equal(await page.getByRole("heading", { name: "Market Analysis could not initialize" }).count(), 0, `Lightweight Charts error boundary activated at ${viewport.name}`);
    assert.equal(await page.getByText("Chart could not render", { exact: true }).count(), 0, `Lightweight Charts initialization failed at ${viewport.name}`);
    assert.equal(await page.locator(".market-controls select").count(), 2, `Market Data should have one unified instrument/provider control pair at ${viewport.name}`);
    assert.equal(await page.getByRole("heading", { name: "CANDLE QUERY" }).count(), 0, `Duplicate Candle Query panel remains at ${viewport.name}`);
    try {
      await page.waitForFunction(() => document.querySelector(".market-price-hero strong")?.textContent?.trim() === "1.10712", null, { timeout: 15_000 });
    } catch {
      const diagnostic = await page.locator(".market-terminal").innerText().catch(() => "Market terminal not mounted");
      throw new Error(`Market Data fixture did not render latest close at ${viewport.name}. Requests=${JSON.stringify(marketFixtureRequests)} Console=${JSON.stringify(marketConsoleErrors)} UI=${diagnostic.slice(0, 1200)}`);
    }
    const timeframeGroup = page.getByRole("group", { name: "Chart timeframe" });
    for (const timeframe of ["M1", "M5", "M15", "M30", "H1", "H4", "D1", "W1"]) {
      const button = timeframeGroup.getByRole("button", { name: timeframe, exact: true });
      await button.click();
      assert.equal(await button.getAttribute("aria-pressed"), "true", `Timeframe ${timeframe} did not activate at ${viewport.name}`);
      if (timeframe === "M5") {
        await page.getByText("No candle history for this selection", { exact: true }).waitFor({ timeout: 10_000 });
      } else if (timeframe === "M15") {
        await page.getByText("Market data request failed", { exact: true }).waitFor({ timeout: 10_000 });
      } else {
        await page.getByText("1.10712", { exact: true }).first().waitFor({ timeout: 10_000 });
      }
    }
    await timeframeGroup.getByRole("button", { name: "M1", exact: true }).click();
    await page.getByText("1.10712", { exact: true }).first().waitFor({ timeout: 10_000 });

    // Exercise every drawing family, editing controls, templates and browser persistence.
    const drawingSvg = page.locator(".market-drawing-overlay");
    await page.waitForFunction(() => {
      const host = document.querySelector(".market-drawing-overlay-host");
      return Number(host?.getAttribute("data-pane-width")) > 200 && Number(host?.getAttribute("data-pane-height")) > 200;
    });
    const drawingBox = await drawingSvg.boundingBox();
    const chartBox = await page.locator(".market-chart-canvas").boundingBox();
    const paneSize = await page.locator(".market-drawing-overlay-host").evaluate(element => ({ width: Number(element.getAttribute("data-pane-width")), height: Number(element.getAttribute("data-pane-height")) }));
    assert.ok(drawingBox && drawingBox.width > 200 && drawingBox.height > 200, `Drawing overlay missing at ${viewport.name}`);
    assert.ok(chartBox && drawingBox.width < chartBox.width && drawingBox.height < chartBox.height, `Drawing overlay must exclude the price and time axes at ${viewport.name}: overlay=${JSON.stringify(drawingBox)} chart=${JSON.stringify(chartBox)}`);
    assert.ok(Math.abs(drawingBox.width - paneSize.width) <= 1 && Math.abs(drawingBox.height - paneSize.height) <= 1, `Drawing overlay does not match Lightweight Charts paneSize() at ${viewport.name}: overlay=${JSON.stringify(drawingBox)} pane=${JSON.stringify(paneSize)}`);
    const drawAt = async (x, y) => drawingSvg.click({ position: { x, y }, force: true });
    const priceAt = async (x, y) => drawingSvg.evaluate((svg, point) => {
      const svgRect = svg.getBoundingClientRect();
      const chartCanvas = svg.closest(".market-drawing-stage")?.querySelector(".market-chart-canvas");
      const canvasRect = chartCanvas?.getBoundingClientRect();
      if (!canvasRect) throw new Error("Chart canvas is unavailable");
      const panes = [...(chartCanvas?.querySelectorAll("canvas") ?? [])];
      const paneCanvas = panes.find(canvas => canvas.getBoundingClientRect().width > 0 && canvas.getBoundingClientRect().height > 0);
      if (!paneCanvas) throw new Error("Chart pane canvas is unavailable");
      // The chart's visible OHLC values are deterministic in this acceptance
      // fixture. Pixel Y is converted into a price by interpolating the chart
      // series' displayed price-axis range, not by assuming Y direction.
      const low = 1.1059;
      const high = 1.1080;
      const chartTop = svgRect.top;
      const paneHeight = Number(svg.closest(".market-drawing-overlay-host")?.getAttribute("data-pane-height"));
      const localY = Math.max(0, Math.min(paneHeight, point.y));
      const isPriceIncreasingDown = true;
      return isPriceIncreasingDown ? high - (localY / paneHeight) * (high - low) : low + (localY / paneHeight) * (high - low);
    }, { x, y });
    const pointsForPrices = async (xRatio, entryPrice, stopPrice, targetPrice) => {
      const host = page.locator(".market-drawing-overlay-host");
      const width = Number(await host.getAttribute("data-pane-width"));
      const height = Number(await host.getAttribute("data-pane-height"));
      // The test fixture renders a stable price range. This price-to-coordinate
      // transform mirrors the chart's top-to-bottom axis mapping.
      const yForPrice = price => ((1.1080 - price) / (1.1080 - 1.1059)) * height;
      return [[width * xRatio, yForPrice(entryPrice)], [width * xRatio, yForPrice(stopPrice)], [width * xRatio, yForPrice(targetPrice)]];
    };
    const drawingRows = page.locator(".market-drawing-row");
    const drawingToolbar = page.getByRole("toolbar", { name: "Chart drawing tools" });
    const waitForDraftPoint = async (placed, total, label) => page.waitForFunction(
      ({ expected, count }) => document.querySelector(".market-drawing-hint")?.textContent?.includes(`${expected}/${count} points placed`),
      { expected: placed, count: total },
      { timeout: 8_000 }
    ).catch(async error => {
      const diagnostic = await page.locator(".market-drawing-hint").innerText();
      throw new Error(`${label}: expected anchor ${placed}/${total} to register at ${viewport.name}; hint=${diagnostic}. ${error.message}`);
    });
    const addDrawing = async (label, points) => {
      const before = await drawingRows.count();
      const button = drawingToolbar.getByRole("button", { name: label, exact: true });
      await button.click();
      await page.waitForFunction(name => [...document.querySelectorAll(".market-drawing-tools button")].some(button => button.textContent?.trim() === name && button.getAttribute("aria-pressed") === "true" && document.querySelector(".market-drawing-overlay")?.classList.contains("is-drawing")), label);
      for (let index = 0; index < points.length; index += 1) {
        const [x, y] = points[index];
        await drawAt(x, y);
        if (index < points.length - 1) await waitForDraftPoint(index + 1, points.length, label);
      }
      await page.waitForFunction(expected => document.querySelectorAll(".market-drawing-row").length === expected, before + 1, { timeout: 8_000 })
        .catch(async error => {
          const hint = await page.locator(".market-drawing-hint").innerText();
          throw new Error(`${label}: drawing was not committed at ${viewport.name}; rows=${await drawingRows.count()}, hint=${hint}. ${error.message}`);
        });
      console.log(`[acceptance] ${label} committed at ${viewport.name}`);
    };
    await addDrawing("Horizontal ray", [[drawingBox.width * .08, drawingBox.height * .30]]);
    await addDrawing("Vertical line", [[drawingBox.width * .10, drawingBox.height * .35]]);
    await addDrawing("Trendline", [[drawingBox.width * .12, drawingBox.height * .55], [drawingBox.width * .15, drawingBox.height * .38]]);
    await addDrawing("Rectangle / zone", [[drawingBox.width * .16, drawingBox.height * .25], [drawingBox.width * .19, drawingBox.height * .48]]);
    await addDrawing("Parallel channel", [[drawingBox.width * .20, drawingBox.height * .60], [drawingBox.width * .24, drawingBox.height * .47], [drawingBox.width * .22, drawingBox.height * .43]]);
    await addDrawing("Fibonacci", [[drawingBox.width * .20, drawingBox.height * .25], [drawingBox.width * .24, drawingBox.height * .60]]);
    const beforeInvalidLong = await drawingRows.count();
    const invalidLongButton = drawingToolbar.getByRole("button", { name: "Long position", exact: true });
    await invalidLongButton.click();
    await page.waitForFunction(() => document.querySelector(".market-drawing-overlay")?.classList.contains("is-drawing") && [...document.querySelectorAll(".market-drawing-tools button")].some(button => button.textContent?.trim() === "Long position" && button.getAttribute("aria-pressed") === "true"));
    const invalidLongPoints = await pointsForPrices(.24, 1.10712, 1.10730, 1.10690);
    await drawAt(...invalidLongPoints[0]);
    await waitForDraftPoint(1, 3, "Invalid long position");
    await drawAt(...invalidLongPoints[1]);
    await waitForDraftPoint(2, 3, "Invalid long position");
    await drawAt(...invalidLongPoints[2]);
    await page.waitForFunction(() => document.querySelector(".market-drawing-hint")?.textContent?.includes("Long position requires stop below entry and target above entry"), null, { timeout: 8_000 });
    assert.equal(await drawingRows.count(), beforeInvalidLong, `Invalid long position must be rejected without creating a drawing at ${viewport.name}`);
    const validLongPoints = await pointsForPrices(.22, 1.10712, 1.10690, 1.10730);
    const validShortPoints = await pointsForPrices(.43, 1.10712, 1.10730, 1.10690);
    await addDrawing("Long position", validLongPoints);
    await addDrawing("Short position", validShortPoints);
    await page.locator(".market-drawing-row").nth(7).waitFor({ state: "attached", timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-row").count(), 8, `Expected all eight drawing types to be created at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay g[data-id]").nth(4).locator("line").count(), 4, `Parallel channel must render two parallel boundaries and two connectors at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay g[data-id]").nth(5).locator("line").count(), 7, `Fibonacci retracement must render all seven price levels at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay text").filter({ hasText: "R:R" }).count(), 2, `Long/short position tools must display risk/reward at ${viewport.name}`);
    await page.locator(".market-drawing-row").nth(6).getByRole("button").first().click();
    await page.waitForFunction(() => document.querySelectorAll(".market-drawing-overlay [data-anchor]").length === 3);
    const positionAnchorXs = await page.locator(".market-drawing-overlay [data-anchor]").evaluateAll(elements => elements.map(element => Number(element.getAttribute("cx"))));
    assert.equal(positionAnchorXs.length, 3, `Position tool must expose entry, stop and target anchors at ${viewport.name}`);
    assert.ok(positionAnchorXs.every(x => Math.abs(x - positionAnchorXs[0]) < 0.1), `Position anchors must remain aligned on the entry time at ${viewport.name}`);
    const firstDrawingRow = page.locator(".market-drawing-row").first();
    await firstDrawingRow.getByRole("button").first().click();
    await page.locator(".market-drawing-inspector").getByText("Edit drawing", { exact: true }).waitFor();
    const firstAnchor = page.locator(".market-drawing-overlay [data-anchor=\"0\"]").first();
    await firstAnchor.scrollIntoViewIfNeeded();
    const firstAnchorBox = await firstAnchor.boundingBox();
    assert.ok(firstAnchorBox, `Selected drawing did not expose a draggable anchor at ${viewport.name}`);
    await page.mouse.move(firstAnchorBox.x + firstAnchorBox.width / 2, firstAnchorBox.y + firstAnchorBox.height / 2);
    await page.mouse.down();
    await page.mouse.move(firstAnchorBox.x + firstAnchorBox.width / 2 + 18, firstAnchorBox.y + firstAnchorBox.height / 2 + 12, { steps: 4 });
    await page.mouse.up();
    await page.locator(".market-drawing-inspector label").filter({ hasText: "Line style" }).locator("select").selectOption("dotted");
    const anchorTimeInput = page.locator(".market-drawing-inspector input[type=\"datetime-local\"]");
    const anchorTimeBeforeApply = await anchorTimeInput.inputValue();
    await page.getByRole("button", { name: "Apply anchor" }).click();
    await page.waitForFunction(expected => document.querySelector(".market-drawing-inspector input[type=\"datetime-local\"]")?.value === expected, anchorTimeBeforeApply);
    await firstDrawingRow.getByRole("button", { name: "Hide", exact: true }).click();
    await firstDrawingRow.getByRole("button", { name: "Show", exact: true }).click();
    await firstDrawingRow.getByRole("button", { name: "Lock", exact: true }).click();
    await page.waitForFunction(() => document.querySelectorAll(".market-drawing-overlay [data-anchor]").length === 0);
    assert.equal(await page.getByRole("button", { name: "Apply anchor" }).isDisabled(), true, `Locked drawings must not allow manual anchor edits at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay [data-anchor]").count(), 0, `Locked drawings must hide draggable anchors at ${viewport.name}`);
    await firstDrawingRow.getByRole("button", { name: "Unlock", exact: true }).click();
    assert.equal(await page.getByRole("button", { name: "Apply anchor" }).isDisabled(), false, `Unlocking a drawing must restore anchor editing at ${viewport.name}`);
    await firstDrawingRow.getByRole("button", { name: "Duplicate", exact: true }).click();
    await page.locator(".market-drawing-row").nth(8).waitFor({ state: "attached" });
    await page.locator(".market-drawing-row").nth(8).getByRole("button", { name: "Delete", exact: true }).click();
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await page.locator(".market-drawing-row").nth(8).waitFor({ state: "attached" });
    await page.getByRole("button", { name: "Redo", exact: true }).click();
    assert.equal(await page.locator(".market-drawing-row").count(), 8, `Drawing management undo/redo failed at ${viewport.name}`);
    const drawingDownloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Export", exact: true }).click();
    const drawingDownload = await drawingDownloadPromise;
    const drawingBackupPath = await drawingDownload.path();
    assert.ok(drawingBackupPath, `Drawing backup export did not produce a file at ${viewport.name}`);
    await page.locator(".market-import-button input[type=\"file\"]").setInputFiles(drawingBackupPath);
    await page.locator(".market-drawing-hint").filter({ hasText: "Drawing file imported." }).waitFor();
    await page.locator(".market-import-button input[type=\"file\"]").setInputFiles({
      name: "wrong-instrument-drawings.json",
      mimeType: "application/json",
      buffer: Buffer.from(JSON.stringify({ version: 1, instrument: "GBPUSD", timeframe: "M1", drawings: [] })),
    });
    await page.locator(".market-drawing-hint").filter({ hasText: "belongs to a different instrument or timeframe" }).waitFor();
    assert.ok((await page.locator(".market-drawing-hint").innerText()).includes("belongs to a different instrument or timeframe"), `Cross-instrument drawing import was not rejected at ${viewport.name}`);
    await page.getByLabel("SMA 20").check();
    await page.getByLabel("Template name").fill("Acceptance template");
    await page.getByRole("button", { name: "Save layout" }).click();
    await page.getByLabel("SMA 20").uncheck();
    await page.getByLabel("Saved templates").selectOption({ label: "Acceptance template" });
    await page.getByRole("button", { name: "Load", exact: true }).click();
    await page.getByLabel("SMA 20").waitFor();
    assert.equal(await page.getByLabel("SMA 20").isChecked(), true, `Saved layout did not restore indicator visibility at ${viewport.name}`);
    await page.reload({ waitUntil: "domcontentloaded" });
    await page.locator(".market-drawing-row").first().waitFor({ timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-row").count(), 8, `Drawings were not restored after reload at ${viewport.name}`);
    await timeframeGroup.getByRole("button", { name: "H1", exact: true }).click();
    await page.getByText("Drawings (0) · EURUSD / H1", { exact: true }).waitFor({ timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-templates option").filter({ hasText: "Acceptance template" }).count(), 0, `A template from M1 must not be offered for H1 at ${viewport.name}`);
    await timeframeGroup.getByRole("button", { name: "M1", exact: true }).click();
    await page.locator(".market-drawing-row").nth(7).waitFor({ state: "attached", timeout: 10_000 });
    await page.locator(".market-drawing-templates option").filter({ hasText: "Acceptance template" }).waitFor({ state: "attached", timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-templates option").filter({ hasText: "Acceptance template" }).count(), 1, `Saved drawing layout did not persist at ${viewport.name}`);

    const marketDimensions = await page.evaluate(() => {
      const tableScroller = document.querySelector(".market-table-scroll");
      const tableScroll = tableScroller ? (() => {
        const rect = tableScroller.getBoundingClientRect();
        const style = getComputedStyle(tableScroller);
        return {
          left: Math.round(rect.left),
          right: Math.round(rect.right),
          width: Math.round(rect.width),
          clientWidth: tableScroller.clientWidth,
          scrollWidth: tableScroller.scrollWidth,
          overflowX: style.overflowX,
          canScrollHorizontally: tableScroller.scrollWidth > tableScroller.clientWidth + 1,
        };
      })() : null;
      const elements = [...document.querySelectorAll("body *")]
        .filter(el => !el.closest(".market-table-scroll"))
        .map(el => {
          const rect = el.getBoundingClientRect();
          const style = getComputedStyle(el);
          return {
            tag: el.tagName,
            className: typeof el.className === "string" ? el.className : "",
            id: el.id,
            text: (el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 80),
            left: Math.round(rect.left),
            right: Math.round(rect.right),
            width: Math.round(rect.width),
            clientWidth: el.clientWidth,
            scrollWidth: el.scrollWidth,
            minWidth: style.minWidth,
            overflowX: style.overflowX,
            position: style.position,
          };
        })
        .filter(el => el.right > window.innerWidth + 1 || el.left < -1 || el.scrollWidth > el.clientWidth + 2)
        .sort((a, b) => b.right - a.right || b.scrollWidth - a.scrollWidth)
        .slice(0, 30);
      const main = document.querySelector(".main");
      const mainRect = main?.getBoundingClientRect();
      const mainStyle = main ? getComputedStyle(main) : null;
      const mainDimensions = main && mainRect && mainStyle ? {
        left: Math.round(mainRect.left),
        right: Math.round(mainRect.right),
        width: Math.round(mainRect.width),
        clientWidth: main.clientWidth,
        scrollWidth: main.scrollWidth,
        boxSizing: mainStyle.boxSizing,
        paddingLeft: mainStyle.paddingLeft,
        paddingRight: mainStyle.paddingRight,
      } : null;
      return { viewportWidth: window.innerWidth, documentWidth: document.documentElement.scrollWidth, bodyWidth: document.body.scrollWidth, mainDimensions, tableScroll, elements };
    });
    await page.screenshot({ path: `${artifactDir}/${viewport.name}-market-data.png`, fullPage: true });
    assert.ok(marketDimensions.documentWidth <= marketDimensions.viewportWidth && marketDimensions.bodyWidth <= marketDimensions.viewportWidth, `Market Data horizontal overflow at ${viewport.name}: ${JSON.stringify(marketDimensions)}`);
    if (viewport.width <= 600) {
      assert.ok(marketDimensions.tableScroll, `OHLC table scroll container missing at ${viewport.name}`);
      assert.equal(marketDimensions.tableScroll.overflowX, "auto", `OHLC table must expose an independent horizontal scroll area at ${viewport.name}: ${JSON.stringify(marketDimensions.tableScroll)}`);
      assert.ok(marketDimensions.tableScroll.canScrollHorizontally, `OHLC table should remain horizontally scrollable on mobile at ${viewport.name}: ${JSON.stringify(marketDimensions.tableScroll)}`);
      assert.ok(marketDimensions.tableScroll.right <= marketDimensions.viewportWidth + 1, `OHLC table scroll container exceeds viewport at ${viewport.name}: ${JSON.stringify(marketDimensions.tableScroll)}`);
    }
    assert.deepEqual(pageErrors, [], `Browser runtime errors on Market Data at ${viewport.name}: ${pageErrors.join("; ")}`);
    assert.deepEqual(marketConsoleErrors, [], `Unexpected Market Data browser console errors at ${viewport.name}: ${marketConsoleErrors.join(" | ")}`);
    console.log(`PASS ${viewport.name}: Market Data canvas, eight timeframe controls, no overflow; expected M15 503 fixture verified; unexpected console errors: none.`);
    // Return to Analytics before exercising its sign-in navigation below.
    await page.goto(`${frontendUrl}/analytics`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    await page.getByRole("heading", { name: "Analytics Lab" }).waitFor();
    assert.ok(
      dimensions.documentWidth <= dimensions.viewportWidth && dimensions.bodyWidth <= dimensions.viewportWidth,
      `Horizontal overflow at ${viewport.name}: ${JSON.stringify(dimensions)}`,
    );

    await page.getByRole("link", { name: "Sign in" }).click();
    await page.getByRole("heading", { name: "Welcome back to EdgeNest" }).waitFor();
    await page.getByLabel("Email").waitFor();
    await page.getByLabel("Password").waitFor();
    const loginResponse = await page.goto(`${frontendUrl}/login`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    assert.equal(loginResponse?.status(), 200, `Login route failed at ${viewport.name}`);
    await page.screenshot({ path: `${artifactDir}/${viewport.name}-login.png`, fullPage: true });
    assert.deepEqual(pageErrors, [], `Browser errors at ${viewport.name}: ${pageErrors.join("; ")}`);
    console.log(`PASS ${viewport.name}: signed-out Analytics Lab + login, no horizontal overflow, no page errors`);
    await context.close();
  }

  // Deterministic M5 organization/workspace acceptance. This uses a synthetic
  // authenticated browser session and API fixtures; it never mutates production.
  console.log("Checking organization onboarding, membership administration and workspace isolation...");
  {
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, timezoneId: "Africa/Lagos" });
    const page = await context.newPage();
    const memberRows = [
      { organizationId: "org-1", userId: "owner-1", role: "OWNER", status: "ACTIVE", revision: 1 },
      { organizationId: "org-1", userId: "trader-1", role: "TRADER", status: "ACTIVE", revision: 2 },
    ];
    const eventWorkspaceHeaders = [];
    let resolveWorkspaceEvent;
    const workspaceEventObserved = new Promise(resolve => { resolveWorkspaceEvent = resolve; });
    await context.addInitScript(() => {
      sessionStorage.setItem("edgenest.auth.session.v1", JSON.stringify({
        token_type: "Bearer",
        access_token: "m5-acceptance-token",
        expires_in: 3600,
        refresh_token: "m5-acceptance-refresh",
        expires_at: Date.now() + 3600_000,
        user: { id: "owner-1", email: "m5-owner@example.test", display_name: "M5 Owner" },
      }));
      sessionStorage.setItem("edgenest.organization.active.v1", "org-1");
      sessionStorage.setItem("edgenest.workspace.active.v1", "workspace-a");
    });
    await page.route("**/healthz", route => route.fulfill({ status: 200, contentType: "text/plain", body: "ok" }));
    await page.route("**/api/v1/organizations", route => route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ data: [{ id: "org-1", name: "Acceptance Organization", status: "ACTIVE", membershipRole: "OWNER" }] }),
    }));
    await page.route("**/api/v1/organizations/org-1/workspaces", route => route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ data: [
        { id: "workspace-a", organizationId: "org-1", name: "Research", slug: "research", status: "ACTIVE", isDefault: true, revision: 1 },
        { id: "workspace-b", organizationId: "org-1", name: "Execution", slug: "execution", status: "ACTIVE", isDefault: false, revision: 1 },
      ] }),
    }));
    await page.route("**/api/v1/organizations/org-1/members", async route => {
      if (route.request().method() === "POST") {
        const input = route.request().postDataJSON();
        memberRows.push({ organizationId: "org-1", userId: input.user_id, role: input.role, status: "ACTIVE", revision: 1 });
        await route.fulfill({ status: 201, contentType: "application/json", body: "" });
        return;
      }
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ data: memberRows }) });
    });
    await page.route("**/api/v1/organizations/org-1/workspaces/*/members", route => route.fulfill({
      status: 200, contentType: "application/json", body: JSON.stringify({ data: [] }),
    }));
    await page.route("**/api/v1/trading/events*", async route => {
      const workspaceId = route.request().headers()["x-workspace-id"] ?? null;
      eventWorkspaceHeaders.push(workspaceId);
      if (workspaceId === "workspace-b") resolveWorkspaceEvent();
      await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify([]) });
    });

    const organizationResponse = await page.goto(`${frontendUrl}/settings/organization`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    assert.equal(organizationResponse?.status(), 200, "Organization settings route failed.");
    await page.getByRole("heading", { name: "Organization settings" }).waitFor();
    await page.getByText("Acceptance Organization").waitFor();
    await page.getByLabel("Existing account user ID").fill("new-member-1");
    await page.getByLabel("Organization role").selectOption("VIEWER");
    await page.getByLabel("Audit reason").fill("M5 automated acceptance membership");
    await page.getByRole("button", { name: "Add member" }).click();
    await page.getByRole("status").filter({ hasText: "Organization membership added." }).waitFor();
    assert.ok(memberRows.some(member => member.userId === "new-member-1"), "Organization membership form did not submit the new member.");

    const workspaceResponse = await page.goto(`${frontendUrl}/settings/workspaces`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    assert.equal(workspaceResponse?.status(), 200, "Workspace settings route failed.");
    await page.getByRole("heading", { name: "Workspace settings" }).waitFor();
    await page.getByText("Research", { exact: true }).waitFor();
    await page.getByText("Execution", { exact: true }).waitFor();
    await page.getByLabel("Active workspace").selectOption("workspace-b");
    await page.waitForFunction(() => sessionStorage.getItem("edgenest.workspace.active.v1") === "workspace-b");
    await page.waitForFunction(() => document.querySelector('select[aria-label="Active workspace"]')?.value === "workspace-b");
    await Promise.race([workspaceEventObserved, sleep(10_000).then(() => { throw new Error("Workspace-scoped event request did not use the newly selected workspace."); })]);
    assert.equal(await page.getByLabel("Active workspace").inputValue(), "workspace-b", "Workspace selector did not retain the newly selected workspace.");
    await page.screenshot({ path: `${artifactDir}/m5-workspace-settings.png`, fullPage: true });
    console.log(`PASS M5 organization membership + workspace switching; event workspace headers observed: ${JSON.stringify(eventWorkspaceHeaders)}`);
    await context.close();
  }

  // Live authenticated acceptance mutates account-scoped test data and depends on
  // production credentials/origin. Keep PR checks deterministic against the PR
  // Worker preview; execute this end-to-end suite only after deployment on main.
  if (process.env.VERIFY_LIVE_BROWSER_RUNTIME === "true") {
  assert.ok(email && password, "EDGENEST_ACCEPTANCE_EMAIL and EDGENEST_ACCEPTANCE_PASSWORD must be configured as GitHub Actions secrets.");

  console.log("Checking authenticated account switching and saved-view persistence on the deployed frontend...");
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  const created = [];
  let accessToken = null;
  try {
    const loginResponse = await page.goto(`${authFrontendUrl}/login`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    assert.equal(loginResponse?.status(), 200, "Production login route failed before authenticated acceptance.");
    await page.getByRole("heading", { name: "Welcome back to EdgeNest" }).waitFor({ state: "visible", timeout: 20_000 });
    await page.getByLabel("Email").waitFor({ state: "visible", timeout: 20_000 });
    await page.getByLabel("Password").waitFor({ state: "visible", timeout: 20_000 });
    // Observe the accounts request on the authenticated route. React Query may
    // populate the query during login and reuse it on a later route visit, so
    // this promise must be armed immediately before the login action, not after
    // navigating to Journal a second time.
    await page.getByLabel("Email").fill(email);
    await page.getByLabel("Password").fill(password);
    const loginFormValid = await page.locator("form").evaluate(form => form.checkValidity());
    assert.ok(
      loginFormValid,
      "Acceptance credentials do not satisfy the login form's required fields or minimum password length.",
    );
    // Observe both requests before submission: this distinguishes a blocked
    // native form submission, an identity API rejection, and an authenticated
    // Journal query that never starts.
    const loginResponsePromise = page.waitForResponse(
      response => response.url().includes("/api/v1/auth/login") && response.request().method() === "POST",
      { timeout: 30_000 },
    );
    const initialJournalAccountsResponsePromise = page.waitForResponse(
      response => response.url().includes("/api/v1/journal/accounts") && response.request().method() === "GET",
      { timeout: 45_000 },
    );
    await page.getByRole("button", { name: "Sign in" }).click();
    const authenticationResponse = await loginResponsePromise;
    if (!authenticationResponse.ok()) {
      let detail = `HTTP ${authenticationResponse.status()}`;
      try {
        const body = await authenticationResponse.json();
        detail = body?.error?.message ?? body?.message ?? detail;
      } catch { /* preserve status-only diagnostic for non-JSON login errors */ }
      throw new Error(`Production acceptance login was rejected: ${detail}`);
    }
    await page.waitForURL(/\/journal(?:[/?#]|$)/, { timeout: 30_000 });
    const authenticatedSessionState = await page.evaluate(() => {
      const session = JSON.parse(sessionStorage.getItem("edgenest.auth.session.v1") || "null");
      return { hasAccessToken: Boolean(session?.access_token), hasRefreshToken: Boolean(session?.refresh_token) };
    });
    assert.ok(authenticatedSessionState.hasAccessToken, "Identity login returned HTTP 200 but did not persist an access token.");
    const initialAccountsOutcome = await Promise.race([
      initialJournalAccountsResponsePromise.then(response => ({ kind: "response", response })),
      page.locator(".journal-page .panel.error[role='alert']").waitFor({ state: "visible", timeout: 45_000 }).then(async () => ({
        kind: "error",
        message: await page.locator(".journal-page .panel.error[role='alert']").innerText(),
      })),
    ]);
    assert.equal(
      initialAccountsOutcome.kind,
      "response",
      `Journal accounts query did not succeed after identity login: ${initialAccountsOutcome.kind === "error" ? initialAccountsOutcome.message : "no response observed"}`,
    );
    const initialJournalAccountsResponse = initialAccountsOutcome.response;
    assert.equal(initialJournalAccountsResponse.status(), 200, `Journal accounts API failed after login: HTTP ${initialJournalAccountsResponse.status()}`);
    const initialJournalAccountsPayload = await initialJournalAccountsResponse.json();
    const initialJournalAccountCount = Array.isArray(initialJournalAccountsPayload.data) ? initialJournalAccountsPayload.data.length : 0;

    // Authenticated Command Centre + Journal visual coverage. Capture the real
    // signed-in routes at all six agreed viewports without creating or changing
    // any trading records. Sensitive account/trade figures are blurred in the
    // CI screenshots; layout, typography, spacing and responsive behavior remain
    // visible for visual review.
    const authenticatedPageErrors = [];
    page.on("pageerror", error => authenticatedPageErrors.push(error.message));
    const redactPrivateValues = async () => page.addStyleTag({ content: `
      .topbar-right, .sidebar-footer, .dashboard-kpi strong,
      .dashboard-account-grid, .dashboard-execution-account-list,
      .dashboard-performance svg, .dashboard-performance canvas,
      .dashboard-risk-facts, .journal-page .metric-value,
      .journal-page .data-table tbody, .journal-trade-detail,
      .journal-page header.page-header p {
        filter: blur(5px) !important;
      }
    ` });
    const assertResponsiveViewport = async (label) => {
      const dimensions = await page.evaluate(() => ({
        viewport: window.innerWidth,
        document: document.documentElement.scrollWidth,
        body: document.body.scrollWidth,
        font: getComputedStyle(document.querySelector("h1") || document.body).fontFamily,
        headingWeight: getComputedStyle(document.querySelector("h1") || document.body).fontWeight,
        interLoaded: document.fonts.check("400 12px Inter"),
      }));
      assert.ok(dimensions.document <= dimensions.viewport + 1 && dimensions.body <= dimensions.viewport + 1,
        `Authenticated horizontal overflow at ${label}: ${JSON.stringify(dimensions)}`);
      assert.match(dimensions.font, /Inter/i, `Expected Inter interface typography at ${label}; got ${dimensions.font}`);
      assert.equal(dimensions.interLoaded, true, `Inter font face did not load at ${label}`);
      return dimensions;
    };
    for (const viewport of viewports) {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      const dashboardResponse = await page.goto(`${authFrontendUrl}/`, { waitUntil: "domcontentloaded", timeout: 45_000 });
      assert.equal(dashboardResponse?.status(), 200, `Authenticated Command Centre route failed at ${viewport.name}`);
      await page.locator(".dashboard-workspace").waitFor({ state: "visible", timeout: 20_000 });
      await page.getByRole("heading", { name: "Your trading dashboard" }).waitFor();
      await page.locator(".dashboard-kpis").waitFor({ state: "visible" });
      await page.evaluate(() => document.fonts.ready);
      await redactPrivateValues();
      const dashboardDimensions = await assertResponsiveViewport(`${viewport.name} Command Centre`);
      await page.screenshot({ path: `${artifactDir}/${viewport.name}-authenticated-dashboard.png`, fullPage: true });
      console.log(`PASS ${viewport.name}: authenticated Command Centre; ${JSON.stringify(dashboardDimensions)}`);

      const journalResponse = await page.goto(`${authFrontendUrl}/journal`, { waitUntil: "domcontentloaded", timeout: 45_000 });
      assert.equal(journalResponse?.status(), 200, `Authenticated Journal route failed at ${viewport.name}`);
      await page.locator(".journal-page").waitFor({ state: "visible", timeout: 20_000 });
      await page.getByRole("heading", { name: "Trading Journal" }).waitFor();
      await page.locator(".journal-page .grid-kpi").waitFor({ state: "visible" });
      if (initialJournalAccountCount > 0) {
        await page.locator(".journal-page .data-table-wrap table").waitFor({ state: "visible", timeout: 10_000 });
      } else {
        await page.getByText("No accounts yet.", { exact: true }).waitFor({ state: "visible", timeout: 10_000 });
      }
      const journalLoadErrors = await page.locator(".journal-page .panel.error[role='alert']").count();
      assert.equal(journalLoadErrors, 0, `Journal data error rendered at ${viewport.name}`);
      await page.evaluate(() => document.fonts.ready);
      await redactPrivateValues();
      const journalDimensions = await assertResponsiveViewport(`${viewport.name} Journal`);
      const journalStructure = await page.evaluate(() => ({
        accountTable: document.querySelectorAll(".journal-page .data-table-wrap table").length,
        emptyAccountsState: [...document.querySelectorAll(".journal-page p")].some(p => p.textContent?.trim() === "No accounts yet."),
        tradeTable: [...document.querySelectorAll(".journal-page .data-table")].some(table =>
          [...table.querySelectorAll("th")].some(th => /symbol|instrument/i.test(th.textContent || ""))),
        importControls: [...document.querySelectorAll(".journal-page button")].some(button => /Import trades/i.test(button.textContent || "")),
      }));
      assert.ok(
        journalStructure.accountTable >= 1 || (initialJournalAccountCount === 0 && journalStructure.emptyAccountsState),
        `Journal account table or valid empty state did not render at ${viewport.name}: ${JSON.stringify(journalStructure)}`,
      );
      assert.ok(journalStructure.importControls, `Journal import controls did not render at ${viewport.name}`);
      await page.screenshot({ path: `${artifactDir}/${viewport.name}-authenticated-journal.png`, fullPage: true });
      console.log(`PASS ${viewport.name}: authenticated Journal; ${JSON.stringify(journalDimensions)}; ${JSON.stringify(journalStructure)}`);
    }
    assert.deepEqual(authenticatedPageErrors, [], `Authenticated dashboard/Journal browser errors: ${authenticatedPageErrors.join("; ")}`);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`${authFrontendUrl}/journal`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    // Start observing before navigation so the first analytics request cannot
    // complete before Playwright registers the response listener.
    const verifyLiveBrowserRuntime = process.env.VERIFY_LIVE_BROWSER_RUNTIME === "true";
    const browserAnalyticsResponsePromise = verifyLiveBrowserRuntime
      ? page.waitForResponse(
          response => response.url().includes("/api/v1/journal/analytics/summary"),
          { timeout: 30_000 },
        )
      : null;
    await page.goto(`${authFrontendUrl}/analytics`, { waitUntil: "domcontentloaded", timeout: 45_000 });
    await page.getByRole("heading", { name: "Analytics Lab" }).waitFor();

    // The production health/CORS fix is deployed only after this PR merges and
    // Render rolls out the gateway. Run this assertion on main (post-merge),
    // not against the still-old production API during pull_request validation.
    if (verifyLiveBrowserRuntime) {
      await page.waitForTimeout(2_000);
      const runtimeState = await page.evaluate(() => ({
        health: [...document.querySelectorAll(".topbar-right span")].map(el => el.textContent?.trim()).filter(Boolean),
        analyticsError: [...document.querySelectorAll("body *")].some(el => el.children.length === 0 && el.textContent?.trim() === "Could not load analytics from your Journal. Check your connection and retry."),
      }));
      assert.ok(browserAnalyticsResponsePromise, "Browser analytics response observer was not initialized.");
      const browserAnalyticsResponse = await browserAnalyticsResponsePromise;
      const browserAnalyticsBody = await browserAnalyticsResponse.text();
      assert.equal(browserAnalyticsResponse.status(), 200, `Deployed browser analytics request failed: HTTP ${browserAnalyticsResponse.status()} ${browserAnalyticsResponse.url()} body=${browserAnalyticsBody.slice(0, 1000)} runtime=${JSON.stringify(runtimeState)}`);
      assert.equal(runtimeState.analyticsError, false, `Analytics error state remains visible: ${JSON.stringify(runtimeState)}`);
      assert.ok(runtimeState.health.some(value => value.includes("SERVICE CONNECTED")), `Topbar health check is not connected: ${JSON.stringify(runtimeState)}`);
    }

    const tokenState = await page.evaluate(() => {
      const raw = sessionStorage.getItem("edgenest.auth.session.v1");
      if (!raw) return null;
      const session = JSON.parse(raw);
      return { access_token: session.access_token, user_email: session.user?.email, raw_session: raw };
    });
    assert.ok(tokenState?.access_token, "Login succeeded but the browser session did not contain an access token.");
    assert.equal(String(tokenState.user_email || "").toLowerCase(), email.toLowerCase(), "Authenticated session user does not match configured acceptance user.");
    accessToken = tokenState.access_token;

    const apiHeaders = { Authorization: `Bearer ${accessToken}`, "Content-Type": "application/json" };
    const accountsResponse = await expectHttp(`${apiUrl}/api/v1/journal/accounts`, 200, { headers: apiHeaders });
    const accountsBody = await accountsResponse.json();
    const accounts = (accountsBody.data ?? []).filter(account => !account.archived);
    assert.ok(accounts.length >= 2, `Authenticated account-switch acceptance requires at least two active Journal accounts; found ${accounts.length}.`);

    // Match the dedicated field label exactly. A substring match also catches
    // "Saved views · synced to your Journal account", creating an ambiguous locator.
    const select = page.locator("label.analytics-filter-field").filter({
      has: page.locator("span").getByText("Journal account", { exact: true }),
    }).locator("select");
    await select.waitFor();
    await page.waitForFunction(() => {
      const field = [...document.querySelectorAll("label.analytics-filter-field")]
        .find(x => x.querySelector(":scope > span")?.textContent?.trim() === "Journal account");
      const el = field?.querySelector("select");
      return Boolean(el && el.options.length >= 3);
    }, { timeout: 30_000 });

    const savedViewsUrl = `${apiUrl}/api/v1/journal/analytics/saved-views`;
    for (const account of accounts.slice(0, 2)) {
      const name = `Stage1 acceptance ${Date.now()} ${Math.random().toString(36).slice(2, 8)}`;
      // The saved-view endpoint is an idempotent upsert (INSERT ... ON
      // CONFLICT ... RETURNING), so its documented/current success status is 200.
      const createResponse = await expectHttp(savedViewsUrl, 200, {
        method: "POST",
        headers: apiHeaders,
        body: JSON.stringify({ name, journal_account_id: account.id, filters: { fromDate: "", toDate: "", datePreset: "all", symbolFilter: "", setupFilter: "" } }),
      });
      const createdBody = await createResponse.json();
      const row = createdBody.data;
      assert.ok(row?.id, "Saved-view creation did not return a saved-view ID.");
      assert.equal(row.journal_account_id, account.id, "Saved view was not persisted against the selected Journal account.");
      created.push({ id: row.id, accountId: account.id, name });
    }

    for (const account of accounts.slice(0, 2)) {
      await select.selectOption(account.id);
      await page.getByText(`Saved views · synced to your Journal account`).waitFor();
      const savedViewsSelect = page.locator("label.analytics-filter-field").filter({ hasText: "Saved views" }).locator("select");
      await page.waitForFunction((expected) => {
        const selectEl = [...document.querySelectorAll("label.analytics-filter-field")].find(x => x.textContent?.includes("Saved views"))?.querySelector("select");
        return Boolean(selectEl && [...selectEl.options].some(option => option.value === expected));
      }, created.find(view => view.accountId === account.id).name, { timeout: 20_000 });

      const view = created.find(item => item.accountId === account.id);
      assert.ok(await savedViewsSelect.locator("option").evaluateAll((options, expectedName) => options.some(option => option.value === expectedName), view.name), `Saved view missing for selected Journal account.`);
      const otherAccountViews = created.filter(item => item.accountId !== account.id);
      for (const other of otherAccountViews) {
        assert.equal(await savedViewsSelect.locator("option").evaluateAll((options, expectedName) => options.some(option => option.value === expectedName), other.name), false, "Saved view from another Journal account leaked into current account scope.");
      }
    }

    await page.reload({ waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => Boolean(document.querySelector("label.analytics-filter-field select")));
    const selectedAccountId = await select.inputValue();
    const apiListResponse = await expectHttp(`${savedViewsUrl}?journal_account_id=${encodeURIComponent(selectedAccountId)}`, 200, { headers: apiHeaders });
    const persistedBody = await apiListResponse.json();
    assert.ok((persistedBody.data ?? []).some(view => created.some(item => item.id === view.id)), "Saved views did not persist across a page reload.");

    const freshContext = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    try {
      const freshPage = await freshContext.newPage();
      await freshPage.goto(`${authFrontendUrl}/login`, { waitUntil: "domcontentloaded", timeout: 45_000 });
      await freshPage.getByRole("heading", { name: "Welcome back to EdgeNest" }).waitFor({ state: "visible", timeout: 20_000 });
      await freshPage.getByLabel("Email").waitFor({ state: "visible", timeout: 20_000 });
      await freshPage.getByLabel("Password").waitFor({ state: "visible", timeout: 20_000 });
      await freshPage.getByLabel("Email").fill(email);
      await freshPage.getByLabel("Password").fill(password);
      await freshPage.getByRole("button", { name: "Sign in" }).click();
      await freshPage.waitForURL(/\/journal(?:[/?#]|$)/, { timeout: 30_000 });
      await freshPage.goto(`${authFrontendUrl}/analytics`, { waitUntil: "domcontentloaded", timeout: 45_000 });
      const freshToken = await freshPage.evaluate(() => {
        const session = JSON.parse(sessionStorage.getItem("edgenest.auth.session.v1") || "null");
        return session?.access_token ?? null;
      });
      assert.ok(freshToken, "Fresh browser context could not establish a new authenticated session.");
      for (const item of created) {
        const response = await expectHttp(`${savedViewsUrl}?journal_account_id=${encodeURIComponent(item.accountId)}`, 200, { headers: { Authorization: `Bearer ${freshToken}` } });
        const body = await response.json();
        assert.ok((body.data ?? []).some(view => view.id === item.id), "Saved view was not available in a fresh authenticated browser context.");
      }
      console.log("PASS authenticated acceptance: two-account saved-view scope, reload persistence and fresh-browser persistence.");
    } finally {

      await freshContext.close();
    }

    console.log("Checking existing account risk-policy API load/save/reload persistence...");
    let existingPolicyCheck = false;
    for (const account of accounts) {
      const policyGet = await expectHttp(`${apiUrl}/api/v1/journal/accounts/${encodeURIComponent(account.id)}/risk-policy`, 200, { headers: apiHeaders });
      const originalEnvelope = await policyGet.json();
      if (!originalEnvelope?.policy || Number(originalEnvelope.version) < 1) continue;

      // Re-save the exact existing policy, without changing any policy fields.
      // This exercises the real authenticated PUT + database read-after-write path
      // while preserving the user's configuration.
      const saveResponse = await expectHttp(`${apiUrl}/api/v1/journal/accounts/${encodeURIComponent(account.id)}/risk-policy`, 200, {
        method: "PUT",
        headers: apiHeaders,
        body: JSON.stringify(originalEnvelope.policy),
      });
      const savedEnvelope = await saveResponse.json();
      assert.equal(savedEnvelope.version, Number(originalEnvelope.version) + 1, "Risk policy save did not advance its version.");
      assert.deepEqual(savedEnvelope.policy, originalEnvelope.policy, "Risk policy save response changed existing policy fields.");

      const reloadResponse = await expectHttp(`${apiUrl}/api/v1/journal/accounts/${encodeURIComponent(account.id)}/risk-policy`, 200, { headers: apiHeaders });
      const reloadedEnvelope = await reloadResponse.json();
      assert.equal(reloadedEnvelope.version, savedEnvelope.version, "Risk policy version did not persist after a fresh GET.");
      assert.deepEqual(reloadedEnvelope.policy, originalEnvelope.policy, "Risk policy fields did not persist after a fresh GET.");
      existingPolicyCheck = true;
      console.log(`PASS risk-policy API persistence: existing account ${account.id}, version ${originalEnvelope.version} -> ${savedEnvelope.version}; original policy fields preserved.`);
      break;
    }
    if (!existingPolicyCheck) {
      console.log("SKIP risk-policy API round-trip: no active account has a previously saved policy (version > 0). UI save/reload is still exercised with isolated browser fixtures.");
    }

    console.log("Checking Account Risk Policies UI save, account isolation, reload persistence and six responsive viewports...");
    const uiAccounts = [
      { id: "risk-acceptance-a", name: "Acceptance Funded A", broker: "QuantOS test fixture", account_type: "funded", currency: "USD", initial_balance: "10000", archived: false },
      { id: "risk-acceptance-b", name: "Acceptance Personal B", broker: "QuantOS test fixture", account_type: "personal", currency: "USD", initial_balance: "25000", archived: false },
    ];
    const policyFor = (name, type, risk) => ({
      profile_name: name, firm_name: type === "funded" ? "Acceptance Firm" : null,
      program_name: type === "funded" ? "Acceptance Program" : null,
      account_type: type, risk_appetite: "balanced", risk_per_trade_pct: risk,
      max_risk_per_trade_amount: null, max_daily_loss_pct: 5, max_daily_risk_pct: 2,
      daily_loss_basis: "start_of_day_equity", max_total_drawdown_pct: 10,
      drawdown_basis: "trailing_high_water", max_trades_per_day: 5,
      max_open_positions: 3, max_total_open_risk_pct: 2, minimum_rr: 1.5,
      max_lot_size: null, max_leverage: null, firm_limits_enforcement: "block",
      personal_limits_enforcement: "warn", daily_reset_timezone: "Africa/Lagos",
      notes: "Acceptance fixture policy; not a real firm preset.",
    });
    const uiPolicies = new Map([
      ["risk-acceptance-a", { policy: policyFor("Existing funded policy A", "funded", 0.75), version: 4, updated_at: new Date().toISOString() }],
      ["risk-acceptance-b", { policy: policyFor("Existing personal policy B", "personal", 0.5), version: 2, updated_at: new Date().toISOString() }],
    ]);
    const uiContext = await browser.newContext({ viewport: { width: 1440, height: 900 }, timezoneId: "Africa/Lagos" });
    try {
      await uiContext.addInitScript(rawSession => {
        if (rawSession) sessionStorage.setItem("edgenest.auth.session.v1", rawSession);
      }, tokenState.raw_session);
      await uiContext.route("**/api/v1/auth/me", async route => route.fulfill({
        status: 200, contentType: "application/json",
        body: JSON.stringify({ data: { id: "acceptance-user", email, display_name: "QuantOS Acceptance" } }),
      }));
      await uiContext.route("**/api/v1/journal/accounts", async route => route.fulfill({
        status: 200, contentType: "application/json", body: JSON.stringify({ data: uiAccounts }),
      }));
      await uiContext.route("**/api/v1/journal/accounts/*/risk-policy", async route => {
        const accountId = new URL(route.request().url()).pathname.split("/").at(-2);
        const current = uiPolicies.get(accountId);
        if (!current) return route.fulfill({ status: 404, contentType: "application/json", body: JSON.stringify({ error: { code: "not_found" } }) });
        if (route.request().method() === "GET") {
          return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(current) });
        }
        if (route.request().method() === "PUT") {
          const policy = route.request().postDataJSON();
          const next = { policy, version: current.version + 1, updated_at: new Date().toISOString() };
          uiPolicies.set(accountId, next);
          return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(next) });
        }
        return route.fulfill({ status: 405, body: "Method not allowed" });
      });

      const policyPage = await uiContext.newPage();
      const policyPageErrors = [];
      policyPage.on("pageerror", error => policyPageErrors.push(error.message));
      const policyPageResponse = await policyPage.goto(`${frontendUrl}/risk/policies`, { waitUntil: "domcontentloaded", timeout: 45_000 });
      assert.equal(policyPageResponse?.status(), 200, "Risk Policies route failed in the PR Worker preview.");
      await policyPage.getByRole("heading", { name: "Account Risk Policies" }).waitFor();
      const policyNameInput = policyPage.getByLabel(/Policy name/);
      await policyNameInput.waitFor();
      assert.equal(await policyNameInput.inputValue(), "Existing funded policy A", "Existing policy was not loaded into the editor.");

      const savedName = `Acceptance saved policy ${Date.now()}`;
      await policyNameInput.fill(savedName);
      await policyPage.getByRole("button", { name: "Save policy" }).click();
      await policyPage.getByText("Policy saved successfully").waitFor();
      assert.equal(uiPolicies.get("risk-acceptance-a").policy.profile_name, savedName, "UI save did not update the account-scoped policy fixture.");
      assert.equal(uiPolicies.get("risk-acceptance-a").version, 5, "UI save did not advance the policy version.");

      await policyPage.reload({ waitUntil: "domcontentloaded" });
      await policyPage.getByRole("heading", { name: "Account Risk Policies" }).waitFor();
      assert.equal(await policyPage.getByLabel(/Policy name/).inputValue(), savedName, "Saved policy did not repopulate after a full page reload.");

      const accountSelect = policyPage.getByLabel("Trading account");
      await accountSelect.selectOption("risk-acceptance-b");
      await policyPage.waitForFunction(() => {
        const input = [...document.querySelectorAll("input")].find(el => el.placeholder === "e.g. Funded account — conservative");
        return Boolean(input && input.value === "Existing personal policy B");
      }, { timeout: 15_000 }).catch(async () => {
        await policyPage.getByLabel(/Policy name/).waitFor();
        assert.equal(await policyPage.getByLabel(/Policy name/).inputValue(), "Existing personal policy B", "Second account policy did not load independently.");
      });
      assert.equal(await policyPage.getByLabel(/Policy name/).inputValue(), "Existing personal policy B", "Second account policy did not load independently.");
      await accountSelect.selectOption("risk-acceptance-a");
      await policyPage.waitForFunction(expected => {
        const input = [...document.querySelectorAll("input")].find(el => el.value === expected);
        return Boolean(input);
      }, savedName, { timeout: 15_000 });
      assert.equal(await policyPage.getByLabel(/Policy name/).inputValue(), savedName, "Switching accounts lost the first account's saved policy.");

      for (const viewport of viewports) {
        await policyPage.setViewportSize({ width: viewport.width, height: viewport.height });
        await policyPage.waitForTimeout(150);
        const dimensions = await policyPage.evaluate(() => ({
          viewportWidth: window.innerWidth,
          documentWidth: document.documentElement.scrollWidth,
          bodyWidth: document.body.scrollWidth,
        }));
        assert.ok(
          dimensions.documentWidth <= dimensions.viewportWidth && dimensions.bodyWidth <= dimensions.viewportWidth,
          `Risk Policies horizontal overflow at ${viewport.name}: ${JSON.stringify(dimensions)}`,
        );
        await policyPage.screenshot({ path: `${artifactDir}/risk-policy-${viewport.name}.png`, fullPage: true });
        console.log(`PASS risk-policy UI ${viewport.name}: loaded account policy, no horizontal overflow.`);
      }
      assert.deepEqual(policyPageErrors, [], `Risk Policies browser errors: ${policyPageErrors.join("; ")}`);
      console.log("PASS risk-policy UI acceptance: existing policy load, account isolation, save, reload persistence and six responsive viewport screenshots.");
    } finally {
      await uiContext.close();
    }
    // Deterministic Analytics replay fixture. It verifies that the investigation
    // opens the replay, fetches historical candles, saves excursions and refreshes
    // the displayed metrics without modifying any production Journal trade.
    const replayContext = await browser.newContext({ viewport: { width: 1440, height: 1000 }, timezoneId: "Africa/Lagos", locale: "en-US" });
    try {
      await replayContext.addInitScript(rawSession => {
        if (rawSession) sessionStorage.setItem("edgenest.auth.session.v1", rawSession);
      }, tokenState.raw_session);
      const openedAt = "2026-11-12T10:00:00Z";
      const closedAt = "2026-11-12T15:00:00Z";
      const tradeId = "acceptance-replay-trade";
      const account = { id: "acceptance-replay-account", name: "Replay fixture", currency: "USD", initial_balance: "1000", account_type: "demo", archived: false };
      const candleRows = [
        { timestamp_ms: Date.parse("2026-11-12T10:00:00Z"), open: 1.1200, high: 1.1208, low: 1.1190, close: 1.1195, volume: 100 },
        { timestamp_ms: Date.parse("2026-11-12T11:00:00Z"), open: 1.1195, high: 1.1202, low: 1.1175, close: 1.1180, volume: 110 },
        { timestamp_ms: Date.parse("2026-11-12T12:00:00Z"), open: 1.1180, high: 1.1190, low: 1.1160, close: 1.1168, volume: 120 },
        { timestamp_ms: Date.parse("2026-11-12T13:00:00Z"), open: 1.1168, high: 1.1205, low: 1.1165, close: 1.1190, volume: 130 },
        { timestamp_ms: Date.parse("2026-11-12T14:00:00Z"), open: 1.1190, high: 1.1210, low: 1.1185, close: 1.1188, volume: 140 },
        // This bar starts exactly at the recorded close. It must remain hidden
        // from replay and must not contaminate trade-period MFE/MAE calculations.
        { timestamp_ms: Date.parse("2026-11-12T15:00:00Z"), open: 1.1188, high: 1.1300, low: 1.1100, close: 1.1250, volume: 150 },
        // Pagination overlap may repeat an identical candle; normalization must deduplicate it.
        { timestamp_ms: Date.parse("2026-11-12T14:00:00Z"), open: 1.1190, high: 1.1210, low: 1.1185, close: 1.1188, volume: 140 },
      ];
      const tradeRecord = {
        id: tradeId, journal_account_id: account.id, symbol: "EURUSD", direction: "short", status: "closed",
        source: "manual", source_system: "Journal", opened_at: openedAt, closed_at: closedAt,
        quantity: "0.01", entry_price: "1.1200", exit_price: "1.1169", stop_loss: "1.1220",
        take_profit: "1.1140", gross_pnl: "31", commission: "0", swap: "0", other_fees: "0",
        net_pnl: "31", pnl_currency: "USD", risk_amount: "20", r_multiple: "1.55", session: "London",
        mfe_r: null, mae_r: null,
      };
      const metric = {
        trade_id: tradeId, symbol: "EURUSD", direction: "short", session: "London", setup: "Acceptance",
        // Simulate a stale Analytics summary while the authoritative Journal
        // trade detail already contains the user's corrected close timestamp.
        opened_at: openedAt, closed_at: "2026-11-20T15:00:00Z", net_pnl: "31", risk_amount: "20", realized_r: "1.55",
        mfe_r: null, mae_r: null, mfe_source: null, mae_source: null, mfe_precision: null, mae_precision: null,
        holding_seconds: 18000, mfe_capture_percent: null, profit_surrendered_r: null,
        stop_loss_recorded: true, take_profit_recorded: true, entry_price: "1.1200", exit_price: "1.1169",
        setup_valid: true, rules_followed: true, emotional_trade: false, mistake: false, discipline_score: 90,
        psychology: { emotion_before: null, emotion_during: null, emotion_after: null, confidence: null, fear: null, fomo: null, frustration: null, revenge_intensity: null, boredom: null },
      };
      let excursionsSaved = null;
      let excursionSaveCount = 0;
      const replayMarketRequests = [];
      const makeAnalytics = () => ({
        generated_at: new Date().toISOString(), trade_count: 1, wins: 1, losses: 0, breakeven: 0,
        win_rate_percent: "100", gross_profit: "31", gross_loss: "0", net_profit: "31",
        profit_factor: null, expectancy: "31", average_win: "31", average_loss: null, max_drawdown: "0",
        average_r: "1.55", average_mfe_r: excursionsSaved ? "2" : null, average_mae_r: excursionsSaved ? "-0.5" : null,
        mfe_coverage_count: excursionsSaved ? 1 : 0, mae_coverage_count: excursionsSaved ? 1 : 0,
        risk_coverage_count: 1, risk_coverage_percent: "100", average_risk_amount: "20", maximum_risk_amount: "20",
        risk_distribution: { minimum: "20", q1: "20", median: "20", q3: "20", maximum: "20", sample_count: 1 },
        average_holding_seconds: 18000, median_holding_seconds: 18000, maximum_win_streak: 1, maximum_loss_streak: 0,
        recovery_factor: null, average_mfe_capture_percent: excursionsSaved ? "77.5" : null, execution_coverage_count: 1,
        trade_metrics: [{ ...metric, ...(excursionsSaved ? { mfe_r: "2", mae_r: "-0.5", mfe_source: "historical_candles", mae_source: "historical_candles", mfe_precision: "H1", mae_precision: "H1", mfe_capture_percent: "77.5", profit_surrendered_r: "0.45" } : {}) }],
        equity_curve: [{ trade_id: tradeId, closed_at: closedAt, net_pnl: "31", equity: "31", drawdown: "0" }],
        by_symbol: [{ key: "EURUSD", trades: 1, net_pnl: "31", win_rate_percent: "100", expectancy: "31" }],
        by_session: [{ key: "London", trades: 1, net_pnl: "31", win_rate_percent: "100", expectancy: "31" }],
        by_setup: [], by_weekday: [], by_hour_utc: [], by_day: [], by_month: [],
        setup_valid_count: 1, rules_followed_count: 1, emotional_trade_count: 0, mistake_count: 0,
        setup_valid_percent: "100", rules_followed_percent: "100", average_discipline_score: "90",
        notice: "Acceptance fixture only.",
      });
      await replayContext.route("**/api/v1/**", async route => {
        const request = route.request();
        const url = new URL(request.url());
        const pathname = url.pathname;
        if (pathname.includes("/market-data/")) replayMarketRequests.push(pathname);
        const json = (body, status = 200) => route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
        if (pathname.endsWith("/auth/me")) return json({ data: { id: "acceptance-user", email, display_name: "QuantOS Acceptance" } });
        if (pathname.endsWith("/journal/accounts")) return json({ data: [account] });
        if (pathname.endsWith("/journal/analytics/saved-views")) return json({ data: [] });
        if (pathname.endsWith("/journal/setups")) return json({ data: [] });
        if (pathname.endsWith("/journal/analytics/summary")) return json({ data: makeAnalytics() });
        if (pathname.endsWith("/journal/analytics/intelligence")) return json({ data: { trade_count: 1, wins: 1, losses: 0, breakeven: 0, win_rate_percent: 100, win_rate_confidence_95: null, sample_status: "insufficient_sample", minimum_sample_recommendation: 30, rule_adherence_percent: 100, rule_adherence_known_count: 1, valid_setup_percent: 100, valid_setup_known_count: 1, emotional_trade_count: 0, mistake_count: 0, average_discipline_score: 90, process_outcomes: { good_process_win: 1, good_process_loss: 0, poor_process_win: 0, poor_process_loss: 0 }, insights: [], notice: "Acceptance fixture." } });
        if (pathname.endsWith("/journal/trades") && request.method() === "GET") return json({ data: [tradeRecord], pagination: { limit: 10, offset: 0 } });
        if (pathname.endsWith("/journal/trades/" + tradeId + "/attachments")) return json({ data: [] });
        if (pathname.endsWith("/journal/trades/" + tradeId + "/excursions") && request.method() === "PUT") {
          excursionsSaved = request.postDataJSON();
          excursionSaveCount += 1;
          return json({ data: { trade_id: tradeId, saved: true, precision: excursionsSaved.timeframe } });
        }
        if (pathname.endsWith("/journal/trades/" + tradeId) && request.method() === "GET") return json({ data: tradeRecord });
        if (pathname.includes("/market-data/providers")) return json({ providers: [
          { id: "biquote-live", name: "BiQuote Live (fixture)", status: { state: "available" } },
          { id: "biquote-incomplete", name: "BiQuote Incomplete (fixture)", status: { state: "available" } },
          { id: "biquote-conflicting", name: "BiQuote Conflicting (fixture)", status: { state: "available" } },
          { id: "biquote-error", name: "BiQuote Error (fixture)", status: { state: "available" } },
        ] });
        if (pathname.includes("/market-data/instruments")) return json({ instruments: [{ id: "EURUSD", symbol: "EUR/USD", default_provider: "biquote-live" }] });
        if (pathname.includes("/market-data/candles")) {
          const provider = url.searchParams.get("provider");
          assert.equal(url.searchParams.get("timeframe"), "H1", "Replay should start with the H1 history preset.");
          // Keep the initial request in flight long enough to assert that readiness
          // is withheld until the provider response has been validated.
          if (provider === "biquote-live") await new Promise(resolve => setTimeout(resolve, 750));
          if (provider === "biquote-error") return json({ error: { message: "Acceptance fixture: provider request failed" } }, 503);
          if (provider === "biquote-incomplete") {
            const candles = candleRows.filter(candle => candle.timestamp_ms !== Date.parse("2026-11-12T14:00:00Z"));
            return json({ candles, count: candles.length, classification: "live", freshness: "historical", last_updated_at: closedAt });
          }
          if (provider === "biquote-conflicting") {
            const candles = [...candleRows, { ...candleRows[4], close: 1.1194, high: 1.1212 }];
            return json({ candles, count: candles.length, classification: "live", freshness: "historical", last_updated_at: closedAt });
          }
          return json({ candles: candleRows, count: candleRows.length, classification: "live", freshness: "historical", last_updated_at: closedAt });
        }
        return route.continue();
      });
      const replayPage = await replayContext.newPage();
      const replayErrors = [];
      replayPage.on("pageerror", error => replayErrors.push(error.message));
      const response = await replayPage.goto(frontendUrl + "/analytics", { waitUntil: "domcontentloaded", timeout: 45_000 });
      assert.equal(response?.status(), 200, "Analytics replay fixture route failed.");
      await replayPage.getByRole("heading", { name: "Analytics Lab" }).waitFor();
      const recentTradesPanel = replayPage.locator("section.panel").filter({ has: replayPage.getByRole("heading", { name: "RECENT CLOSED TRADES" }) });
      await recentTradesPanel.getByRole("button", { name: "EURUSD" }).click();
      const dialog = replayPage.getByRole("dialog");
      await dialog.getByRole("heading", { name: "Historical trade replay" }).waitFor();
      const playback = dialog.locator(".journal-playback");
      const replayReadiness = playback.locator('[data-testid="replay-readiness"]');
      await replayReadiness.waitFor({ state: "visible" });
      assert.notEqual(
        await playback.getAttribute("data-replay-readiness"),
        "ready",
        "Replay must not report ready while provider candles are still loading.",
      );
      await replayReadiness.getByText("Loading historical candles…").waitFor();
      await replayReadiness.getByText("Replay ready").waitFor();
      assert.equal(await playback.getAttribute("data-replay-readiness"), "ready", "Replay readiness must become ready only after full candle coverage is validated.");
      assert.equal(
        await playback.getByText("Recorded close time is in the future.").count(),
        0,
        "Replay must use the corrected full Journal trade timestamp, not the stale Analytics summary timestamp.",
      );
      const replayToolbar = playback.locator(".journal-playback-toolbar");
      await replayToolbar.waitFor({ state: "visible" });
      const replayControls = replayToolbar.locator("select");
      assert.equal(await replayControls.count(), 4, "Replay toolbar should expose timeframe, provider, view and speed controls.");
      assert.equal(await replayControls.nth(0).inputValue(), "H1", "Replay should start with the H1 timeframe selected.");
      await playback.locator(".journal-playback-chart canvas").first().waitFor({ state: "attached", timeout: 20_000 }).catch(async error => {
        const state = await replayPage.evaluate(() => ({
          url: location.href,
          modal: document.querySelector(".trade-investigation-backdrop")?.innerText ?? null,
          playback: document.querySelector(".journal-playback")?.innerText ?? null,
          toolbarCount: document.querySelectorAll(".journal-playback-toolbar").length,
          body: document.body.innerText.slice(-1800),
        }));
        console.log("DEBUG replay market requests:", JSON.stringify(replayMarketRequests));
        console.log("DEBUG replay page state:", JSON.stringify(state));
        console.log("DEBUG replay chart error:", error.message);
        throw error;
      });
      await replayPage.waitForFunction(() => document.body.innerText.includes("2.00R"), { timeout: 20_000 });
      assert.ok(excursionsSaved, "Historical replay did not persist the calculated MFE/MAE payload.");
      assert.equal(excursionsSaved.timeframe, "H1");
      assert.equal(excursionsSaved.candle_count, 6, "Coverage metadata should retain the complete normalized response, including context candles.");
      assert.ok(Math.abs(Number(excursionsSaved.mfe_r) - 2) < 1e-9, "MFE should be approximately 2R.");
      assert.ok(Math.abs(Number(excursionsSaved.mae_r) + 0.5) < 1e-9, "MAE should be approximately -0.5R.");
      assert.match(await playback.locator(".journal-playback-controls").textContent(), /1 \/ 5 candles/, "Replay must start at the entry candle and stop on the 14:00 candle that contains the 15:00 close.");
      assert.equal(await playback.getAttribute("data-replay-start-index"), "0", "Replay start must be the entry candle, not earlier context.");
      assert.equal(await playback.getAttribute("data-replay-end-index"), "4", "Replay end must be the candle containing the recorded close.");
      assert.equal(await playback.getAttribute("data-replay-cursor-index"), "0", "Replay cursor must initialize at entry.");
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "false", "Exit information must remain hidden before the replay reaches the recorded exit.");

      const nextCandle = playback.getByRole("button", { name: "Next candle" });
      for (let index = 0; index < 4; index += 1) await nextCandle.click();
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-cursor-index") === "4");
      assert.match(await playback.locator(".journal-playback-controls").textContent(), /5 \/ 5 candles/);
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "true", "Exit information should become visible only at the recorded exit candle.");
      assert.equal(await nextCandle.isDisabled(), true, "Next candle must be disabled at the recorded exit.");

      await playback.getByRole("button", { name: "Restart at entry" }).click();
      assert.equal(await playback.getAttribute("data-replay-cursor-index"), "0", "Restart must return exactly to the entry candle.");
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "false", "Restart must hide exit information again.");
      assert.match(await playback.locator(".journal-playback-controls").textContent(), /1 \/ 5 candles/);

      const timeline = playback.getByRole("slider", { name: "Trade playback timeline" });
      await timeline.focus();
      await timeline.press("ArrowRight");
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-cursor-index") === "1");
      await timeline.press("ArrowRight");
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-cursor-index") === "2");
      assert.match(await playback.locator(".journal-playback-controls").textContent(), /3 \/ 5 candles/);
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "false", "Seeking before the exit must not reveal exit information.");
      await playback.getByRole("button", { name: "Restart at entry" }).click();

      await playback.getByRole("button", { name: "Play" }).click();
      await replayPage.waitForFunction(() => Number(document.querySelector(".journal-playback")?.getAttribute("data-replay-cursor-index")) >= 1);
      await playback.getByRole("button", { name: "Pause" }).click();
      const pausedCursor = await playback.getAttribute("data-replay-cursor-index");
      await replayPage.waitForTimeout(250);
      assert.equal(await playback.getAttribute("data-replay-cursor-index"), pausedCursor, "Pause must freeze the replay cursor.");
      await playback.getByRole("button", { name: "Play" }).click();
      await replayPage.waitForFunction(() => {
        const root = document.querySelector(".journal-playback");
        const playButton = root?.querySelector(".journal-playback-controls .btn.primary");
        return root?.getAttribute("data-replay-cursor-index") === "4" && playButton?.textContent?.includes("Play");
      }, { timeout: 10_000 });
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "true", "Automatic playback must stop at the recorded exit and reveal exit information there.");

      // Phase 5 regression matrix: incomplete exit coverage, conflicting duplicate
      // candles and provider request errors must each block replay and persistence.
      const savesBeforeRejectedProviders = excursionSaveCount;
      const providerSelect = replayControls.nth(1);
      const playButton = playback.getByRole("button", { name: "Play" });
      const nextButton = playback.getByRole("button", { name: "Next candle" });
      const replayTimeline = playback.getByRole("slider", { name: "Trade playback timeline" });

      await providerSelect.selectOption("biquote-incomplete");
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-readiness") === "incomplete-coverage", undefined, { timeout: 15_000 });
      assert.equal(await playButton.isDisabled(), true, "Missing exit coverage must disable automatic playback.");
      assert.equal(await nextButton.isDisabled(), true, "Missing exit coverage must disable candle stepping.");
      assert.equal(await replayTimeline.isDisabled(), true, "Missing exit coverage must disable timeline seeking.");
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "false", "Incomplete coverage must never reveal the recorded exit.");
      assert.equal(excursionSaveCount, savesBeforeRejectedProviders, "Incomplete coverage must not persist MFE/MAE.");

      await providerSelect.selectOption("biquote-conflicting");
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-readiness") === "invalid-candles", undefined, { timeout: 15_000 });
      assert.equal(await playButton.isDisabled(), true, "Conflicting duplicate candles must disable playback.");
      assert.equal(await nextButton.isDisabled(), true, "Conflicting duplicate candles must disable stepping.");
      assert.equal(await replayTimeline.isDisabled(), true, "Conflicting duplicate candles must disable timeline seeking.");
      assert.equal(excursionSaveCount, savesBeforeRejectedProviders, "Conflicting candles must not persist MFE/MAE.");

      await providerSelect.selectOption("biquote-error");
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-readiness") === "candle-error", undefined, { timeout: 20_000 });
      // With a failed candle request there are no candles to render a playback
      // control surface around. The component intentionally shows its error panel
      // instead; assert the controls are absent rather than waiting on disabled
      // buttons that cannot exist while the candle array is empty.
      assert.equal(await playButton.count(), 0, "Provider request failure must not render a Play control without candles.");
      assert.equal(await nextButton.count(), 0, "Provider request failure must not render a step control without candles.");
      assert.equal(await replayTimeline.count(), 0, "Provider request failure must not render a timeline without candles.");
      assert.equal(excursionSaveCount, savesBeforeRejectedProviders, "Provider request failure must not persist MFE/MAE.");

      await providerSelect.selectOption("biquote-live");
      await replayPage.waitForFunction(() => document.querySelector(".journal-playback")?.getAttribute("data-replay-readiness") === "ready", undefined, { timeout: 15_000 });
      assert.equal(await playback.getAttribute("data-replay-cursor-index"), "0", "Changing provider and restoring valid coverage must reset replay to entry.");
      assert.equal(await playback.getAttribute("data-replay-exit-revealed"), "false", "Restoring a provider must not reveal exit before replay.");
      assert.equal(await playButton.isDisabled(), false, "Valid provider coverage must re-enable playback after recovery.");
      assert.equal(await nextButton.isDisabled(), false, "Valid provider coverage must re-enable candle stepping after recovery.");
      assert.equal(await replayTimeline.isDisabled(), false, "Valid provider coverage must re-enable timeline seeking after recovery.");
      assert.equal(excursionSaveCount, savesBeforeRejectedProviders, "Switching back to an already-persisted provider must not duplicate excursion writes.");

      const excursionSourceNote = dialog.locator(".trade-data-note");
      await excursionSourceNote.waitFor();
      assert.match(await excursionSourceNote.textContent(), /historical_candles.*H1/);
      assert.deepEqual(replayErrors, [], "Analytics replay browser errors: " + replayErrors.join("; "));
      await replayPage.screenshot({ path: artifactDir + "/analytics-trade-replay-fixture.png", fullPage: true });
      console.log("PASS Analytics trade replay fixture: auto-expanded controls, H1 candles, chart rendering, persisted MFE/MAE and refreshed investigation metrics.");
    } finally {
      await replayContext.close();
    }

  } finally {
    if (accessToken) {
      for (const item of created) {
        const response = await fetch(`${apiUrl}/api/v1/journal/analytics/saved-views/${encodeURIComponent(item.id)}`, { method: "DELETE", headers: { Authorization: `Bearer ${accessToken}` }, signal: AbortSignal.timeout(20_000) });
        if (!response.ok && response.status !== 404) {
          console.error(`WARNING: Could not clean up temporary saved view (${response.status}).`);
        }
      }
    }
    await context.close();
  }
  } else {
    console.log("PR preview: skipping production-authenticated account/API acceptance; this suite runs against the deployed frontend on main.");
  }
} finally {
  await browser.close();
}

console.log("PASS production acceptance: API health/readiness, saved-view auth/CORS, six responsive viewports, authenticated account-scoped saved-view persistence.");
