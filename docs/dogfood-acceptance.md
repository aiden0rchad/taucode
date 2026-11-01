# Dogfood acceptance — October 30, 2026

Branch: `m1-wiring`. Signed macOS debug app, version 0.7.68. This is the current
summary of the eight-item plan; dated sections in the specs are historical evidence.
The user authorized merging this batch into `main` on October 30, 2026.
Merge integration preserved the existing Windows packaging changes and passed
1,830 frontend tests, TypeScript, the production frontend build, five packaging
tests and `cargo check --all-targets --features custom-protocol` on macOS.

| Plan item | Verified outcome | Remaining limits |
| --- | --- | --- |
| Crash recovery | Pi/OMP checkpoints retain interrupted root output and delegation transcripts; engine/parent death stops owned POSIX commands. Signed native force-quit, engine-kill and follow-up checks passed. | Last second of unflushed output may be lost. Other engines, Windows cleanup and power-loss durability are unverified. |
| Chain of Command | Real reviewer evidence gates success. Shared concurrency, inherited deadlines and project daily usage limits passed automated and signed native checks. | Enforcement uses reported usage; in-flight requests can overshoot a token allowance. |
| Cloud/local measurement | Sol Account 2 and self-hosted Halogen passed six independent bugfix checks. Mixed-team implementation and independent review also passed. | Pricing remains **unknown**, not zero. One sequential baseline does not justify automatic routing or general quality claims. |
| Conversation teams | Overrides, project isolation, fork/side-chat copying and restart persistence passed. CoC stays in Code. | Native coverage is Pi-RPC/OMP on this Mac. |
| Work polish | Interactive agent transcripts, concise titles and preview Open/Stop/Restart controls passed live checks; stopping preserved the follow-up draft and closed owned ports. | Historical Work CoC records remain readable; new Work turns do not enable CoC. |
| Browser stability | Failure-state cleanup and Retry have regression coverage. Native blank-page navigation, close/reopen, zoom, focus and ten repeated create/close cycles passed. The follow-up fixes evicted tabs stuck loading; twelve-tab eviction and interactive recovery passed in the signed app. | Earlier empty-tab process exit remains unreproduced. Actual native creation failure/Retry has not been fault-injected. |
| Signing / Keychain | Free Xcode Personal Team signing plus the user's Always Allow approval added Apple-team trust to the provider key. Fresh-process provider requests passed both on relaunch and after rebuilding to a different code hash. | Verified on this Mac and provider key. Certificate renewal, another Mac and release notarization are outside this check. |
| Final acceptance | Code recovery/team tasks, Chat checks, Work artifacts/previews, browser interactions, settings keyboard flows and responsive Evals checks have passed within the recorded scope. The user approved the batch; local merge `b6848c7` retained the main branch's Windows packaging changes. | No claim of a full VoiceOver audit, cross-platform acceptance or release notarization. |

## Recovery-message acceptance

The interrupted Pi/OMP engine now shows **Agent stopped unexpectedly**, explains
how to continue with a follow-up and states that unfinished commands do not restart
automatically. It retains Error details but offers no blind Retry/Change model.
Completed or stopped elapsed-time labels say **Worked for**, without implying success.

119 focused frontend tests, TypeScript and changed-file ESLint passed. The signed
build passed strict signature verification. In the native app, GPT 6.1 Sol
(Account 2), Low, Full access ran one bounded timer. Killing its observed OMP
engine retained `RECOVERY_UI_READY 46662`, displayed the new card, and removed the
engine, supervisor and worker. A tool-free follow-up returned `RECOVERY_UI_OK`.
The earlier test cancelled during Keychain startup remained stopped after approval.
Screenshot: `taucode-recovery-message-live.png` in the task's visualization folder.

## Browser acceptance

After the page title settled, ten new blank tabs were created and closed using
distinct tab labels. Each cycle preserved the original page; its native content
was visible afterward. No process exit occurred. An initial attempt using two
identical “New tab” labels could not establish which tab was closed and is excluded.
The only retained `codex-monitor` crash report is October 26's already-fixed
AppKit/KVO issue; it does not establish the cause of the later reported exit.

### Follow-up: evicted tabs stuck loading

After the merge, a twelve-tab native test reproduced a different failure. The
browser keeps eight live webviews and evicts older hidden views. Returning to an
evicted tab showed its snapshot and **Loading page…** indefinitely. Explicit Reload
recovered it. The app stayed running; this does not establish the cause of the
earlier empty-tab process exit.

`NativeSlot` withheld visible placement until `ready`, while the native backend
needed visible placement to recreate the evicted view. Placement now describes
the unobstructed slot independently of readiness. The backend still waits for the
page to be ready before showing it; the frontend retains the loading snapshot.
Blocked, background and overlay-obscured slots still send hidden placement.

The new loading-tab regression failed before the fix. All 59 browser tests,
TypeScript and changed-file ESLint passed afterward. The debug app rebuilt and
passed strict deep signature verification with the saved Apple identity.

In that signed build, twelve localhost fixture pages were opened. The native-view
count stabilized at eight (plus the frontend's placeholder). Revisiting tabs
1, 2, 3, 4, 12 and 1 automatically restored interactive content without Reload;
each accepted text input and cleared the loading message. Opening Settings hid
the native content behind its snapshot; closing Settings restored the page with
its input retained. All twelve fixture tabs were closed and their server stopped.
Screenshots: `taucode-evicted-tab-before.png` and `taucode-evicted-tab-after.png`
in the task's visualization folder.

The plain fixture also exposed a separate contrast issue: default black page text
appeared over the dark browser background. See the following appearance check;
this is not evidence of a loading or process failure.

### Follow-up: browser appearance and tab visibility

The installed Wry 0.54.2 implementation disables WebKit's default page background
whenever an explicit background color is supplied on macOS. Its color setter only
sets the under-page/overscroll color; it does not supply the missing document
background. Removing the browser builder's explicit white override leaves WebKit
responsible for normal page rendering, including site-defined color schemes.

A reusable native rendering fixture covers an unstyled page, an explicit dark
site theme, system color-scheme support, editable text and button interaction.
From the repository root, serve it with:

```sh
python3 -m http.server 60423 --bind 127.0.0.1 --directory apps/taucode/src-tauri/tests/fixtures
```

Open `http://127.0.0.1:60423/browser-appearance.html` in taucode's native browser.
Check the three page-theme links, Back/Forward, Reload, zoom and Settings, then
return after opening eight other tabs. The plain page must be black on white;
the dark page must preserve its own green background and pale text. This is a
manual native-rendering regression: a DOM-only color check would miss the
transparent WebKit canvas that caused this bug.

The same session found active tabs disappearing outside the horizontally scrolled
tab strip. Selection, opening the panel, conversation changes and page-title updates
now reveal the active tab together with its close button. A regression failed on
the old behavior. All 82 browser/panel tests, TypeScript and changed-file ESLint
passed; 36 Rust browser tests passed. The final panel changes also passed their
23-test suite, TypeScript and lint. The rebuilt app passed strict deep signature
verification using the saved Apple Development identity.

Live verification reproduced black-on-dark text before the fix and confirmed
black-on-white after it. The explicit green/pale site theme and the system dark
color scheme rendered correctly. Back/Forward and Reload worked across those
pages. Settings hid the native page behind its snapshot and restored the typed
input. Zooming out and in kept the page aligned, then Actual Size restored 100%.
Nine named fixture tabs exercised eviction: returning to the first page restored
its white canvas and working input without Reload. Its selected tab and close
button remained visible in the overflowing strip.

After using that native input, returning to the Chat composer worked; GPT 6.1 Sol
(Account 2), Low, returned `BROWSER_POLISH_OK` in the rebuilt app. All nine fixture
tabs were closed and their localhost server stopped. Screenshots in the task's
visualization folder: `taucode-browser-contrast-before.png`,
`taucode-browser-contrast-after.png`, `taucode-browser-site-dark.png` and
`taucode-browser-polish-sol-check.png`.

Native page-process termination now records `browser.native.process_terminated`
in diagnostics with fixed text only, without URLs or page content. It retains the
existing Reload recovery message; this is instrumentation, not a claimed fix for
the earlier unreproduced exit.

## Signing and Keychain acceptance

The prior self-signed build stopped in `SecKeychainFindGenericPassword`.
Read-only inspection of the existing taucode provider item's **access metadata**
(without retrieving its secret) found six partition entries, all `cdhash:` values.
Its trusted-app entry points to this app bundle, while its partition list recognizes
individual binaries. The self-signed signature's designated requirement was stable:
bundle identifier plus that certificate's fingerprint. A stable designated
requirement does not satisfy a partition list containing only older build hashes.

[Apple DTS confirms](https://developer.apple.com/forums/thread/845980) that trusted-app
ACLs and partition lists impose independent conditions. Matching designated
requirements alone was insufficient evidence of durable access. The item metadata
and macOS trust settings were not modified.

The user authorized free Xcode signing setup. The Mac App Store installed Xcode
26.6 at `/Applications/Xcode.app`; its minimum macOS version is 26.2, compatible
with this Mac's 26.5.1. The user accepted the agreement and signed into Xcode; their
account has a Personal Team. With the user's explicit approval, assigning that team
to `/private/tmp/TaucodeSigningCheck` created a valid Apple Development identity.
The certificate expires October 30, 2027. The development signer reads its identity
from the Git-ignored `apps/taucode/.signing-identity.local`; an explicit environment
override takes precedence. Empty local preferences fail instead of silently falling
back. Four packaging-policy tests passed, including literal config handling.

| Apple-signed binary | CDHash | Fresh-process provider response |
| --- | --- | --- |
| Recovery-copy baseline | `c1a3fc03f263fe851ba04235a28509a0451e019c` | `APPLE_SIGNING_BASELINE_OK` |
| Chat focus / Evals layer changes | `f06a1b0f1ff9a9b2dc950735d01381f0668211b5` | `APPLE_SIGNING_CHANGED_BUILD_OK` |

Both used the same Apple Development identity and passed
`codesign --verify --deep --strict`. The changed binary was compiled, bundled and
signed using the saved local preference. GPT 6.1 Sol (Account 2), Low, completed
both checks. The second request ran in a new app process, so the previous process's
in-memory credential cache could not satisfy it. No new approval was requested
through this task. The user replied **I didn’t notice** when asked whether either
launch displayed a prompt. This is not confirmation of prompt-free access.

A further fresh launch of the changed binary (PID 60729) returned
`APPLE_SIGNING_FRESH_LAUNCH_OK` using GPT 6.1 Sol (Account 2), Low. The old app had
exited completely. A one-second process sample captured the real
`keyring::Entry::get_password` / `SecKeychainFindGenericPassword` path; the request
then completed. The user confirmed that a prompt appeared and they chose **Allow**.
That approves one read; it is not persistent access. No additional binary change
was made for this relaunch.
Screenshot: `taucode-apple-signing-fresh-launch.png` in the task's visualization folder.

The original metadata probe still reported six build-hash partitions after the
baseline request; it does not independently prove trust for the new identity.
On the next fresh launch (PID 61010), the user confirmed choosing **Always Allow**;
the request returned `APPLE_SIGNING_SAVED_ACCESS_OK`. Read-only inspection then
found seven partitions: the original six build hashes and a new `teamid:` entry
matching the Apple-signed app. macOS saved this through the user's approval; no
script rewrote the item's access rules or retrieved its secret.

After quitting that process, PID 77732 returned `APPLE_SIGNING_RELAUNCH_OK` using
GPT 6.1 Sol (Account 2), Low. The user was asked to leave any new prompt untouched
during verification. A rebuild from the same source with a different Rust compiler
metadata value changed the executable bytes and code hash without adding product
code solely for the test:

```sh
cargo rustc --features custom-protocol --bin codex-monitor -- -C metadata=keychain-acceptance-20261030
```

The normal bundler and saved development signing identity produced a new app;
`codesign --verify --deep --strict` passed. During signing, the user separately
approved **Always Allow** for Apple's `codesign` tool to use the Apple Development
private key. This is distinct from taucode's provider-key access.

| Check after saved approval | App PID | CDHash | Provider response |
| --- | --- | --- | --- |
| Relaunch | 77732 | `f06a1b0f1ff9a9b2dc950735d01381f0668211b5` | `APPLE_SIGNING_RELAUNCH_OK` |
| Rebuilt executable | 18916 | `0aea05f5efe4587b2e943ecbf4f3ac2a55c424a7` | `APPLE_SIGNING_REBUILT_TRUST_OK` |

Both requests completed without another provider-key approval step in the session.
The partition list still contained the same seven entries after the rebuilt app's
request; its new code hash was absent while the matching Apple-team entry remained.
Together, these checks verify saved access across this relaunch and changed build.
They do not guarantee access after certificate/team changes or on another Mac.
Screenshot: `taucode-keychain-rebuilt-trust.png` in the task's visualization folder.

[Free Personal Team development](https://developer.apple.com/help/account/basics/about-your-developer-account)
does not replace the separate Developer ID distribution/notarization workflow in
[packaging](packaging.md). Renew the configured identity before its expiry.

## Additional Chat acceptance

GPT 6.1 Sol (Account 2), Low, correctly converted supplied fictional meeting notes
into three action items with the specified owners and dates, without tools. The
model picker retained Low, omitted Chain of Command, and returned keyboard focus
to its trigger on Escape. Edit-and-resend created a separately titled conversation
with the requested three-row table; reopening the original retained its prompt and
answer. Edit and Cancel now focus the composer; regression, WebKit and signed-native
keyboard checks passed. Cancel left the conversation unchanged.

The full frontend suite found an Evals header using an ad hoc layer calculation.
It now uses the shared panel token above the sticky table columns. The WebKit Evals
layout fixture passed at 1280px with opaque headers at layer 20. All 1,830 frontend
tests, TypeScript and changed-file ESLint passed. The current Apple-signed native
app includes these changes. Its Evals header stayed opaque above the results while
scrolling; saved results still show six passing tests and unknown provider cost.

## Detailed evidence

- [Chain of Command, recovery, limits, baseline and mixed team](specs/m9-chain-of-command.md)
- [Work artifacts, agent navigation and preview lifecycle](specs/m8d-work-dogfood.md)
- [Evals setup, results and retained-run behavior](evals.md)

The Evals redesign passed 1,825 frontend tests plus its native/layout checks before
this small recovery-copy change. The focused 119-test run covers the latter; these
counts are separate runs, not an additive total.

## Progress and recovery follow-up — October 30

Work now distinguishes Starting, Working, Waiting for you, Stopping, Stopped,
Failed and Completed beside the composer and in the header. Failed/stopped tasks
no longer use the success color. Review request scrolls to and focuses the current
task's pending request; it does not cycle to another task or answer automatically.
Stopped/failed tasks offer a draft-preserving follow-up action. A task stopped
before thread creation can reuse its brief and attachments without starting it.

Chat records terminal turn state from the saved transcript and scopes provider
errors to their conversation. Code restores the last turn on resume, labels stopped
and failed sidebar entries, and shows recovery guidance without automatic replay.
A pending Code question/approval suppresses the working indicator.

Validation: 237 distinct targeted frontend tests passed (235 in the broad focused
run, then the 52-test thread integration file including two added hydration tests),
TypeScript, changed-file ESLint, signed rebuild and strict codesign verification.
These tests cover failures and background-error isolation; live provider failures
were not deliberately induced in this pass.

Native checks used GPT 6.1 Sol (Account 2), Low:
- Work, Full access: ask-user displayed Waiting for you; Review request focused the
  current question. Answering Blue transitioned through Working to Completed.
  Stop removed a second pending question. The task stayed stopped after relaunch,
  then a follow-up returned WORK_PROGRESS_RECOVERY_OK in the same task.
- Code, Full access: a 30-second Node timer emitted CODE_STATUS_READY. Stop removed
  the timer process and displayed Stopped in the sidebar and conversation. Reopening
  after relaunch retained that state. A tool-free follow-up returned
  CODE_PROGRESS_RECOVERY_OK and cleared the stopped notice.
- Chat: Stop interrupted a streamed list. New chat showed no stopped notice;
  reopening the original restored it. A follow-up returned CHAT_PROGRESS_RECOVERY_OK.

Screenshots in the task visualization folder: taucode-work-waiting.png,
taucode-work-stopped.png, taucode-code-stopped.png, taucode-chat-stopped.png.

Follow-up findings: Code's current OMP tool set did not expose an ask-user tool
(the agent answered with a plain question); the Work tool worked. Code conversation titles and navigation have additional coverage below. Native
verification of the navigation batch and the new small-project dogfood run remain
pending.


## Conversation and browser navigation acceptance

Code fallback titles use the same first-line/first-sentence shortening when a turn
starts, when history is listed, and when the conversation reopens. Custom names
are preserved. Work groups tasks into Needs attention, Working, Queued, Stopped
and Completed. Search matches all entered terms across titles, briefs, output
paths and space names; matching spaces expand temporarily and recover their saved
collapse state when search clears. Chat separates Working and Yesterday and
searches titles, previews, model and provider names. Both modes expose an explicit
no-results message with Clear search.

Side panels and the Code dock have an accessible native full-tab selector with
numbered titles and URLs, including duplicate titles. It and the new-tab control
stay outside the scrolling tab strip. Browser tabs show loading and error icons;
accessible descriptions and the full-tab list include the status. These transient
states are not saved in the panel layout. Loading motion respects reduced motion.

Restoration retains saved URLs and titles while replacement pages are blank, opens
only the selected page, and keeps tab order. Opening operations are serialized;
changing selection during a restore applies the latest selection after the current
operation finishes. A regression reproduced overlapping page creation before the
fix and passes with serialization. Workspace history also distinguishes waiting,
stopped and failed turns, including reviews whose terminal turn has arrived.

Validation:
- All 1,859 frontend tests passed, including restoration, rapid selection,
  title hydration, grouping/search and terminal-state regressions.
- TypeScript passed. Changed-file ESLint had no errors; two existing effect-cleanup
  ref warnings remain in the panel hooks/components.
- A final heading-specificity correction passed the 25-test UI/style suite.
- The final debug app rebuilt, was Apple Development-signed, and passed strict
  codesign verification (`/tmp/taucode-navigation-release-build.log`).
- Browser-based UI preview: Work search found output filenames inside a collapsed
  space; Clear search restored the collapse state. Status groups were visually
  inspected at 1280 × 720 with 11px labels and corrected spacing.
- Browser-based UI preview: Code opened nine tabs, kept the full-tab and new-tab
  controls reachable, and selected the original page from the numbered list.
  Chat combined provider/topic search and recovered from no results.

Preview screenshots in the task visualization folder:
`taucode-work-grouping-preview.png`, `taucode-tab-list-preview.png`, and
`taucode-chat-grouping-preview.png`. These use fixture data, not live provider runs.

Native navigation checks: Chat combined Account 2 and cancellation search terms,
showed a useful no-results state, and cleared it. Work found
`action-dashboard.html`, including the renamed Validate action dashboard task,
inside a collapsed space and restored that collapse state when search closed.
Code displayed and retained “Code progress check” when reopening its long prompt.
Screenshot: `taucode-work-output-search-live.png`.

The native multi-tab check exposed delayed subscription delivery: after `new`
returned, the selection effect ran before the tab received its backend target and
created a second native page. The panel then bound to a blank page while the visible
page navigated, leaving stale titles/loading indicators. Commands now retain their
requested selection and keep opening controls busy until the pending target is
bound. A delayed creation notification also respects a newer user selection.
Both regressions failed before the fix; all 1,861 frontend tests, TypeScript and
changed-file lint (one existing cleanup-ref warning) now pass. The debug app
rebuilt, was signed with Apple Development, and passed strict codesign verification
(`/tmp/taucode-native-handoff-build.log`). The temporary fixture server is stopped.

Native verification after rebuilding passed: nine new pages retained their correct
URLs and titles without extra blank tabs. The numbered overflow menu selected the
second of two pages named Preview. A delayed page showed Loading and Stop loading,
then cleared both; an unavailable localhost port showed the connection error and
Retry. Renaming the conversation to Code navigation verification and restarting
preserved its custom name, all nine saved tabs in order, and the selected /two page.
Evidence: `taucode-native-tabs-restored.png`. All owned fixture tabs were then
closed, and the temporary server stopped.

The failure check exposed contradictory Loading page text behind the error panel.
NativeSlot now suppresses loading guidance while blocked by an error or overlay.
A regression covers loading → failure → retry → loading; it failed before the fix.

The isolated Focus Board build is now running inside taucode with GPT 6.1 Sol
(Account 2), Low, Full access, in `/Users/rolandleyco/taucode-dogfood-focus-board`.
The agent has begun creating files through real tools. Remaining acceptance:
inspect the completed project, preview interactions, cancellation/follow-up and
owned-server cleanup; rebuild/sign and verify the loading-error polish live.

Loading/error polish validation: all 1,862 frontend tests (226 files), TypeScript,
changed-file ESLint and diff whitespace checks passed. Logs:
`/tmp/taucode-loading-error-tests.log`, `/tmp/taucode-loading-error-typecheck.log`,
`/tmp/taucode-loading-error-lint.log`. Live build verification remains pending.

The real Code run exposed an opaque OMP native `eval` approval. Native eval/debug/
LSP approvals now include their actual JSON arguments in the command preview;
existing approval decisions remain unchanged. The new regression failed before the
fix; all 50 Pi extension tests pass (`/tmp/taucode-native-approval-tests.log`).
The initial project turn was stopped at that approval, retained all six generated
files, and continued successfully in the same conversation. Its six Node tests
passed. Preview-service argument errors required a targeted follow-up; this is an
agent/tool-schema usability finding. The native browser then reproduced a stale
delete-dialog returnValue bug in the generated app and the agent fixed it.

Focus Board completed inside taucode with Sol Account 2 / Low / Full access. The
agent reported six passing Node tests and native browser checks for create/edit/
move/filter/persistence, delete-confirm-then-Escape, keyboard shortcuts and 390px
layout. Independent native clicks added Ship the browser polish, moved it to Done,
filtered by its notes, and verified it survived reload. Screenshot:
`taucode-focus-board-live.png`. The output is seven uncommitted files in the
isolated project, including restart instructions in README.md.

Follow-up priority: OMP's bash service (`focus-board-preview`, initially PID 69586)
was ready and serving 127.0.0.1:4173, while taucode's Session rail reported Servers
(0) and Background tasks (0). Bridge OMP-owned service lifecycle and URLs into the
shared task UI. Native tools' eval approval remains intentional, but its preview
now includes arguments rather than an opaque tool name. Routine file/shell/browser
operations otherwise ran under Full access. No Keychain prompt was observed in
this native pass.

Cleanup follow-up completed: the agent stopped its `focus-board-preview` service,
reported exit 143, and independent `ps`/`lsof` checks confirmed PID 69586 and the
4173 listener were gone. Successful service startup used a write to `xd://bash`
with command `node server.mjs`, the project cwd, pty=true,
name=`focus-board-preview`, ready={log:"Focus Board preview:",port:4173,
host:"127.0.0.1",timeout:30}; top-level async/timeout were omitted. Compare OMP's
model-facing schema/null defaults with service validation in the next integration
pass, because repeated direct bash calls failed despite the follow-up guidance.

Final build: `/tmp/taucode-final-polish-build.log` completed successfully, including
Apple Development signing and strict codesign verification. TypeScript passed
again after the native approval change. The rebuilt native page showed the
connection error and Retry without stale Loading page guidance. The live eval
approval displayed language, exact console.log code, title, timeout and reset
arguments before approval. Evidence: `taucode-error-recovery-live.png` and
`taucode-native-approval-preview-live.png`.

The final recovery check caught a second issue: Retry cleared an initial connection
failure but WKWebView.reload() had no committed page to reload, leaving readiness
false indefinitely. Explicit navigation to the same URL immediately recovered.
BrowserPane now navigates to the stored URL when Retry or toolbar Reload targets a
failed/unready native page; healthy pages retain normal reload behavior. The
regression failed before this change and covers Retry, toolbar Reload and successful
recovery. The eval check returned APPROVAL_PREVIEW_OK after approval.

Retry correction validation: all 1,862 frontend tests, TypeScript and changed-file
ESLint pass (`/tmp/taucode-retry-final-*.log`). Blank-tab creation retries retain
normal reload behavior. Quitting taucode also stopped its nonpersistent OMP preview
service (PIDs 78005/78010 gone), confirming app-exit ownership cleanup.

Final native acceptance passed on the corrected signed build
(`/tmp/taucode-retry-build.log`): with the preview initially stopped, the restored
page showed a connection error. After the agent restarted its owned service,
clicking Retry loaded Focus Board immediately, restored its title, removed the
error/loading guidance, and retained Ship the browser polish in Done. Final
screenshot: `taucode-focus-board-final.png`. The preview is left running at
127.0.0.1:4173 under OMP ownership (service PID reported 85126) for inspection.
All four planned areas have implementation and acceptance evidence above. The
next separate improvement is OMP service visibility/schema ergonomics; no merge
or push has been performed for this batch.

## Computer-use resume reliability — October 30

Pause during capture, planning or approval could leave a pre-pause action queued.
A regression reproduced an obsolete click being delivered after Resume. The
runner now checks the observation's pause epoch after waits and before input,
captures again and re-plans within the existing stale-retry limits. It tells the
planner that the discarded action was not executed. Invalidated captures do not
consume a provider request; partial input is not replayed automatically.

Validation: 108 Rust computer-use tests and 131 Python sidecar tests passed;
TypeScript and `cargo check` passed. The regression covers capture, planning,
ordinary approval and foreground-fallback approval with no stale input or
foreground takeover. Signed debug bundle rebuilt and verified with strict
codesign validation (`/tmp/taucode-computer-build.log`).

The live settings inspection found internal Computer Use disabled and unconfigured.
With explicit user approval, it is now configured for Calculator only, background
mode, accessibility-element grounding, and GPT 6.1 Sol (Account 2) for both models.
Foreground fallback remains off. These choices and runtime readiness survived a
restart. Both macOS permission lists initially showed an enabled taucode entry,
but the current signed build's native checks returned false. Toggling the entries
and adding the exact path without removing the old entries did not fix it.
After user authentication, removing each stale taucode entry and adding the exact
`taucode-m1w/.../bundle/macos/taucode.app`, followed by Quit & Reopen, made both
native permission checks return true. Other app permissions were preserved.

The first internal agent test exposed a separate OMP integration failure:
`computer_run_task` timed out after 30000ms while the native run was executing
Calculator steps. The run was stopped before retrying. Connector delivery now
allows 3660 seconds (the maximum native wall limit plus result overhead), while
the native runner retains the user's shorter wall/step limits. Pi and Codex use
seconds; OMP's identically named timeout field uses milliseconds, so its profile
converts the generated configuration. Other connectors retain their defaults.
The deadline regression failed before the fix. All 110 computer-related Rust
tests, TypeScript and `cargo check` pass. The signed debug rebuild and strict
signature verification passed (`/tmp/taucode-computer-timeout-build.log`). A
mock Unix-socket host verified the real MCP executable disconnects on numeric
and string request cancellation, and on client EOF; no native input was involved.
The corrected build passed live Calculator acceptance: run
`092ae7c9-4984-4ca7-aa22-265726a571d0` completed 12 times 7 in eight steps,
and independent Calculator accessibility inspection confirmed 84. It ran for
about 87 seconds without an MCP timeout. Native input used accessibility elements
without foreground takeover or input errors. Model prediction consumed 72,058 ms
versus 2,641 ms executing input; prediction latency is the next performance target.
The main agent used GPT 6.1 Sol (Account 2), Low, Full access. Computer Use's two
models also used that account/model; they have separate reasoning configuration.

Normal chat Stop cancelled run `68e8e463-2c91-45ba-83f2-9b9146ad474d` after
one action: native status became stopped with error `Stopped`, and the UI said
nothing restarts automatically. Both native macOS permissions remained granted
across changed signed builds. No new authorization prompt was observed.

The active Computer Use tool now includes inline progress, Pause/Resume and Stop,
so control does not depend on the separate native HUD. Controls bind to the
matching run and recheck its ID before acting; failures remain visible and retryable.
Live OMP testing exposed a connector-name mismatch: OMP sends `Computer Use`,
while Codex uses `taucode-computer`. Both names now render the controls and pending
approval, including when the tool details are collapsed. The OMP regression failed
before the alias fix.

Live inline Pause/Resume passed in run `166b2e2f-fd87-4ef8-a9a9-cf4c4e5eb6e5`:
the card showed paused, Resume re-planned the pending Equals action (`staleRetries=1`),
and independent Calculator accessibility inspection confirmed 42. No input errors
or focus restorations were recorded. An earlier test stopped because its instruction
said not to retry; it is not counted as a successful resume test.

The user's report of failure while using another app was reproduced in run
`bd67ea9f-29a5-4375-b651-97fcac7f52b0`. Typing in taucode's composer remained
intact, but native window selection chose Calculator window 32806, a 66x20 layer-zero
transient titled `Window`, instead of Calculator's actual window. The run used pixel
fallback against that tiny capture and then failed when the transient disappeared.
Window selection now prefers the target app's own focused window (without activation)
before front-to-back fallback. Minimized or unreadable off-screen preferred windows
remain excluded; another usable window can still be selected.

The selection regression failed before the fix. All 109 targeted Rust tests,
`cargo check`, TypeScript and strict signed-bundle verification passed. Frontend
validation: 1,866 tests across 226 files and changed-file ESLint passed.
Logs: `/tmp/taucode-window-selection-{tests,check,typecheck,build}.log` and
`/tmp/taucode-controls-alias-suite.log`.

Live concurrent-use acceptance passed on the corrected signed build in run
`d448c0ca-e906-42de-9299-b9800d34de2e`: while typing four sentences and moving
the caret in taucode's composer, Calculator completed 123 times 45 = 5,535.
Independent Calculator accessibility inspection confirmed the expression and
result. The draft stayed intact. All nine steps used main window 32194 (230x408),
with no errors or focus restorations. Proof: `taucode-background-typing.png` and
`taucode-calculator-isolation-result.png` in the task's visualization directory.
This validates Calculator concurrency; broader app and keyboard-delivery coverage
still needs separate live tests.

Inline Stop also passed: run `b401ea21-0253-417e-8719-452b3f332d93` stopped
after two Calculator actions; native status returned `running=false`, `paused=false`,
`error=Stopped`. The third prediction was cancelled before input.

## Computer-use overlay polish — October 30

The user reported the large native HUD and labeled virtual cursor covering another
app. Background runs now keep their controls in taucode's inline tool card and do
not create a floating HUD. Foreground runs retain their native controls; the global
Stop shortcut remains armed in both modes. Run state includes its input mode so a
late HUD update cannot show a background run.

The optional virtual cursor is a compact translucent blue pointer with a lavender glow and no label. It appears over uncovered
parts of the controlled app, including background windows, and fades between 2.5
and 3 seconds after an action. System-wide accessibility hit testing checks the
app under its tip; transparent Dock/system windows do not count as occlusion. Native
menu-item and row-selection clicks also emit the hint. Idle ticks do not redraw an
image and poll at 10 Hz instead of 60 Hz. It remains nonactivating and ignores mouse input.
111 targeted Rust tests, `cargo check`, and TypeScript pass. Regressions cover HUD
mode/running/paused combinations and cursor focus/visibility/fade behavior.
Logs: `/tmp/taucode-overlay-{tests-final,check,typecheck,build}.log`.

The rebuilt signed bundle passed strict signature verification. Live run
`b6c63e60-8d8c-4f8a-807e-9a97d1572d86` reported `mode=background`; inline
Pause/Resume worked and the run completed 6 times 7 in six steps without errors.
Independent Calculator inspection confirmed 42. Screenshot
`taucode-inline-controls-overlay-polish.png` verifies the inline paused controls.
Native overlay panels are excluded from automated captures, so the cursor's final
appearance and absence over unrelated apps are not claimed as screenshot evidence;
their visibility policy is covered by the regression tests above.

## Computer-use reliability, progress, and latency — October 30

Expanded the user-approved live scope to Calculator and the disposable TextEdit
`Background QA.txt` fixture. Main conversation: GPT 6.1 Sol (Account 2), Low,
Full access. Computer planner and grounding use the same model/account; their
separate sidecar configuration does not expose the conversation's reasoning setting.
Background mode and per-action typing approvals remain enabled; no foreground
fallback was authorized. No personal documents were used.

Implemented Observing / Planning / Acting / Waiting for approval / Waiting for the
app phases in the inline controls. Removed AgentS3's separate reflection request:
the primary planner already verifies the previous action from each fresh screenshot.
Its instructions now explicitly preserve partial work and avoid replaying uncertain
actions. A pause during input supplies that same recovery context on resume.

Matched Calculator task, 12 × 7 = 84:

| Run | Steps | Prediction wait | Input execution |
| --- | ---: | ---: | ---: |
| `092ae7c9-4984-4ca7-aa22-265726a571d0` (baseline) | 8 | 72.058 s | 2.641 s |
| `839a0fe6-e91e-4c5b-83a7-87a079d0a13d` | 7 | 47.516 s | 2.278 s |

This single comparison also differs in step count; it is evidence of lower latency
in this run, not a general performance guarantee. Independent Calculator inspection
confirmed 84 while a concurrent taucode draft remained intact. No focus restorations
or input errors were recorded.

TextEdit checks:

- Scrolling: `1e680c55-5ca6-4740-87f6-f5d212532f22`; two background scrolls,
  independently observed later fixture lines.
- About dialog: `852e48ff-0832-45e5-92ed-c2c2421a8f66`; opened from the app
  menu, observed TextEdit 1.20 (415), closed only About, returned to the fixture.
- Window switching: `2411c34b-6d17-4c06-aaaa-e258f3f7debe`; created one empty
  Untitled, switched between it and the fixture using Window menu, closed only
  Untitled. All 120 sample lines remained and the concurrent taucode draft
  `Window switching isolation 8426` stayed in taucode.

Live testing found and fixed two native bugs. Open submenus were labeled
`Context > Find…` while verification reconstructed `Edit > Find > Find…`, causing
false stale-target failures. Snapshot labels now use the same native ancestry;
genuine changes use bounded fresh-observation recovery. The next run
`14202901-7772-4e11-b123-d4ac8f402c1d` passed the submenu and located Line 100,
but stopped after Escape dismissed Find: key-up incorrectly required the dismissed
field's focus. Keys already held now release through the same bound input as cleanup;
new key-downs retain all context checks. Regression coverage distinguishes cleanup
from a genuinely stale second key-down, which remains non-replayable after input.

Finished run cards were also missing: compact stored tool replies intentionally omit
thumbnails, but the frontend required them. Valid stored references now load their
full run log. Errors show a compact explanation, expandable diagnostics, and a
Copy continuation request action. The copied request records uncertain effects and
requires a fresh observation; it never sends or replays anything automatically.

An initial broad TextEdit run (`acedb161-1612-4ea8-9d6f-ff3d2e6ae821`) failed in
model planning before GUI input. Its claim about an unavailable image tool was model
output, not an observed native failure. A pre-existing extra `I` at the fixture's
start was observed before the final typing checks; its origin is unestablished and
it was preserved. Do not count either as a successful typing test.

Validation: 1,874 frontend tests / 226 files, 114 targeted Rust computer tests,
132 Python tests, TypeScript, changed-file ESLint, and `cargo check` passed.
The rebuilt Apple Development-signed bundle passed `codesign --verify --deep --strict`.
Logs: `/tmp/taucode-recovery-suite.log`, `/tmp/taucode-release-rust.log`,
`/tmp/taucode-release-check.log`, `/tmp/taucode-final-reliability-build.log`.
The final build's live append and recovery-card verification is pending a Mac unlock;
do not treat the earlier Find success as proof of append or the key-release fix.

Startup selection was repaired in the follow-up below. Internal metadata requests
now explicitly use ephemeral Codex threads, matching their response collector,
instead of creating persistent Pi/OMP threads through the default-engine router.
The selected model is retained for the turn; ephemeral start events are hidden
before sidebar insertion. The verified leaked helper session
`85879f85-00ff-4b04-9bbe-008e634d9d7b` was moved, with the app closed, from
`omp/sessions/47a7f4e9-f915-4b6d-87b9-b0656fe66da8/` into the matching directory
under `omp/archived_internal/` in app data. Its complete JSONL is preserved; no
user conversation was removed.

### Follow-up verification after unlock

The final build executed Escape without the prior key-release error in
`26dc9341-d132-49e1-ba5f-ed90f7b326e4`, but that run did not open Find (its native
menu item was disabled), so it does not independently prove dismissal of Find.
The same run exposed an unsupported planner key name. Structural action validation
now feeds an explicit rejection back to the planner before input, using the existing
three-consecutive/six-total recovery caps. Unknown-key errors name the bounded key
and suggest supported names. A regression proves correction to `down` sends only
that valid keystroke, while repeated invalid proposals stop without any input.

A subsequent run `efc0caf8-5a59-4dd3-ab90-15e7b1d013cf` moved to the document end,
then cancelled a scroll because its AX policy target changed before input. This
fail-closed check remains intact; the remaining task was continued from a fresh
observation rather than replayed.

Background append passed in `7385679d-7169-4fe1-b8e6-f9f0f27d92c3`: clicked the
empty line after Line 120, navigated to the end, and inserted `BACKGROUND_QA_OK`
with `overwrite=false`, `enter=false` through `ax-hit-test`. Independent TextEdit
AX inspection found exactly 120 sample lines and exactly one marker at document end.
The complete preceding text matched the fixture on disk (including the initial I).
No Save command was sent; TextEdit can autosave independently. Concurrent draft
`Concurrent append isolation 9638 remains in taucode.` stayed in taucode and was
absent from TextEdit. Screenshot: `taucode-textedit-background-append.png` in this
thread's visualization directory.

Live history exposed a second result-format mismatch: failed OMP MCP calls store
structured JSON prefixed with `Error:` inside `error.message`. The computer-result
parser now unwraps both layers, with a regression starting at actual
`buildConversationItem` conversion. The exact-prefix regression failed before the
fix; the corrected parser also accepted all ten real saved run results from this
session.
Validation after these follow-ups: 1,875 frontend tests / 226 files, 115 targeted
Rust tests, TypeScript, changed-file ESLint, and `cargo check` passed. Python remains
unchanged since its 132-test pass. Build log: `/tmp/taucode-recovery-final-build.log`.
The follow-up build's recovery-card verification is recorded below.


### Final live recovery verification

The signed build from `/tmp/taucode-recovery-prefix-build.log` passed strict
signature verification and displayed the actual failed run
`14202901-7772-4e11-b123-d4ac8f402c1d` as a structured card. Its concise error was
visible, Error details was collapsed, and Copy continuation request reported success.
Pasting into the empty test composer confirmed fresh-observation instructions,
preservation of completed work, explicit uncertain-action status, and unchanged app
permissions/approval policy. It did not auto-send; the test draft was cleared and
Send returned to disabled. No action was replayed. Screenshot:
`taucode-computer-recovery-verified.png` in this thread's visualization directory.

Final acceptance for this batch: background Calculator arithmetic, TextEdit typing,
scrolling, menu navigation, About dialog, and document-window switching have live
evidence above. Progress phases were observed live; lower prediction latency is
supported by the single matched Calculator comparison. Recovery UI is now verified
against real persisted failures, with bounded non-executed-action recovery and
partial-input safeguards covered by regressions. Full frontend rerun: 1,875 tests
passed (`/tmp/taucode-error-prefix-suite.log`); TypeScript and changed-file ESLint
passed. Native checks remain the 115-test pass and `cargo check` for the exact native
source bundled here. This is scoped acceptance, not a claim of general desktop-app
parity: disabled background menus and transient policy-target changes can still stop
individual routes safely, as the failed live runs document.

## Current reliability verification

Late `ensureThread` events preserve an explicit empty selection as well as a
selected conversation. A regression reproduced the empty-composer selection being
replaced by a background load; initial, uninitialized selection still works.

A policy-target change immediately after approval discards the unexecuted action
and uses bounded recovery with a fresh observation and prediction. Any required
typing approval is requested again. Regressions verify that only the newly
predicted text is delivered, and that a changed AX binding still stops without
input. Partial-input and later target checks retain their fail-closed behavior.

Validation: 1,877 frontend tests, 117 computer-use Rust tests, TypeScript,
changed-file ESLint and `cargo check` passed. Logs are
`/tmp/taucode-reliability-{frontend,rust,typecheck,check,build}.log`.
The debug app was rebuilt, signed with the configured Apple Development identity,
and passed strict signature verification. Live workspace refresh preserved an
empty selection and an unsent draft. The test draft was cleared without sending.

Calculator run `8379bacb-3535-4789-a535-63eebabffd0a` used background button clicks
to produce 6×9=54 while an independent unsent taucode draft remained intact. Both
the planner and independent AX inspection verified 54. A redundant capture after
the Done action then timed out in ScreenCaptureKit and incorrectly failed the run.
Done now returns the same fresh observation the planner verified, without another
capture or settling delay. Its regression also checks that no input is sent.
Native validation logs: `/tmp/taucode-completion-{rust,check,build}.log`.

The signed completion-fix build passed strict signature verification. Live run
`1e5fbb27-e19f-4243-b218-da9e30f9db9a` completed in background mode with five button
clicks followed by Done, no errors, and a verified 6×8=48 result. An independent
Calculator AX read also showed 48. The concurrent `FINAL_QA_DRAFT` remained intact
in taucode; it was cleared without sending, leaving Send disabled. The main agent
used GPT 6.1 Sol (Account 2), Low, Full access. Evidence in this chat's visualization
directory: `taucode-completion-background-pass.png` and
`taucode-calculator-background-48.png`.

Native overlay panels are excluded from captures. The user reported no cursor in
the live run. Native inspection found a screen-sized Dock window ahead of app
windows; rectangle-based occlusion incorrectly treated its transparent area as
opaque. The cursor now uses the actual accessibility hit owner, with unknown or
other-app hits hidden. Panels ignore mouse input and are excluded from accessibility.
Run diagnostics include `cursorHintsShown`, counted once per motion after AppKit
reports the panel visible. Fade and hit-owner policy have regression coverage;
the obsolete rectangle-occlusion test was removed with its implementation.
Logs: `/tmp/taucode-cursor-hit-{rust,check,build}.log`. The corrected build passed
strict signature verification and launched. With Calculator exposed on the user's
other display, run `1cde3258-8689-4eff-9673-76ce4a1873ce` completed Clear → 7 → 2,
verified 72, and recorded two displayed cursor hints. At the user's request,
run `c75bb5d1-92ba-4048-9862-b6a9f416fb69` repeated Clear → 4 → 5 → 6, verified 456,
and recorded four displayed hints. Both runs completed without errors; independent
AX reads confirmed both results. The user confirmed seeing the cursor, but found
it too small and faint. The current pointer follows the user's rounded, notched
reference shape, approximately 25 points tall, with a 42%-opacity neon-blue fill,
an icy-blue edge and a soft four-point lavender glow. The click ripple
expands with cubic easing and fades over 0.75 seconds; the pointer holds for 2.5
seconds before a 0.5-second fade. Native rendering was previewed on light and dark
backgrounds in `taucode-cursor-glass-design.png` in this chat's visualization directory.
117 computer-use Rust tests and `cargo check` passed. Logs:
`/tmp/taucode-cursor-polish-{rust,check,build}.log`. The signed build passed strict
verification. Live run `04b8dc1e-8633-49e7-b4f1-3e52f2d54ed6` completed Clear → 8 → 2
→ 1 in background mode with no errors and four displayed cursor hints. The run
and independent Calculator AX inspection both verified 821. Taucode returned idle
with an empty composer. The new shape was inspected in the native rendering preview;
the user's assessment of the updated appearance is not assumed. The reference-shape
update passed the same 117 native tests and `cargo check`; its logs are
`/tmp/taucode-cursor-glass-{rust,check,build}.log`. The signed build passed strict
signature verification. Live run `1e58965e-22ed-44b6-bc20-5a31d56c43ed` completed
Clear → 9 in background mode with no errors and two displayed cursor hints.
An independent Calculator AX read confirmed 9. The renderer preview verifies
the new shape, transparency and glow; automated app captures exclude the overlay.


### Background actions and recovery clarity

TextEdit Find run `f11f1661-7e53-4d05-b669-f44fd322f6d3` confirmed that
Edit → Find was disabled in the background. A later AXPress returned
CannotComplete (`-25204`); its pixel fallback hit window chrome and left the
document minimized. The disposable document retained its original contents.

CannotComplete now stops without another input: macOS has not established that
the first action did nothing. Only unsupported operations (`-25205`, `-25206`)
retain the event fallback. Regression coverage verifies that ambiguous native
actions never enter the event path. Background Find is state-dependent: taucode
does not force a disabled command or take foreground control without approval.
After an independent Find/close check refreshed TextEdit's state, native run
`7e8fda82-2e62-406a-9c5e-76b4d85b34b1` opened Find once through `ax-menu`, closed
it with Escape through `pixel-skylight`, and finished without errors or recorded
focus restorations. The fixture matched the saved baseline byte-for-byte. This
proves the ready-state path; recovery from an initially disabled menu remains
unverified.

Recovery cards distinguish blocked tasks, interrupted input, and completion.
Rejected steps say “Not executed”; uncertain input says “Check result”. The
continuation request carries those distinctions and requires fresh inspection,
with no automatic replay. Persisted run results are reduced to valid JSON references
before transcript text limits apply; a long native error previously truncated the
JSON and silently removed the recovery card. Details load from the saved run.

The first signed build passed strict signature verification. Background Calculator
run `415bd4dd-c399-40e2-9681-6988444422e1` completed Clear → 4 → 8 without errors;
independent AX inspection confirmed 48 and the native transcript displayed
“Computer Use completed”. TextEdit run `96d44bd0-9936-4b46-a125-6157f7c4a526`
rejected Command-F before input and stopped without retries or alternate controls.
The fixture remains byte-for-byte equal to the saved baseline. Automatic titles
now recognize the first optimistic user message and its prompt-derived fallback
name. Older messages, nonmatching pending messages, existing custom titles, and
replayed confirmations do not trigger another generation. Manual renames made
while generation is in flight still win.

Signed-build verification with GPT 6.1 Sol (Account 2), Low and Full access
generated “Explain Autumn Leaf Color Changes” from a first-message question
about autumn leaves. The metadata request completed in 4,008 ms; the workspace
gained only the requested conversation, with no internal helper in the sidebar.
Automatic titles were restored to their original off setting. The engine routing,
model retention, and success/error cleanup also have native regression coverage.
Screenshot: `taucode-automatic-title.png` in this chat's visualization directory.
The restart check exposed OMP's leading title-slot record: the old reader expected
the session header on line one and silently omitted renamed conversations from
history. Reading and forking now normalize that metadata in memory, preserve the
session ID and message tree, and restore the saved title without rewriting the
source. Regression coverage includes listing, reading, forking, rewind/undo, and
malformed prefixes. The frontend title fix passed 1,886 tests, typecheck and
targeted ESLint. Logs: `/tmp/taucode-title-{suite,typecheck,lint,build,rust,check}.log`.
The final signed build restored the existing conversation and original answer
without rewriting its transcript (SHA256 unchanged before resuming). A follow-up
on GPT 6.1 Sol (Account 2), Low returned `TITLE_RESUME_OK`, retaining the title.
574 native tests passed (two ignored), along with cargo check and strict signature
verification. Screenshot: `taucode-title-restored.png`; final build log:
`/tmp/taucode-title-final-build.log`.

573 native tests passed (two ignored); typecheck, targeted ESLint and cargo check
passed. The final frontend suite passed 1,882 tests. Both signed builds passed
strict codesign verification. The final build reopened the real persisted Find
failure and displayed “Computer Use blocked”, “Step 1: hotkey · Not executed”,
collapsed error details and Copy continuation request. The internal metadata chat
remained absent after relaunch; the archived JSONL is available for restoration.
Screenshots: `taucode-reliability-completed.png` and
`taucode-reliability-blocked.png` in this chat’s visualization directory.
Logs: `/tmp/taucode-reliability-{rust,check,final-suite,final-build}.log`.

### Background recovery investigation and live controls

The live card now receives the background app and window title during capture,
instead of only after completion. Stop remains available while Pause is pending.
Controls carry a run ID; stale controls cannot affect a replacement run, and a
delayed resume cannot revive a stopped run. Native and UI regression tests cover
these races. Live run `68066fdd-711b-4021-ba5e-149a5278d6a8` showed TextEdit and
Background QA.txt, paused, then resumed with a fresh capture. Calculator remained
usable independently (7 + 8 = 15). Screenshot: `taucode-background-paused.png`.
Pause → Stop passed live on Calculator in run
`6be60911-8434-4bed-8b9d-3492c2e44a8d`: running=false, paused=false,
error=Stopped, only wait/predict actions, no recorded focus restorations.

Find passed after a fresh TextEdit launch and after an About-dialog open/close
cycle. Run `8b76b812-6a5e-4006-a0d5-8305d7b21e78` used `ax-menu` for Find and
`pixel-skylight` for Escape, with no recorded focus restorations. The disabled
state was reproduced after minimizing/restoring the document. Two experimental
keyboard routes failed to open Find: authenticated events in
`79cb956e-801d-4775-abc8-4f9bba1bb48b` and AppKit menu events in
`cf9af070-d222-49db-ac7b-4c4a128aed55`. Both stopped without replay or edits.
Those unsuccessful routing changes were removed; disabled menu commands remain
blocked before input. Independent computer use also could not open Find in that
state. Reopening the same file through the native background open action did not
establish recovery; the planner stopped without attempting the subsequent shortcut.

Recovery and the longer editing/scrolling/dialog workflow remain unfinished.
Computer use timed out twice while opening the fixture after a TextEdit restart;
the Open action's final state has not yet been verified. No foreground fallback
or new app permissions were enabled.

The longer Calculator workflow passed in run
`906ec986-321a-4352-97e1-fe2f7f7299d6`: 12 × 34 produced 408, then adding 92
produced 500. All 12 actions completed through background control, with no
recorded focus restorations. Independent accessibility inspection confirmed
the final expression `408 + 92` and display `500`.

Final focused checks: 37 Computer Use frontend tests, 118 native Computer Use
tests, typecheck, targeted ESLint and cargo check passed. The broader frontend
run had 21 failures under load; all failed cases passed in isolated reruns,
with two Theme Gallery tests requiring a longer timeout. The native suite had
one cancellation timing failure (575 passed, two ignored); that case passed
in isolation. These are rerun results, not clean full-suite passes.

The final controls-only build was rebuilt, Apple Development signed and passed
strict signature verification (`/tmp/taucode-controls-build.log`). Live relaunch
restored the completed Calculator conversation and retained GPT 6.1 Sol
(Account 2), Low. The paused-card screenshot and Pause/Stop checks above cover
the shipped controls; the unsuccessful menu experiments are absent from this build.

### TextEdit viewport and recovery acceptance

The document becomes accessible after the earlier inspection timeouts. Live
run `eeb92ad4-603c-4c5f-99f6-69f0fbaee13d` opened Find and searched for
`Line 100`; independent screenshot inspection confirmed one highlighted match.
Find Next returned AX error -25204. In
`c53c1146-a773-43f8-b547-2ab6d340872f`, Done also returned -25204, but independent
inspection confirmed Find closed. The 50 ms mutation acknowledgement deadline
can report failure after an effective action; this remains unresolved, and
uncertain mutations are not automatically replayed.

Run `6e38efd8-e076-4b4c-9a16-10056a992632` opened About TextEdit, verified
version 1.20 (415), closed only About, and returned to Background QA.txt. Its
scroll grounding chose document element 2 but discarded it with “Element center
outside screenshot”, then cancelled the pixel fallback after a target mismatch.
Grounding now clips frames to the window and enclosing scroll-area viewports,
while retaining original geometry for preflight validation. Fully clipped
elements remain unavailable; menu anchors retain their existing handling.

Run `259d2a52-1f64-4989-b989-3e99c86244d0` inserted RELIABILITY_QA_TEMP exactly
once at a separately verified document-end caret, verified it, invoked Undo
once, and verified the original BACKGROUND_QA_OK ending. Independent inspection
confirmed the temporary marker is gone and the disk fixture matches its baseline.
Command-Up delivered by taucode did not move the view; independent Command-Down
did. Keyboard routing still needs investigation. No focus restorations were
recorded in these runs.

The disabled-menu case is reproducible by minimizing/restoring Background QA.txt:
Find, Paste and Select All become disabled. Reopening the same file with the
background native open implementation did not recover it in
`ca7dbb0e-0f93-4a3a-a5d9-2b02f9b772ee`. Cancelling a lingering Edit menu and
raising the document through independent computer use also did not restore Find.
Recovery remains open; these experiments are not a successful recovery claim.

Viewport regression checks: 119 native Computer Use tests, cargo check and
typecheck passed (`/tmp/taucode-viewport-{rust,check,types}.log`).

The viewport build passed Apple Development signing and strict verification
(`/tmp/taucode-viewport-build.log`). Live run
`8e5c8823-eabc-4954-b161-5073b3d2ed63` scrolled up twice using fresh text-area
element 2 via `ax-element`, with no pixel fallback, and verified the exact header.
Independent inspection confirmed vertical scroll position 0 and the visible
header. Evidence: `taucode-textedit-viewport.png` in this chat's visualization
directory. This closes the offscreen-center scrolling defect; disabled-menu
recovery, uncertain acknowledgements and the combined workflow remain open.

### Background document recovery — live findings

Run `e38be47b-4dea-499d-9b5f-d03595275790` reproduced disabled Find after
minimize/restore. Native diagnostics showed AXMain=1, AXMainWindow=36175,
AXMinimized=0 and AXStandardWindow. The missing-main-document hypothesis is
ruled out; its experimental mutation and menu exception have been removed.

Run `ef0a2480-a98e-41f4-b955-33cadb0a9e16` exposed a separate routing defect:
a requested left click on the document chose its advertised AXShowMenu action,
opening a context menu. Independent accessibility inspection confirmed that
menu; it was dismissed without editing the fixture. Left-click routing now
considers Press/Pick only; ShowMenu remains exclusive to right clicks. An
unsupported Press can use the existing positional fallback, while ambiguous
mutation failures still stop without replay.

Validation: 120 native Computer Use tests, cargo check, typecheck, signed build
and strict codesign verification passed. Logs:
`/tmp/taucode-left-click-{rust,check,types,build}.log`.
Live run `d4080ad7-5b8b-4a12-9e86-1634295e4c26` used pixel-skylight for the
left click and produced no context menu, independently confirmed through the
accessibility tree. No focus restoration was recorded. The fixture remains
byte-identical to its baseline. Find was still disabled and the run stopped
without sending the menu command. This closes the left-click/context-menu bug;
disabled Find recovery, uncertain acknowledgements and the combined workflow
remain open. The main-document experiment and its temporary diagnostics are
removed from the current build.

### Saved image previews

OMP persists screenshots as `blob:sha256:<hash>` references. The history adapter
previously forwarded these references as base64, producing broken thumbnails and
lightboxes after reload. The shared Pi/OMP adapter now restores image bytes from
the workspace's isolated agent blob store for saved messages and live results.
It validates the hash, MIME type, file size and containment; missing or corrupt
images become an explicit unavailable message. Inline images and saved session
files are unchanged.

The saved screenshot for run `1803147c-ea31-4297-9227-dd53f389bbf2` was reproduced
broken before rebuilding, then verified live as a decoded thumbnail and full-size
lightbox after a fresh launch. Evidence: `taucode-restored-image-preview.png` in
this chat's visualization directory.

The image restoration regression, typecheck, cargo check, signed build and strict
signature verification passed. The broader Pi suite had 59 passes, one ignored
and one preview-server startup timeout; that timeout also occurred on an isolated
rerun and remains unresolved. Logs: `/tmp/taucode-image-{tests,pi-tests,preview-retest,types,check,build}.log`.

### Disabled document-menu diagnostics

Disabled shortcut diagnostics now include the bound window, whether its app is
frontmost, and the roles/window IDs/focus flags returned by AXMainWindow,
AXFocusedWindow and AXFocusedUIElement. They exclude field contents and do not
mutate focus. This runs only when a shortcut is disabled.

Run `c4468827-f36d-4172-977a-07ba2e9dd79e` reproduced disabled Find after
minimize/restore. All three AX references pointed to fixture window 36175; the
text area reported focused=1, both window references focused=0, and TextEdit was
not frontmost. The main document and focused control are not missing. Changing
the insertion point and opening/closing the enabled Select Line dialog through
computer use did not re-enable Find or Select All.

Run `56262e8f-e2e6-47aa-9854-6fd727db5dac` inserted the temporary marker once via
ax-hit-test. Its next attempt to click a menu was refused because the AX target
left the captured window. Independent inspection confirmed Undo and Find were
both disabled. The marker was removed through the fixture's editable AX value
only after confirming it was the sole difference from the baseline; a fresh AX
read confirmed the original contents. The file on disk was not saved or changed.
A normal user click subsequently re-enabled Find, Undo and Select All. Native
diagnostics then reported TextEdit frontmost, so this is not a background pass.

120 native Computer Use tests, typecheck, cargo check and strict signature
verification passed for this diagnostic build. Logs:
`/tmp/taucode-focus-diagnostic-{tests,types,check,build}.log`.

The longer Calculator run `f9a58252-7e33-4e8f-aa74-9f0eb5d90da5` completed
14 native button actions and a final verification: 125 × 8 = 1,000, ÷ 4 = 250,
then + 75 = 325. Independent AX observations confirmed all three checkpoints
(the intermediate results while the following operator was being entered) and
final 325. No action errors or focus restorations were recorded. TextEdit's
combined workflow and disabled-command recovery remain incomplete.

### Exact-field keyboard delivery

Run `b632ba5c-c9cd-48f3-b3c8-1f86c71cd329` inserted a disposable marker,
successfully undid it and opened Find. Its targeted Find-field type action
(`overwrite:true, enter:true`) then replaced the document body through
`pixel-skylight`. AX focus still pointed at the document, not the selected Find
field. The agent stopped without saving. Independent AX inspection confirmed
the wrong-field mutation; the fixture was restored from the baseline and a fresh
full-text read verified the restoration. No disk save was performed.

Keyboard fallback now binds the exact selected/hit-tested field and compares it
with AXFocusedUIElement before delivery and before each key/text event. Missing,
unreadable or mismatched focus refuses preflight before clicking, selecting all,
typing or pressing Enter. Later focus changes stop delivery without claiming
that earlier input was never sent. Direct AX field writes remain available.

Signed-build run `85e7aed9-9611-421f-9571-4816ca4b77b3` repeated the targeted
Find overwrite+Enter action while AX focus remained on the document. Preflight
refused it with `background_keyboard_target_not_focused`, without dispatch.
Independent full-text AX comparison confirmed the document matched its baseline;
Find retained `Line 100` rather than the attempted `Line 099`. No retry occurred.

Positive run `0992816a-4a64-42bd-82d7-f2390a181f79` used the same selected
Find field with `enter:false`. Direct `ax-element` delivery changed Find to
`Line 099`; independent full-document AX comparison still matched the baseline.
Both checks used GPT 6.1 Sol (Account 2), Low, with Full access and foreground
fallback off.

Follow-up `29e6fa62-c9a2-4ed3-b4f2-e65e02718d17` invoked Find Next once via
the bound AX button. macOS returned `ax_press_failed(-25204)` and the run stopped
without retry. Independent inspection showed `Line 099` selected and the viewport
moved to lines 086 onward; the action did take effect. Full document text still
matched baseline. Done/top/bottom checks were not executed. Reconciling uncertain
AX action completion without replay remains open alongside minimize/restore
menu recovery.

121 native Computer Use tests, typecheck, cargo check, signed rebuild and strict
signature verification passed. Logs: `/tmp/taucode-field-focus-{tests,types,check,build}.log`.

### Document-window recovery investigation

Minimize through TextEdit's Window menu and restore with AXRaise reproduced the
disabled Find/Undo/Select All state. Baseline run
`48269c11-88b9-4c34-9268-314b2fbb9f11` confirmed Find disabled with TextEdit
not frontmost and AXFocusedUIElement still the document in window 36175.

An experimental AXFocused=true refresh followed by a routed document click did
not recover Find (`fa918be8-1749-4c5b-99d6-256a143e96c4`). Both foreground
readings were pid 18785 and no focus restorations occurred. That ineffective
source change was removed.

The next candidate refreshes TextEdit's AppKit document-window state using only
the target focus record from Cua's `activate_without_raise_with(..., false)`.
It does not defocus the user's app or call SetFrontProcess. It is restricted to
plain left clicks on a TextEdit AXTextArea when the captured Find command is
disabled; other apps (including Notes), modified clicks and hotkeys are excluded.
Foreground/key-focus readings must be available and unchanged before proceeding
with the click.

Live recovery passed twice on the signed build:
- `e96f4341-4561-4dcc-b3fa-09196fe098ce`: document click refreshed window state;
  Command-F opened Find. Done closed it but returned AX timeout -25204, so the
  run correctly stopped on uncertainty. Independent AX inspection confirmed closed.
- `0b0bb1b9-b04f-4a06-bfa9-ef333969c3b2`: repeated Window > Minimize / AXRaise,
  independently confirmed disabled Find/Undo/Select All, then document click and
  Command-F recovered Find again. Left Find open; independent AX and screenshot
  confirmed the search field, navigation buttons and Done.

Both refreshes kept WindowServer foreground and keyboard-focus pid 2726 unchanged;
all steps recorded zero focus restorations. The complete in-memory document
matched the baseline after each run. GPT 6.1 Sol (Account 2), Low; Full access;
foreground fallback off. This covers TextEdit's reproduced stale document-window
state, not arbitrary apps or every source of disabled menu commands.

122 native Computer Use tests, cargo check, typecheck, signed rebuild and strict
signature verification passed. Logs: `/tmp/taucode-window-primer-{tests,check,types,build}.log`.
The ambiguous AX-completion issue remains separate.

### Read-only verification after ambiguous AX completion

The recovered document exposed a separate false failure: AXPress on Find's Done
button closed Find but returned -25204. A new timeout path releases input and
restores action state, then captures fresh evidence within the existing run/time
limits. The planner can report the entire task complete or explain what remains;
the host rejects every input action during this verification. The original step
retains its uncertainty error and is never labelled “not executed.” No automatic
retry or pixel fallback is introduced. Intermediate incomplete tasks still stop
for review rather than blindly continuing after an ambiguous mutation.

123 native Computer Use tests passed, including completion, failure, rejected
second clicks and overwriting typing; one AX attempt, two captures, no replay,
focus cleanup before verification. Signed build and strict verification passed.
Logs: `/tmp/taucode-ax-verification-{tests,build}.log`.

Initial live timeout verification `6bb75ec2-20df-4305-94ab-e5b2613235e6`
recorded one Done click with -25204, followed by `verificationOnly:true` and a
fresh screenshot confirming Find closed. Independent AX confirmed closed Find
and unchanged complete document. Zero focus restorations and no repeated input.
The planner incorrectly claimed no timeout was supplied: its per-step message
forwarded the two older recovery prefixes but omitted READ-ONLY VERIFICATION.
Added that prefix to the existing forwarding path and extended the real planner
message regression; all 132 Python tests passed.

Final signed-build retest `be0c3f29-87eb-4977-906f-2c2a9e16541f` passed:
Command-F opened Find, one Done AXPress returned -25204, and a fresh read-only
verification confirmed Find closed. The planner now explicitly acknowledged the
host timeout. Independent AX inspection confirmed closed Find and unchanged full
document; every step recorded zero focus restorations. No repeated input.
Build/signature log: `/tmp/taucode-ax-verification-final-build.log`.

### Longer background workflow acceptance

Typing/Undo `9be34d8e-3486-40f8-90f6-5556c4851ab3` passed: document click,
explicit AXTextArea insertion of `BG_RELIABILITY_1031` (overwrite/enter false),
then one Command-Z. Marker appearance/removal were observed by the planner;
independent full-document AX comparison after Undo matched baseline. Delivery
paths were pixel-skylight, ax-element, ax-menu; no errors or focus restorations.

Scrolling `6e77b20f-c934-45e4-813d-b5f134e4c925` reached the end via a -12
AX scroll. Independent screenshot confirmed Line 120 and BACKGROUND_QA_OK.
The planner then proposed +160, exceeding the allowed +/-100 range; validation
correctly sent no input but incorrectly terminated the run as a task failure.
The action schema now exposes that bound; local argument-validation failures are
explicitly “Action not executed” and use the existing bounded replan path.
Ambiguous native actions and provider failures retain their distinct handling.
133 Python tests passed; see final native and live results below.

Native dialog run `16ce1da3-2457-44b8-a000-fb77d014319e` passed: native Edit >
Find > Select Line navigation, inspection without typing, then one close-control
click dismissed the line-number window. No errors or focus restorations.
Independent AX inspection confirmed only the main fixture window and unchanged
complete document text. 124 native Computer Use tests and 133 Python tests now
pass for the rejected-input correction. Signed rebuild and strict signature verification passed.
Logs: `/tmp/taucode-rejected-input-{tests,python-tests,build}.log`.


Final scroll regression `b99e7e0f-f533-4785-bd59-15a59a411c55` passed on the
signed build. The intentionally proposed +160 was rejected before grounding or
native input (`recovered:true`, path absent). The same run continued with valid
+100 and +30 ax-element scrolls and verified the header and Line 001. Independent
screenshot confirmed the top viewport; complete in-memory AX text and the disk
file both matched the baseline (SHA256
`4cb1afec0fbdd78dcfe1fef13baa0883da9508e1eca000e754bdb884b3f2be50`).
No focus restorations, edits or saves occurred.

### Current background-reliability acceptance — October 31

The scoped three-part plan is complete: reproduced TextEdit minimize/restore
Find recovery passed twice; Calculator arithmetic and TextEdit typing/Undo,
scrolling, native menu/dialog/window transitions passed; target/capture status
and Pause/Stop were verified in the earlier control acceptance. All live planner
checks used GPT 6.1 Sol (Account 2), Low, Full access, with foreground fallback off.
The foreground/key-focus process stayed Safari across both recovery primers.
This is evidence for Calculator and the disposable TextEdit fixture on this Mac,
not a claim of arbitrary-app compatibility or universal concurrent-keyboard safety.
An uncertain intermediate mutation still stops for review if the entire task
cannot be confirmed complete from the read-only verification observation.

### OMP-owned service visibility and schema — October 31

The Focus Board gap is fixed in `8469f7e` plus the follow-up action-error fix.
OMP supervises `bash` services in its own per-project daemon broker, so taucode's
managed tasks never saw them. `ManagedTasks` now merges the services owned by the
current conversation (broker `owner` equals the thread id), listed through OMP's
own `ps --json` CLI with the engine's environment. Stop/Restart go through
`ps stop|restart` after a fresh ownership check; nothing kills PIDs or port owners
directly, and `spec.json` (service environment) is never read. Listing never
spawns an engine and is cached on broker `meta.json` changes with a 15 s ceiling.
Restart is allowed only while the engine process that started the service (the
commandGuard owner PID) is alive; otherwise the UI says to ask the agent again.

Schema: strict tool-call layers materialize every optional `bash` field
(`timeout: 0`, `async: false`, empty `ready`). OMP normalizes blanks but rejects
any `timeout` in service mode, which caused the three historical direct-call
failures. The gate now drops `timeout` for named services only.

Action errors (for example the Restart refusal) previously vanished on the next
2 s poll; they now persist until the next action, while recovered poll errors clear.

Tests: 7 service-bridge and 11 managed-task Rust tests, 449 `shared::` Rust tests
(0 failed), 51 Pi extension tests, 1,890 frontend tests / 226 files, TypeScript,
`cargo check --all-targets --features custom-protocol`. New regressions failed
before their fixes (gate timeout, persistent action error).

Live acceptance on the signed build, Sol Account 2 / Low / Full access, new OMP
conversation in `/Users/rolandleyco/taucode-dogfood-focus-board`:
the agent's first direct `bash` call with `timeout: 0` started
`focus-board-preview` in 7 s (previously rejected). Session rail showed Servers (1)
`http://127.0.0.1:4173/` and Background tasks (1) running; another conversation
in the same project showed none. The native browser loaded Focus Board from the
URL. Rail Stop ended the listener, guard and broker record (`exited` 143, shown
stopped). Restart from Background tasks brought a new guard/listener up on
127.0.0.1:4173 only. Quitting taucode removed the engine, guard, node and listener.
After relaunch the history showed `failed · 143`; Restart was refused with the
earlier-session message, which stayed visible, and spawned nothing. Project
files kept their October 30 modification times.

Limits: after an app relaunch the service history appears a few seconds after the
conversation loads (first polls precede thread resume). Opening a server from the
rail navigates the Browser panel but does not reveal a hidden panel. Only
OMP 18.12.6 on this Mac was exercised; Pi and Codex have no equivalent services.

### Service rail polish, keyboard safety, and browser fault injection — October 31

Signed debug build, Sol Account 2 / Low / Full access, Focus Board fixture.

- **Rail Open with a hidden Browser.** Not a defect: live, Open on a server URL
  with the Browser pane hidden revealed it at once. The earlier symptom was a
  capture taken before the first native `attach` returned. A DockWorkspace
  regression test now locks the reveal.
- **App-quit label.** OMP records a service ended by the commandGuard on app quit
  as `failed` with exit 143. Signal exits 129/130/143 now show as `stopped`;
  137 (possible OOM kill) and other codes stay `failed`. Live: `stopped`.
- **History after relaunch.** The first poll could not route to the OMP backend
  until `thread/read` recorded the owner. `has_session` now also accepts a saved
  `<sessions>/<workspace>/<uuid>.jsonl` (no scan, no spawn), and the hook polls
  every 500 ms three times after a session opens. Live cold relaunch: history
  visible about 1 s after opening (one cold `omp ps` ≈ 0.6 s), previously 2–5 s.
- **Keyboard delivery while the user types.** Review finding: background keyboard
  delivery holds `SLPSStealKeyFocusReturningID`, so physical keystrokes during
  that lease reach the target app, and `user_event_age` ignored key-down events.
  Now: no key-focus theft if a hardware key was pressed in the last 0.5 s
  (recoverable, no input dispatched); during a lease, a hardware key press stops
  further key-down/text posts with an `UNCERTAIN` error, held keys are released,
  and the runner switches to read-only verification (no replay, no further input).
  Held-key releases are never blocked. Residual window: a keystroke after the
  last checked post and before release is not detected.
  Live limit: a faithful concurrent-typing test was not possible from this
  session. Full-screen control hides apps the session may not touch, and
  Calculator/TextEdit access was declined, so taucode correctly refused the
  hidden Calculator window ("window was off-screen; no keyboard input"). Covered
  by unit/runner tests; a manual check is still recommended.
- **Computer Use app names.** Agents passed `apps: ["Calculator"]` and got the
  misleading "Select allowed apps" error. Requested apps now resolve to exactly
  one allowed bundle ID (exact, case-insensitive, or name = last bundle
  component; ambiguous names refused), and unknown apps get an error listing the
  allowed IDs. Live: the retried task passed app selection.
- **Native browser fault injection.** Debug builds honor
  `TAUCODE_BROWSER_FAULT=<stage>[:count]` (configuration, window, webview,
  accessibility, layout, navigation); release builds compile it out. Live with
  `layout:1` (after the child webview exists): the tab showed "Could not open this
  tab… Choose Retry", Retry recovered it, no crash. Found and fixed: the dock's
  duplicate error toast stayed after a successful Retry; it now clears when that
  page recovers.
- **Uncertain actions.** Unchanged policy: uncertain outcomes verify read-only and
  never replay. Mid-action `UNCERTAIN` errors now use the same path.
- **Handoff preview test.** `managed_preview_bridge_serves_restarts_and_stops_without_user_dialogs` passes.

Tests: 453 `shared::` Rust tests (0 failed, 1 pre-existing ignored live test),
127 computer tests, 7 service-bridge tests, frontend 1,893 tests (one known
load flake, `ThemeGallery`, passes in isolation), TypeScript. New regressions
failed before their fixes (status mapping, saved-session routing, runner
uncertain execution, stale browser toast).

Follow-up — keyboard safety live acceptance: the remaining gap (a keystroke after
the last checked post but before key focus is released) is closed by a final
check inside `execute`, before `Control::release`; a hit becomes `UNCERTAIN` and
read-only verification. Live (Calculator granted to the operator session, which
typed 9s continuously into taucode's composer through the HID path): run
`4c7418b9` recorded "Action not executed: You are typing, so taucode held its
keyboard input until you pause; no input dispatched"; Calculator showed `0` and
no 9 ever reached it. With typing stopped, run `a9d4af81` typed
`123456789+987654321=` in one delivery and finished `done` (display
`1,234,567,899`, because Calculator's Escape cleared only the last entry and left
`123,456,789+`). Separate pre-existing observation: in run `4c7418b9`, the
retried type step (no user typing) stopped part-way with "Target window no longer
has the app's keyboard focus"; partial input remained and the run ended without
replay.

Follow-up — browser tab pile-up: when restoring a saved tab failed, native `new`
had already broadcast its page, but the open request was dropped on the error, so
the late page was appended as an extra panel tab and the saved tab kept a stale
target (observed 2 → 3 across relaunches). A failed request is now kept as an
orphan that the late page binds to; the next open clears it. The dock's duplicate
error toast clears once that tab's own error clears. Live: two relaunches with
`TAUCODE_BROWSER_FAULT=layout:1` and one without kept the count at 3, and Retry
recovered the tab with no lingering toast. Tabs already duplicated before the fix
stay until closed.

### Mid-typing keyboard-focus stops — October 31

Reproduced without user typing (run `eb3bf0e1`, Calculator): typing stopped after
`1,234,567,890+123` with `keyboard_target_unfocused`. The new diagnostics showed
Calculator's `AXFocusedUIElement` still in the target window (role `AXUnknown`,
window 32194) but a different object from the one captured when typing began:
Calculator rebuilds that element as its display redraws, and the check compared
identity only. Elementless typing now accepts a rebuilt focus element only when it
is in the same window with the same role and that role is `AXUnknown`; text inputs
and element-targeted typing still require the identical element. After the fix
(one live run, `abbe067a`) `1234567890+1234567890=` typed in one delivery
(`2,469,135,780`).

Also found:
- After that run, later runs refused their first Escape before any input ("focus
  could not be confirmed"); the preparation refusal now records which check failed
  and the focus state, to diagnose on the next live session.
- The planner's own `fail`/`done` steps ran the pre-input target check, so runs
  were recorded as "AX policy target left the captured window" and the planner's
  reason was lost. Done/Fail/Wait no longer resolve an input target.
- Planner-side: GPT 6.1 Sol repeatedly emitted a call to a hosted `imagegen` tool
  ("Prompt is required for imagegen tool"), failing 5 of 11 runs before any input.
  Not addressed here.

Tests: 130 computer tests, 456 `shared::` Rust tests (0 failed).

Live re-run on the latest signed build (17:23), nobody typing: four consecutive
runs (`be37eea2`, `1ce4fb27`, `91161e25`, `a03bf62b`) each pressed Escape and typed
a 35-character string in one delivery (540–634 ms) and finished `done`. No focus
stops, no first-Escape refusals, no planner errors. The earlier first-Escape
refusals did not recur, so their cause is still unknown; the preparation
diagnostics now in place will name the failing check if they come back.
