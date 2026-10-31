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
