// The popup: save with a note, and say up front if the page is already here.
//
// Checking before saving matters because a repeat capture is meaningful data
// rather than a mistake — the count is the strongest signal the archive has
// about what actually mattered.

import { api, savePage, lookUp, status } from "./torimemo.js";

const statusEl = document.getElementById("status");
const titleEl = document.getElementById("title");
const note = document.getElementById("note");
const save = document.getElementById("save");

const show = (text, state) => status(statusEl, text, state);

const tabPromise = api.tabs
  .query({ active: true, currentWindow: true })
  .then(([tab]) => tab);

async function init() {
  const tab = await tabPromise;
  titleEl.textContent = tab?.title ?? "";

  if (!tab?.url || !/^https?:/.test(tab.url)) {
    show("Only http(s) pages can be saved.", "error");
    save.disabled = true;
    return;
  }

  const existing = await lookUp(tab.url);
  if (existing) {
    show(`Already saved ${existing.saved_times}×`, "known");
    save.textContent = "Save again";
  } else {
    show("Not saved yet");
  }
  note.focus();
}

async function submit() {
  const tab = await tabPromise;
  save.disabled = true;
  show("Saving…");
  try {
    // Fall back to the page title so a note-less save is still searchable.
    const result = await savePage({
      url: tab.url,
      note: note.value.trim() || tab.title,
    });
    show(
      result.created ? "Saved" : `Saved — ${result.saved_times}× total`,
      "saved",
    );
    setTimeout(() => window.close(), 700);
  } catch (error) {
    show(error.message, "error");
    save.disabled = false;
  }
}

save.addEventListener("click", submit);

// Enter saves. The popup exists to make capture cost as little as possible,
// and requiring a modifier put a second keystroke in front of every save.
//
// Shift+Enter still inserts a newline, so a multi-line note is possible for
// anyone who wants one — it is just no longer the default cost.
note.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    submit();
  }
});

document.getElementById("options").addEventListener("click", (event) => {
  event.preventDefault();
  api.runtime.openOptionsPage();
});

init();
