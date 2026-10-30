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
| Signing / Keychain | Inside-out signatures verify. The latest credential prompt was approved; subsequent provider requests completed. | **Open:** a changed signed build prompted again. Earlier prompt-free rebuilds were insufficient evidence of durable trust. |
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

## Keychain finding and next step

The latest process sample again stopped in `SecKeychainFindGenericPassword`.
Read-only inspection of the existing taucode provider item's **access metadata**
(without retrieving its secret) found six partition entries, all `cdhash:` values.
Its trusted-app entry points to this app bundle, while its partition list recognizes
individual binaries. The current signature's designated requirement is stable:
bundle identifier plus the existing certificate's fingerprint. A stable designated
requirement does not satisfy a partition list containing only older build hashes.

[Apple DTS confirms](https://developer.apple.com/forums/thread/845980) that trusted-app
ACLs and partition lists impose independent conditions. This explains why repeated
approvals can be necessary despite signing each rebuild with the same local identity.
The item metadata and macOS trust settings were not modified.

Only the self-signed `taucode Dev` identity was found; it is unexpired but reports
`CSSMERR_TP_NOT_TRUSTED`. No Apple-issued identity was found. This does not establish
whether the user has an Apple Developer membership elsewhere.

The user authorized free Xcode signing setup. The Mac App Store installed Xcode
26.6 at `/Applications/Xcode.app`; its minimum macOS version is 26.2, compatible
with this Mac's 26.5.1. First launch is waiting at the Xcode and Apple SDKs Agreement
for the user's acceptance. Account setup and certificate creation remain pending.

Next: obtain an Apple-issued signing identity through the user's Xcode account,
configure `TAUCODE_SIGNING_IDENTITY`, then validate two changed binaries with actual
credential reads. Do not treat certificate installation alone as acceptance.
[Apple offers a free Personal Team for local development](https://developer.apple.com/help/account/basics/about-your-developer-account);
its suitability for this bundle must be verified. Distribution/notarization uses
the separate Developer ID setup in [packaging](packaging.md). Account sign-in and
accepting Apple's agreements require the user.

## Additional Chat acceptance

GPT 6.1 Sol (Account 2), Low, correctly converted supplied fictional meeting notes
into three action items with the specified owners and dates, without tools. The
model picker retained Low, omitted Chain of Command, and returned keyboard focus
to its trigger on Escape. Message editing has not yet passed a live check.

## Detailed evidence

- [Chain of Command, recovery, limits, baseline and mixed team](specs/m9-chain-of-command.md)
- [Work artifacts, agent navigation and preview lifecycle](specs/m8d-work-dogfood.md)
- [Evals setup, results and retained-run behavior](evals.md)

The Evals redesign passed 1,825 frontend tests plus its native/layout checks before
this small recovery-copy change. The focused 119-test run covers the latter; these
counts are separate runs, not an additive total.
