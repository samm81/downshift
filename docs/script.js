const dom = {
  page: document.body,
  downloadGrid: document.getElementById("download-grid"),
  macosDownloadOption: document.getElementById("macos-download-option"),
  heroDownload: document.getElementById("hero-download"),
  macosDownloadButton: document.getElementById("macos-download-button"),
  windowsDownloadOption: document.getElementById("windows-download-option"),
  windowsDownloadButton: document.getElementById("windows-download-button"),
  downloadError: document.getElementById("download-error"),
  releaseNotesLink: document.getElementById("release-notes-link"),
  checksumLink: document.getElementById("checksum-link"),
};

// Keep the demo geometry in step with the app: regular polygons with one
// shared side length and symmetric top-growth insertion slots.
const polygonSideLength = 25;
const polygonBaseline = 97;
const polygonCenterX = 50;
const terminalShapeFraction = 0.2;
const regularPolygonStartIndices = new Map([
  [3, 0],
  [4, 0],
  [5, 4],
  [6, 5],
  [7, 5],
  [8, 6],
]);
const regularPolygonCache = new Map();

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function easeInOut(value) {
  return 0.5 - Math.cos(Math.PI * clamp(value, 0, 1)) / 2;
}

function growthInsertionAfterIndex(sides) {
  return Math.floor((sides - 4) / 2);
}

function stageSlots(sides) {
  const slots = [
    "triangle-top",
    "triangle-bottom-right",
    "triangle-bottom-left",
  ];
  for (let nextSides = 4; nextSides <= sides; nextSides += 1) {
    slots.splice(
      growthInsertionAfterIndex(nextSides) + 1,
      0,
      `growth-${nextSides}`,
    );
  }
  return slots;
}

function regularPolygonPoints(sides) {
  if (regularPolygonCache.has(sides)) {
    return regularPolygonCache.get(sides);
  }
  const radius = polygonSideLength / (2 * Math.sin(Math.PI / sides));
  const startAngle =
    sides % 2 === 0 ? -Math.PI / 2 - Math.PI / sides : -Math.PI / 2;
  const points = Array.from({ length: sides }, (_, index) => {
    const angle = startAngle + (index * 2 * Math.PI) / sides;
    return [radius * Math.cos(angle), radius * Math.sin(angle)];
  });
  const bottom = Math.max(...points.map(([, y]) => y));
  const startIndex = regularPolygonStartIndices.get(sides);
  const rotated = points.slice(startIndex).concat(points.slice(0, startIndex));
  const translated = rotated.map(([x, y]) => [
    polygonCenterX + x,
    polygonBaseline + y - bottom,
  ]);
  regularPolygonCache.set(sides, translated);
  return translated;
}

function expandedPolygonPoints(sides, vertexCount, cache) {
  const cacheKey = `${sides}:${vertexCount}`;
  if (cache.has(cacheKey)) {
    return cache.get(cacheKey);
  }
  const expanded = stageSlots(sides).map((slot, index) => ({
    slot,
    point: regularPolygonPoints(sides)[index],
  }));
  for (let nextSides = sides + 1; nextSides <= vertexCount; nextSides += 1) {
    const insertAfter = growthInsertionAfterIndex(nextSides);
    const insertIndex = insertAfter + 1;
    const before = expanded[insertAfter].point;
    const after = expanded[insertIndex % expanded.length].point;
    const point =
      nextSides % 2 === 0
        ? [...before]
        : [(before[0] + after[0]) / 2, (before[1] + after[1]) / 2];
    expanded.splice(insertIndex, 0, {
      slot: `growth-${nextSides}`,
      point,
    });
  }
  const points = expanded.map(({ point }) => point);
  cache.set(cacheKey, points);
  return points;
}

function terminalPointsForProgress(vertexCount, progress, cache) {
  const triangle = expandedPolygonPoints(3, vertexCount, cache);
  const line = triangle.map(([x]) => [x, polygonBaseline]);
  const dot = line.map(() => [polygonCenterX, polygonBaseline]);
  if (progress < 0.5) {
    return interpolatePolygon(dot, line, easeInOut(progress * 2));
  }
  return interpolatePolygon(line, triangle, easeInOut((progress - 0.5) * 2));
}

function interpolatePolygon(from, to, amount) {
  return from.map((point, index) => [
    point[0] + (to[index][0] - point[0]) * amount,
    point[1] + (to[index][1] - point[1]) * amount,
  ]);
}

function pathData(points) {
  return `M ${points
    .map(([x, y]) => `${x.toFixed(3)} ${y.toFixed(3)}`)
    .join(" L")} Z`;
}

function updateDemoBreathingHitTarget(hitTarget, artwork, progress) {
  const terminalProgress = clamp(progress / terminalShapeFraction, 0, 1);
  const hitTargetActive =
    progress < terminalShapeFraction && terminalProgress <= 0.55;
  if (hitTargetActive) {
    hitTarget.removeAttribute("hidden");
  } else {
    hitTarget.setAttribute("hidden", "");
  }
  if (!hitTargetActive) {
    return;
  }

  const artworkWidth = artwork.getBoundingClientRect().width;
  const viewBoxUnitsPerPixel = 100 / Math.max(artworkWidth, 1);
  const targetHeight = 56 * viewBoxUnitsPerPixel;
  const lineWidth = polygonSideLength + 24 * viewBoxUnitsPerPixel;
  const dotToLineProgress = clamp(terminalProgress / 0.5, 0, 1);
  const targetWidth =
    targetHeight +
    (lineWidth - targetHeight) * easeInOut(dotToLineProgress);

  hitTarget.setAttribute("x", (polygonCenterX - targetWidth / 2).toFixed(3));
  hitTarget.setAttribute(
    "y",
    (polygonBaseline - targetHeight / 2).toFixed(3),
  );
  hitTarget.setAttribute("width", targetWidth.toFixed(3));
  hitTarget.setAttribute("height", targetHeight.toFixed(3));
  hitTarget.setAttribute("rx", (targetHeight / 2).toFixed(3));
}

function renderDemoBreathing(polygonNodes, hitTarget, artwork, progress, cache) {
  const terminalProgress = clamp(progress / terminalShapeFraction, 0, 1);
  const polygonProgress = clamp(
    (progress - terminalShapeFraction) / (1 - terminalShapeFraction),
    0,
    1,
  );
  polygonNodes.forEach((polygon) => {
    const layerIndex = Number(polygon.dataset.layerIndex);
    const vertexCount = Number(polygon.dataset.sides);
    let points;
    if (progress <= terminalShapeFraction) {
      points = terminalPointsForProgress(vertexCount, terminalProgress, cache);
    } else {
      const stagePosition = polygonProgress * 5;
      const stage = Math.min(Math.floor(stagePosition), 4);
      const transition = stagePosition - stage;
      const currentSides = Math.min(3 + layerIndex, 3 + stage);
      const nextSides = Math.min(currentSides + 1, 3 + layerIndex);
      const from = expandedPolygonPoints(currentSides, vertexCount, cache);
      points =
        currentSides === nextSides
          ? from
          : interpolatePolygon(
              from,
              expandedPolygonPoints(nextSides, vertexCount, cache),
              easeInOut(transition),
            );
    }
    polygon.setAttribute("d", pathData(points));
  });
  updateDemoBreathingHitTarget(hitTarget, artwork, progress);
}

function wireDemoBreathing() {
  const polygonNodes = Array.from(
    document.querySelectorAll(".demo-breath-polygon"),
  );
  if (polygonNodes.length === 0) {
    return;
  }

  const hitTarget = document.getElementById("demo-breath-hit-target");
  const artwork = document.querySelector(".demo-breath-artwork");
  const cache = new Map();
  let elapsedMs = 0;
  let previousTimestamp = null;

  function animate(timestamp) {
    if (previousTimestamp === null) {
      previousTimestamp = timestamp;
    }
    elapsedMs += Math.min(timestamp - previousTimestamp, 100);
    const cyclePosition = elapsedMs % 11000;
    const progress =
      cyclePosition < 5500
        ? easeInOut(cyclePosition / 5500)
        : 1 - easeInOut((cyclePosition - 5500) / 5500);
    renderDemoBreathing(polygonNodes, hitTarget, artwork, progress, cache);
    previousTimestamp = timestamp;
    window.requestAnimationFrame(animate);
  }

  renderDemoBreathing(polygonNodes, hitTarget, artwork, 0, cache);
  window.requestAnimationFrame(animate);
}

function wireDraggableDemoBall() {
  const stage = document.querySelector(".demo-stage");
  const ball = document.querySelector(".demo-ball");
  if (!stage || !ball) {
    return;
  }

  let activePointerId = null;
  let grabOffsetX = 0;
  let grabOffsetY = 0;

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function placeBall(clientX, clientY) {
    const stageRect = stage.getBoundingClientRect();
    const ballRect = ball.getBoundingClientRect();
    const nextLeft = clientX - stageRect.left - grabOffsetX;
    const nextTop = clientY - stageRect.top - grabOffsetY;
    const maxLeft = Math.max(0, stageRect.width - ballRect.width);
    const maxTop = Math.max(0, stageRect.height - ballRect.height);

    ball.style.right = "auto";
    ball.style.left = `${clamp(nextLeft, 0, maxLeft)}px`;
    ball.style.top = `${clamp(nextTop, 0, maxTop)}px`;
  }

  ball.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) {
      return;
    }
    const stageRect = stage.getBoundingClientRect();
    const ballRect = ball.getBoundingClientRect();
    activePointerId = event.pointerId;
    grabOffsetX = event.clientX - ballRect.left;
    grabOffsetY = event.clientY - ballRect.top;
    ball.style.right = "auto";
    ball.style.left = `${ballRect.left - stageRect.left}px`;
    ball.style.top = `${ballRect.top - stageRect.top}px`;
    ball.classList.add("is-dragging");
    ball.setPointerCapture(event.pointerId);
    event.preventDefault();
  });

  ball.addEventListener("pointermove", (event) => {
    if (event.pointerId !== activePointerId) {
      return;
    }
    placeBall(event.clientX, event.clientY);
  });

  function stopDragging(event) {
    if (event.pointerId !== activePointerId) {
      return;
    }
    activePointerId = null;
    ball.classList.remove("is-dragging");
    if (ball.hasPointerCapture(event.pointerId)) {
      ball.releasePointerCapture(event.pointerId);
    }
  }

  ball.addEventListener("pointerup", stopDragging);
  ball.addEventListener("pointercancel", stopDragging);
}

function setDownloadButton(button, url, available, label) {
  button.textContent = label;
  button.href = url || "#download";
  button.setAttribute("aria-disabled", String(!available));
  button.classList.toggle("is-disabled", !available);
}

function applyHeroLabel(version, hasMacos, hasWindows) {
  const platform =
    hasMacos && hasWindows
      ? "macOS and Windows"
      : hasMacos
        ? "macOS"
        : "Windows";
  dom.heroDownload.textContent = `Download ${version} for ${platform}`;
  dom.heroDownload.href = "#download";
  dom.heroDownload.removeAttribute("target");
  dom.heroDownload.removeAttribute("rel");
}

function applyReadyState({
  version,
  macos_url: macosUrl,
  windows_url: windowsUrl,
  release_url: releaseNotesUrl,
  checksums_url: checksumUrl,
}) {
  const hasMacos = Boolean(macosUrl);
  const hasWindows = Boolean(windowsUrl);
  dom.downloadGrid.hidden = false;
  dom.macosDownloadOption.hidden = !hasMacos;
  dom.windowsDownloadOption.hidden = !hasWindows;
  dom.downloadError.hidden = true;
  applyHeroLabel(version, hasMacos, hasWindows);

  setDownloadButton(
    dom.macosDownloadButton,
    macosUrl,
    hasMacos,
    `Download ${version} for macOS (Apple Silicon)`,
  );
  setDownloadButton(
    dom.windowsDownloadButton,
    windowsUrl,
    hasWindows,
    `Download ${version} for Windows (x64)`,
  );
  dom.releaseNotesLink.href = releaseNotesUrl;

  if (checksumUrl) {
    dom.checksumLink.href = checksumUrl;
    dom.checksumLink.removeAttribute("aria-hidden");
    dom.checksumLink.classList.remove("is-hidden");
  } else {
    dom.checksumLink.href = dom.releaseNotesLink.href;
    dom.checksumLink.setAttribute("aria-hidden", "true");
    dom.checksumLink.classList.add("is-hidden");
  }
}

function applyErrorState() {
  dom.heroDownload.href = "#download";
  dom.heroDownload.removeAttribute("target");
  dom.heroDownload.removeAttribute("rel");
  dom.downloadGrid.hidden = true;
  dom.macosDownloadOption.hidden = true;
  dom.windowsDownloadOption.hidden = true;
  dom.downloadError.hidden = false;
  setDownloadButton(
    dom.macosDownloadButton,
    "#download",
    false,
    "Download for macOS (Apple Silicon)",
  );
  setDownloadButton(
    dom.windowsDownloadButton,
    "#download",
    false,
    "Download for Windows (x64)",
  );
}

function validateReleaseManifest(manifest) {
  const errors = [];
  const repository = "samm81/downshift";
  const stableVersionPattern = /^v\d+\.\d+\.\d+$/;
  const version = manifest?.version;

  if (
    manifest === null ||
    typeof manifest !== "object" ||
    Array.isArray(manifest)
  ) {
    return ["manifest must be a JSON object"];
  }
  if (typeof version !== "string" || !stableVersionPattern.test(version)) {
    errors.push("version must be a stable release tag");
  }

  const expectedReleaseUrl =
    typeof version === "string" && stableVersionPattern.test(version)
      ? `https://github.com/${repository}/releases/tag/${version}`
      : "";
  if (manifest.release_url !== expectedReleaseUrl) {
    errors.push("release_url does not match the release tag");
  }
  if (
    manifest.published_at !== undefined &&
    (typeof manifest.published_at !== "string" ||
      !Number.isFinite(Date.parse(manifest.published_at)))
  ) {
    errors.push("published_at is invalid");
  }

  const assetPrefix =
    typeof version === "string" && stableVersionPattern.test(version)
      ? `/${repository}/releases/download/${version}/`
      : "/";
  const validateAsset = (field, extension) => {
    const value = manifest[field];
    if (value === null || value === undefined) {
      return false;
    }
    if (typeof value !== "string" || value.length === 0) {
      errors.push(`${field} must be a non-empty string or null`);
      return false;
    }
    let parsed;
    try {
      parsed = new URL(value);
    } catch {
      errors.push(`${field} is not a valid URL`);
      return false;
    }
    if (
      parsed.protocol !== "https:" ||
      parsed.hostname !== "github.com" ||
      parsed.port ||
      parsed.username ||
      parsed.password ||
      parsed.search ||
      parsed.hash ||
      !parsed.pathname.startsWith(assetPrefix) ||
      !parsed.pathname.slice(assetPrefix.length) ||
      parsed.pathname.slice(assetPrefix.length).includes("/") ||
      !parsed.pathname.toLowerCase().endsWith(extension)
    ) {
      errors.push(`${field} is not an expected GitHub release asset URL`);
      return false;
    }
    return true;
  };

  const hasMacos = validateAsset("macos_url", ".dmg");
  const hasWindows = validateAsset("windows_url", ".exe");
  if (!hasMacos && !hasWindows) {
    errors.push("at least one platform asset is required");
  }
  if (manifest.checksums_url !== null && manifest.checksums_url !== undefined) {
    validateAsset("checksums_url", "");
    if (
      typeof manifest.checksums_url === "string" &&
      !/\.(txt|sha256)$/i.test(manifest.checksums_url)
    ) {
      errors.push("checksums_url is not a checksum text asset");
    }
  }

  return errors;
}

function loadReleaseManifest() {
  try {
    const manifestElement = document.getElementById("release-manifest");
    if (!manifestElement) {
      throw new Error("embedded release manifest is missing");
    }

    const manifestText = manifestElement.textContent?.trim();
    if (!manifestText) {
      throw new Error("embedded release manifest is empty");
    }

    const manifest = JSON.parse(manifestText);
    const errors = validateReleaseManifest(manifest);
    if (errors.length > 0) {
      throw new Error(`invalid release manifest: ${errors.join(", ")}`);
    }

    applyReadyState(manifest);
  } catch (error) {
    console.warn("failed to load release manifest", error);
    applyErrorState();
  }
}

wireDraggableDemoBall();
wireDemoBreathing();
loadReleaseManifest();
