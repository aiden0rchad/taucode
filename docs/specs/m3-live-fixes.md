# M3 live-test fixes: approval false positives, approval visibility, sidecar logs

Found in a live test on the user's Mac (Calculator, 12 × 7, clicks only).

## 1. Risky-click detection uses the wrong text

`computer/runner.rs` passes `prediction.plan_text`, the planner's whole reasoning prose, to
`policy::decide`. `policy.rs` then substring-matches words like "confirm", "order", "send",
"delete". Harmless clicks such as Calculator's "=" are flagged whenever the prose says "confirm
the result" or "in order to". The run then waits for approval until wall time and fails with
"Maximum wall time reached".

Fix:
- Decide on the **grounded action's own target description**: the first string argument of
  the planned call, e.g. `agent.click("The = button …", …)`. The sidecar must return it as
  `action.target` (parsed from the AST; it's already available there).
- Match **whole words**, case-insensitive (word-boundary regex): submit, send, buy, purchase,
  pay, checkout, confirm, order, delete, transfer, remove, erase, unsubscribe, publish, post.
- Never match on reflection or plan prose.
- Unit tests with real Agent-S plan prose that contains "in order to" / "confirm the result"
  and must not trigger; and `click("Delete account")` / `click("Send")`, which must.

## 2. Waiting for approval must be visible and attributable

- While a run waits on approval, record a step entry `{ kind: "approval_wait", reason, target }`
  in the run log, show it in the HUD and thread item, and post a macOS notification
  ("taucode needs your approval"). The notification plugin is already a dependency.
- If wall time expires while waiting, the error must say
  `Timed out waiting for approval: <reason> (<target>)`, not "Maximum wall time reached".
- Approval decisions stay UI-only (never on the socket or MCP).

## 3. Sidecar logs and timings

- `computer/sidecar.rs` sends the sidecar's stderr to `/dev/null`. Write it to
  `<app data>/computer/logs/sidecar.log` (rotate at 2 MB, keep 2) and include the last ~20
  lines in run errors that come from the sidecar.
- Record per-step timings in the run log: `predictMs`, `executeMs`, `waitMs`.

## Constraints and verification

Offline; no new dependencies; always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. Same verification command as
`m3-computer-use.md`. Don't run `tauri build`.
