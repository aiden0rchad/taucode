# Dogfood acceptance — October 30, 2026

Branch: `m1-wiring`. Signed macOS debug app, version 0.7.68. This is the current
summary of the eight-item plan; dated sections in the specs are historical evidence.
No merge is authorized for this batch.

| Plan item | Verified outcome | Remaining limits |
| --- | --- | --- |
| Crash recovery | Pi/OMP checkpoints retain interrupted root output and delegation transcripts; engine/parent death stops owned POSIX commands. Signed native force-quit, engine-kill and follow-up checks passed. | Last second of unflushed output may be lost. Other engines, Windows cleanup and power-loss durability are unverified. |
| Chain of Command | Real reviewer evidence gates success. Shared concurrency, inherited deadlines and project daily usage limits passed automated and signed native checks. | Enforcement uses reported usage; in-flight requests can overshoot a token allowance. |
| Cloud/local measurement | Sol Account 2 and self-hosted Halogen passed six independent bugfix checks. Mixed-team implementation and independent review also passed. | Pricing remains **unknown**, not zero. One sequential baseline does not justify automatic routing or general quality claims. |
| Conversation teams | Overrides, project isolation, fork/side-chat copying and restart persistence passed. CoC stays in Code. | Native coverage is Pi-RPC/OMP on this Mac. |
| Work polish | Interactive agent transcripts, concise titles and preview Open/Stop/Restart controls passed live checks; stopping preserved the follow-up draft and closed owned ports. | Historical Work CoC records remain readable; new Work turns do not enable CoC. |
| Browser stability | Failure-state cleanup and Retry have regression coverage. Native blank-page navigation, close/reopen, zoom, focus and ten repeated create/close cycles passed. | Earlier empty-tab process exit remains unreproduced. Actual native creation failure/Retry has not been fault-injected. |
| Signing / Keychain | Free Xcode Personal Team setup produced a valid Apple Development identity. Two different binaries with that identity passed strict signature checks and real provider requests after fresh launches. Another fresh launch also completed a confirmed Keychain read and provider request. | The user did not notice whether the first two launches prompted; prompt-free access remains unconfirmed. Certificate renewal, another Mac and release notarization are outside this check. |
| Final acceptance | Code recovery/team tasks, Chat checks, Work artifacts/previews, browser interactions, settings keyboard flows and responsive Evals checks have passed within the recorded scope. | User review before merge remains required. No claim of a full VoiceOver audit, cross-platform acceptance or release notarization. |

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
then completed. This confirms a successful cold-process credential read, not
whether macOS briefly displayed a prompt. The user was asked about this latest
check separately. No additional binary change was made for this relaunch.
Screenshot: `taucode-apple-signing-fresh-launch.png` in the task's visualization folder.

The original metadata probe still reported six build-hash partitions after the
baseline request; it does not independently prove trust for the new identity.
Evidence is the changed-binary credential checks and the additional fresh launch;
prompt visibility remains unconfirmed.
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
