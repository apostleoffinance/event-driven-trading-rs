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
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } });
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
    const browserAnalyticsResponsePromise = page.waitForResponse(
      response => response.url().includes("/api/v1/journal/analytics/summary"),
      { timeout: 30_000 },
    );
    await page.goto(`${authFrontendUrl}/analytics`, { waitUntil: "networkidle", timeout: 45_000 });
    await page.getByRole("heading", { name: "Analytics Lab" }).waitFor();

    // Capture the topbar health result and analytics API independently. The
    // health indicator uses /healthz; Journal analytics uses /summary. Do not
    // fail with a generic label timeout: report the exact browser-visible state.
    await page.waitForTimeout(2_000);
    const runtimeState = await page.evaluate(() => ({
      health: [...document.querySelectorAll(".topbar-right span")].map(el => el.textContent?.trim()).filter(Boolean),
      analyticsError: [...document.querySelectorAll("body *")].some(el => el.children.length === 0 && el.textContent?.trim() === "Could not load analytics from your Journal. Check your connection and retry."),
      apiBase: document.querySelector("meta[name=\"api-base-url\"]")?.getAttribute("content") ?? null,
    }));
    const browserAnalyticsResponse = await browserAnalyticsResponsePromise;
    const browserAnalyticsBody = await browserAnalyticsResponse.text();
    assert.equal(browserAnalyticsResponse.status(), 200, `Deployed browser analytics request failed: HTTP ${browserAnalyticsResponse.status()} ${browserAnalyticsResponse.url()} body=${browserAnalyticsBody.slice(0, 1000)} runtime=${JSON.stringify(runtimeState)}`);
    assert.equal(runtimeState.analyticsError, false, `Analytics error state remains visible: ${JSON.stringify(runtimeState)}`);
    assert.ok(runtimeState.health.some(value => value.includes("SERVICE CONNECTED")), `Topbar health check is not connected: ${JSON.stringify(runtimeState)}`);

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
