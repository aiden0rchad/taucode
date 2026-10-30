# Chain of Command

Taucode supports a single model and, in Code, a Chain of Command selected from the
same provider registry. Cloud and local models use identical role configuration.
Chat and Work use a single model when inheriting a Chain of Command policy, without
changing the saved roles or models. Custom delegation remains available separately.

## Current contract

Settings → Agents → Defaults exposes three choices:

- **Single model:** disables subagent delegation while keeping role/model choices.
- **Chain of Command:** uses the hierarchy below on Pi-RPC engines (Pi and OMP).
- **Custom delegation:** preserves existing arbitrary roles and delegation behavior.

| Role | Responsibility | May delegate to |
| --- | --- | --- |
| Commander | Conversation model; goal, constraints, acceptance criteria, integration | Colonel, major, enlisted |
| Colonel | Break work into bounded tasks and integrate results | Major, enlisted |
| Major | Independently verify artifacts and report evidence | None |
| Enlisted | Implement a bounded task and run relevant checks | None |

Team models → **Add role** offers Researcher, Designer, Frontend engineer,
Backend engineer, Test engineer and Documentation presets. Each adds a separate
model selector and editable description, tool access and instructions in Roles.
Repeated presets receive unique names (for example, Researcher 2). Added roles
can be removed; the three core ranks cannot. Additions/removals stay in the draft
until Save changes; Cancel discards them. Roles uses a selector for larger teams.

Specialists carry an optional `commandRank` (`colonel`, `major` or `enlisted`).
Test engineer is a major; the other presets are enlisted. Researcher starts with
read-only tools. Commanders and colonels can delegate to these specialists by name,
using the same hierarchy, model restrictions, shared budget and access rules.
Core names retain their canonical rank, and saved policies cannot override it or
assign a child the commander role. Legacy custom roles without a rank stay outside
the command tree; switching modes retains their settings.

The hierarchy and role model assignments are enforced before child processes launch.
In Chain of Command, a tool call cannot override the configured role model. Model
selection is role → default subagent model → parent model. Only enabled registry
models are usable; configured allow-lists also apply. A cheaper/local role must be
selected explicitly; inheriting does not promise a cheaper model.

Switching modes preserves custom roles. The first Chain of Command selection adds
missing command roles and sets depth to at least two. Default engine selection moves
to Pi when the current engine cannot run this mode. Unsupported engines reject new
Chain of Command turns with an actionable error; history and interruption remain
available. Claude's Single model setting removes Task/Agent tools and applies the
same restriction to permission requests, with a runtime refresh when policy changes.

## Limits and access

- Chain of Command's per-turn limit is shared across the entire tree using atomic
  reservations in a private managed directory. Descendants cannot start a fresh
  budget. Each top-level turn gets a new budget; end/shutdown removes its directory.
- Chain of Command concurrency limits active child model work across the whole
  tree for one root turn; the commander is excluded. Coordinators lend their slot
  while awaiting delegation, then reacquire one before resuming. This permits
  nested work even at a limit of one. Custom delegation retains per-delegator limits.
  Atomic, uniquely owned slot leases prevent stale cleanup from freeing another
  worker's slot. Exited owners are reclaimed; a missing budget aborts a coordinator
  instead of letting it resume without a slot.
- On POSIX systems, Stop snapshots the delegation's descendants before signalling
  them, including shell tools in separate process groups. Only groups whose leader
  belongs to that tree receive a group signal. Forced cleanup runs after a one-second
  grace period, even if the coordinator exits first, before releasing its concurrency
  slot. Queued tasks do not start after Stop. Failed process inspection reports a
  diagnostic and falls back to the known child/group.
- Progress arriving after a tool finishes cannot reopen the activity indicator or
  replace its final state. Final result frames still reconcile usage.
- Depth remains enforced. Majors and enlisted cannot delegate even if depth remains.
- Root and child tools retain the thread's access restrictions. Full-access
  children can run ordinary validation commands. Read-only roles and protected
  writes remain restricted; non-interactive children cannot approve protected work.
- Child processes explicitly inherit the parent's current reasoning level through
  the Pi/OMP CLI, including Low and Off. Each engine handles the chosen model's
  supported levels; a child no longer silently starts with its own default.
- Empty or ambiguous delegation requests fail before launching processes. Empty
  arrays/nulls in unused optional modes are accepted for strict-schema providers.
  The activity tree uses the same nonempty-mode selection and preserves actual
  delegation errors for display when a failed row is expanded.

Commander/colonel instructions reserve independent QA. Code turns that use tools
now require a major's evidence-backed report before receiving a successful status.
Chat, Work, custom delegation, and replies without tools keep their existing behavior.

### Evidence-backed QA

Delegations accept optional `acceptanceCriteria` (1–20 unique strings); when omitted,
the assigned task is the criterion. Major-rank agents receive `report_verification`.
Each verdict cites evidence IDs generated from that major's actual tool results.
Empty or failed results cannot support a passing verdict, and write/edit responses
are not verification evidence. Read-only majors can report without gaining shell or
write tools. Reports retain tool names, IDs, bounded inputs/output, verdicts, notes,
and SHA-256 hashes of reviewed project files (up to 16 MiB per file).

Files observed through `read` are included automatically. Changes between reading
and reporting, or between reporting and root completion, invalidate that evidence.
Later implementation or root commands/edits require a fresh review; review running
in parallel with implementation does not qualify. A subsequent check can replace
an earlier failed verdict for the same criterion. Missing, malformed, failed, or
stale QA leaves the turn failed with an explicit “Verification incomplete” message.
Cancellation keeps its interrupted status. The summary is persisted outside model
context and uses the same status when the conversation is reopened.

This gate establishes that QA used tools and recorded verdicts against unchanged
reviewed files. It cannot prove that a model selected sufficient tests or acceptance
criteria. It does not automatically spend another model turn trying to repair QA.

## Verification

- Pi tests run the actual extension in nested fake CLI processes, proving shared
  budget enforcement, local/cloud routing, denied hierarchy edges/model overrides,
  missing-budget failures, Single model, turn reset and cleanup without provider calls.
- Concurrency stress tests use two colonels with overlapping delegation calls at
  limits of one and two. They cover slot lending/reacquisition, failed children,
  aborted queued claims, stale releases, dead-owner reclamation and aborting when
  reacquisition fails. The old implementation exceeds the limit (three active
  children with a limit of two).
- Frontend tests exercise mode selection, model assignment, saving, restoration,
  validation and preservation of legacy roles.
- Rust tests cover mode serialization/overrides, policy file delivery, unsupported
  engine rejection before launch, and Claude delegation disabling/runtime refresh.

The extension suite also starts real subprocesses to verify cancellation: a
coordinator exits on SIGTERM while its grouped or detached descendant ignores it.
Root and nested cancellation must stop those descendants without killing the parent,
discard queued work, and release the concurrency slot for the next turn. Rust replay
tests cover late subagent progress after normal completion and interruption, while
allowing new-turn progress. These regressions fail against the old behavior.
They do not establish Windows process-tree cleanup or crash recovery.

The shared concurrency build passed a live signed macOS OMP check in
`command-stop-qfgg_8ap`: one colonel submitted enlisted and major README inspections
in a parallel call with concurrency set to one. Both completed and the commander
returned `SHARED_SLOT_OK`. A 100 ms filesystem trace observed at most one held slot,
ownership passing through the coordinator and both children, then budget cleanup.
This sampled trace supplements the deterministic process stress tests. All roles
used GPT 6.1 Sol (Account 2), with Full access and Low selected for the commander.
The run also exposed missing explicit child reasoning inheritance, fixed in the
follow-up below. Validation: 1,794 frontend tests, 23 extension tests, 42 Pi Rust tests
(one live test ignored), typechecks, changed-file lint, cargo check, build and signing.

After the reasoning fix, a second signed build ran two colonels, each delegating to
an enlisted agent and an independent major, with the shared cap set to two. All six
subprocesses used GPT 6.1 Sol (Account 2) and `--thinking low`; all six completed,
and the UI returned `SHARED_TWO_OK`. The sampled trace peaked at two held slots and
ended with no turn budget. All 12 fixture files were unchanged. Regression tests
prove reasoning reaches grandchildren and follows a change from Low to Off on the
next turn; they fail without the fix. The 23 extension tests, engine typecheck,
native build and signature verification passed again.

Live Stop verification on that build interrupted a colonel with two enlisted
agents running foreground Node timers. Both observed worker PIDs and their agent
parents exited, the budget disappeared, and a new major delegation completed with
`STOP_RECOVERY_OK`. The original team models and concurrency of four were restored
after testing; the test conversation retains Account 2 Sol, Low and Full access.

Live macOS verification used the signed app, OMP, GPT 6.1 Sol and Full access in a
disposable Code workspace. A colonel delegated a foreground Node worker through an
enlisted agent. The worker ignored SIGTERM and ran in a separate process group.
Stop removed the process and stopped its heartbeat before the 120-second safety
timeout; the composer remained idle after delayed cleanup. A subsequent prompt in
the same conversation returned `Ready`. Validation: 18 extension tests, 42 Pi Rust
tests (one unrelated live OMP test ignored), typecheck, changed-file lint, cargo
check, app build and signature verification passed.

## Composer control

The Code model/reasoning picker exposes Chain of Command through a compact 20px
five-star rank toggle with a 32px click target. After a successful activation, stars
light sequentially, followed by a steady soft aura. Opening an already enabled
picker does not replay the animation. Reduced motion and disabled reasoning effects
skip the animation. The adjacent chevron appears only while enabled and opens a
focused editor for the same policy fields exposed by Settings. Off selects Single model
without discarding role choices. Code saves the effective project policy; Chat
and Work do not expose the controls. Unsupported engines cannot enable Chain from the
picker. Saves are acknowledged before the toggle changes, and failures remain
visible. The editor loads on demand and supports keyboard focus and Escape.
Its Team models tab connects the conversation commander to compact,
color-coded role rows, with an inline preset picker for adding specialists.
Limits groups concurrency, turn budget and depth, with
model defaults/restrictions under a disclosure. Roles edits one specialist at a
time, keeping draft values while switching roles or tabs. The fixed header and
footer frame a scrollable body on smaller windows; light and dark themes use the
shared design-system controls and tokens. Save stays disabled until values change,
and invalid limits show an inline error without committing the draft.
Changing tabs starts the new panel at the top without losing role drafts.
Regression tests cover mode visibility, successful activation, failed saves, role
preservation, keyboard role navigation, cancelled drafts, limit validation, and
Chat/Work runtime normalization without changing saved policy.
Preset regressions cover duplicates, independent models, guidance, removal and
cancel/save persistence. Extension tests run a specialist child with read-only
tools, configured model and instructions, and enforce its rank and shared budget.
Rust tests cover rank persistence, policy-file delivery and invalid core overrides.

The rebuilt, signed app was verified live in Code with GPT 6.1 Sol: the smaller
icon, sequential star illumination, settled glow, keyboard toggle, conditional
chevron, editor opening/Escape dismissal, and preserved team models all passed.
Chat and Work pickers showed no Chain controls. The original enabled Code policy
was restored, and Work retained Full access. Validation: 222 frontend files /
1,732 tests; 41 Pi Rust tests (one unrelated live OMP test ignored); typecheck,
changed-file lint, cargo check, build and signature verification passed.

The redesigned editor was verified in the signed app with the saved GPT 6.1 Sol
team: all three tabs, model popup dismissal, keyboard role switching, draft
retention/cancellation, invalid limits, and save persistence after relaunch.
The original policy, window dimensions and 100% zoom were restored after testing
at 960×650 and enlarged text. Light/dark previews passed. Validation: 1,733
frontend tests, final focused regressions, typecheck, changed-file lint, app build
and signature verification passed.

Preset roles were verified in the signed macOS app: adding roles, independent
model choices, custom instructions surviving relaunch, duplicate names, cancelled
removals, saved removals and scrolling at 100–200% zoom. A fresh Code conversation
on OMP used GPT 6.1 Sol and Full access to delegate a README inspection to
Researcher and independent verification to Test engineer; both returned the expected
evidence. Temporary roles were removed and the original team and zoom restored.
Validation: 1,735 frontend tests, 20 extension tests, 55 provider/Pi Rust tests
(one unrelated live OMP test ignored), typecheck, changed-file lint, cargo check,
app build and signature verification passed.

## OMP nested activity verification

OMP virtual subagent tools return results under `details.xdev.inner` through
`write`/`edit`. The translator and extension now consume that wrapper as well as
Pi's direct results. Nested updates retain stable IDs, models, states, usage and
individual transcripts; final tool messages replace progress snapshots. Parent
transcripts omit embedded child transcripts, and repeated completion events do
not double-count usage. Agent cards show two-line assignment/result previews;
opening a card exposes the full transcript and tool evidence.

Regressions reproduced the missing progress before the fix, then passed for both
virtual transports, nested success/failure, final-message replacement, transcript
separation, replay and usage deduplication. Validation: 1,794 frontend tests,
24 extension tests, 43 Pi Rust tests (one unrelated live test ignored), both
TypeScript checks, changed-file lint, cargo check, signed debug build.

Live verification on 2026-10-29 used OMP, GPT 6.1 Sol (Account 2), Low reasoning
and Full access in `command-stop-qfgg_8ap`. Previously stuck completed agents now
restore with their nested children, models, usage and readable tool transcripts.
A fresh colonel delegated to an enlisted worker and independent major; both
appeared before the colonel completed. The worker's foreground eight-second
timer returned `NESTED_PROGRESS_DONE`; all three agents completed and the parent
returned `NESTED_PROGRESS_OK`. Relaunch restored the new tree and selected model.
Original project team models were restored; the fixture content was unchanged
(CodeGraph's own database/log/process files changed). No Keychain prompt appeared.
Screenshots: `taucode-agents-stuck-before.png`,
`taucode-agents-restored-after.png`, `taucode-agents-progress-live.png`, and
`taucode-agents-completed-live.png` in the task's visualization directory.

## Stop and timing recovery

Subagent results now store actual start/end times. Queued work has no invented
start time; older results without timestamps show elapsed time as unavailable.
On cancellation, the root extension writes one branch-aware custom session entry
outside model context. This preserves the final available tree, usage and partial
transcripts when OMP replaces its tool result with a generic abort error.
Reopening a stopped run restores aborted descendants while retaining completed
children and their original duration. Rewound branches exclude abandoned results.

Stop is recorded before the backend cancellation request, so the live tree also
settles as aborted. Delegation rows and groups display a neutral `Stopped` label;
an actual child failure still displays as a failure. Late progress cannot reopen
stopped agents.

Validation: 1,795 frontend tests, 24 extension tests, 45 Pi Rust tests (one
unrelated live test ignored), both TypeScript checks, changed-file lint, cargo
check, signed debug build. Regressions cover real subprocess cleanup, timestamp
preservation, never-started queued work, cancellation entry ordering, branch
isolation, usage/transcripts and neutral stop labels without hiding real failures.

Live verification used OMP, GPT 6.1 Sol (Account 2), Low reasoning and Full access.
A colonel delegated a foreground 60-second timer to an enlisted worker and an
independent README check to a major. Stop preserved the completed major, marked
the running agents aborted and left no timer or child-agent processes. Relaunch
restored elapsed time and partial tool evidence from the saved cancellation.
A fresh major delegation then returned `TIMING_RECOVERY_OK`; another relaunch
preserved its seven-second duration, model and README evidence. Original project
team models were restored. No Keychain prompt appeared during these checks.
Screenshots: `taucode-stopped-team-restored.png` and
`taucode-agents-stopped-live.png`, plus `taucode-agents-recovery-restored.png`,
in the task's visualization directory.

Known limits: old cancelled runs cannot recover evidence never saved by earlier
builds. Pricing corrections are documented below.

## Active delegation crash recovery

Pi/OMP root delegations keep one private, atomically replaced recovery file per
tool call beside the session. Streaming updates are coalesced to at most one write
per second; final evidence is flushed immediately. Normal turn completion removes
its temporary snapshots. Interrupted snapshots remain available across later runs.
They are outside model context and do not append a full tree for every update.

History restores checkpoints only for calls lacking saved agent results on the
selected branch. Durable agent results take precedence; malformed or mismatched checkpoints are
ignored. Unfinished nodes become aborted, completed children retain their results,
and exact end times remain unavailable where no completion was observed. Recovery
through Pi/OMP thread resume does not request credentials, start an engine or rerun a command. POSIX child
engines detect a lost parent and abort their owned nested work.

Automated verification covers atomic replacement, coalescing, private permissions,
final flush/cleanup, a killed writer, a killed commander with detached descendants,
branch isolation, final-result precedence, malformed snapshots, separate follow-up
turns and offline Pi/OMP resume without credential access. Frontend validation:
1,796 tests plus typecheck and changed-file lint. Pi backend: 46 tests passed,
one unrelated live test ignored; cargo check passed. All 28 extension tests pass,
including killed-commander cleanup under Node and Bun.

In the signed native build, an OMP commander was killed while one Account 2 Sol
major was running a foreground timer and another had completed. The checkpoint
retained both states and their messages; the test agents and timer exited. The
remaining CodeGraph helpers also exited. Relaunch did not restart the test engine.
Native restoration verified the stopped/completed states, unavailable end time
for the interrupted major, and the saved README transcript. Screenshots:
`taucode-crash-agents-restored.png` and `taucode-crash-transcript-restored.png` in
the task's visualization directory.

A subsequent full app force-quit stopped the test engine, agents and foreground
timer. It exposed OMP's shutdown behavior: a generic abort tool result was saved
without agent details and hid the richer checkpoint. The precedence rule now
keeps recovery evidence in that case, with a regression test covering the retained
completed child and partial transcript. The signed native build subsequently
restored Turn 10 with the interrupted major, completed major, unavailable end time
and actual README tool output. A fresh Account 2 Sol/Low delegation completed with
`CRASH_RECOVERY_OK`; the two previous crash checkpoints remained and the new turn's
checkpoint was removed. Original team models and limits (4 concurrent, 8 per turn,
depth 2) were restored after testing. Screenshots: `taucode-forcequit-recovered-live.png`,
`taucode-forcequit-transcript-live.png`, `taucode-crash-followup-live.png`.
A sampled stack traced the startup prompt to Codex thread
listing (`prepare_codex`), separate from offline Pi/OMP thread resume.
The last second of unflushed progress may be unavailable after an abrupt kill.

## Root response and tool recovery

Pi/OMP root responses and ordinary tool progress now use private, atomically
replaced checkpoints alongside the session. Recovery restores text/thinking and
actual partial output for display only. It does not add unfinished output to model
context, replay tool calls or restart execution. A durable successful result wins;
an error result retains missing partial output alongside its actual error. Branch
changes, explicit rewinds and later saved responses suppress obsolete response
snapshots. Follow-up turns stay separate, and normal successful turns remove their
own checkpoints. Interrupted records remain available after later successful turns.

Validation: 546 backend tests passed (two explicit live tests ignored), 31 Node
extension tests, four Bun recovery tests, typecheck and changed-file lint. The real
installed OMP runtime also passed an offline transport test that killed the engine
after its first streamed text was checkpointed, recovered that text outside model
context and completed a subsequent follow-up. Regression tests cover ordinary
tool output, generic abort results, branch identity, final-result precedence,
filtering unexecuted tool calls and killed writers.

The first signed native test force-quit the app during an Account 2 Sol/Low
foreground timer. Its engine, browser/CodeGraph helpers and timer exited; reopening
the conversation restored the partial output. This exposed OMP's cancellation
snapshot containing the same output as its durable tool result. Recovery now adds
only the missing cancellation prefix. An OMP exit with pending calls also marks
the turn interrupted before a later follow-up, and does not invent an exit code.
The targeted Pi/OMP suite passed 49 tests (one explicit live test ignored),
including display of the requested command instead of the supervision wrapper.
The signed build restored the cancellation output once and completed a native
Account 2 Sol/Low follow-up with `ROOT_RECOVERY_OK`. A separate abrupt engine kill
also restored `ROOT_HARD_READY` from its checkpoint.

That abrupt engine kill exposed an orphaned foreground timer, which was manually
stopped. Pi/OMP shell calls now use a POSIX supervisor that watches the owning
engine and parent, terminates the command's process group and escalates after one
second. OMP's native runner uses a direct executable invocation; Pi preserves its
shell. Stopped/exited engines are reaped even when diagnostics are disabled.
Policy approval still runs against the original command before supervision.

Five supervisor tests pass under Node and Bun, covering literal input/environment,
exit status, cancellation, engine death, pipelines and TERM-resistant commands.
The actual installed OMP runtime passes an offline tool-call regression: execute
the supervised command, kill the engine after its output checkpoint, verify the
worker exits, restore its output, complete a follow-up, then start another command
and verify normal Stop cleans it up. Earlier full extension validation passed 35
tests; the additional OMP launcher case passed in the targeted five-test suite.

The final native supervisor recheck is pending macOS Keychain approval. Startup
and several changed-build executions were prompt-free, but the subsequent build
blocked in `SecKeychainFindGenericPassword`, so cross-build credential trust is
still unresolved. The queued timer was cancelled while waiting. Screenshots:
`taucode-root-tool-recovered-live.png` in the task's visualization directory.
The final signed app is open on the disposable project's recovery conversation,
with only a text-only `ROOT_READY` request queued. Chain of Command is temporarily
off for `command-stop-qfgg_8ap` to isolate root execution; restore it after the
remaining native supervisor checks. Team models and limits were not changed.

Cross-engine recovery, Windows process cleanup and power-loss durability remain
unverified. Checkpoints coalesce to one write per second; they are not a guarantee
that the final streamed token survives abrupt termination.
The supervisor covers the foreground process group; deliberately detached child
groups and killing the supervisor itself are not covered by these checks.

## Credential-free history loading

Codex `thread/list` and `thread/read` now start a history-only backend without
reading provider keys, running command authentication or loading MCP connectors.
Execution replaces it with a fully configured backend and resumes saved threads
before sending new turns. Later history reads reuse the authenticated backend.
Credential or policy changes still wait for active turns to finish; this does not
change model access, approval policies or how credentials are stored.

Validation: 545 backend tests passed (two explicit live tests ignored), typecheck
and cargo check. The installed Codex CLI also passed an explicit offline history
test with command authentication set to fail if invoked. Regression tests cover
no Keychain calls for history, credential promotion, thread resume before a new
turn, and no downgrade after authentication. The debug app rebuilt and passed
signature verification with the same designated requirement as the previous
build. Native startup loaded saved conversations without waiting for credentials.
Raw subagent transcript reads still load credentials for redaction. This removes
an unnecessary startup credential read; it does not yet
establish Keychain trust across rebuilds when execution needs a credential.

## Unknown model costs

Unpriced custom-model catalog zeros no longer imply free usage. Delegated results
retain token counts, report unavailable total cost when any response is unpriced,
and keep a separate known subtotal. A zero backed by a nonzero pricing table is
distinguished from a placeholder zero. This does not infer free pricing for an
all-zero local/custom catalog.

Nested agent totals and conversation usage preserve uncertainty. Later complete
snapshots can resolve a delegation's cost without counting usage twice. Evaluation
and diagnostic normalization cannot replace an explicitly unknown total with the
last paid response. Historical placeholder zeros also display as unavailable.

Validation: 1,797 frontend tests, typecheck and changed-file lint; 47 Pi backend
tests passed (one unrelated live test ignored), cargo check; all 29 extension tests;
all 19 diagnostics tests. Native verification showed “cost unavailable” on both
historical crash records and the newly completed delegation. This establishes
honest missing-price handling, not measured cloud/local quality or billing accuracy.

## Work still required for the full product goal

The evidence gate passed 40 extension tests and 50 Pi/OMP backend tests (one
unrelated live test ignored). A loopback transport fixture drives the installed
Pi and OMP runtimes through missing QA, passing QA, and failing QA, then reloads
each conversation and checks the same final status. It also exposed and fixed
Pi hiding the reporting tool from read-only majors. The new verification tests
pass in Node and Bun; they cover stale files, invented/failed evidence, retries,
parallel implementation, nested virtual tools, and cancellation. A recovered
provider error cannot bypass QA. Engine TypeScript checking and lint for the new
helper/tests pass. Existing lint findings in the semaphore loop and slot-cleanup
`finally` block are unchanged.

These are deterministic runtime tests, not a live GPT evaluation. Desktop QA is
still pending: computer use reports the Mac locked, and the previous running
build was waiting for Keychain access. The disposable `command-stop-qfgg_8ap`
project still has Chain of Command temporarily off for the earlier foreground
command crash test. Restore it after that test; use GPT 6.1 Sol (Account 2), Low,
and Full access for the live acceptance run. Do not claim the recurring Keychain
prompt is resolved.

The browser smoke pass created two empty tabs, navigated to Example Domain, and
returned focus to the composer. Narrow panes at 120% UI zoom collapsed the URL
field; the toolbar now puts the address on its own row below 400 CSS pixels. The
signed build showed the usable address and all controls at that width. Frontend
validation: 1,797 tests, typecheck, script syntax and changed-file lint passed.
The WebKit geometry check now covers 220/320/480px panes, but that script has not
been run for this change. A foreground native recheck verified expanded and narrow
page alignment, zoom from native-page focus, correct selected-tab accessibility,
and closing/reopening the last empty tab. Earlier stale rendering disappeared
when the user made the window visible after direct input had reported no available
window. Zoom was restored to 100% and test tabs closed. Screenshots:
`taucode-browser-narrow-toolbar-after.png`, `taucode-browser-wide-toolbar-after.png`.
This smoke pass does not establish the cause of the earlier empty-tab failure.

- Per-conversation policy overrides beyond the current project defaults.
- Further durable orchestration state; evidence-backed QA gates are implemented,
  with signed-app acceptance still pending.
- Resource/usage budgets across conversations, deadlines and whole-tree crash
  recovery. Shared turn concurrency and POSIX Stop cleanup are implemented.
- Cost/capability-aware model selection backed by evaluation evidence, including
  actual cloud/local mixed-model dogfood runs.
- Work agent-tree navigation for custom delegation and recovery after errors;
  Work currently renders delegated results as expandable tool rows rather than the
  full interactive tree. Chain of Command remains Code-only.
