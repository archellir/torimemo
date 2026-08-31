// Endpoint and token, stored per-browser.
//
// The token is kept in extension storage rather than synced storage: it is a
// credential for a service on this machine, and syncing it to other devices
// would put it somewhere it cannot be used and should not be.

import { api, settings, status, checkConnection, DEFAULTS } from "./torimemo.js";

const endpoint = document.getElementById("endpoint");
const token = document.getElementById("token");
const statusEl = document.getElementById("status");

const show = (text, state) => status(statusEl, text, state);

settings().then((current) => {
  endpoint.value = current.endpoint;
  token.value = current.token;
});

async function persist() {
  await api.storage.local.set({
    endpoint: endpoint.value.trim() || DEFAULTS.endpoint,
    token: token.value.trim(),
  });
}

document.getElementById("save").addEventListener("click", async () => {
  await persist();
  show("Saved", "saved");
});

document.getElementById("test").addEventListener("click", async () => {
  // Save first: testing the values on screen rather than the stored ones is
  // what the user means by "test", and it avoids reporting success for a
  // configuration that was never written.
  await persist();
  show("Testing…");
  const { ok, message } = await checkConnection();
  show(message, ok ? "saved" : "error");
});
