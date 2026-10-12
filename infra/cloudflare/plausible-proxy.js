const PLAUSIBLE_SCRIPT_URL =
  "https://plausible.io/js/pa-wUnuB97Iei2qopC9zqHUE.js";
const SCRIPT_PATH = "/f3c8d1/script.js";
const EVENT_PATH = "/f3c8d1/event";

async function script_proxy(request, context) {
  const cache_request = new Request(request.url, { method: "GET" });
  let response = await caches.default.match(cache_request);

  if (!response) {
    response = await fetch(PLAUSIBLE_SCRIPT_URL);
    if (response.ok) {
      context.waitUntil(caches.default.put(cache_request, response.clone()));
    }
  }

  return response;
}

async function event_proxy(request) {
  const event_request = new Request("https://plausible.io/api/event", request);
  event_request.headers.delete("cookie");
  return fetch(event_request);
}

export default {
  async fetch(request, _env, context) {
    const url = new URL(request.url);

    if (request.method === "GET" && url.pathname === SCRIPT_PATH) {
      return script_proxy(request, context);
    }

    if (request.method === "POST" && url.pathname === EVENT_PATH) {
      return event_proxy(request);
    }

    return new Response("not found", { status: 404 });
  },
};
