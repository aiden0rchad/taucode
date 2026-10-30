# Evals

Settings → Evals compares engines and provider/model pairs against an offline,
versioned task suite. Settings shows one compact result card per configuration;
**Open results** opens that run in the main area. **Open full view** also opens
the main-area evaluator without requiring a selected run.
New-run setup is grouped into **Models & engines**, **Test tasks**, and **Run limits**.
On first use it selects the configured default engine/model and last composer reasoning
level. Missing or disabled defaults fall back to the first available choice. The draft
then persists across Settings/full view and app relaunches; removed models are not
silently replaced. Spending confirmation is never persisted.
Select engines, models, reasoning, task sets/types, CodeGraph, repeats, concurrency
and optional token/dollar caps. The preview shows test/task counts, the selected
model/engine multiplier, and a token estimate from prior completed runs when available.
**Start run → Confirm start** spends provider tokens.

Pending runtimes install once before any cells are scheduled; the UI and CLI show
`Preparing <engine>…`. A preparation failure saves a failed run without model calls.
Disabled, missing and errored engines are rejected with their name and state.

Progress updates arrive through `eval/progress`. Cancel stops active sessions and
cancels queued tests. The progress summary names finished/running/queued counts;
active cards identify both task and model. Cancelled runs show explicit outcomes,
not pending labels or an empty metrics table.

Results start with one card per configuration: pass rate with a finished-test
count, median tokens, median elapsed time and total reported cost. Unknown cost
stays unknown. **Detailed metrics** expands the sortable comparison with cache
share, first response, tools and errors; its best values are highlighted. Tables
scroll horizontally inside the centered page. Its sticky header and reading
surface stay opaque in glass themes, so underlying controls cannot show through. The task matrix labels passed,
failed, cancelled and queued outcomes. Selecting a result shows check output and
model response, with raw events behind a disclosure. Switching runs clears the
previous transcript/configuration selection.

Run headers show recorded app/engine versions, requested reasoning and team mode.
The engine applies its model's supported reasoning levels; the requested level is
sent on every turn, including check retries. **Compare with a previous run** shows
configuration/task deltas; pass-rate differences use percentage points. Expand a
model card to select a policy role; **Confirm default** writes only that role's
model reference. Engine defaults are unchanged. **Recent runs** separates date,
model names, status and average pass rate rather than concatenating them into one label.

## CLI and RPC

Build the binaries in `apps/taucode/src-tauri`, then run:

```sh
codex_monitor_daemonctl evals run --engines pi,omp --models 'lab/GLM 5.3' --tags smoke --codegraph both --repeats 1
codex_monitor_daemonctl evals run --engines pi --models lab/model --effort low --concurrency 2 --max-total-tokens 100000 --max-cost 2 --json
codex_monitor_daemonctl evals gate --baseline latest-passing --tags smoke --max-pass-drop 0.1 --max-token-increase 0.25
```

The CLI uses the existing daemon address/token and data-directory resolution.
It connects to the running daemon or hosts the shared evaluator in-process when no daemon is reachable.
`--data-dir`, `--listen`, `--token`, and `--daemon-path` retain their normal
daemonctl meanings, before or after the subcommand. Use `--insecure-no-auth` for a
local throwaway daemon without a token. Ctrl-C requests cancellation. `--keep-workdirs` retains all
cell directories. Exit status is nonzero on RPC errors, incomplete runs, or a failed
regression gate. A regular completed run may contain failed cells; use the gate
to enforce thresholds.

The CLI summary includes run ID, elapsed duration and recorded versions, followed
by Pass, Tokens (eff.), Cache, Cost, Tools, 1st token, Wall and Errors for each
engine/model and `cg:on/off`. Missing costs/timings remain `unknown`. Gate output
adds a Regression column. `--json` includes the full run and summary, plus gate
fields when running a gate.

The Tauri command and daemon method share:
`eval_request {action, params}`. Actions: `suites`, `plan`, `start`, `status`,
`cancel`, `runs`, `run`, `delete`, `gate`. Start/plan params contain the plan
directly; run/status/cancel/delete use `{runId}`. Gate takes
`{runId, baseline, tags?, maxPassDrop?, maxTokenIncrease?}` and compares stored runs.
CLI gate starts a fresh candidate using the baseline plan, then compares it.

## Measurements

- Effective input = uncached input + cached input. **Tokens** includes effective
  input plus output, aggregated over all turns of a task, then median per config.
- Cache rate = cached input / effective input, summed across tasks.
- Cost is summed only when every included turn reports a known cost; otherwise
  it is **unknown**. Missing prices are never treated as free.
- Engine-ready time covers thread startup. First-token time measures the first
  agent-message delta from turn submission; absent samples are excluded.
- Wall time includes fixture setup, startup, turns and checks. Tool calls and
  errors come from canonical engine events.
- Usage normalization is shared with diagnostics, but collection never depends
  on diagnostics being enabled.
- Skipped and cancelled cells do not enter summary denominators. Failed checks
  and runtime failures do. Gates require completed runs with the same task,
  engine/model/Codegraph configurations and repeat keys.
- Pass-drop thresholds are absolute (0.1 = ten percentage points); token-increase
  thresholds are relative (0.25 = 25%). A zero-token baseline regresses on any
  positive increase. Equality at the threshold passes.

Budget caps stop scheduling and cancel active cells as soon as observed usage
reaches the cap. Provider reporting latency and concurrent cells can overshoot.
Unknown costs cannot enforce a dollar cap; use a token cap as well.

## Isolation and storage

Every cell gets a separate generated Git repository and an ephemeral `kind: eval`
workspace under `<data>/evals/work/<runId>/<cellId>`. These workspaces never enter
the user-project sidebar or its persistent workspace list. Codegraph settings
apply only to that cell. User settings are unchanged by runs.

Checks and setup use a cleared environment with PATH/LANG/platform essentials and
HOME/TMPDIR pointing at the cell. Commands have timeouts; Unix child process groups
are killed on timeout/cancellation. The engine is requested in full-access mode.
Approval requests are auto-accepted only for explicit paths contained in the
canonical workdir; traversal, external symlinks, opaque shell approval requests,
and requests without verifiable paths are denied and recorded. This approval
filter is not an operating-system sandbox for a full-access engine.

Browser and computer tasks are skipped unless the corresponding feature and
connector are enabled. Compatible engine adapters deliver connectors through the
existing registry. A loopback-only server serves the two static fixture files for
these tasks, and shuts down with the cell.

Results are atomically replaced after progress updates at
`<data>/evals/runs/<runId>.json`, including the plan, runtime/version status,
model IDs, the app's agent policy at run start, usage and transcripts (bounded to 4 MB per cell).
The evaluator inherits this app-wide policy; project-specific overrides do not apply.
Restarted runs retain
partial results and are marked interrupted when opened. Check output retains its
last 2 KB. Default cleanup deletes passing workdirs and retains failed/cancelled
workdirs, capped at the 20 most recent cells. Explicit `keepWorkdirs: true`
disables that cap for the run. Deleting a run deletes its workdirs too.

## Task authoring and offline checks

`apps/taucode/evals/task.schema.json` defines task records. The suite contains six
questions, ten fixes, seven features, three refactors, one browser and one optional
computer task (28 total). Fixtures use Node built-ins (including native TypeScript
stripping for the hard TS tasks) and Python's standard library.

The eight smoke tasks are `ts-bugfix-range`, `ts-bugfix-reservations`,
`ts-feature-chunk`, `py-bugfix-parse`, `py-feature-frequency`, `q-python`,
`py-refactor-total`, and `ts-feature-discount`. The two hard smoke tasks cover
atomic multi-file reservations and proportional discount allocation across two
call sites. The other `hard` tasks cover Python stdin/argument handling and a lazy
TypeScript parser refactor with stable public APIs. Every task's reference check
requires base failure and patched success; distribution assertions enforce the
smoke language/category mix and the four hard tasks.

Edit `evals/generate.py` or `evals/hard_tasks.py` to add a task or fixture, then regenerate:

```sh
python3 apps/taucode/evals/generate.py
cd apps/taucode
npm run test:evals
```

The generator writes small source repos, task JSON, one reference patch per task,
and the embedded catalog used by the Rust runner. Each task needs a deterministic
command/exit check, optional file-content checks, and/or `answerContains` tokens.
Question/browser/computer reference patches add a `.answer` file: the offline
test treats that as the reference final response. The live runner checks the
agent's final message, never that file. These answer patches validate the
deterministic answer check; they do not validate live browser/computer interaction.

The validity test validates every JSON record against the schema, applies each
patch to a fresh temporary repo, and requires base failure plus reference success.
It does not make model calls or drive a browser/computer. Keep source, generated
catalog and patches together. Do not add dependencies or network setup steps.

Rust fake-engine tests cover lifecycle, retry nudges, usage, budgets, cancellation,
persistence, cleanup, approval boundaries and gate thresholds. UI tests cover
confirmation, count preview, events, comparison sorting/highlighting, A/B deltas,
transcripts and policy writes. `?settings=evals` includes completed A/B and live
preview runs in History. Open `?settings=evals&evals=layout` at each of 1024px and
1440px to run real-DOM form, model-picker, results, A/B and progress assertions.
The document's `data-evals-layout` attribute reports `passed` or the failure.
It also checks the New run disclosure alignment and opaque sticky header.
Native acceptance covers completed/cancelled saved runs, draft retention, result
navigation, expandable metrics, and 730px/1260px window widths.

Before merging an engine/model/Codegraph upgrade, save a passing smoke baseline,
run CLI gate with the same matrix, and inspect flagged task transcripts. A missing
runtime, unavailable feature or incomplete run must not be accepted as evidence
that an upgrade passed.

