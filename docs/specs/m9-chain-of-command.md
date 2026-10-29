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

The extension suite also starts real subprocesses to verify cancellation: a
coordinator exits on SIGTERM while its grouped or detached descendant ignores it.
Root and nested cancellation must stop those descendants without killing the parent,
discard queued work, and release the concurrency slot for the next turn. Rust replay
tests cover late subagent progress after normal completion and interruption, while
allowing new-turn progress. These regressions fail against the old behavior.
They do not establish Windows process-tree cleanup or crash recovery.

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
skip the animation. The adjacent chevron appears only while enabled and opens the same
team models, limits and role editor used by Settings. Off selects Single model
without discarding role choices. Code saves the effective project policy; Chat
and Work do not expose the controls. Unsupported engines cannot enable Chain from the
picker. Saves are acknowledged before the toggle changes, and failures remain
visible. The editor loads on demand and supports keyboard focus and Escape.
Regression tests cover mode visibility, successful activation, failed saves, role
preservation, and Chat/Work runtime normalization without changing saved policy.

The rebuilt, signed app was verified live in Code with GPT 6.1 Sol: the smaller
icon, sequential star illumination, settled glow, keyboard toggle, conditional
chevron, editor opening/Escape dismissal, and preserved team models all passed.
Chat and Work pickers showed no Chain controls. The original enabled Code policy
was restored, and Work retained Full access. Validation: 222 frontend files /
1,732 tests; 41 Pi Rust tests (one unrelated live OMP test ignored); typecheck,
changed-file lint, cargo check, build and signature verification passed.

## Work still required for the full product goal

- Per-conversation policy overrides beyond the current project defaults.
- Durable orchestration state and evidence-backed QA completion gates.
- Global resource/usage budgets, deadlines, cancellation and crash recovery across
  the tree; concurrency is currently per delegator.
- Cost/capability-aware model selection backed by evaluation evidence, including
  actual cloud/local mixed-model dogfood runs.
- Work agent-tree navigation and recovery after errors; Work currently renders
  delegated results as expandable tool rows rather than the full interactive tree.
