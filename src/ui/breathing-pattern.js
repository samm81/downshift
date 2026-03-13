(() => {
  const expandInput = document.getElementById("expand");
  const expandHoldInput = document.getElementById("expand-hold");
  const compressInput = document.getElementById("compress");
  const compressHoldInput = document.getElementById("compress-hold");
  const summary = document.getElementById("summary");
  const presetNameInput = document.getElementById("preset-name");
  const cancelButton = document.getElementById("cancel");
  const applyButton = document.getElementById("apply");
  const state = {
    pattern: {
      expanding_seconds: 5.5,
      expanded_hold_seconds: 0,
      compressing_seconds: 5.5,
      compressed_hold_seconds: 0,
    },
  };

  function post(payload) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(payload));
    }
  }

  function normalizePattern(pattern) {
    const next = {
      expanding_seconds: Number(pattern && pattern.expanding_seconds),
      expanded_hold_seconds: Number(pattern && pattern.expanded_hold_seconds),
      compressing_seconds: Number(pattern && pattern.compressing_seconds),
      compressed_hold_seconds: Number(
        pattern && pattern.compressed_hold_seconds,
      ),
    };
    if (
      !Number.isFinite(next.expanding_seconds) ||
      next.expanding_seconds <= 0
    ) {
      next.expanding_seconds = 5.5;
    }
    if (
      !Number.isFinite(next.expanded_hold_seconds) ||
      next.expanded_hold_seconds < 0
    ) {
      next.expanded_hold_seconds = 0;
    }
    if (
      !Number.isFinite(next.compressing_seconds) ||
      next.compressing_seconds <= 0
    ) {
      next.compressing_seconds = 5.5;
    }
    if (
      !Number.isFinite(next.compressed_hold_seconds) ||
      next.compressed_hold_seconds < 0
    ) {
      next.compressed_hold_seconds = 0;
    }
    return next;
  }

  function readInputs() {
    return normalizePattern({
      expanding_seconds: expandInput.value,
      expanded_hold_seconds: expandHoldInput.value,
      compressing_seconds: compressInput.value,
      compressed_hold_seconds: compressHoldInput.value,
    });
  }

  function writeInputs(pattern) {
    expandInput.value = String(pattern.expanding_seconds);
    expandHoldInput.value = String(pattern.expanded_hold_seconds);
    compressInput.value = String(pattern.compressing_seconds);
    compressHoldInput.value = String(pattern.compressed_hold_seconds);
  }

  function updateSummary(pattern) {
    const total =
      pattern.expanding_seconds +
      pattern.expanded_hold_seconds +
      pattern.compressing_seconds +
      pattern.compressed_hold_seconds;
    summary.textContent = `cycle: ${pattern.expanding_seconds} / ${pattern.expanded_hold_seconds} / ${pattern.compressing_seconds} / ${pattern.compressed_hold_seconds} (${total}s total)`;
  }

  window.breathingPatternApplyState = function (next) {
    const payload = next || {};
    state.pattern = normalizePattern(payload.pattern || state.pattern);
    writeInputs(state.pattern);
    updateSummary(state.pattern);
  };

  [expandInput, expandHoldInput, compressInput, compressHoldInput].forEach(
    (input) => {
      input.addEventListener("input", () => {
        const pattern = readInputs();
        state.pattern = pattern;
        updateSummary(pattern);
      });
    },
  );

  applyButton.addEventListener("click", () => {
    const pattern = readInputs();
    state.pattern = pattern;
    const name = String(presetNameInput.value || "").trim();
    if (!name) {
      presetNameInput.focus();
      return;
    }
    post({ cmd: "save_breathing_preset", name, pattern });
    presetNameInput.value = "";
  });

  cancelButton.addEventListener("click", () => {
    post({ cmd: "close_breathing_pattern" });
  });
  presetNameInput.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      applyButton.click();
    }
  });
})();
