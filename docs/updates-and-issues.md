# Updates & reporting issues

taucode can tell you when a newer release is published on GitHub, and it can prepare a GitHub issue for you. Downloads start only when requested. Neither feature replaces the running app, uploads or posts anything by itself.

## Update check

taucode asks the GitHub Releases API for the latest release of [aiden0rchad/taucode](https://github.com/aiden0rchad/taucode/releases) (`GET /repos/aiden0rchad/taucode/releases/latest`) and compares its tag (for example `v0.8.0`) with the running version.

- **On startup**, at most once a day, while **Settings → About → Check for updates automatically** is on (it is on by default). If a newer version exists, a small notice shows the version, its release notes and a **Download** button. **Later** hides that version until the next one is published.
- **Settings → About → Check for updates** checks immediately and shows the result there.
- **Download update** downloads the compatible release installer inside taucode, with progress and retry on failure. macOS uses `.dmg`, Windows uses `.exe`, and Linux uses `.AppImage`. If no matching installer exists, taucode says so.
- The backend rechecks the selected version and resolves its asset from the official repository. It streams into a unique folder under Downloads, verifies the published size and [SHA-256 digest](https://docs.github.com/en/rest/releases/assets) when present, and removes partial downloads after errors. Closing About or dismissing the notice does not lose the transfer during the app session.
- **Open installer** opens the completed download using the operating system. macOS downloads retain the quarantine attribute for normal security checks. Finish installation there; taucode does not replace its running bundle.
- Automatic replacement requires signed updater artifacts and a configured updater public key; those are not configured. The existing native app-signing identity is separate from updater signing.

What you may see:

| Message | Meaning |
| --- | --- |
| *taucode X is available* | A newer release exists. |
| *You're up to date.* | The latest release is the version you run (or older). |
| *No releases published yet.* | The repository has no published release. |
| *Couldn't reach GitHub…* / *GitHub's rate limit was reached…* | The check failed. Startup checks stay silent and try again on the next launch; nothing else is affected. |

Drafts and pre-releases are not offered, because GitHub's `releases/latest` excludes them. The request carries only taucode's version in its `User-Agent`; there is no account, token or identifier. The last successful result is kept in the app's local storage so the daily throttle survives restarts; a check from an older build is ignored after you update.

## Report an issue

Open **Settings → About → Report an issue…**, or **Settings → Diagnostics → Report on GitHub**. In an issue's details in the Diagnostics inbox, **Report on GitHub** starts the report from that issue and lists it first.

The dialog says what it includes and lets you edit everything before anything leaves taucode:

- taucode version, build type and commit
- macOS version and processor architecture
- the engine and model of the conversation in front
- open issues from the [diagnostics journal](diagnostics.md): title, code, area, severity, count, last-seen date and fingerprint (up to eight rows)

It never includes API keys or other secrets, your home folder path, e-mail addresses, file contents, prompts or replies. The text is cleaned twice, when the draft is made and again when you open it on GitHub: known provider keys and key-shaped tokens become `[REDACTED]`, and `/Users/<name>` paths become `~`, using the same redaction rules as the diagnostics journal (`docs/fixtures/diagnostics-redaction.json`). If diagnostics are off, the report says the journal is empty.

**Open on GitHub** opens `https://github.com/aiden0rchad/taucode/issues/new` in your browser with the title and description filled in. The issue is only created if you click **Submit** on GitHub, signed in with your own account. If the text is too long for a link (about 8 KB), taucode shortens it and puts the full text on your clipboard so you can paste it over the shortened body.

### Screenshots

A GitHub link cannot carry images, so the dialog offers:

- **Copy screenshot**: captures taucode's window and puts the image on the clipboard. Paste it into the issue body on GitHub.
- **Save screenshot**: saves `taucode-screenshot-<date>-<time>.png` to your Downloads folder and shows it in Finder, so you can drag it into the issue.

The capture shows taucode's own window content without the Settings and report dialogs, and needs no Screen Recording permission. It can include whatever is on screen in taucode, such as conversation text and file names, so look at it before you attach it. Content in an open browser tab inside taucode is not captured.

## For developers

- Backend: `apps/taucode/src-tauri/src/github.rs` (`app_update_check`, `download::app_update_download`, `issue_report_draft`, `issue_report_open`, `issue_report_screenshot`).
- Frontend: `src/features/update/` (startup check, throttle, notice) and `src/features/report/` (dialog and screenshot capture).
- The inherited Tauri updater plugin stays unused: no updater artifacts or signing keys are configured, so taucode downloads platform installers and leaves installation to the operating system.
