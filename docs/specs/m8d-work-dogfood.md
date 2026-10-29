# M8d — Work dogfood and desktop interaction fixes

Verified on macOS, October 28, 2026, using the signed debug app from `m1-wiring`.
The dogfood agent used GPT 6.1 Sol with Full access. Fixtures live outside this
repository in `~/taucode-dogfood/work-notes`.

## Changes

- Work has a pinned follow-up composer with bounded width and side padding.
  Follow-ups resume the existing task/thread; running tasks accept steering.
  Drafts survive failed sends. Pending requests and concurrency limits are enforced.
- New Work tasks honor the default access mode. Full access permits ordinary local
  shell/file work; read-only, child-agent and protected-path restrictions remain.
- Stop cancels pending Pi questions/approvals before waiting for the abort response.
- Work tool rows use concise descriptions, output links open registered files, and
  the initial task brief is no longer rendered twice.
- Native browser placement belongs to its mounted tab. Stale bounds cannot float a
  newly created page over another panel. Bounds include the main WebKit page zoom;
  Chat and Work now install the same zoom shortcuts as Code.
- Native `browser_evaluate` advertises its actual inline-result schema. Optional
  null/empty targets work; unsupported output filenames are no longer advertised.
  Read/evaluate actions without an open page return a navigation hint.
- Sidebar dividers have 16px hit areas. Window edges/corners have larger transparent
  targets; macOS uses drag-event screen coordinates because Tauri's native resize
  drag API is unsupported there. The browser leaves the outer resize gutter free.
- Concurrent provider-key reads share one cached Keychain request. Failed reads can
  retry; set/delete operations serialize with reads.

## Live evidence

- Sol read meeting notes and feedback, wrote a weekly brief and seven action items,
  then built `outputs/action-dashboard.html` with owner/priority filters.
- After the browser schema fix, Sol used native browser evaluation to discover and
  fix a malformed option. It verified all 20 filter combinations, reset and empty
  states, keyboard focus, and widths 320/768/1280 without horizontal overflow.
- The Work task resumed in place and completed its plan. No ordinary tool approval
  was required under Full access.
- Stop resolved an unanswered `ask_user` request in under one second; the same task
  accepted another follow-up afterward.
- Native page text remained crisp, clicking back into the composer worked, and
  Settings covered the browser with a snapshot. Browser bounds stayed aligned at
  80%, 100%, and 120% app zoom. Glass Dark remained selected.
- Both sidebar dividers, left/right window edges, and the enlarged bottom-right
  corner were exercised with the browser open. The right edge also worked without
  the browser. An 80px drag at 80% zoom changed the window width by 80px. Final
  state: 100% zoom, a 1260×850-point window, and the completed task/composer visible.

## Verification

- TypeScript typecheck and `cargo check`: passed.
- Frontend: 220 files / 1,720 tests passed with `vitest run --maxWorkers=2`.
  The unrestricted parallel run hit three theme/settings timing failures;
  the reduced-worker full run passed without changing those tests.
- Rust: browser 34, Pi 39 (one ignored), Work 6, providers 11 tests passed.
- Pi runtime/policy: 13 tests passed.
- Changed-file ESLint: no errors; the earlier broad run reported one existing
  effect-cleanup warning in `useSidePanelState.ts`.
- Tauri debug app build and local `taucode Dev` signature verification passed;
  the resulting app was relaunched and the final resize checks used that binary.

## Keychain limitation and account check

macOS `securityd` explicitly reported **ACL partition mismatch**: the local
self-signed app is trusted by its code hash, which changes on each rebuild. A
stable designated requirement alone does not fix this partition check. This patch
does not alter Keychain ACLs or bypass macOS protection.

Only the local `taucode Dev` certificate was installed; no Apple-issued signing
identity, Xcode installation, or saved Xcode account was found. That does not
establish whether the user has an online Apple Developer account.

[Apple permits free account/Xcode development and personal testing](https://developer.apple.com/help/account/membership/programs-overview/).
[Developer ID distribution signing requires Developer Program membership](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/),
listed at [99 USD per membership year](https://developer.apple.com/get-started/).
Free Personal Team signing can be investigated first; resolution of repeated
Keychain prompts must be verified across two rebuilt binaries before claiming it.
No account enrollment or payment was performed.

## Remaining follow-ups

- Set up and verify Apple-issued signing across rebuilds.
- Give agent-started preview servers visible lifecycle controls and cleanup.
- Replace long prompt-derived task titles and raw provider error payloads with
  concise, actionable UI.
- Investigate an earlier process exit when evaluation attempted to create an empty
  native tab. The new no-open-page guard avoids that invalid call; the underlying
  WebKit creation failure was not conclusively diagnosed.

The temporary gateway quota override expires October 29 at 12:44:27 a.m. PDT;
automatic quota protection remains configured afterward.
