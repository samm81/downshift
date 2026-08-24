import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

import {
  manifestFromRelease,
  validateReleaseManifest,
} from "./release-manifest.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPOSITORY_ROOT = path.resolve(SCRIPT_DIR, "../..");
const DOCS_DIR = path.join(REPOSITORY_ROOT, "docs");
const RELEASE_MANIFEST_PATH = path.join(DOCS_DIR, "release.json");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertEqual(actual, expected, message) {
  assert(
    actual === expected,
    `${message}: expected ${expected}, received ${actual}`,
  );
}

function readManifest() {
  return JSON.parse(fs.readFileSync(RELEASE_MANIFEST_PATH, "utf8"));
}

function findInstalledBrowser() {
  const candidates = [
    process.env.DOWNSHIFT_BROWSER_PATH,
    process.env.ProgramFiles &&
      path.join(
        process.env.ProgramFiles,
        "Google",
        "Chrome",
        "Application",
        "chrome.exe",
      ),
    process.env["ProgramFiles(x86)"] &&
      path.join(
        process.env["ProgramFiles(x86)"],
        "Microsoft",
        "Edge",
        "Application",
        "msedge.exe",
      ),
    process.env.ProgramFiles &&
      path.join(
        process.env.ProgramFiles,
        "Microsoft",
        "Edge",
        "Application",
        "msedge.exe",
      ),
  ].filter(Boolean);
  return candidates.find((candidate) => fs.existsSync(candidate));
}

function contentType(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  return (
    {
      ".css": "text/css",
      ".html": "text/html",
      ".js": "text/javascript",
      ".json": "application/json",
      ".png": "image/png",
      ".svg": "image/svg+xml",
      ".webm": "video/webm",
    }[extension] || "application/octet-stream"
  );
}

async function serveDirectory(directory) {
  const root = path.resolve(directory);
  const server = http.createServer(async (request, response) => {
    try {
      const requestUrl = new URL(request.url || "/", "http://127.0.0.1");
      const decodedPath = decodeURIComponent(requestUrl.pathname);
      const relativePath = decodedPath === "/" ? "/index.html" : decodedPath;
      const filePath = path.resolve(root, `.${relativePath}`);
      if (filePath !== root && !filePath.startsWith(`${root}${path.sep}`)) {
        response.writeHead(403);
        response.end("forbidden");
        return;
      }

      const stats = await fs.promises.stat(filePath);
      if (!stats.isFile()) {
        response.writeHead(404);
        response.end("not found");
        return;
      }
      response.writeHead(200, { "Content-Type": contentType(filePath) });
      fs.createReadStream(filePath).pipe(response);
    } catch (error) {
      if (error.code !== "ENOENT") {
        console.error(error);
      }
      response.writeHead(404);
      response.end("not found");
    }
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  return {
    url: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  };
}

async function createFixture(manifest, { missing = false } = {}) {
  const fixture = await fs.promises.mkdtemp(
    path.join(os.tmpdir(), "downshift-pages-smoke-"),
  );
  await fs.promises.cp(DOCS_DIR, fixture, { recursive: true });
  const fixtureManifestPath = path.join(fixture, "release.json");
  if (missing) {
    await fs.promises.rm(fixtureManifestPath);
  } else if (manifest !== undefined) {
    await fs.promises.writeFile(
      fixtureManifestPath,
      `${JSON.stringify(manifest, null, 2)}\n`,
      "utf8",
    );
  }
  return fixture;
}

async function runJavaScriptCase(browser, fixture, expectations) {
  const server = await serveDirectory(fixture);
  const context = await browser.newContext();
  const page = await context.newPage();
  const requests = [];
  const responses = [];
  const consoleMessages = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("response", (response) => responses.push(response));
  page.on("console", (message) => consoleMessages.push(message.text()));

  try {
    await page.goto(`${server.url}/index.html`, { waitUntil: "networkidle" });
    await page.waitForFunction(
      () =>
        document
          .querySelector("#macos-download-button")
          ?.getAttribute("aria-disabled") === "false" ||
        document.querySelector("#download-error")?.hidden === false,
    );

    const manifestResponse = responses.find((response) =>
      response.url().endsWith("/release.json"),
    );
    assert(
      manifestResponse,
      `${expectations.name}: release.json was not requested`,
    );
    assertEqual(
      manifestResponse.status(),
      expectations.manifestStatus,
      `${expectations.name}: release.json status`,
    );
    assert(
      !requests.some((url) => url.includes("api.github.com")),
      `${expectations.name}: browser requested api.github.com`,
    );
    assert(
      !consoleMessages.some((message) => message.includes("api.github.com")),
      `${expectations.name}: browser console mentioned api.github.com`,
    );

    const macosOption = page.locator("#macos-download-option");
    const windowsOption = page.locator("#windows-download-option");
    const downloadError = page.locator("#download-error");
    assertEqual(
      await macosOption.isHidden(),
      !expectations.hasMacos,
      `${expectations.name}: macOS card visibility`,
    );
    assertEqual(
      await windowsOption.isHidden(),
      !expectations.hasWindows,
      `${expectations.name}: Windows card visibility`,
    );
    assertEqual(
      await downloadError.isHidden(),
      expectations.valid,
      `${expectations.name}: fallback visibility`,
    );

    if (expectations.valid) {
      assertEqual(
        await page.locator("#macos-download-button").getAttribute("href"),
        expectations.macosUrl,
        `${expectations.name}: macOS URL`,
      );
      if (expectations.hasWindows) {
        assertEqual(
          await page.locator("#windows-download-button").getAttribute("href"),
          expectations.windowsUrl,
          `${expectations.name}: Windows URL`,
        );
      }
    } else {
      assertEqual(
        await page
          .locator("#macos-download-button")
          .getAttribute("aria-disabled"),
        "true",
        `${expectations.name}: macOS button disabled state`,
      );
    }
  } finally {
    await context.close();
    await server.close();
  }
}

async function runNoJavaScriptCase(browser, fixture) {
  const server = await serveDirectory(fixture);
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  try {
    await page.goto(`${server.url}/index.html`, {
      waitUntil: "domcontentloaded",
    });
    const fallbackLink = page.getByRole("link", { name: "latest releases" });
    assertEqual(
      await fallbackLink.count(),
      1,
      "no-JavaScript fallback link count",
    );
    assert(
      await fallbackLink.isVisible(),
      "no-JavaScript fallback link is not visible",
    );
  } finally {
    await context.close();
    await server.close();
  }
}

function testManifestGenerator(manifest) {
  const release = {
    tag_name: manifest.version,
    html_url: manifest.release_url,
    published_at: manifest.published_at,
    draft: false,
    prerelease: false,
    assets: [
      {
        name: `Downshift-notarized-${manifest.version}.dmg`,
        browser_download_url: manifest.macos_url,
        state: "uploaded",
      },
      {
        name: `Downshift-Setup-${manifest.version.slice(1)}.exe`,
        browser_download_url: manifest.windows_url,
        state: "uploaded",
      },
      {
        name: "SHA256SUMS.txt",
        browser_download_url: manifest.checksums_url,
        state: "uploaded",
      },
    ],
  };
  assert(
    JSON.stringify(manifestFromRelease(release)) === JSON.stringify(manifest),
    "release manifest generator did not reproduce the checked-in manifest",
  );

  const macosOnlyRelease = {
    ...release,
    assets: release.assets.filter((asset) => !asset.name.endsWith(".exe")),
  };
  const macosOnly = manifestFromRelease(macosOnlyRelease);
  assert(macosOnly.windows_url === null, "Windows asset should be nullable");

  for (const flag of ["draft", "prerelease"]) {
    assert(
      (() => {
        try {
          manifestFromRelease({ ...release, [flag]: true });
          return false;
        } catch {
          return true;
        }
      })(),
      `release generator accepted ${flag} release`,
    );
  }
}

async function main() {
  const source = fs.readFileSync(path.join(DOCS_DIR, "index.html"), "utf8");
  assert(
    !source.includes("api.github.com"),
    "index.html still contains the GitHub API endpoint",
  );

  const manifest = readManifest();
  const manifestErrors = validateReleaseManifest(manifest);
  assert(
    manifestErrors.length === 0,
    `checked-in manifest is invalid: ${manifestErrors.join(", ")}`,
  );
  testManifestGenerator(manifest);

  const macosOnlyManifest = { ...manifest, windows_url: null };
  const malformedManifest = {
    version: manifest.version,
    release_url: "not-a-release-url",
  };
  const fixtures = [];
  try {
    const bothPlatforms = await createFixture(manifest);
    fixtures.push(bothPlatforms);
    const macosOnly = await createFixture(macosOnlyManifest);
    fixtures.push(macosOnly);
    const malformed = await createFixture(malformedManifest);
    fixtures.push(malformed);
    const missing = await createFixture(undefined, { missing: true });
    fixtures.push(missing);

    const executablePath = findInstalledBrowser();
    const browser = await chromium.launch({
      headless: true,
      ...(executablePath ? { executablePath } : {}),
    });
    try {
      await runJavaScriptCase(browser, bothPlatforms, {
        name: "macOS-plus-Windows manifest",
        manifestStatus: 200,
        valid: true,
        hasMacos: true,
        hasWindows: true,
        macosUrl: manifest.macos_url,
        windowsUrl: manifest.windows_url,
      });
      await runJavaScriptCase(browser, macosOnly, {
        name: "macOS-only manifest",
        manifestStatus: 200,
        valid: true,
        hasMacos: true,
        hasWindows: false,
        macosUrl: manifest.macos_url,
      });
      await runJavaScriptCase(browser, malformed, {
        name: "malformed manifest",
        manifestStatus: 200,
        valid: false,
        hasMacos: false,
        hasWindows: false,
      });
      await runJavaScriptCase(browser, missing, {
        name: "missing manifest",
        manifestStatus: 404,
        valid: false,
        hasMacos: false,
        hasWindows: false,
      });
      await runNoJavaScriptCase(browser, bothPlatforms);
    } finally {
      await browser.close();
    }
  } finally {
    await Promise.all(
      fixtures.map((fixture) =>
        fs.promises.rm(fixture, { recursive: true, force: true }),
      ),
    );
  }

  console.log(
    "Pages browser smoke passed: API-free, macOS-only, Windows-present, invalid, missing, and no-JavaScript cases",
  );
}

main().catch((error) => {
  console.error(`error: ${error.message}`);
  process.exitCode = 1;
});
