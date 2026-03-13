(() => {
  const spinner = document.getElementById("spinner");
  const message = document.getElementById("message");
  const okButton = document.getElementById("ok");
  const downloadButton = document.getElementById("download");

  function post(payload) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(payload));
    }
  }

  window.updateDialogApplyState = function (next) {
    const state = next || {};
    const mode = String(state.mode || "checking");
    if (mode === "checking") {
      spinner.hidden = false;
      message.textContent = "checking for updates...";
      downloadButton.hidden = true;
      return;
    }
    spinner.hidden = true;
    if (mode === "available") {
      const latest = state.latest_version || "latest";
      message.textContent = `new update available (${latest})`;
      downloadButton.hidden = false;
      return;
    }
    message.textContent = "you are on the latest version!";
    downloadButton.hidden = true;
  };

  okButton.addEventListener("click", () => {
    post({ cmd: "close_update_dialog" });
  });

  downloadButton.addEventListener("click", () => {
    post({ cmd: "download_update" });
  });
})();
