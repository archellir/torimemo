// Talking to the local torimemo API.
//
// Chrome exposes `chrome`, Firefox exposes both `browser` and `chrome`, so
// preferring `browser` and falling back keeps one codebase for both.
const api = globalThis.browser ?? globalThis.chrome;

const DEFAULTS = {
  // The API binds loopback, so the extension and the server must be on the
  // same machine. That is the design: nothing leaves the device.
  endpoint: "http://127.0.0.1:7645",
  // Empty until the user issues one with `torimemo token issue`. The server
  // runs open while no token exists, so an empty value is valid, not broken.
  token: "",
};

async function settings() {
  const stored = await api.storage.local.get(DEFAULTS);
  return { ...DEFAULTS, ...stored };
}

/// Calls one entry in the /v1/tools registry.
async function invoke(name, input) {
  const { endpoint, token } = await settings();
  const headers = { "Content-Type": "application/json" };
  if (token) headers.Authorization = `Bearer ${token}`;

  let response;
  try {
    response = await fetch(`${endpoint}/v1/tools/${name}`, {
      method: "POST",
      headers,
      body: JSON.stringify({ input }),
    });
  } catch (cause) {
    // A refused connection is the common case and has a specific fix, so it
    // is worth distinguishing from a server-side failure.
    throw new Error(`torimemo is not running at ${endpoint}`, { cause });
  }

  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    if (response.status === 401) throw new Error("token rejected — check the extension options");
    if (response.status === 403) throw new Error("this token is read-only");
    throw new Error(payload.detail ?? `save failed (${response.status})`);
  }
  return payload;
}

/// Saves one page, returning what the archive did with it.
export async function savePage({ url, note }) {
  const input = { url };
  if (note) input.note = note;
  return invoke("bookmarks.save", input);
}

/// Reports whether the configured endpoint and token actually work.
///
/// Lives here rather than in the options page so the failure cases are
/// described once. The options page previously repeated this fetch with its
/// own status handling, which meant a 401 could be worded two different ways
/// depending on which screen you were looking at.
///
/// Returns a message and whether it is good news, rather than throwing: the
/// caller is a diagnostic button, so a failure is the expected answer half
/// the time.
export async function checkConnection() {
  const { endpoint, token } = await settings();
  const headers = token ? { Authorization: `Bearer ${token}` } : {};

  try {
    const response = await fetch(`${endpoint}/v1/tools`, { headers });
    if (response.status === 401) {
      return { ok: false, message: "Reachable, but the token was rejected." };
    }
    if (!response.ok) {
      return { ok: false, message: `Reachable, but returned ${response.status}.` };
    }
    const { tools } = await response.json();
    return { ok: true, message: `Connected — ${tools.length} tools available.` };
  } catch {
    return { ok: false, message: `No response from ${endpoint}. Is torimemo serve running?` };
  }
}

/// Looks a page up without saving, so the UI can say "already saved" before
/// the user commits to anything.
export async function lookUp(url) {
  try {
    return await invoke("bookmarks.get", { url });
  } catch {
    return null;
  }
}

/// Writes a one-line status into an element, with a state the stylesheet
/// colours. Shared because both screens report the same kinds of outcome and
/// had identical copies of this.
export function status(element, text, state) {
  element.textContent = text;
  if (state) element.dataset.state = state;
  else delete element.dataset.state;
}

export { api, settings, DEFAULTS };
