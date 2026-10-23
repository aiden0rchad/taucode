# M1: port the CodexMonitor base (`apps/taucode`) to codex app-server 0.164

## Goal

`apps/taucode` is CodexMonitor (MIT, Tauri 2 + React) imported as taucode's app base. It was
last updated upstream in March 2026. Make it work correctly against **codex-cli 0.164**, the
version taucode pins, and rename the product to taucode so it doesn't clash with an installed
CodexMonitor.

Done means:
- every protocol message in the recorded 0.164 fixtures is handled or deliberately ignored
- new item types render
- thread history loads with the paginated APIs instead of deprecated full hydration
- approval UI clears when the server resolves a request
- the verification command passes

## Ground truth

1. **Generated protocol types** for 0.164, including experimental methods:
   `/Users/rolandleyco/taucode-m1/packages/protocol/src` (`ClientRequest.ts`,
   `ServerNotification.ts`, `ServerRequest.ts`, `v2/*.ts`). These are authoritative for
   every method name and payload shape. ~620 files: look things up with codegraph
   (`codegraph explore "<names>"`), never by listing the folder.
2. **Recorded real traffic** in `apps/taucode/src-tauri/tests/fixtures/codex-0.164/*.jsonl`.
   Each line is `{"dir": "c2s"|"s2c", "msg": <JSON-RPC message>}`, recorded from a real
   0.164 app-server, with personal data scrubbed.

| Fixture | Covers |
|---|---|
| `lifecycle.jsonl` | initialize with `experimentalApi`, `model/list`, `collaborationMode/list`, `skills/list`, `account/read`, `config/read` (result redacted), `thread/start` with `approvalPolicy: "untrusted"`, a turn with `item/fileChange/requestApproval` and `item/commandExecution/requestApproval` + `serverRequest/resolved`, `turn/diff/updated`, `thread/status/changed` with `activeFlags`, compaction (`contextCompaction` item), `thread/name/set`, `thread/fork`, `thread/list`, `thread/read`, `thread/turns/list` |
| `resume_steer_interrupt.jsonl` | `thread/resume` with `excludeTurns`, `turn/steer` with `expectedTurnId`, `turn/interrupt`, the interrupted `turn/completed` |
| `errors_without_experimental_api.jsonl` | what breaks when `experimentalApi` is missing (`collaborationMode/list` → -32600) and the full-history deprecation notices |
| `errors_steer_without_expected_turn_id.jsonl` | `turn/steer` without `expectedTurnId` → -32600 |

Already correct upstream, so keep it: `initialize` sends `capabilities.experimentalApi = true`
(`src-tauri/src/backend/app_server.rs`), and `turn/steer` sends `expectedTurnId`
(`src-tauri/src/shared/codex_core.rs`).

## Required changes

1. **Thread history without deprecated hydration.**
   - `thread/resume` and `thread/read` in `src-tauri/src/shared/codex_core.rs` currently hydrate
     full history. Use `excludeTurns: true` / omit `includeTurns`, then load history with
     `thread/turns/list` and `thread/items/list` (see their Params/Response types for
     cursors).
   - Load the most recent page first; page older history in on demand (scroll to top), reusing
     whatever "load older" or virtualization mechanism the thread view already has.
   - No `deprecationNotice` may be triggered by normal use.
2. **New item types.** Render, in the existing message/item components and following their
   patterns: `subAgentActivity` and `dynamicToolCall` (same family as `collabAgentToolCall` /
   `mcpToolCall`), `imageGeneration` (as images), `hookPrompt`, `functionCallOutput` and
   `sleep` (compact system rows). Unknown future item types must render as a collapsed
   generic row, never crash or vanish.
3. **Changed shapes.** Diff the hand-written types in `src/types.ts` and the Rust parsing in
   `src-tauri/src` against the generated 0.164 types for every message CodexMonitor uses.
   Fix renamed/added/required fields, e.g. `agentMessage` gained `phase`, `memoryCitation`,
   `delivery`, `questions`; `Turn` gained `rootTurnId`, `itemsView`; thread status has
   `activeFlags`. Prefer importing types from the generated protocol (add a `@protocol/*` path
   alias to `../../packages/protocol/src/*`) over extending hand-written copies, for any
   type you touch. Do not rewrite untouched areas.
4. **New notifications.**
   - `serverRequest/resolved`: remove the matching pending approval/question UI (it can be
     resolved by another client or by the server).
   - `thread/status/changed` `activeFlags` (e.g. `waitingOnApproval`): show it in the
     thread's sidebar state.
   - `thread/readState/changed`: feed the existing unread indicator.
   - `turn/moderationMetadata`, `thread/goal/*`, `deprecationNotice`,
     `remoteControl/status/changed`, `account/updated`: handle where the app has a
     matching feature, otherwise ignore explicitly, with no console errors.
   - Log `deprecationNotice` once to the debug panel.
5. **Rename to taucode.**
   - `productName` "taucode", bundle `identifier` "dev.taucode.desktop", window title, and the
     user-visible app name in the UI and menus. The data dir follows from the identifier.
   - Keep code identifiers, crate names, the daemon binary names and the LICENSE as they are.
   - Add a line to `README.md`: based on CodexMonitor by Dimillian (MIT), with a link.

## Tests to add

- **Rust:** a test that replays every `s2c` message of each fixture through the backend's
  app-server event routing (the code in `src-tauri/src/backend/app_server.rs` that classifies
  and forwards messages) and asserts nothing errors or is dropped unexpectedly.
- **TypeScript (vitest):** a test that replays each fixture's `s2c` notifications through the
  frontend event pipeline (`src/utils/appServerEvents.ts` and the thread event handlers) and
  asserts the end state:
  - items present with the right types
  - the approval is shown and then cleared by `serverRequest/resolved`
  - the turn ends `completed` (or `interrupted` for the steer/interrupt fixture)
  - the thread name is updated
  - the compaction item is rendered
- Unit tests for each new item renderer and for history pagination (cursor handling, ordering,
  no duplicates when a page overlaps the live stream).

## Constraints

- Offline sandbox: npm `node_modules` and Rust deps are pre-installed. Always build Rust with
  `CARGO_HOME=/Users/rolandleyco/taucode-m1/.cargo-home` (the crate's `.cargo/config.toml`
  sets offline). **Add no npm or Rust dependencies.**
- Follow the repo's own conventions in `apps/taucode/AGENTS.md`.
- Keep the 1,402 existing tests passing (403 Rust, 999 vitest). Update existing tests only
  where the old protocol behaviour they encode is genuinely gone, and say which in the report.
- Do not touch: `packages/protocol/**` (generated), `apps/desktop/**`, `crates/**`,
  `docs/PLAN.md`, the fixtures (read-only), `.mcp.json`, `.claude/**`, `.codex/**`.

## Verification

From `/Users/rolandleyco/taucode-m1/apps/taucode`:

```
export CARGO_HOME=/Users/rolandleyco/taucode-m1/.cargo-home TMPDIR=$PWD/src-tauri/target/tmp && mkdir -p $TMPDIR && (cd src-tauri && cargo build --bins && cargo test) && npm run typecheck && npm test && npm run build && npm run lint
```

Lint must report no errors. Baseline is 5 warnings; don't add warnings.

## Out of scope

codegraph/proxy wiring, the browser pane, providers/LiteLLM, computer use, upstream-only
features you don't need to touch (dictation, remote daemon, mobile, git/GitHub), visual
redesign, running the real codex (it's done outside your sandbox).
