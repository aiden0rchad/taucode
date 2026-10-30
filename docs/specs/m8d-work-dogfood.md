# M8d — Work dogfood and desktop interaction fixes

Verified on macOS, October 28–29, 2026, using the signed debug app from `m1-wiring`.
The latest Work checks use GPT 6.1 Sol (Account 2), Low reasoning, with Full access.
Account 2 is the saved default, verified after relaunch. Fixtures live outside this
repository in `~/taucode-dogfood/work-notes`.

## Changes

- Work has a pinned follow-up composer with bounded width and side padding.
  Follow-ups resume the existing task/thread; running tasks accept steering.
  Drafts survive failed sends. Pending requests and concurrency limits are enforced.
- Existing Work tasks expose the shared model/reasoning picker for follow-ups.
  Selections stay with each task's draft and persist when the follow-up is accepted.
  The backend validates enabled registry models and supported reasoning levels;
  running tasks accept steering but reject model/reasoning changes mid-turn.
- Work failures use a single compact recovery card with expandable original details.
  Retry resends the failed request using the current model and keeps unsent drafts;
  failed turns without a user request cannot replay an unrelated older message.
- Plan steps render Markdown formatting with readable accessibility labels.
- Work conversations open at their latest result and follow new output while the
  reader stays near the bottom. Scrolling up pauses following; Jump to latest
  resumes it. Pending questions and approvals follow the progress they concern,
  and the input callout returns to them even when that task is already selected.
- Transcript-only events no longer rewrite the full Work task store or trigger
  metadata refreshes. Checkpoint observation and raw event forwarding still run;
  status, approvals, questions, plans and outputs persist immediately. Additional
  pending requests refresh the UI even when the task already needs input. Queue
  settings are read only when a task has capacity to start.
- Local Work tasks can be renamed from the header or sidebar menu, including
  queued tasks. Names persist without starting an engine, changing task status or
  losing composer drafts. Imported histories remain read-only. The shared rename
  dialog keeps keyboard focus inside, restores it on close, and retains failed
  edits with an inline error.
- New Work tasks honor the default access mode. Full access permits ordinary local
  shell/file work; read-only, child-agent and protected-path restrictions remain.
- Stop cancels pending Pi questions/approvals before waiting for the abort response.
- Work tool rows use concise descriptions and the initial task brief is no longer
  rendered twice. Local file links open inputs and unchanged outputs inside the
  task's space without listing them as changed outputs. Imported histories still
  open only registered copies; traversal and symlink escapes remain blocked.
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

- The signed streaming persistence build completed a read-only follow-up after
  macOS Keychain approval: Account 2/Low read the CSV and brief, produced a 40-line
  checklist, and presented its question after the progress. Yes resumed the turn,
  completed all three plan steps and returned `STREAM_PERSISTENCE_OK`. The final
  response, completed plan, Full access and model selection survived relaunch.
  All nine current fixture files remained unchanged; no output files were created.
- The rebuilt app opened Weekly action review at its final answer. Scrolling back
  kept older progress visible and exposed Jump to latest; clicking it restored the
  final answer and hid the button. Switching from older messages to Validate action
  dashboard opened its finished result instead of its historical quota error.
  The composer stayed pinned, with Account 2/Low and Full access still selected.
- Renamed two completed tasks through the header and sidebar menus. Both names
  survived relaunch; the active task and unsent draft stayed intact while renaming
  another task. Search found the new name. Escape from Cancel discarded an edit,
  Tab/Shift-Tab stayed inside the dialog, and focus returned to its menu trigger.
  The dialog covered the native browser snapshot. Only the two title fields changed
  across all six tasks; all seven fixture files remained unchanged.
- A failed Work task switched from the original Sol account to Account 2/Low and
  returned `WORK_ACCOUNT2_RECOVERY_OK` in the same conversation. Its model,
  reasoning, Full access and response survived relaunch.
- The quota-failed dashboard task recovered through the native Retry button after
  selecting Account 2/Low. It validated the existing dashboard, completed its plan,
  and removed its temporary preview server/log. All seven baseline files and an
  unsent composer draft were preserved. Error details expanded/collapsed correctly;
  the review task's plan showed bold text without literal Markdown markers.
- After rebuilding, that task's `Open action-dashboard.html` link opened the
  existing dashboard in Safari without "Unknown task output". Its Outputs count
  stayed at zero because validation did not change files. Account 2/Low and Full
  access remained selected after relaunch.
- Account 2 created `outputs/review-pack-20261029.html` and its checks document from
  the existing CSV and brief. A follow-up added Reset filter, completed the plan,
  and cleaned up its temporary loopback server. Independent Safari checks confirmed
  P1/P3 each show one item, P2 shows five, and Reset restores all seven. Source
  SHA-256 hashes remained unchanged. Opening output links and cards worked.
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
- Frontend: 225 files / 1,794 tests passed with `vitest run --maxWorkers=2`.
  The unrestricted parallel run hit three theme/settings timing failures;
  the reduced-worker full run passed without changing those tests.
- Rust: browser 34, Pi 39 (one ignored), Work 9, providers 11 tests passed.
  Work's file-opening regression covers existing inputs/outputs, persistence,
  traversal, outside absolute paths, symlink escapes, missing files and folders.
  Rename coverage checks every local status, persistence, input validation and
  rejection of imported-history edits without initializing an engine.
  Streaming coverage forwards 303 transcript events unchanged without touching the
  state file's modification time. Metadata persistence, multiple pending requests
  and checkpoint user/turn bindings are checked separately. All 10 checkpoint
  lifecycle tests passed after the persistence change.
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
identity, Xcode installation, or saved Xcode account was found. The signed-in Apple
Developer account page shows a free account and offers paid enrollment; no paid
membership is shown for that account.

[Apple permits free account/Xcode development and personal testing](https://developer.apple.com/help/account/membership/programs-overview/).
[Developer ID distribution signing requires Developer Program membership](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/),
listed at [99 USD per membership year](https://developer.apple.com/get-started/).
Free Personal Team signing can be investigated first; resolution of repeated
Keychain prompts must be verified across two rebuilt binaries before claiming it.
No account enrollment or payment was performed.

## Remaining follow-ups

- Set up and verify Apple-issued signing across rebuilds.
- Native acceptance for agent-started preview servers and their lifecycle controls.
- Native acceptance for the Work agent panel and shorter default titles below.
- Investigate an earlier process exit when evaluation attempted to create an empty
  native tab. The new no-open-page guard avoids that invalid call; the underlying
  WebKit creation failure was not conclusively diagnosed.

The temporary gateway quota override expires October 29 at 12:44:27 a.m. PDT;
automatic quota protection remains configured afterward.

## Work delegation and titles — 2026-10-30

Work now opens the shared Agents tree from the Task inspector or an expanded
delegation row. Selecting a root or nested agent reads its retained transcript in
the side panel while the task and follow-up composer stay visible. Failed reads
offer Retry transcript; changing tasks cancels stale reads and clears the selected
agent. Imported tasks retain their historical summaries without offering an
unavailable detailed transcript. Chain of Command remains Code-only.

The Work API resolves the task's own space and thread before reading a node. Reads
do not rewrite Work metadata, start an engine, or pump queued tasks. Existing
transcript credential redaction remains in effect. Polling is sequential, only
for the selected pending/running agent (2 seconds visible, 10 seconds hidden),
with one final read on completion. Unknown prices remain explicitly unavailable.
Narrow-panel transcript gutters no longer produce a horizontal scrollbar; tree
styles no longer affect Markdown lists/headings inside transcripts. Delegation
labels fall back to agent names when receiver metadata is missing.

New tasks use a compact first-line preview of the brief: eight words / 64 Unicode
characters, retaining word boundaries when practical and marking truncation with
an ellipsis. This is an extractive default, not a model-generated summary. It
adds no provider request and preserves the full brief, existing titles, and manual
renames.

Validation: 1,815 frontend tests / 226 files; 11 Work backend tests; TypeScript,
changed-file ESLint and `cargo check --all-targets` passed. Regressions cover nested
transcripts after restart, invalid/imported task requests, unchanged state-file
mtime, queue isolation, retry, non-overlapping polling, final snapshots, stale
reads, draft preservation and Unicode title limits.

Computer-use verification in the browser fixture preview opened an agent result,
resized the side panel to 320px, switched tasks and returned with the original draft
intact. Evidence: `taucode-work-agents-preview.png`. This is fixture UI evidence,
not a live model run. The signed native app is still the earlier evaluation build;
native acceptance and cloud/local benchmarks remain pending macOS access/Keychain
approval. Preview-server controls are implemented below; native acceptance remains pending.


## Managed Work previews

The parent Work agent uses `preview_server` to start, list and stop a foreground
preview command in its own space. Start shares the bash access policy: Ask shows
an approval, Full access permits ordinary commands, and read-only blocks starts.
The host rechecks Work/Terminal availability and current task access. Chat and
child agents cannot use this tool. The host resolves workspace/thread/cwd itself;
arguments cannot select another task. The preview inherits the filtered Work
environment, isolated home, bundled Python and Node PATH, without provider keys.

Local tasks expose Previews in the side panel and View previews in their Task
inspector. Controls show detected URLs, elapsed time, status and bounded logs,
with Stop, Restart and Clear finished. Opening a URL uses the browser panel;
a URL printed in output is not a readiness check. Read-only or disabled features
prevent restarting but retain cleanup. Task switching preserves composer drafts
and rejects old responses. Imported and queued tasks have no managed previews.

The app and daemon share the existing managed-task registry. Previews survive
turn completion and idle engine shutdown; Stop task stops that task's previews,
and normal host shutdown joins all managed process cleanup. On POSIX, a guard in
the owned process group also terminates it if the host crashes, including stubborn
children. Windows retains normal managed-task cleanup; crash cleanup there is not
verified. External processes are never adopted or killed by port number. History
lasts until backend restart; servers are not automatically relaunched.

Validation includes an actual loopback HTTP server through the host bridge,
secret-redacted tool results, late requests after cancellation, task isolation,
restart, engine shutdown survival, policy/feature changes, unchanged Work state,
and host-crash/process-group cleanup. Frontend: 1,817 tests / 226 files; engine
extensions: 49 tests. Rust: 57 Pi tests passed (one ignored), 12 Work tests and
9 managed-task tests passed. TypeScript, changed-file ESLint and
`cargo check --all-targets` passed. Browser-fixture
computer use exercised Stop, retained logs, Restart, Clear finished and the URL
link. At 320px, the panel has no horizontal overflow and Stop clears the status
text by 8px. Evidence: `taucode-work-previews.png`.

The native app has not been rebuilt/relaunched for this batch while macOS access
and Keychain approval remain unresolved. Signed-app acceptance and cloud/local
model benchmarks remain open; the fixture screenshot is not a live model run.
