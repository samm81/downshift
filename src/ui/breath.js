(() => {
  const ball = document.getElementById("ball");
  const menu = document.getElementById("menu");
  const pauseButton = document.getElementById("menu-pause");
  const resetButton = document.getElementById("menu-reset");
  const quitButton = document.getElementById("menu-quit");
  const updatePrimaryButton = document.getElementById("menu-update-primary");
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
  };
  const useNativeMenu = Boolean(init.use_native_menu);
  let breathAnimation = null;

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

  function totalPatternSeconds(pattern) {
    return (
      pattern.expanding_seconds +
      pattern.expanded_hold_seconds +
      pattern.compressing_seconds +
      pattern.compressed_hold_seconds
    );
  }

  function keyframesForPattern(pattern) {
    const total = Math.max(totalPatternSeconds(pattern), 0.1);
    const expandEnd = pattern.expanding_seconds / total;
    const topHoldEnd =
      (pattern.expanding_seconds + pattern.expanded_hold_seconds) / total;
    const compressEnd =
      (pattern.expanding_seconds +
        pattern.expanded_hold_seconds +
        pattern.compressing_seconds) /
      total;
    return [
      { transform: "scale(0.8)", offset: 0 },
      { transform: "scale(1)", offset: expandEnd },
      { transform: "scale(1)", offset: topHoldEnd },
      { transform: "scale(0.8)", offset: compressEnd },
      { transform: "scale(0.8)", offset: 1 },
    ];
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

  function post(payload) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(payload));
    }
  }

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
    if (!breathAnimation) {
      return;
    }
    if (state.paused) {
      breathAnimation.pause();
    } else {
      breathAnimation.play();
    }
  }

  function restartBreathingAnimation() {
    if (breathAnimation) {
      breathAnimation.cancel();
    }
    breathAnimation = ball.animate(
      keyframesForPattern(state.breathingPattern),
      {
        duration: Math.round(
          totalPatternSeconds(state.breathingPattern) * 1000,
        ),
        iterations: Infinity,
        easing: "linear",
      },
    );
    syncAnimationPauseState();
  }

  function applyBallState() {
    ball.classList.toggle("paused", state.paused);
    pauseButton.textContent = state.paused ? "resume" : "pause";
    updatePrimaryButton.textContent = state.updateLabel;
    updatePrimaryButton.dataset.newVersion = state.updateHasNewVersion
      ? "1"
      : "0";
    syncAnimationPauseState();
    positionBadge();
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
    const rect = ball.getBoundingClientRect();
    const badgeSize = 16;
    const inset = 1;
    const x = Math.round(rect.right - badgeSize - inset);
    const y = Math.round(rect.top + inset);
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
      }, 240);
    } else {
      updateBadge.classList.remove("is-dismissing");
      updateBadge.hidden = true;
    }
  }

  function applyUpdateBadge(animateIn) {
    if (!state.updateShowBadge) {
      dismissBadge(false);
      return;
    }
    updateBadge.hidden = false;
    positionBadge();
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

  updateBadge.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    state.updateShowBadge = false;
    dismissBadge(true);
    post({ cmd: "dismiss_update_badge" });
    if (useNativeMenu) {
      post({
        cmd: "show_context_menu",
        x: Math.round(event.clientX),
        y: Math.round(event.clientY),
      });
    } else {
      showMenu(event.clientX, event.clientY);
    }
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
