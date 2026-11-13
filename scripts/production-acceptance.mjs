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

async function expectHttp(url, expected, options = {}) {
  const response = await fetch(url, { ...options, signal: AbortSignal.timeout(20_000) });
  assert.equal(response.status, expected, `${url} returned HTTP ${response.status}; expected ${expected}`);
  return response;
}

console.log("Checking production API health and readiness...");
const health = await expectHttp(`${apiUrl}/healthz`, 200);
assert.match(await health.text(), /ok/i);
const ready = await expectHttp(`${apiUrl}/readyz`, 200);
assert.match(await ready.text(), /ready/i);

console.log("Checking saved-view API authentication boundary...");
const unauthorizedResponse = await expectHttp(`${apiUrl}/api/v1/journal/analytics/saved-views`, 401);
assert.equal((await unauthorizedResponse.json())?.error?.code, "unauthorized");

console.log("Checking browser CORS preflight...");
const preflight = await expectHttp(`${apiUrl}/api/v1/journal/analytics/saved-views`, 200, {
  method: "OPTIONS",
  headers: {
    Origin: authFrontendUrl,
    "Access-Control-Request-Method": "GET",
    "Access-Control-Request-Headers": "authorization",
  },
});
assert.equal(preflight.headers.get("access-control-allow-origin"), authFrontendUrl);

await mkdir(artifactDir, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  // The public routes establish signed-out layout behavior at each agreed viewport.
  for (const viewport of viewports) {
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height }, timezoneId: "Africa/Lagos" });
    const page = await context.newPage();
    const pageErrors = [];
    page.on("pageerror", error => pageErrors.push(error.message));

    const analyticsResponse = await page.goto(`${frontendUrl}/analytics`, { waitUntil: "networkidle", timeout: 45_000 });
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
    page.on("console", message => { if (message.type() === "error") marketConsoleErrors.push(message.text()); });
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
    const marketResponse = await page.goto(`${frontendUrl}/market-data`, { waitUntil: "networkidle", timeout: 45_000 });
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
    await page.reload({ waitUntil: "networkidle" });
    await page.locator(".market-drawing-row").first().waitFor({ timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-row").count(), 8, `Drawings were not restored after reload at ${viewport.name}`);
    await timeframeGroup.getByRole("button", { name: "H1", exact: true }).click();
    await page.getByText("Drawings (0) · EURUSD / H1", { exact: true }).waitFor({ timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-templates option").filter({ hasText: "Acceptance template" }).count(), 0, `A template from M1 must not be offered for H1 at ${viewport.name}`);
    await timeframeGroup.getByRole("button", { name: "M1", exact: true }).click();
    await page.locator(".market-drawing-row").nth(7).waitFor({ state: "attached", timeout: 10_000 });
    await page.locator(".market-drawing-templates option").filter({ hasText: "Acceptance template" }).waitFor({ state: "attached", timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-templates option").filter({ hasText: "Acceptance template" }).count(), 1, `Saved drawing layout did not persist at ${viewport.name}`);

    const marketDimensions = await page.evaluate(() => ({ viewportWidth: window.innerWidth, documentWidth: document.documentElement.scrollWidth, bodyWidth: document.body.scrollWidth }));
    await page.screenshot({ path: `${artifactDir}/${viewport.name}-market-data.png`, fullPage: true });
    assert.ok(marketDimensions.documentWidth <= marketDimensions.viewportWidth && marketDimensions.bodyWidth <= marketDimensions.viewportWidth, `Market Data horizontal overflow at ${viewport.name}: ${JSON.stringify(marketDimensions)}`);
    assert.deepEqual(pageErrors, [], `Browser runtime errors on Market Data at ${viewport.name}: ${pageErrors.join("; ")}`);
    console.log(`PASS ${viewport.name}: Market Data Lightweight Charts canvas, eight timeframe controls, no horizontal overflow; console errors: ${marketConsoleErrors.join(" | ") || "none"}`);
    // Return to Analytics before exercising its sign-in navigation below.
    await page.goto(`${frontendUrl}/analytics`, { waitUntil: "networkidle", timeout: 45_000 });
    await page.getByRole("heading", { name: "Analytics Lab" }).waitFor();
    assert.ok(
      dimensions.documentWidth <= dimensions.viewportWidth && dimensions.bodyWidth <= dimensions.viewportWidth,
      `Horizontal overflow at ${viewport.name}: ${JSON.stringify(dimensions)}`,
    );

    await page.getByRole("link", { name: "Sign in" }).click();
    await page.getByRole("heading", { name: "Welcome back to EdgeNest" }).waitFor();
    await page.getByLabel("Email").waitFor();
    await page.getByLabel("Password").waitFor();
    const loginResponse = await page.goto(`${frontendUrl}/login`, { waitUntil: "networkidle", timeout: 45_000 });
    assert.equal(loginResponse?.status(), 200, `Login route failed at ${viewport.name}`);
    await page.screenshot({ path: `${artifactDir}/${viewport.name}-login.png`, fullPage: true });
    assert.deepEqual(pageErrors, [], `Browser errors at ${viewport.name}: ${pageErrors.join("; ")}`);
    console.log(`PASS ${viewport.name}: signed-out Analytics Lab + login, no horizontal overflow, no page errors`);
    await context.close();
  }

  assert.ok(email && password, "EDGENEST_ACCEPTANCE_EMAIL and EDGENEST_ACCEPTANCE_PASSWORD must be configured as GitHub Actions secrets.");

  console.log("Checking authenticated account switching and saved-view persistence on the deployed frontend...");
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  const created = [];
  let accessToken = null;
  try {
    const loginResponse = await page.goto(`${authFrontendUrl}/login`, { waitUntil: "networkidle", timeout: 45_000 });
    assert.equal(loginResponse?.status(), 200, "Production login route failed before authenticated acceptance.");
    await page.getByLabel("Email").fill(email);
    await page.getByLabel("Password").fill(password);
    await page.getByRole("button", { name: "Sign in" }).click();
    await page.waitForURL(/\/journal(?:[/?#]|$)/, { timeout: 30_000 });
    // Start observing before navigation so the first analytics request cannot
    // complete before Playwright registers the response listener.
    const verifyLiveBrowserRuntime = process.env.VERIFY_LIVE_BROWSER_RUNTIME === "true";
    const browserAnalyticsResponsePromise = verifyLiveBrowserRuntime
      ? page.waitForResponse(
          response => response.url().includes("/api/v1/journal/analytics/summary"),
          { timeout: 30_000 },
        )
      : null;
    await page.goto(`${authFrontendUrl}/analytics`, { waitUntil: "networkidle", timeout: 45_000 });
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

    await page.reload({ waitUntil: "networkidle" });
    await page.waitForFunction(() => Boolean(document.querySelector("label.analytics-filter-field select")));
    const selectedAccountId = await select.inputValue();
    const apiListResponse = await expectHttp(`${savedViewsUrl}?journal_account_id=${encodeURIComponent(selectedAccountId)}`, 200, { headers: apiHeaders });
    const persistedBody = await apiListResponse.json();
    assert.ok((persistedBody.data ?? []).some(view => created.some(item => item.id === view.id)), "Saved views did not persist across a page reload.");

    const freshContext = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    try {
      const freshPage = await freshContext.newPage();
      await freshPage.goto(`${authFrontendUrl}/login`, { waitUntil: "networkidle", timeout: 45_000 });
      await freshPage.getByLabel("Email").fill(email);
      await freshPage.getByLabel("Password").fill(password);
      await freshPage.getByRole("button", { name: "Sign in" }).click();
      await freshPage.waitForURL(/\/journal(?:[/?#]|$)/, { timeout: 30_000 });
      await freshPage.goto(`${authFrontendUrl}/analytics`, { waitUntil: "networkidle", timeout: 45_000 });
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
      const policyPageResponse = await policyPage.goto(`${frontendUrl}/risk/policies`, { waitUntil: "networkidle", timeout: 45_000 });
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

      await policyPage.reload({ waitUntil: "networkidle" });
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
    // Deterministic Analytics replay fixture. It verifies that the investigation
    // opens the replay, fetches historical candles, saves excursions and refreshes
    // the displayed metrics without modifying any production Journal trade.
    const replayContext = await browser.newContext({ viewport: { width: 1440, height: 1000 }, timezoneId: "Africa/Lagos" });
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
        opened_at: openedAt, closed_at: closedAt, net_pnl: "31", risk_amount: "20", realized_r: "1.55",
        mfe_r: null, mae_r: null, mfe_source: null, mae_source: null, mfe_precision: null, mae_precision: null,
        holding_seconds: 18000, mfe_capture_percent: null, profit_surrendered_r: null,
        stop_loss_recorded: true, take_profit_recorded: true, entry_price: "1.1200", exit_price: "1.1169",
        setup_valid: true, rules_followed: true, emotional_trade: false, mistake: false, discipline_score: 90,
        psychology: { emotion_before: null, emotion_during: null, emotion_after: null, confidence: null, fear: null, fomo: null, frustration: null, revenge_intensity: null, boredom: null },
      };
      let excursionsSaved = null;
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
          return json({ data: { trade_id: tradeId, saved: true, precision: excursionsSaved.timeframe } });
        }
        if (pathname.endsWith("/journal/trades/" + tradeId) && request.method() === "GET") return json({ data: tradeRecord });
        if (pathname.includes("/market-data/providers")) return json({ providers: [{ id: "biquote-live", name: "BiQuote Live (fixture)", status: { state: "available" } }] });
        if (pathname.includes("/market-data/instruments")) return json({ instruments: [{ id: "EURUSD", symbol: "EUR/USD", default_provider: "biquote-live" }] });
        if (pathname.includes("/market-data/candles")) {
          assert.equal(url.searchParams.get("timeframe"), "H1", "Replay should start with the H1 history preset.");
          return json({ candles: candleRows, count: candleRows.length, classification: "live", freshness: "historical", last_updated_at: closedAt });
        }
        return route.continue();
      });
      const replayPage = await replayContext.newPage();
      const replayErrors = [];
      replayPage.on("pageerror", error => replayErrors.push(error.message));
      const response = await replayPage.goto(frontendUrl + "/analytics", { waitUntil: "networkidle", timeout: 45_000 });
      assert.equal(response?.status(), 200, "Analytics replay fixture route failed.");
      await replayPage.getByRole("heading", { name: "Analytics Lab" }).waitFor();
      const recentTradesPanel = replayPage.locator("section.panel").filter({ has: replayPage.getByRole("heading", { name: "RECENT CLOSED TRADES" }) });
      await recentTradesPanel.getByRole("button", { name: "EURUSD" }).click();
      const dialog = replayPage.getByRole("dialog");
      await dialog.getByRole("heading", { name: "Historical trade replay" }).waitFor();
      await dialog.getByText("Replay ready").waitFor();
      const playback = dialog.locator(".journal-playback");
      await replayPage.waitForFunction(() => {
        const controls = [...document.querySelectorAll(".journal-playback-toolbar select")];
        return controls.length === 3 && controls[0].value === "H1";
      }, undefined, { timeout: 20_000 }).catch(async error => {
        const state = await replayPage.evaluate(() => ({
          url: location.href,
          modal: document.querySelector(".trade-investigation-backdrop")?.innerText ?? null,
          playback: document.querySelector(".journal-playback")?.innerText ?? null,
          toolbarCount: document.querySelectorAll(".journal-playback-toolbar").length,
          body: document.body.innerText.slice(-1800),
        }));
        console.log("DEBUG replay market requests:", JSON.stringify(replayMarketRequests));
        console.log("DEBUG replay page state:", JSON.stringify(state));
        console.log("DEBUG replay readiness error:", error.message);
        throw error;
      });
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
      assert.equal(excursionsSaved.candle_count, 5);
      assert.equal(Number(excursionsSaved.mfe_r), 2);
      assert.equal(Number(excursionsSaved.mae_r), -0.5);
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
      await uiContext.close();
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
} finally {
  await browser.close();
}

console.log("PASS production acceptance: API health/readiness, saved-view auth/CORS, six responsive viewports, authenticated account-scoped saved-view persistence.");
