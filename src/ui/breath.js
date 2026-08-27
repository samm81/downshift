(() => {
  const ball = document.getElementById("ball");
  const cursorHalo = document.getElementById("cursor-halo");
  const breathArt = document.getElementById("breath-art");
  const breathGeometry = breathArt.querySelector(".breath-geometry");
  const breathPolygons = Array.from(
    breathArt.querySelectorAll(".breath-polygon"),
  );
  const breathArtwork = document.querySelector(".breath-artwork");
  const breathHitTarget = document.getElementById("breath-hit-target");
  const menu = document.getElementById("menu");
  const pauseButton = document.getElementById("menu-pause");
  const followCursorButton = document.getElementById("menu-follow-cursor");
  const resetButton = document.getElementById("menu-reset");
  const quitButton = document.getElementById("menu-quit");
  const updatePrimaryButton = document.getElementById("menu-update-primary");
  const updateIgnoreCurrentButton = document.getElementById(
    "menu-update-ignore-current",
  );
  const updateBadge = document.getElementById("update-badge");
  const customSnoozeButton = document.getElementById("menu-snooze-custom");
  const analyticsToggleButton = document.getElementById(
    "menu-analytics-toggle",
  );
  const analyticsSubmenu = document.getElementById("analytics-submenu");
  const breathingPatternButton = document.getElementById(
    "menu-breathing-pattern",
  );
  const breathingSubmenu = document.getElementById("breathing-submenu");
  const breathingPresetList = document.getElementById("breathing-preset-list");
  const breathingEditButton = document.getElementById("menu-breathing-edit");
  const breathingDeleteButton = document.getElementById(
    "menu-breathing-delete",
  );
  const breathingDeleteSubmenu = document.getElementById(
    "breathing-delete-submenu",
  );
  const breathingDeleteList = document.getElementById("breathing-delete-list");
  const usageOnButton = document.getElementById("menu-usage-on");
  const usageOffButton = document.getElementById("menu-usage-off");
  const crashOnButton = document.getElementById("menu-crash-on");
  const crashOffButton = document.getElementById("menu-crash-off");
  const whatWeCollectButton = document.getElementById("menu-what-we-collect");
  const sizeButtons = Array.from(document.querySelectorAll("[data-size-slot]"));
  const snoozeButtons = Array.from(
    document.querySelectorAll("[data-snooze-minutes]"),
  );
  const init = window.__BB_INIT__ || {
    paused: false,
    use_native_menu: false,
    follow_cursor_active: false,
    follow_cursor_available: false,
    follow_cursor_unavailable_reason:
      "cursor following is unavailable on this platform",
  };
  const useNativeMenu = Boolean(init.use_native_menu);
  let animationFrameId = null;
  let animationElapsedMs = 0;
  let animationTimestamp = null;

  function normalizePattern(pattern) {
    const fallback = {
      expanding_seconds: 5.5,
      expanded_hold_seconds: 0,
      compressing_seconds: 5.5,
      compressed_hold_seconds: 0,
    };
    const candidate =
      pattern && typeof pattern === "object" ? pattern : fallback;
    const next = {
      expanding_seconds: Number(candidate.expanding_seconds),
      expanded_hold_seconds: Number(candidate.expanded_hold_seconds),
      compressing_seconds: Number(candidate.compressing_seconds),
      compressed_hold_seconds: Number(candidate.compressed_hold_seconds),
    };
    if (
      !Number.isFinite(next.expanding_seconds) ||
      next.expanding_seconds <= 0
    ) {
      next.expanding_seconds = fallback.expanding_seconds;
    }
    if (
      !Number.isFinite(next.expanded_hold_seconds) ||
      next.expanded_hold_seconds < 0
    ) {
      next.expanded_hold_seconds = fallback.expanded_hold_seconds;
    }
    if (
      !Number.isFinite(next.compressing_seconds) ||
      next.compressing_seconds <= 0
    ) {
      next.compressing_seconds = fallback.compressing_seconds;
    }
    if (
      !Number.isFinite(next.compressed_hold_seconds) ||
      next.compressed_hold_seconds < 0
    ) {
      next.compressed_hold_seconds = fallback.compressed_hold_seconds;
    }
    return next;
  }

  const {
    clamp,
    easeInOut,
    pathData,
    polygonBaseline,
    polygonCenterX,
    polygonPointsForProgress,
    polygonSideLength,
    terminalPointsForProgress,
    terminalShapeFraction,
  } = window.downshiftPolygonAnimation;
  const terminalHitTargetSizePx = 56;
  const terminalHitTargetPaddingPx = 12;
  const terminalHitTargetMaxProgress = 0.55;
  const animationBoundsPaddingPx = 2;
  const animationBoundsEpsilon = 0.25;
  let currentAnimationBounds = null;
  let lastWindowBounds = null;
  let maximumAnimationBounds = null;
  function updateBreathingHitTarget(progress) {
    const terminalProgress = clamp(progress / terminalShapeFraction, 0, 1);
    const hitTargetActive =
      progress < terminalShapeFraction &&
      terminalProgress <= terminalHitTargetMaxProgress;
    if (hitTargetActive) {
      breathHitTarget.removeAttribute("hidden");
    } else {
      breathHitTarget.setAttribute("hidden", "");
    }
    if (!hitTargetActive) {
      return;
    }

    const artworkWidth = breathArtwork.getBoundingClientRect().width;
    const viewBoxUnitsPerPixel = 100 / Math.max(artworkWidth, 1);
    const targetHeight = terminalHitTargetSizePx * viewBoxUnitsPerPixel;
    const lineWidth =
      polygonSideLength + terminalHitTargetPaddingPx * 2 * viewBoxUnitsPerPixel;
    const dotToLineProgress = clamp(terminalProgress / 0.5, 0, 1);
    const targetWidth =
      targetHeight + (lineWidth - targetHeight) * easeInOut(dotToLineProgress);

    breathHitTarget.setAttribute(
      "x",
      (polygonCenterX - targetWidth / 2).toFixed(3),
    );
    breathHitTarget.setAttribute(
      "y",
      (state.followCursorActive
        ? 100 - polygonBaseline
        : polygonBaseline - targetHeight
      ).toFixed(3),
    );
    breathHitTarget.setAttribute("width", targetWidth.toFixed(3));
    breathHitTarget.setAttribute("height", targetHeight.toFixed(3));
    breathHitTarget.setAttribute("rx", (targetHeight / 2).toFixed(3));
  }

  function renderBreathingProgress(progress, syncBounds = true) {
    const terminalProgress = clamp(progress / terminalShapeFraction, 0, 1);
    const polygonProgress = clamp(
      (progress - terminalShapeFraction) / (1 - terminalShapeFraction),
      0,
      1,
    );
    breathPolygons.forEach((polygon) => {
      const layerIndex = Number(polygon.dataset.layerIndex);
      const vertexCount = Number(polygon.dataset.sides);
      const points =
        progress <= terminalShapeFraction
          ? terminalPointsForProgress(vertexCount, terminalProgress)
          : polygonPointsForProgress(layerIndex, vertexCount, polygonProgress);
      const orientedPoints = state.followCursorActive
        ? points.map(([x, y]) => [x, 100 - y])
        : points;
      polygon.setAttribute("d", pathData(orientedPoints));
    });
    applyCursorHalo(progress);
    updateBreathingHitTarget(progress);
    if (syncBounds) {
      syncAnimationBounds();
    }
  }

  function breathingProgressAt(elapsedMs) {
    const pattern = state.breathingPattern;
    const expandingMs = pattern.expanding_seconds * 1000;
    const expandedHoldMs = pattern.expanded_hold_seconds * 1000;
    const compressingMs = pattern.compressing_seconds * 1000;
    const compressedHoldMs = pattern.compressed_hold_seconds * 1000;
    const cycleMs = Math.max(
      expandingMs + expandedHoldMs + compressingMs + compressedHoldMs,
      100,
    );
    let remaining = elapsedMs % cycleMs;
    if (remaining < expandingMs) {
      return easeInOut(remaining / expandingMs);
    }
    remaining -= expandingMs;
    if (remaining < expandedHoldMs) {
      return 1;
    }
    remaining -= expandedHoldMs;
    if (remaining < compressingMs) {
      return 1 - easeInOut(remaining / compressingMs);
    }
    return 0;
  }

  const state = {
    paused: Boolean(init.paused),
    breathingPattern: normalizePattern(init.breathing_pattern),
    activeBreathingPresetId: String(
      init.active_breathing_preset_id || "coherent_breathing",
    ),
    breathingPresets: Array.isArray(init.breathing_presets)
      ? init.breathing_presets
      : [],
    usageDataSharing: Object.prototype.hasOwnProperty.call(
      init,
      "usage_data_sharing",
    )
      ? Boolean(init.usage_data_sharing)
      : true,
    crashReportsSharing: Object.prototype.hasOwnProperty.call(
      init,
      "crash_reports_sharing",
    )
      ? Boolean(init.crash_reports_sharing)
      : true,
    analyticsOpen: false,
    breathingOpen: false,
    updateLabel: String(init.update_menu_label || "check for updates"),
    updateHasNewVersion: Boolean(init.update_has_new_version),
    updateShowBadge: Boolean(init.update_show_badge),
    updateIgnoreCurrentEnabled: Boolean(init.update_ignore_current_enabled),
    updateIgnoreCurrentChecked: Boolean(init.update_ignore_current_checked),
    followCursorActive: Boolean(init.follow_cursor_active),
    followCursorAvailable: Boolean(init.follow_cursor_available),
    followCursorUnavailableReason: String(
      init.follow_cursor_unavailable_reason ||
        "cursor following is unavailable on this platform",
    ),
    followCursorHaloSize:
      Number.isFinite(Number(init.follow_cursor_halo_size)) &&
      Number(init.follow_cursor_halo_size) > 0
        ? Number(init.follow_cursor_halo_size)
        : 56,
    followCursorWindowSize:
      Number.isFinite(Number(init.follow_cursor_window_size)) &&
      Number(init.follow_cursor_window_size) > 0
        ? Number(init.follow_cursor_window_size)
        : 64,
    artworkSize:
      Number.isFinite(Number(init.artwork_size)) &&
      Number(init.artwork_size) > 0
        ? Number(init.artwork_size)
        : Math.max(window.innerWidth, 1),
    sizePresets:
      Array.isArray(init.size_presets) && init.size_presets.length === 4
        ? init.size_presets
            .map((value) => Number(value))
            .filter((value) => Number.isFinite(value) && value > 0)
        : [64, 96, 128, 160],
  };
  updateBadge.title = String(init.update_tooltip || "new version available");
  if (state.sizePresets.length !== 4) {
    state.sizePresets = [64, 96, 128, 160];
  }

  const cssRootFontSizeLogical = (() => {
    const value = Number.parseFloat(
      window.getComputedStyle(document.documentElement).fontSize,
    );
    return Number.isFinite(value) && value > 0 ? value : 16;
  })();

  function cssRem(value) {
    return `${value / cssRootFontSizeLogical}rem`;
  }

  function applyCursorHalo(progress) {
    const normalizedProgress = Math.min(Math.max(Number(progress) || 0, 0), 1);
    const scale = 0.9 + normalizedProgress * 0.14;
    const opacity = 0.52 + normalizedProgress * 0.18;
    cursorHalo.style.setProperty(
      "--cursor-halo-size",
      cssRem(state.followCursorHaloSize),
    );
    cursorHalo.style.setProperty("--cursor-halo-scale", scale.toFixed(3));
    cursorHalo.style.setProperty("--cursor-halo-opacity", opacity.toFixed(3));
  }

  function post(payload) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(payload));
    }
  }

  function applyArtworkLayout(bounds) {
    const artworkSize = Math.max(state.artworkSize, 1);
    const viewBoxUnitsPerPixel = 100 / artworkSize;
    const maximum = maximumAnimationBounds || bounds;
    const followWindowCenter = state.followCursorWindowSize / 2;
    const followArtworkCenterX =
      (bounds.x + bounds.width / 2) / viewBoxUnitsPerPixel;
    const followArtworkCenterY =
      (bounds.y + bounds.height / 2) / viewBoxUnitsPerPixel;
    document.documentElement.style.setProperty(
      "--artwork-size",
      cssRem(artworkSize),
    );
    document.documentElement.style.setProperty(
      "--artwork-left",
      cssRem(
        state.followCursorActive
          ? followWindowCenter - followArtworkCenterX
          : animationBoundsPaddingPx +
              (maximum.width / 2 - (bounds.x + bounds.width / 2)) /
                viewBoxUnitsPerPixel,
      ),
    );
    document.documentElement.style.setProperty(
      "--artwork-top",
      cssRem(
        state.followCursorActive
          ? followWindowCenter - followArtworkCenterY
          : animationBoundsPaddingPx +
              (maximum.height - (bounds.y + bounds.height)) /
                viewBoxUnitsPerPixel,
      ),
    );
  }

  function animationBoundsChanged(previous, next) {
    if (!previous) {
      return true;
    }
    return (
      ["x", "y", "width", "height"].some(
        (key) => Math.abs(previous[key] - next[key]) > animationBoundsEpsilon,
      ) || previous.badge_visible !== next.badge_visible
    );
  }

  function readAnimationBounds() {
    let box;
    try {
      box = breathGeometry.getBBox();
    } catch (_error) {
      return null;
    }
    if (
      !box ||
      ![box.x, box.y, box.width, box.height].every(Number.isFinite) ||
      box.width < 0 ||
      box.height < 0
    ) {
      return null;
    }
    return {
      x: box.x,
      y: box.y,
      width: box.width,
      height: box.height,
    };
  }

  function measureMaximumAnimationBounds() {
    renderBreathingProgress(1, false);
    const bounds = readAnimationBounds();
    renderBreathingProgress(0, false);
    return (
      bounds || {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      }
    );
  }

  function syncAnimationBounds() {
    const bounds = readAnimationBounds();
    if (!bounds) {
      return;
    }
    currentAnimationBounds = bounds;
    applyArtworkLayout(bounds);
    // Keep the native hitbox fixed at the largest shape; only the artwork
    // layout changes as the animation morphs.
    const windowBounds = {
      ...maximumAnimationBounds,
      badge_visible: !updateBadge.hidden,
    };
    if (!animationBoundsChanged(lastWindowBounds, windowBounds)) {
      return;
    }
    lastWindowBounds = windowBounds;
    post({ cmd: "set_animation_bounds", ...windowBounds });
  }

  maximumAnimationBounds = measureMaximumAnimationBounds();

  function hideMenu() {
    menu.hidden = true;
    analyticsSubmenu.hidden = true;
    breathingSubmenu.hidden = true;
    breathingDeleteSubmenu.hidden = true;
    state.analyticsOpen = false;
    state.breathingOpen = false;
  }

  function showMenu(x, y) {
    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;
    menu.hidden = false;
  }

  function syncAnimationPauseState() {
    renderBreathingProgress(breathingProgressAt(animationElapsedMs));
  }

  function animateBreathing(timestamp) {
    if (animationTimestamp === null) {
      animationTimestamp = timestamp;
    }
    if (!state.paused) {
      animationElapsedMs += Math.min(timestamp - animationTimestamp, 100);
      renderBreathingProgress(breathingProgressAt(animationElapsedMs));
    }
    animationTimestamp = timestamp;
    animationFrameId = window.requestAnimationFrame(animateBreathing);
  }

  function restartBreathingAnimation() {
    if (animationFrameId !== null) {
      window.cancelAnimationFrame(animationFrameId);
    }
    animationElapsedMs = 0;
    animationTimestamp = null;
    renderBreathingProgress(0);
    animationFrameId = window.requestAnimationFrame(animateBreathing);
  }

  function applyBallState() {
    document.body.classList.toggle("follow-cursor", state.followCursorActive);
    ball.classList.toggle("paused", state.paused);
    pauseButton.textContent = state.paused ? "resume" : "pause";
    applyFollowCursorButton();
    updatePrimaryButton.textContent = state.updateLabel;
    updatePrimaryButton.dataset.newVersion = state.updateHasNewVersion
      ? "1"
      : "0";
    updateIgnoreCurrentButton.disabled = !state.updateIgnoreCurrentEnabled;
    updateIgnoreCurrentButton.textContent =
      `do not remind me about the current update again ${state.updateIgnoreCurrentChecked ? "✓" : ""}`.trim();
    syncAnimationPauseState();
    if (state.followCursorActive) {
      dismissBadge(false);
    } else {
      positionBadge();
    }
  }

  function applyFollowCursorButton() {
    if (!state.followCursorAvailable) {
      followCursorButton.disabled = true;
      followCursorButton.textContent = `follow cursor (${state.followCursorUnavailableReason})`;
      followCursorButton.title = state.followCursorUnavailableReason;
      return;
    }
    followCursorButton.disabled = state.followCursorActive;
    followCursorButton.textContent = `follow cursor${state.followCursorActive ? " ✓" : ""}`;
    followCursorButton.title = state.followCursorActive
      ? "cursor halo is active; restart downshift to return to fixed mode"
      : "surround the mouse cursor with a breathing halo";
  }

  function applyAnalyticsButtons() {
    usageOnButton.textContent =
      `share anonymous usage data ${state.usageDataSharing ? "✓" : ""}`.trim();
    usageOffButton.textContent =
      `don’t share usage data ${!state.usageDataSharing ? "✓" : ""}`.trim();
    crashOnButton.textContent =
      `share anonymous crash reports ${state.crashReportsSharing ? "✓" : ""}`.trim();
    crashOffButton.textContent =
      `don't share crash reports ${!state.crashReportsSharing ? "✓" : ""}`.trim();
    analyticsToggleButton.textContent = "help improve downshift";
  }

  function applyBreathingButtons() {
    const activeId = state.activeBreathingPresetId;
    breathingPatternButton.textContent = "breathing pattern";
    breathingPresetList.textContent = "";
    breathingDeleteList.textContent = "";
    state.breathingPresets.forEach((preset) => {
      const button = document.createElement("button");
      button.dataset.breathingPreset = preset.id;
      const isActive = preset.id === activeId;
      button.textContent = `${preset.name}${isActive ? " ✓" : ""}`;
      button.addEventListener("click", () => {
        post({
          cmd: "apply_breathing_pattern",
          preset_id: preset.id,
          pattern: state.breathingPattern,
        });
        hideMenu();
      });
      breathingPresetList.appendChild(button);

      const deleteButton = document.createElement("button");
      deleteButton.textContent = preset.name;
      deleteButton.addEventListener("click", () => {
        post({ cmd: "delete_breathing_preset", preset_id: preset.id });
        hideMenu();
      });
      breathingDeleteList.appendChild(deleteButton);
    });
    breathingDeleteButton.disabled = state.breathingPresets.length === 0;
  }

  function applySizePresetButtons() {
    const labels = ["S", "M", "L", "XL"];
    sizeButtons.forEach((button) => {
      const rawIndex = Number(button.dataset.sizeSlot);
      const index = Number.isFinite(rawIndex) ? rawIndex : -1;
      const value = state.sizePresets[index];
      if (!Number.isFinite(value) || value <= 0) {
        return;
      }
      const rounded = Math.round(value);
      button.dataset.size = String(rounded);
      button.textContent = `${labels[index] || "size"} (${rounded}px)`;
    });
  }

  function positionBadge() {
    if (updateBadge.hidden) {
      return;
    }
    const shape = breathPolygons[0] || breathArtwork || ball;
    const rect = shape.getBoundingClientRect();
    const badgeSize = 16;
    const gap = 8;
    const maxX = Math.max(0, window.innerWidth - badgeSize);
    const maxY = Math.max(0, window.innerHeight - badgeSize);
    const x = clamp(
      Math.round(rect.left + rect.width / 2 - badgeSize / 2),
      0,
      maxX,
    );
    const y = clamp(Math.round(rect.bottom + gap), 0, maxY);
    updateBadge.style.left = `${x}px`;
    updateBadge.style.top = `${y}px`;
  }

  function dismissBadge(withAnimation) {
    if (updateBadge.hidden) {
      return;
    }
    if (withAnimation) {
      updateBadge.classList.remove("is-appearing");
      updateBadge.classList.add("is-dismissing");
      window.setTimeout(() => {
        updateBadge.classList.remove("is-dismissing");
        updateBadge.hidden = true;
        syncAnimationBounds();
      }, 240);
    } else {
      updateBadge.classList.remove("is-dismissing");
      updateBadge.hidden = true;
      syncAnimationBounds();
    }
  }

  function applyUpdateBadge(animateIn) {
    if (state.followCursorActive) {
      dismissBadge(false);
      return;
    }
    if (!state.updateShowBadge) {
      dismissBadge(false);
      return;
    }
    updateBadge.hidden = false;
    positionBadge();
    syncAnimationBounds();
    updateBadge.classList.remove("is-dismissing");
    if (animateIn) {
      updateBadge.classList.remove("is-appearing");
      void updateBadge.offsetWidth;
      updateBadge.classList.add("is-appearing");
      window.setTimeout(() => {
        updateBadge.classList.remove("is-appearing");
      }, 420);
    }
  }

  window.breathBallApplyState = function (next) {
    if (Object.prototype.hasOwnProperty.call(next, "paused")) {
      state.paused = Boolean(next.paused);
    }
    if (Object.prototype.hasOwnProperty.call(next, "breathing_pattern")) {
      state.breathingPattern = normalizePattern(next.breathing_pattern);
      restartBreathingAnimation();
    }
    if (
      Object.prototype.hasOwnProperty.call(next, "active_breathing_preset_id")
    ) {
      state.activeBreathingPresetId = String(
        next.active_breathing_preset_id || "custom",
      );
    }
    if (Object.prototype.hasOwnProperty.call(next, "breathing_presets")) {
      state.breathingPresets = Array.isArray(next.breathing_presets)
        ? next.breathing_presets
        : [];
    }
    if (Object.prototype.hasOwnProperty.call(next, "artwork_size")) {
      const artworkSize = Number(next.artwork_size);
      if (Number.isFinite(artworkSize) && artworkSize > 0) {
        state.artworkSize = artworkSize;
        if (currentAnimationBounds) {
          applyArtworkLayout(currentAnimationBounds);
        }
      }
    }
    if (Object.prototype.hasOwnProperty.call(next, "follow_cursor_active")) {
      state.followCursorActive = Boolean(next.follow_cursor_active);
    }
    if (Object.prototype.hasOwnProperty.call(next, "follow_cursor_halo_size")) {
      const haloSize = Number(next.follow_cursor_halo_size);
      if (Number.isFinite(haloSize) && haloSize > 0) {
        state.followCursorHaloSize = haloSize;
      }
    }
    if (
      Object.prototype.hasOwnProperty.call(next, "follow_cursor_window_size")
    ) {
      const windowSize = Number(next.follow_cursor_window_size);
      if (Number.isFinite(windowSize) && windowSize > 0) {
        state.followCursorWindowSize = windowSize;
      }
    }
    if (Object.prototype.hasOwnProperty.call(next, "follow_cursor_available")) {
      state.followCursorAvailable = Boolean(next.follow_cursor_available);
    }
    if (
      Object.prototype.hasOwnProperty.call(
        next,
        "follow_cursor_unavailable_reason",
      )
    ) {
      state.followCursorUnavailableReason = String(
        next.follow_cursor_unavailable_reason ||
          "cursor following is unavailable on this platform",
      );
    }
    if (Object.prototype.hasOwnProperty.call(next, "size_presets")) {
      const values = Array.isArray(next.size_presets)
        ? next.size_presets
            .map((value) => Number(value))
            .filter((value) => Number.isFinite(value) && value > 0)
        : [];
      if (values.length === 4) {
        state.sizePresets = values;
        applySizePresetButtons();
      }
    }
    if (Object.prototype.hasOwnProperty.call(next, "usage_data_sharing")) {
      state.usageDataSharing = Boolean(next.usage_data_sharing);
    }
    if (Object.prototype.hasOwnProperty.call(next, "crash_reports_sharing")) {
      state.crashReportsSharing = Boolean(next.crash_reports_sharing);
    }
    let animateBadge = false;
    if (Object.prototype.hasOwnProperty.call(next, "update_menu_label")) {
      state.updateLabel = String(next.update_menu_label || "check for updates");
    }
    if (Object.prototype.hasOwnProperty.call(next, "update_has_new_version")) {
      state.updateHasNewVersion = Boolean(next.update_has_new_version);
    }
    if (Object.prototype.hasOwnProperty.call(next, "update_show_badge")) {
      const previous = state.updateShowBadge;
      state.updateShowBadge = Boolean(next.update_show_badge);
      animateBadge = !previous && state.updateShowBadge;
      if (previous && !state.updateShowBadge) {
        dismissBadge(true);
      }
    }
    if (
      Object.prototype.hasOwnProperty.call(
        next,
        "update_ignore_current_enabled",
      )
    ) {
      state.updateIgnoreCurrentEnabled = Boolean(
        next.update_ignore_current_enabled,
      );
    }
    if (
      Object.prototype.hasOwnProperty.call(
        next,
        "update_ignore_current_checked",
      )
    ) {
      state.updateIgnoreCurrentChecked = Boolean(
        next.update_ignore_current_checked,
      );
    }
    applyBallState();
    applyAnalyticsButtons();
    applyBreathingButtons();
    applyUpdateBadge(animateBadge);
  };

  ball.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const direction = event.deltaY < 0 ? 1 : -1;
      post({ cmd: "resize", delta: direction, fine: event.shiftKey });
    },
    { passive: false },
  );

  ball.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    if (useNativeMenu) {
      post({
        cmd: "show_context_menu",
        x: Math.round(event.clientX),
        y: Math.round(event.clientY),
      });
      return;
    }
    post({ cmd: "analytics_menu_opened" });
    applyBallState();
    applyAnalyticsButtons();
    showMenu(event.clientX, event.clientY);
  });

  followCursorButton.addEventListener("click", () => {
    if (!state.followCursorAvailable || state.followCursorActive) {
      return;
    }
    post({ cmd: "set_follow_cursor", enabled: true });
    hideMenu();
  });

  pauseButton.addEventListener("click", () => {
    state.paused = !state.paused;
    applyBallState();
    post({ cmd: "set_paused", paused: state.paused });
    hideMenu();
  });

  sizeButtons.forEach((button) => {
    button.addEventListener("click", () => {
      const size = Number(button.dataset.size);
      if (!Number.isFinite(size) || size <= 0) {
        return;
      }
      post({ cmd: "set_size", size });
      hideMenu();
    });
  });

  snoozeButtons.forEach((button) => {
    button.addEventListener("click", () => {
      const minutes = Number(button.dataset.snoozeMinutes);
      if (!Number.isFinite(minutes) || minutes <= 0) {
        return;
      }
      post({ cmd: "set_snooze", minutes: Math.round(minutes) });
      hideMenu();
    });
  });

  customSnoozeButton.addEventListener("click", () => {
    post({ cmd: "show_custom_snooze" });
    hideMenu();
  });

  resetButton.addEventListener("click", () => {
    state.paused = false;
    applyBallState();
    post({ cmd: "reset" });
    hideMenu();
  });

  breathingPatternButton.addEventListener("click", () => {
    state.breathingOpen = !state.breathingOpen;
    breathingSubmenu.hidden = !state.breathingOpen;
  });

  breathingEditButton.addEventListener("click", () => {
    post({ cmd: "show_breathing_pattern" });
    hideMenu();
  });

  breathingDeleteButton.addEventListener("click", () => {
    breathingDeleteSubmenu.hidden = !breathingDeleteSubmenu.hidden;
  });

  quitButton.addEventListener("click", () => {
    post({ cmd: "quit" });
    hideMenu();
  });

  updatePrimaryButton.addEventListener("click", () => {
    post({ cmd: "update_primary_action" });
    hideMenu();
  });

  updateIgnoreCurrentButton.addEventListener("click", () => {
    if (!state.updateIgnoreCurrentEnabled) {
      return;
    }
    state.updateIgnoreCurrentChecked = !state.updateIgnoreCurrentChecked;
    if (state.updateIgnoreCurrentChecked) {
      state.updateShowBadge = false;
      dismissBadge(true);
    }
    applyBallState();
    post({
      cmd: "set_ignore_current_update",
      ignored: state.updateIgnoreCurrentChecked,
    });
    hideMenu();
  });

  updateBadge.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    state.updateShowBadge = false;
    dismissBadge(true);
    post({ cmd: "dismiss_update_badge" });
    post({ cmd: "show_update_dialog" });
  });

  analyticsToggleButton.addEventListener("click", () => {
    state.analyticsOpen = !state.analyticsOpen;
    analyticsSubmenu.hidden = !state.analyticsOpen;
    applyAnalyticsButtons();
    if (state.analyticsOpen) {
      post({ cmd: "analytics_menu_opened" });
    }
  });

  usageOnButton.addEventListener("click", () => {
    state.usageDataSharing = true;
    applyAnalyticsButtons();
    post({ cmd: "set_usage_data_sharing", enabled: true });
  });

  usageOffButton.addEventListener("click", () => {
    state.usageDataSharing = false;
    applyAnalyticsButtons();
    post({ cmd: "set_usage_data_sharing", enabled: false });
  });

  crashOnButton.addEventListener("click", () => {
    state.crashReportsSharing = true;
    applyAnalyticsButtons();
    post({ cmd: "set_crash_reports_sharing", enabled: true });
  });

  crashOffButton.addEventListener("click", () => {
    state.crashReportsSharing = false;
    applyAnalyticsButtons();
    post({ cmd: "set_crash_reports_sharing", enabled: false });
  });

  whatWeCollectButton.addEventListener("click", () => {
    post({ cmd: "show_telemetry_info" });
  });

  document.addEventListener("mousedown", (event) => {
    if (!menu.hidden && !menu.contains(event.target)) {
      hideMenu();
    }
  });

  document.addEventListener("blur", hideMenu);
  window.addEventListener("resize", () => {
    hideMenu();
    positionBadge();
  });

  const drag = {
    active: false,
    pointerId: null,
  };

  ball.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) {
      return;
    }
    drag.active = true;
    drag.pointerId = event.pointerId;
    if (typeof ball.setPointerCapture === "function") {
      ball.setPointerCapture(event.pointerId);
    }
    post({
      cmd: "start_drag",
      screen_x: Math.round(event.screenX),
      screen_y: Math.round(event.screenY),
    });
  });

  ball.addEventListener("pointermove", (event) => {
    if (!drag.active || event.pointerId !== drag.pointerId) {
      return;
    }
    post({
      cmd: "drag_to",
      screen_x: Math.round(event.screenX),
      screen_y: Math.round(event.screenY),
    });
  });

  function endDrag(event) {
    if (!drag.active) {
      return;
    }
    if (
      event &&
      drag.pointerId !== null &&
      event.pointerId !== drag.pointerId
    ) {
      return;
    }
    if (event && typeof ball.releasePointerCapture === "function") {
      ball.releasePointerCapture(event.pointerId);
    }
    drag.active = false;
    drag.pointerId = null;
    post({ cmd: "end_drag" });
  }

  ball.addEventListener("pointerup", endDrag);
  ball.addEventListener("pointercancel", endDrag);

  restartBreathingAnimation();
  applyBallState();
  applyAnalyticsButtons();
  applyBreathingButtons();
  applySizePresetButtons();
  applyUpdateBadge(false);
})();
