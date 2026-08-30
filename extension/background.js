// The hotkey path: save the current tab without opening anything.
//
// This is the reason the extension exists. Saving has to cost one keystroke,
// or the browser's own Ctrl+D wins and the archive stays empty.

import { api, savePage } from "./torimemo.js";

/// Briefly says what happened, then gets out of the way.
///
/// A notification rather than a popup: the point is not to interrupt. The
/// badge carries the same information for anyone who has notifications off.
function report(title, message, ok) {
  api.notifications?.create({
    type: "basic",
    iconUrl: api.runtime.getURL("icons/icon-48.png"),
    title,
    message,
  });
  api.action.setBadgeText({ text: ok ? "ok" : "!" });
  api.action.setBadgeBackgroundColor({ color: ok ? "#2d7d46" : "#b3261e" });
  setTimeout(() => api.action.setBadgeText({ text: "" }), 2500);
}

async function saveCurrentTab() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab?.url || !/^https?:/.test(tab.url)) {
    report("torimemo", "Only http(s) pages can be saved.", false);
    return;
  }

  try {
    // The page title travels as the note, so an imported link is searchable
    // before the enrichment pass has fetched anything.
    const result = await savePage({ url: tab.url, note: tab.title });
    report(
      "torimemo",
      result.created
        ? "Saved."
        : `Already saved — that's ${result.saved_times} times now.`,
      true,
    );
  } catch (error) {
    report("torimemo", error.message, false);
  }
}

api.commands.onCommand.addListener((command) => {
  if (command === "save-page") saveCurrentTab();
});
