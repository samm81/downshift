(() => {
  const input = document.getElementById("minutes");
  const cancelButton = document.getElementById("cancel");
  const confirmButton = document.getElementById("confirm");

  function post(payload) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(payload));
    }
  }

  function submit() {
    const minutes = Number(input.value);
    if (!Number.isFinite(minutes) || minutes < 1) {
      input.focus();
      input.select();
      return;
    }
    post({ cmd: "set_snooze", minutes: Math.round(minutes) });
  }

  cancelButton.addEventListener("click", () => {
    post({ cmd: "close_custom_snooze" });
  });
  confirmButton.addEventListener("click", submit);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      submit();
    }
  });
})();
