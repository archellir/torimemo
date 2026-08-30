// Endpoint and token, stored per-browser.
//
// The token is kept in extension storage rather than synced storage: it is a
// credential for a service on this machine, and syncing it to other devices
// would put it somewhere it cannot be used and should not be.

import { api, settings, DEFAULTS } from "./torimemo.js";

const endpoint = document.getElementById("endpoint");
const token = document.getElementById("token");
const status = document.getElementById("status");

function show(text, state) {
  status.textContent = text;
  if (state) status.dataset.state = state;
  else delete status.dataset.state;
}

settings().then((current) => {
  endpoint.value = current.endpoint;
  token.value = current.token;
});

document.getElementById("save").addEventListener("click", async () => {
  await api.storage.local.set({
    endpoint: endpoint.value.trim() || DEFAULTS.endpoint,
    token: token.value.trim(),
  });
  show("Saved", "saved");
});

document.getElementById("test").addEventListener("click", async () => {
  show("Testing…");
  const base = endpoint.value.trim() || DEFAULTS.endpoint;
  const headers = token.value.trim()
    ? { Authorization: `Bearer ${token.value.trim()}` }
    : {};
  try {
    const response = await fetch(`${base}/v1/tools`, { headers });
    if (response.status === 401) {
      show("Reachable, but the token was rejected.", "error");
      return;
    }
    if (!response.ok) {
      show(`Reachable, but returned ${response.status}.`, "error");
      return;
    }
    const { tools } = await response.json();
    show(`Connected — ${tools.length} tools available.`, "saved");
  } catch {
    show(`No response from ${base}. Is torimemo serve running?`, "error");
  }
});
