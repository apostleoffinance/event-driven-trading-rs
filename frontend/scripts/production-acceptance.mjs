import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { chromium } from "playwright";

const frontendUrl = (process.env.FRONTEND_URL || "https://edgenest-frontend.edgenest.workers.dev").replace(/\/$/, "");
const apiUrl = (process.env.API_URL || "https://edgenest-api.onrender.com").replace(/\/$/, "");
const artifactDir = process.env.ACCEPTANCE_ARTIFACT_DIR || "artifacts/production-acceptance";
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
const savedViews = await expectHttp(`${apiUrl}/api/v1/journal/analytics/saved-views`, 401);
const unauthorized = await savedViews.json();
assert.equal(unauthorized?.error?.code, "unauthorized");

console.log("Checking browser CORS preflight...");
const preflight = await expectHttp(`${apiUrl}/api/v1/journal/analytics/saved-views`, 200, {
  method: "OPTIONS",
  headers: {
    Origin: frontendUrl,
    "Access-Control-Request-Method": "GET",
    "Access-Control-Request-Headers": "authorization",
  },
});
assert.equal(preflight.headers.get("access-control-allow-origin"), frontendUrl);

await mkdir(artifactDir, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  for (const viewport of viewports) {
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } });
    const page = await context.newPage();
    const pageErrors = [];
    page.on("pageerror", error => pageErrors.push(error.message));

    const analyticsResponse = await page.goto(`${frontendUrl}/analytics`, { waitUntil: "networkidle", timeout: 45_000 });
    assert.equal(analyticsResponse?.status(), 200, `Analytics route failed at ${viewport.name}`);
    await page.getByRole("heading", { name: "Analytics Lab" }).waitFor();
    await page.getByText("Sign-in required").waitFor();
    await page.screenshot({ path: `${artifactDir}/${viewport.name}-analytics.png`, fullPage: true });

    const dimensions = await page.evaluate(() => ({
      viewportWidth: window.innerWidth,
      documentWidth: document.documentElement.scrollWidth,
      bodyWidth: document.body.scrollWidth,
    }));
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
    console.log(`PASS ${viewport.name}: Analytics Lab + login, no horizontal overflow, no page errors`);
    await context.close();
  }
} finally {
  await browser.close();
}

console.log("PASS production acceptance: API health/readiness, saved-view auth boundary, CORS, six viewport sizes.");
