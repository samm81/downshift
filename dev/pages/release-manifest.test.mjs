import assert from "node:assert/strict";
import test from "node:test";

import {
  manifestFromRelease,
  validateReleaseManifest,
} from "./release-manifest.mjs";

const linuxUrl =
  "https://github.com/samm81/downshift/releases/download/v0.3.2/Downshift-linux-x86_64-v0.3.2.tar.gz";

const release = {
  tag_name: "v0.3.2",
  html_url: "https://github.com/samm81/downshift/releases/tag/v0.3.2",
  published_at: "2026-08-31T00:00:00Z",
  draft: false,
  prerelease: false,
  assets: [
    {
      name: "Downshift-notarized-v0.3.2.dmg",
      browser_download_url:
        "https://github.com/samm81/downshift/releases/download/v0.3.2/Downshift-notarized-v0.3.2.dmg",
      state: "uploaded",
    },
    {
      name: "Downshift-Setup-0.3.2.exe",
      browser_download_url:
        "https://github.com/samm81/downshift/releases/download/v0.3.2/Downshift-Setup-0.3.2.exe",
      state: "uploaded",
    },
    {
      name: "Downshift-linux-x86_64-v0.3.2.tar.gz",
      browser_download_url: linuxUrl,
      state: "uploaded",
    },
  ],
};

test("release manifest accepts the canonical Linux asset", () => {
  const manifest = manifestFromRelease(release);
  assert.equal(manifest.linux_url, linuxUrl);
  assert.deepEqual(validateReleaseManifest(manifest), []);
});

test("release manifest rejects a non-canonical Linux asset", () => {
  const errors = validateReleaseManifest({
    ...manifestFromRelease(release),
    linux_url: linuxUrl.replace("x86_64", "aarch64"),
  });
  assert.ok(errors.some((error) => error.includes("canonical")));
});
