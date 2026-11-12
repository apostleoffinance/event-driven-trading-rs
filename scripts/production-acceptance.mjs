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
    const drawingBox = await drawingSvg.boundingBox();
    const chartBox = await page.locator(".market-chart-canvas").boundingBox();
    const paneSize = await page.locator(".market-drawing-overlay-host").evaluate(element => ({ width: Number(element.getAttribute("data-pane-width")), height: Number(element.getAttribute("data-pane-height")) }));
    assert.ok(drawingBox && drawingBox.width > 200 && drawingBox.height > 200, `Drawing overlay missing at ${viewport.name}`);
    assert.ok(chartBox && drawingBox.width < chartBox.width && drawingBox.height < chartBox.height, `Drawing overlay must exclude the price and time axes at ${viewport.name}: overlay=${JSON.stringify(drawingBox)} chart=${JSON.stringify(chartBox)}`);
    assert.ok(Math.abs(drawingBox.width - paneSize.width) <= 1 && Math.abs(drawingBox.height - paneSize.height) <= 1, `Drawing overlay does not match Lightweight Charts paneSize() at ${viewport.name}: overlay=${JSON.stringify(drawingBox)} pane=${JSON.stringify(paneSize)}`);
    const drawAt = async (x, y) => page.mouse.click(drawingBox.x + x, drawingBox.y + y);
    const addDrawing = async (label, points) => {
      await page.getByRole("toolbar", { name: "Chart drawing tools" }).getByRole("button", { name: label, exact: true }).click();
      for (const [x, y] of points) await drawAt(x, y);
    };
    await addDrawing("Horizontal ray", [[drawingBox.width * .24, drawingBox.height * .30]]);
    await addDrawing("Vertical line", [[drawingBox.width * .31, drawingBox.height * .35]]);
    await addDrawing("Trendline", [[drawingBox.width * .34, drawingBox.height * .55], [drawingBox.width * .52, drawingBox.height * .38]]);
    await addDrawing("Rectangle / zone", [[drawingBox.width * .40, drawingBox.height * .25], [drawingBox.width * .57, drawingBox.height * .48]]);
    await addDrawing("Parallel channel", [[drawingBox.width * .44, drawingBox.height * .60], [drawingBox.width * .62, drawingBox.height * .47], [drawingBox.width * .49, drawingBox.height * .43]]);
    await addDrawing("Fibonacci", [[drawingBox.width * .50, drawingBox.height * .25], [drawingBox.width * .68, drawingBox.height * .60]]);
    await addDrawing("Long position", [[drawingBox.width * .58, drawingBox.height * .42], [drawingBox.width * .58, drawingBox.height * .25], [drawingBox.width * .58, drawingBox.height * .58]]);
    assert.equal(await page.locator(".market-drawing-row").count(), 6, `Invalid long position was incorrectly accepted at ${viewport.name}`);
    assert.ok((await page.locator(".market-drawing-hint").innerText()).includes("Long position requires stop below entry and target above entry"), `Invalid long-position orientation was not explained at ${viewport.name}`);
    await addDrawing("Long position", [[drawingBox.width * .58, drawingBox.height * .42], [drawingBox.width * .58, drawingBox.height * .58], [drawingBox.width * .58, drawingBox.height * .25]]);
    await addDrawing("Short position", [[drawingBox.width * .70, drawingBox.height * .42], [drawingBox.width * .70, drawingBox.height * .25], [drawingBox.width * .70, drawingBox.height * .58]]);
    await page.locator(".market-drawing-row").nth(7).waitFor({ state: "attached", timeout: 10_000 });
    assert.equal(await page.locator(".market-drawing-row").count(), 8, `Expected all eight drawing types to be created at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay g[data-id]").nth(4).locator("line").count(), 4, `Parallel channel must render two parallel boundaries and two connectors at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay g[data-id]").nth(5).locator("line").count(), 7, `Fibonacci retracement must render all seven price levels at ${viewport.name}`);
    assert.equal(await page.locator(".market-drawing-overlay text").filter({ hasText: "R:R" }).count(), 2, `Long/short position tools must display risk/reward at ${viewport.name}`);
    const firstDrawingRow = page.locator(".market-drawing-row").first();
    await firstDrawingRow.getByRole("button").first().click();
    await page.locator(".market-drawing-inspector").getByText("Edit drawing", { exact: true }).waitFor();
    const firstAnchor = page.locator(".market-drawing-overlay [data-anchor=\"0\"]").first();
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
      return { access_token: session.access_token, user_email: session.user?.email };
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
