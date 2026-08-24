const dom = {
  page: document.body,
  section: document.getElementById("download"),
  downloadGrid: document.getElementById("download-grid"),
  heroDownload: document.getElementById("hero-download"),
  macosDownloadButton: document.getElementById("macos-download-button"),
  windowsDownloadOption: document.getElementById("windows-download-option"),
  windowsDownloadButton: document.getElementById("windows-download-button"),
  releaseNotesLink: document.getElementById("release-notes-link"),
  checksumLink: document.getElementById("checksum-link"),
};

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

function findAssetByExtension(assets, ext) {
  return assets.find((asset) => asset.name.toLowerCase().endsWith(ext));
}

function findChecksumAsset(assets) {
  return (
    assets.find((asset) => asset.name.toLowerCase() === "sha256sums.txt") ||
    findAssetByExtension(assets, ".sha256") ||
    findAssetByExtension(assets, ".sha256.txt")
  );
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
  dmgUrl,
  exeUrl,
  releaseNotesUrl,
  checksumUrl,
}) {
  const hasMacos = Boolean(dmgUrl);
  const hasWindows = Boolean(exeUrl);
  dom.downloadGrid.classList.toggle("single-platform", !hasWindows);
  dom.windowsDownloadOption.hidden = !hasWindows;
  applyHeroLabel(version, hasMacos, hasWindows);

  setDownloadButton(
    dom.macosDownloadButton,
    dmgUrl,
    hasMacos,
    `Download ${version} for macOS (Apple Silicon)`,
  );
  setDownloadButton(
    dom.windowsDownloadButton,
    exeUrl,
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
  dom.downloadGrid.classList.add("single-platform");
  dom.windowsDownloadOption.hidden = true;
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

async function loadLatestRelease() {
  const apiUrl = dom.section?.dataset?.githubApiLatestRelease || "";
  if (!apiUrl) {
    applyErrorState();
    return;
  }

  try {
    const response = await fetch(apiUrl, {
      headers: { Accept: "application/vnd.github+json" },
    });

    if (!response.ok) {
      throw new Error(`GitHub API status ${response.status}`);
    }

    const data = await response.json();
    const assets = Array.isArray(data.assets) ? data.assets : [];

    const dmgAsset = findAssetByExtension(assets, ".dmg");
    const exeAsset = findAssetByExtension(assets, ".exe");
    const checksumAsset = findChecksumAsset(assets);

    if (
      (!dmgAsset?.browser_download_url && !exeAsset?.browser_download_url) ||
      !data.tag_name ||
      !data.html_url
    ) {
      applyErrorState();
      return;
    }

    const dmgUrl = dmgAsset?.browser_download_url || "";
    const exeUrl = exeAsset?.browser_download_url || "";
    const version = data.tag_name;
    const releaseNotesUrl = data.html_url;
    const checksumUrl = checksumAsset?.browser_download_url || "";

    applyReadyState({ version, dmgUrl, exeUrl, releaseNotesUrl, checksumUrl });
  } catch (error) {
    console.warn("failed to load latest release", error);
    applyErrorState();
  }
}

wireDraggableDemoBall();
loadLatestRelease();
