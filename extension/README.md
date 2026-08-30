# torimemo browser extension

Saves the current page to a torimemo running on this machine. Firefox and
Chrome from one codebase.

## Why it exists

Saving has to be cheaper than the browser's own bookmark, or the archive stays
empty. `Ctrl/Cmd+Shift+U` opens the save popup on the current page; so does
clicking the toolbar button.

There is deliberately **no background page**. Firefox would not start one here
— `Inspect` opened nothing and no listener ever registered, with no error in
any console — so the shortcut uses the reserved `_execute_action` command,
which opens the popup directly and needs no background context. That also
removes the only piece that differed between the two browsers.

## Install

The extension talks to `http://127.0.0.1:7645`, so start the server first:

```sh
torimemo serve
```

**Firefox** — `about:debugging` → This Firefox → Load Temporary Add-on → pick
`manifest.json`. Temporary add-ons are removed on restart; for a permanent
install the extension has to be signed by Mozilla.

**Chrome** — rename `manifest.chrome.json` over `manifest.json` first: Chrome
requires a service worker and rejects `browser_specific_settings`, so the two
browsers need different manifests. Then `chrome://extensions` → enable
Developer mode → Load unpacked → pick this directory.

## Settings

Only needed once you issue a token:

```sh
torimemo token issue --name browser --scope read-write
```

Paste it into the extension's settings. The API runs open while no token
exists, so until then there is nothing to configure. **Test connection** in
settings reports exactly what is wrong when something is.

## What a save does

`POST /v1/tools/bookmarks.save`, the same entry an agent uses. The URL is
canonicalized and deduplicated server-side, so saving a page twice records a
second capture rather than a duplicate bookmark — and the popup says how many
times you have saved it.

## Notes

- **Local only.** The API binds loopback, so this cannot reach a torimemo on
  another machine, by design.
- **The token is stored per-browser**, not in synced storage: it is a
  credential for a service on this machine.
- Requests need CORS on the server side; that ships in the API.
