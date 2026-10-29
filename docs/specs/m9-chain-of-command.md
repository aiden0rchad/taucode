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
failures before any child launched. The fix has regression coverage and is in the
rebuilt, signed app. Its resumed task is currently waiting for macOS Keychain
approval; no successful end-to-end hierarchy run is claimed yet.

Task: `ef8c2e02-b3c2-460b-8fc0-af864542af23` in the dogfood `work-notes` space.
Acceptance: colonel delegates JSON generation to enlisted; a separate major checks
`outputs/command-summary.json` against `outputs/action-items.csv` with Python.
The later activity-tree/error-display fix has passed checks but still needs the
next rebuild/live check. Do not interrupt the pending Keychain request to replace
the running build.

## Work still required for the full product goal

- Per-conversation mode visibility and switching, rather than settings-only control.
- Durable orchestration state and evidence-backed QA completion gates.
- Global resource/usage budgets, deadlines, cancellation and crash recovery across
  the tree; concurrency is currently per delegator.
- Cost/capability-aware model selection backed by evaluation evidence, including
  actual cloud/local mixed-model dogfood runs.
- Live verification of this mode, agent-tree navigation and recovery after errors.
