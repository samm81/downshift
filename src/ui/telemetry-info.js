(() => {
  const ok = document.getElementById("ok");
  ok.addEventListener("click", () => {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify({ cmd: "close_telemetry_info" }));
    }
  });
})();
