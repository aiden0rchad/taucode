# Evals

Settings → Evals compares engines and provider/model pairs against an offline,
versioned task suite. **Open full view** opens the same page in the main area.
Select engines, models, tags/categories, Codegraph, repeats, concurrency and optional
token/dollar caps. The preview counts the matrix and estimates tokens from prior
completed runs. **Start run → Confirm start** spends provider tokens.

Pending runtimes install once before any cells are scheduled; the UI and CLI show
“Preparing <engine>…”. A preparation failure saves a failed run without model calls.
Disabled, missing and errored engines are rejected with their name and state.

Progress updates arrive through `eval/progress`. Cancel stops active sessions and
cancels queued cells. Results show sortable configuration summaries, highlighted
best values, individual check output, errors, and saved read-only transcripts.
Select a history run as B and another as A to see configuration and task deltas.
Expand a configuration to select a policy role; **Confirm default** writes only
that role's model reference. Engine defaults are unchanged.

## CLI and RPC

Build the binaries in `apps/taucode/src-tauri`, then run:

```sh
codex_monitor_daemonctl evals run --engines pi,omp --models 'lab/GLM 5.3' --tags smoke --codegraph both --repeats 1
codex_monitor_daemonctl evals run --engines pi --models lab/model --concurrency 2 --max-total-tokens 100000 --max-cost 2 --json
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
model IDs, usage and transcripts (bounded to 4 MB per cell). Restarted runs retain
partial results and are marked interrupted when opened. Check output retains its
last 2 KB. Default cleanup deletes passing workdirs and retains failed/cancelled
workdirs, capped at the 20 most recent cells. Explicit `keepWorkdirs: true`
disables that cap for the run. Deleting a run deletes its workdirs too.

## Task authoring and offline checks

`apps/taucode/evals/task.schema.json` defines task records. The suite contains six
questions, eight fixes, six features, two refactors, one browser and one optional
computer task; eight tasks have the smoke tag. Fixtures use only Node built-ins
and Python's standard library.

Edit `evals/generate.py` to add a task or fixture, then regenerate:

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

Before merging an engine/model/Codegraph upgrade, save a passing smoke baseline,
run CLI gate with the same matrix, and inspect flagged task transcripts. A missing
runtime, unavailable feature or incomplete run must not be accepted as evidence
that an upgrade passed.

