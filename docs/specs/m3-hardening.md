# Hardening: local control sockets, upstream updater, upstream branding

## Goal

Fix three issues found in review before the app is used for real computer use. Done = the
verification command passes.

## 1. Local control sockets (computer use and browser)

The computer-use control socket (`src-tauri/src/computer/transport.rs`, currently
`/tmp/taucode-cua-<uid>-<hash>.sock`) and the browser host socket (`src-tauri/src/browser/`,
`paths.rs`) accept commands that drive the user's mouse, keyboard and browser. Today:

- the socket is created, *then* chmod'ed to 0600, which leaves a race window
- connections are never checked for the peer's uid
- clients don't verify who owns the socket they connect to

Required, for **both** sockets and every client of them (the `--tau-computer-mcp` and
`--tau-browser-mcp` launchers, the app, the daemon):

- Bind inside a **private directory** owned by the current euid with mode **0700**:
  - prefer `<app data>/run/` when the full socket path fits the platform limit; otherwise
    `/tmp/taucode-<euid>/`
  - create it with 0700 (set the umask around the mkdir/bind so the socket is never briefly
    world-accessible)
  - if the directory exists, verify it's a real directory (not a symlink), owned by euid,
    with mode exactly 0700; otherwise refuse with a clear error
  - keep the existing short-path fallback logic and the published-path file
- **Server:** on every accepted connection, check the peer credentials (`peer_cred()` on
  tokio's `UnixStream`, or `getpeereid`). Reject and close any peer whose uid ≠ euid, before
  reading any bytes, and log it once.
- **Client:** before connecting, `lstat` the socket path. It must be a socket owned by euid,
  inside a directory that passes the same checks. After connecting, verify the peer uid too.
  Refuse otherwise.
- Windows named pipes: keep current behaviour, plus reject remote clients and set an ACL
  limited to the current user if the existing code doesn't already.
- Tests (no real sockets needed where the sandbox forbids binding; put the checks behind
  small functions): the directory validation matrix (missing, wrong owner, wrong mode, symlink,
  ok), the peer-uid decision, the client ownership check, and the path choice with long
  paths.

## 2. Upstream auto-updater

`src-tauri/tauri.conf.json` still has CodexMonitor's updater: its endpoint
`https://github.com/Dimillian/CodexMonitor/releases/latest/download/latest.json` and its
minisign public key. A taucode build would offer to replace itself with a CodexMonitor
release.

- Remove the updater `pubkey` and `endpoints`, set `createUpdaterArtifacts: false`, and make
  the updater a no-op until taucode has its own release channel. Keep the plugin dependency
  if removing it is invasive, but it must never fetch anything.
- Hide or remove the update UI (update toasts, "Check for updates", post-update release-notes
  fetch in `src/features/update/`). Remove any code that fetches CodexMonitor GitHub releases
  or notes.
- Tests: updater disabled (no endpoint configured, no network call path); update UI absent.

## 3. Upstream branding

Search the frontend, the Rust code (menus, window titles, notifications, tray, dialogs)
and `Info.plist`/config for user-visible "CodexMonitor" / "Codex Monitor" / Dimillian links.
Replace them with taucode, except:

- an attribution line in About: "Based on CodexMonitor by Dimillian (MIT)", with a link
- the LICENSE/README attribution
- internal identifiers and crate/binary names (leave as they are)

## Constraints

Offline sandbox; no new dependencies; always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. Don't touch
protocol/fixtures/crates/docs/agent configs. No accounts or logins.

## Verification

Same command as `m3-computer-use.md`.
