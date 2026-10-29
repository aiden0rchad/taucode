# Chain of Command

Taucode supports a single model and a team of models selected from the same provider
registry. Cloud and local models use identical role configuration.

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
- Concurrency limits each delegator's immediate children, not the whole tree.
- Depth remains enforced. Majors and enlisted cannot delegate even if depth remains.
- Root and child tools retain the thread's access restrictions. Full-access Work
  children can run ordinary validation commands. Read-only roles and protected
  writes remain restricted; non-interactive children cannot approve protected work.
- Empty or ambiguous delegation requests fail before launching processes. Empty
  arrays/nulls in unused optional modes are accepted for strict-schema providers.
  The activity tree uses the same nonempty-mode selection and preserves actual
  delegation errors for display when a failed row is expanded.

Commander/colonel instructions reserve independent QA and require reporting missing
verification. This is currently an instruction contract, not a host-enforced release
gate. The app must not represent unperformed QA as verified.

## Verification

- Pi tests run the actual extension in nested fake CLI processes, proving shared
  budget enforcement, local/cloud routing, denied hierarchy edges/model overrides,
  missing-budget failures, Single model, turn reset and cleanup without provider calls.
- Frontend tests exercise mode selection, model assignment, saving, restoration,
  validation and preservation of legacy roles.
- Rust tests cover mode serialization/overrides, policy file delivery, unsupported
  engine rejection before launch, and Claude delegation disabling/runtime refresh.

October 28 checks: typecheck and changed-file ESLint passed; full frontend suite
passed (220 files, 1,723 tests). Pi extension tests passed (15); Rust Pi tests passed
(40, one ignored), as did ACP (11), provider (12), and engine routing tests (7).
`cargo check` passed with existing warnings.

## Live dogfood status

Saved through Settings: Chain of Command, GPT 6.1 Sol as default/commander and
colonel/major, GPT 6 Luna as enlisted, Full access for the Work test. The existing
OMP default engine remains selected; Work runs Pi.

The first live smoke test exposed strict-schema calls containing a valid single
agent plus empty `tasks` and `chain` arrays. This caused repeated validation
failures before any child launched. The fix has regression coverage and was
verified in the rebuilt, signed app after the user approved Keychain access.

Task `ef8c2e02-b3c2-460b-8fc0-af864542af23` completed at 8:52 p.m. PDT October 28:
Sol commander → one Sol colonel → one Luna enlisted worker, then a separate Sol
major. The worker created `outputs/command-summary.json`; the major ran independent
Python QA (exit 0). Host session records show the nested roles/models and successful
tool results. The gateway's internal model routing is outside that evidence.

An independent host check confirmed the JSON equals the CSV: 7 items; Lee, Priya,
and Sam each 1, Unassigned 4; P1 1, P2 5, P3 1. SHA-256 values for the CSV, existing
HTML dashboard and weekly brief match the agents' recorded before values. Work
shows done, all three plan steps checked, and one output. Settings confirms the
three team model choices persisted after restart.

The late credential approval exposed another defect: Work held its global state
lock during engine startup. Startup now runs in cancellable jobs with visible
starting/stopping states, and unsent messages persist for retry. Credential reads
run on a blocking worker without runtime file writes; cancellation drops their
result and cannot start the agent later. Transcript reads release the Work lock.
Checkpoint preparation finishes before cancellation cleanup, and the space remains
occupied until cleanup ends. Regression coverage deliberately blocks credentials
while checking list/read/stop responsiveness and absence of a late process launch.
Live startup recovery check (`b8ac0e44-ce8e-4300-b138-fa899ac0137b`): stopped
while macOS awaited credentials at 9:02 p.m.; after approval, the task remained
stopped and its session contained only the header (no submitted message). A manual
retry launched, submitted exactly the saved message, and reached the gateway.
LiteLLM returned HTTP 502 after runtime retries, so a successful model response on
this recovery test remains unverified. The earlier full hierarchy run succeeded.

Pending drafts now store only the user's text, without internal attachment headings.
Validation: 220 frontend files / 1,724 tests; typecheck; changed frontend lint;
7 Work and 40 Pi Rust tests (1 ignored); cargo check. The draft cleanup also passed
the Work suite and cargo check after the live startup check.

## Composer control

The model/reasoning picker now exposes Chain of Command through a five-star
rank toggle. The adjacent chevron appears only while enabled and opens the same
team models, limits and role editor used by Settings. Off selects Single model
without discarding role choices. Code saves the effective project policy; Chat
and Work save the app policy. Unsupported engines cannot enable Chain from the
picker. Saves are acknowledged before the toggle changes, and failures remain
visible. The editor loads on demand and supports keyboard focus and Escape.
Live checks confirmed controls in Work, Chat and Code, the conditional chevron,
and saved limit edits. The original policy was restored byte-equivalently after
JSON normalization. Full frontend suite: 221 files / 1,727 tests passed; the final
compact editor also passed 13 focused tests, typecheck and lint.

## Work still required for the full product goal

- Per-conversation mode visibility and switching, rather than settings-only control.
- Durable orchestration state and evidence-backed QA completion gates.
- Global resource/usage budgets, deadlines, cancellation and crash recovery across
  the tree; concurrency is currently per delegator.
- Cost/capability-aware model selection backed by evaluation evidence, including
  actual cloud/local mixed-model dogfood runs.
- Work agent-tree navigation and recovery after errors; Work currently renders
  delegated results as expandable tool rows rather than the full interactive tree.
