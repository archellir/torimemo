// The hotkey path: save the current tab without opening anything.
//
// This is the reason the extension exists. Saving has to cost one keystroke,
// or the browser's own Ctrl+D wins and the archive stays empty.
//
// Deliberately has no `import`. Firefox implements the MV3 background as an
// event page loaded from `scripts[]` as a *classic* script, so an ESM import
// throws before any listener is registered — the extension then loads without
// error and silently does nothing. Chrome's service worker would accept a
// module, but one file that works in both is worth the small duplication.

const api = globalThis.browser ?? globalThis.chrome;

const DEFAULTS = {
  endpoint: "http://127.0.0.1:7645",
  token: "",
};

async function settings() {
  const stored = await api.storage.local.get(DEFAULTS);
  return { ...DEFAULTS, ...stored };
}

async function savePage({ url, note }) {
  const { endpoint, token } = await settings();
  const headers = { "Content-Type": "application/json" };
  if (token) headers.Authorization = `Bearer ${token}`;

  const input = note ? { url, note } : { url };

  let response;
  try {
    response = await fetch(`${endpoint}/v1/tools/bookmarks.save`, {
      method: "POST",
      headers,
      body: JSON.stringify({ input }),
    });
  } catch (cause) {
    throw new Error(`torimemo is not running at ${endpoint}`, { cause });
  }

  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    if (response.status === 401) throw new Error("token rejected — check settings");
    if (response.status === 403) throw new Error("this token is read-only");
    throw new Error(payload.detail ?? `save failed (${response.status})`);
  }
  return payload;
}

/// Briefly says what happened, then gets out of the way.
function report(message, ok) {
  api.notifications?.create({
    type: "basic",
    iconUrl: api.runtime.getURL("icons/icon-48.png"),
    title: "torimemo",
    message,
  });
  api.action.setBadgeText({ text: ok ? "ok" : "!" });
  api.action.setBadgeBackgroundColor({ color: ok ? "#2d7d46" : "#b3261e" });
  setTimeout(() => api.action.setBadgeText({ text: "" }), 2500);
}

async function saveCurrentTab() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab?.url || !/^https?:/.test(tab.url)) {
    report("Only http(s) pages can be saved.", false);
    return;
  }

  try {
    // The page title travels as the note, so a saved link is searchable
    // before the enrichment pass has fetched anything.
    const result = await savePage({ url: tab.url, note: tab.title });
    report(
      result.created
        ? "Saved."
        : `Already saved — that's ${result.saved_times} times now.`,
      true,
    );
  } catch (error) {
    report(error.message, false);
  }
}

api.commands.onCommand.addListener((command) => {
  if (command === "save-page") saveCurrentTab();
});
