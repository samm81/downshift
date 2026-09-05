import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const RELEASE_REPOSITORY = "samm81/downshift";
export const STABLE_VERSION_PATTERN = /^v\d+\.\d+\.\d+$/;
export const EMBEDDED_MANIFEST_START =
  "<!-- DOWNSHIFT_RELEASE_MANIFEST_START -->";
export const EMBEDDED_MANIFEST_END = "<!-- DOWNSHIFT_RELEASE_MANIFEST_END -->";

const ASSET_FIELDS = [
  "macos_url",
  "windows_url",
  "linux_url",
  "checksums_url",
];

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function expectedReleaseUrl(version) {
  return `https://github.com/${RELEASE_REPOSITORY}/releases/tag/${version}`;
}

function validateGitHubUrl(value, expectedPrefix, field, errors) {
  if (value === null || value === undefined) {
    return;
  }
  if (typeof value !== "string" || value.length === 0) {
    errors.push(`${field} must be a non-empty string or null`);
    return;
  }

  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    errors.push(`${field} must be a valid HTTPS URL`);
    return;
  }

  if (
    parsed.protocol !== "https:" ||
    parsed.hostname !== "github.com" ||
    parsed.port ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash
  ) {
    errors.push(
      `${field} must be an HTTPS URL on github.com without query parameters`,
    );
    return;
  }

  if (!parsed.pathname.startsWith(expectedPrefix)) {
    errors.push(
      `${field} must point to the expected ${RELEASE_REPOSITORY} release`,
    );
    return;
  }

  const assetName = parsed.pathname.slice(expectedPrefix.length);
  if (!assetName || assetName.includes("/")) {
    errors.push(`${field} must point to a single release asset`);
  }
}

export function validateReleaseManifest(manifest) {
  const errors = [];
  if (!isRecord(manifest)) {
    return ["manifest must be a JSON object"];
  }

  const { version } = manifest;
  if (typeof version !== "string" || !STABLE_VERSION_PATTERN.test(version)) {
    errors.push("version must be a stable tag such as v0.2.0");
  }

  const expectedUrl =
    typeof version === "string" && STABLE_VERSION_PATTERN.test(version)
      ? expectedReleaseUrl(version)
      : "";
  if (
    typeof manifest.release_url !== "string" ||
    manifest.release_url !== expectedUrl
  ) {
    errors.push(
      `release_url must be ${expectedUrl || "the matching stable release URL"}`,
    );
  }

  if (manifest.published_at !== undefined) {
    if (
      typeof manifest.published_at !== "string" ||
      !Number.isFinite(Date.parse(manifest.published_at))
    ) {
      errors.push("published_at must be an ISO-8601 timestamp when present");
    }
  }

  for (const field of ASSET_FIELDS) {
    if (manifest[field] !== null && manifest[field] !== undefined) {
      const prefix =
        typeof version === "string" && STABLE_VERSION_PATTERN.test(version)
          ? `/${RELEASE_REPOSITORY}/releases/download/${version}/`
          : "/";
      validateGitHubUrl(manifest[field], prefix, field, errors);
    }
  }

  if (
    typeof manifest.macos_url === "string" &&
    !manifest.macos_url.toLowerCase().endsWith(".dmg")
  ) {
    errors.push("macos_url must point to a .dmg asset");
  }
  if (
    typeof manifest.windows_url === "string" &&
    !manifest.windows_url.toLowerCase().endsWith(".exe")
  ) {
    errors.push("windows_url must point to a .exe asset");
  }
  if (
    typeof manifest.linux_url === "string" &&
    typeof version === "string" &&
    STABLE_VERSION_PATTERN.test(version) &&
    !manifest.linux_url.endsWith(
      `/Downshift-linux-x86_64-v${version.slice(1)}.tar.gz`,
    )
  ) {
    errors.push(
      "linux_url must point to the canonical Downshift Linux x86_64 tarball",
    );
  }
  if (
    typeof manifest.checksums_url === "string" &&
    !/\.(txt|sha256)$/i.test(manifest.checksums_url)
  ) {
    errors.push("checksums_url must point to a checksum text asset");
  }

  if (!manifest.macos_url && !manifest.windows_url && !manifest.linux_url) {
    errors.push("at least one platform asset is required");
  }

  return errors;
}

function readJson(filePath) {
  const text =
    filePath === "-"
      ? fs.readFileSync(0, "utf8")
      : fs.readFileSync(filePath, "utf8");
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`invalid JSON in ${filePath}: ${error.message}`);
  }
}

function selectAsset(assets, name, required) {
  const matches = assets.filter(
    (asset) =>
      isRecord(asset) &&
      asset.name === name &&
      (asset.state === undefined || asset.state === "uploaded"),
  );
  if (matches.length > 1) {
    throw new Error(`release contains duplicate asset ${name}`);
  }
  if (matches.length === 0) {
    if (required) {
      throw new Error(`release is missing required asset ${name}`);
    }
    return null;
  }
  const url = matches[0].browser_download_url;
  if (typeof url !== "string" || url.length === 0) {
    throw new Error(`release asset ${name} has no browser download URL`);
  }
  return url;
}

export function manifestFromRelease(release) {
  if (!isRecord(release)) {
    throw new Error("GitHub release response must be a JSON object");
  }
  const version = release.tag_name;
  if (typeof version !== "string" || !STABLE_VERSION_PATTERN.test(version)) {
    throw new Error(
      "the requested release must be a stable tag such as v0.2.0",
    );
  }
  if (release.draft || release.prerelease) {
    throw new Error(`release ${version} is a draft or prerelease`);
  }
  if (typeof release.published_at !== "string") {
    throw new Error(`release ${version} has no publication timestamp`);
  }

  const assets = Array.isArray(release.assets) ? release.assets : [];
  const versionWithoutV = version.slice(1);
  const macosName = `Downshift-notarized-${version}.dmg`;
  const windowsName = `Downshift-Setup-${versionWithoutV}.exe`;
  const linuxName = `Downshift-linux-x86_64-${version}.tar.gz`;
  const macosUrl = selectAsset(assets, macosName, true);
  const windowsUrl = selectAsset(assets, windowsName, false);
  const linuxUrl = selectAsset(assets, linuxName, false);
  const checksumsUrl = selectAsset(assets, "SHA256SUMS.txt", false);

  const manifest = {
    version,
    release_url: release.html_url || expectedReleaseUrl(version),
    published_at: release.published_at,
    macos_url: macosUrl,
    windows_url: windowsUrl,
    linux_url: linuxUrl,
    checksums_url: checksumsUrl,
  };
  const errors = validateReleaseManifest(manifest);
  if (errors.length > 0) {
    throw new Error(`generated manifest is invalid:\n- ${errors.join("\n- ")}`);
  }
  return manifest;
}

function writeManifest(filePath, manifest) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
}

function embeddedManifestBlock(manifest, { validate = true } = {}) {
  if (validate) {
    const errors = validateReleaseManifest(manifest);
    if (errors.length > 0) {
      throw new Error(
        `cannot embed invalid release manifest:\n- ${errors.join("\n- ")}`,
      );
    }
  }

  // Escape HTML-sensitive characters before placing JSON inside a script
  // element. This keeps a future manifest value from closing the element.
  const json = JSON.stringify(manifest, null, 2)
    .replaceAll("<", "\\u003c")
    .replaceAll(">", "\\u003e")
    .replaceAll("&", "\\u0026");
  const indentedJson = json
    .split("\n")
    .map((line) => `      ${line}`)
    .join("\n");

  return [
    `    ${EMBEDDED_MANIFEST_START}`,
    '    <script id="release-manifest" type="application/json">',
    indentedJson,
    "    </script>",
    `    ${EMBEDDED_MANIFEST_END}`,
  ].join("\n");
}

function embeddedManifestRange(indexHtml) {
  const start = indexHtml.indexOf(EMBEDDED_MANIFEST_START);
  const end = indexHtml.indexOf(EMBEDDED_MANIFEST_END, start);
  if (start < 0 || end < 0) {
    throw new Error(
      "index.html is missing the release manifest start/end markers",
    );
  }

  const startLine = indexHtml.lastIndexOf("\n", start) + 1;
  const endLine = indexHtml.indexOf("\n", end);
  return {
    startLine,
    endLine: endLine < 0 ? indexHtml.length : endLine,
    afterEndLine: endLine < 0 ? indexHtml.length : endLine + 1,
  };
}

export function embedManifestIntoHtml(indexHtml, manifest, options = {}) {
  const range = embeddedManifestRange(indexHtml);
  const block = embeddedManifestBlock(manifest, options);
  return `${indexHtml.slice(0, range.startLine)}${block}${indexHtml.slice(range.endLine)}`;
}

export function removeEmbeddedManifestFromHtml(indexHtml) {
  const range = embeddedManifestRange(indexHtml);
  return `${indexHtml.slice(0, range.startLine)}${indexHtml.slice(range.afterEndLine)}`;
}

export function validateEmbeddedManifest(manifest, indexHtml) {
  try {
    const expected = embedManifestIntoHtml(indexHtml, manifest);
    return expected === indexHtml
      ? []
      : ["index.html does not contain the current release manifest"];
  } catch (error) {
    return [error.message];
  }
}

function writeEmbeddedManifest(manifestPath, indexPath) {
  const manifest = readJson(manifestPath);
  const errors = validateReleaseManifest(manifest);
  if (errors.length > 0) {
    throw new Error(
      `invalid release manifest ${manifestPath}:\n- ${errors.join("\n- ")}`,
    );
  }
  const indexHtml = fs.readFileSync(indexPath, "utf8");
  const updatedIndexHtml = embedManifestIntoHtml(indexHtml, manifest);
  fs.writeFileSync(indexPath, updatedIndexHtml, "utf8");
  console.log(
    `embedded ${manifestPath} into ${indexPath} for ${manifest.version}`,
  );
}

function usage() {
  console.error("usage:");
  console.error(
    "  node dev/pages/release-manifest.mjs validate [manifest-path|-]",
  );
  console.error(
    "  node dev/pages/release-manifest.mjs validate-embedded <manifest-path> [index-path]",
  );
  console.error(
    "  node dev/pages/release-manifest.mjs generate <release-json-path> [manifest-path]",
  );
  console.error(
    "  node dev/pages/release-manifest.mjs embed <manifest-path> [index-path]",
  );
}

function main() {
  const [command, firstPath, secondPath] = process.argv.slice(2);
  if (command === "validate") {
    const filePath = firstPath || "docs/release.json";
    const manifest = readJson(filePath);
    const errors = validateReleaseManifest(manifest);
    if (errors.length > 0) {
      throw new Error(
        `invalid release manifest ${filePath}:\n- ${errors.join("\n- ")}`,
      );
    }
    console.log(`validated ${filePath} for ${manifest.version}`);
    return;
  }

  if (command === "validate-embedded" && firstPath) {
    const manifestPath = firstPath;
    const indexPath = secondPath || "docs/index.html";
    const manifest = readJson(manifestPath);
    const indexHtml = fs.readFileSync(indexPath, "utf8");
    const errors = validateEmbeddedManifest(manifest, indexHtml);
    if (errors.length > 0) {
      throw new Error(
        `invalid embedded release manifest ${indexPath}:\n- ${errors.join("\n- ")}`,
      );
    }
    console.log(`validated embedded ${manifestPath} in ${indexPath}`);
    return;
  }

  if (command === "generate" && firstPath) {
    const release = readJson(firstPath);
    const manifest = manifestFromRelease(release);
    writeManifest(secondPath, manifest);
    console.log(`generated ${secondPath} for ${manifest.version}`);
    return;
  }

  if (command === "embed" && firstPath) {
    writeEmbeddedManifest(firstPath, secondPath || "docs/index.html");
    return;
  }

  usage();
  process.exitCode = 2;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    main();
  } catch (error) {
    console.error(`error: ${error.message}`);
    process.exitCode = 1;
  }
}
