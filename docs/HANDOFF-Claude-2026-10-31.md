# Claude handoff: taucode desktop polish and Computer Use reliability

You are continuing development of **taucode** for Roland Leyco. Read this handoff, inspect the current repository, and continue from the verified state below. Do not restart completed investigations or assume that a historical “pending” note is still current. This handoff is a current-state map with evidence references, not a substitute for checking the live app.

## 1. Immediate state and Git

- Main checkout: `/Users/rolandleyco/taucode`, branch `main`.
- Established development/build worktree: `/Users/rolandleyco/taucode-m1w`, branch `m1-wiring`.
- The user explicitly requested merging the completed work into main and preparing this handoff.
- Merge commit: **`d64b08e6fc17403bc6dedbd163726023df98ceeb`**, “Merge verified desktop and computer-use reliability work”. The merge was clean.
- It merges the 32 commits through **`6a225fa`** from `m1-wiring`, preserving main's two independent documentation commits, `3937739` and `65100c2`.
- Main's application implementation is identical to the tested `6a225fa` application tree; `apps/taucode/README.md` differs because main has the newer documentation. The new documentation site and onboarding guides were retained.
- This handoff is committed after the merge. Read `git log -3 --oneline` for the final documentation commit. The development worktree is aligned to this completed main state, retaining the documentation and its existing build/signing setup. Confirm both HEADs before starting new work.
- **Local merge only. No push or release publication is included.** Check local/remote status before assuming GitHub has these commits.
- Preserved unrelated untracked files: `/Users/rolandleyco/taucode/.claude/launch.json` and `/Users/rolandleyco/taucode-m1w/docs/HANDOFF-window-resize.md`. Do not delete, overwrite, or add them incidentally.
- Other old worktrees (`taucode-m0`, `taucode-m1`, `taucode-m5p`, `taucode-m6`, `taucode-m7`) exist. Do not mistake them for the active implementation or clean them up without a reason.

Read `AGENTS.md` and `apps/taucode/AGENTS.md`. Use CodeGraph first for code location/understanding when `.codegraph/` exists. The old untracked resize handoff is historically useful but contains obsolete branch hashes, self-signing status, log paths, and workflow/model instructions. Current human instructions and verified current state take precedence.

## 2. What the user wants

The product should feel as polished, understandable, efficient, and reliable as Claude Desktop/Codex Desktop. The user wants actual dogfooding: use taucode to build small things, watch tools and approvals, inspect outputs, reproduce failures, improve the implementation, rebuild/sign, and verify the actual native app.

Standing preferences:

- In taucode, use **GPT 6.1 Sol (Account 2)** through LiteLLM, with conversation reasoning **Low** (the user called this “light”). Use **Full access** for these authorized local development tests.
- The Computer Use planner and grounder have their own configuration. Both were configured to the same Account 2/model; their sidecar does not expose the conversation's reasoning selector. Do not claim that selecting Low in the composer changes their reasoning setting.
- Code, Chat, and Work should have clear, centered layouts with sensible side padding, discoverable controls, good resizing, useful progress, and recoverable errors.
- Chain of Command is **Code only**. Keep it out of Chat and Work.
- Its composer control uses a compact five-star general/admiral motif; activation lights stars sequentially and adds a glow. A conditional `>` opens configuration when enabled. The user disliked an oversized, intrusive icon.
- Team configuration should be understandable, support additional team models and preset roles, and preserve choices when toggled off/on.
- Basic slash commands are wanted; avoid turning them into a complicated command language.
- Computer Use must work while the user uses a **different app**. Avoid foreground stealing, input leakage, giant floating HUDs, and intrusive labels.
- The desired virtual pointer follows the supplied rounded/notched arrow, with translucent neon-blue fill and lavender glow. Preserve that design.
- Prefer completing authorized work over repeated permission questions. Ask only when genuinely necessary; explain the actual blocker. Native authentication, Keychain/password, and screen-unlock steps must be handled by the user when the tool cannot operate them.
- Commit completed work. This merge is authorized and done; do not infer blanket authorization for unrelated future merges, pushes, releases, or new app permissions.

## 3. Product and architecture map

Taucode is a Tauri v2 desktop agent harness derived from CodexMonitor. Current app/package version is **0.7.68**; the executable is still named `codex-monitor`, and the bundle identifier is `dev.taucode.desktop`.

- Frontend: React, TypeScript, Vite, feature directories under `apps/taucode/src/features`.
- Backend: Rust/Tauri, with shared domain logic under `apps/taucode/src-tauri/src/shared` and thin app/daemon adapters.
- Engines include Pi, optional oh-my-pi/OMP, Codex, Claude ACP, and custom integrations. Check the actual selected engine; Work's profile and Code's OMP choice are distinct.
- Native browser: WKWebView child views, not merely an iframe in the frontend. CSS screenshots or Chromium UI previews cannot prove native placement, focus, or lifecycle correctness.
- Computer Use: a Python planner/grounder sidecar emits structured proposals. Rust validates policy, target binding, observations, approvals, and input delivery. The Python sidecar does not inject GUI input.
- Engine/model configuration, credentials, tool schemas, native policies, and UI permission labels are separate layers. Full access does not erase connector-specific boundaries or macOS permissions.

Important references:

- `docs/dogfood-acceptance.md`: detailed implementation and live evidence. Read the **last/current sections first**, then relevant earlier sections. Older pending statements are superseded by later verification.
- `docs/specs/m9-chain-of-command.md`: team configuration, role enforcement, limits, checkpoints, crash recovery, model measurements.
- `docs/specs/m8d-work-dogfood.md`: Work follow-up composer, sizing/zoom, delegation, titles, managed previews, native acceptance.
- `docs/codebase-map.md`, `docs/architecture.md`, `docs/development.md`: navigation/build architecture.
- `docs/computer-use.md`, `apps/taucode/engines/computer/README.md`: product and implementation details.
- New user-facing guides on main include `docs/start-here.md`, `docs/first-task.md`, `docs/getting-started.md`, `docs/code.md`, `docs/chat.md`, `docs/work.md`, and the VitePress site. Preserve them.

## 4. Completed product work to preserve

### Layout, Work, and commands

The earlier polish work established centered conversation/composer layouts, a pinned Work follow-up composer, improved sidebar/window resize targets, native browser geometry under zoom, and useful View-menu controls. The Work composer is expected to exist; its historical absence is not intended behavior.

Work now supports in-task follow-ups, visible plans/outputs, clearer task titles/status groups, delegation activity, and managed preview controls. It preserves drafts across relevant navigation/stopping flows. Task search can match output paths and temporarily expand matching collapsed spaces, restoring their collapse state when search clears.

Code/Chat navigation and status handling distinguish working, waiting for input, stopped, failed, and completed states. A stopped task must not restart because a late approval arrives. Recovery copy preserves completed work and asks for a fresh continuation rather than replaying the original request automatically.

Basic slash commands are implemented; see `docs/code.md` for the exact current list and behavior. In particular, `/plan` and `/goal` are **editable prompt starters**, not persistent autonomous background goal engines. `/sidechat` uses supported engine forks. Do not advertise stronger semantics than the implementation has.

### Teams, recovery, and Evals

Chain of Command is Code-only, with compact animated stars and a configuration entry point, conversation overrides, preset roles/additional model entries, persistence, and fork/side-chat copying. Review evidence gates success. Concurrency/depth/deadline/usage limits have engine-specific enforcement; reported usage can lag, and in-flight requests can overshoot token limits.

Crash-recovery work preserves Pi/OMP root response/tool checkpoints and delegation transcripts. Parent/engine death stops owned POSIX commands. The app tells users a response was interrupted and how to continue; it does not restart unfinished commands automatically. Last-second unflushed output, other engines, Windows cleanup, and power-loss durability are not universally verified.

Evals setup/results/history were redesigned for readability; sticky header/layer issues were fixed. Cloud/local bugfix baselines and a mixed-team run have evidence. **Halogen Qwen3.8 Flash Next** was identified by the user as self-hosted/local. Unknown pricing stays unknown, never zero. These small measured baselines do not justify automatic routing or general claims about model quality.

### Native browser

Merged fixes include:

- Evicted tabs stuck loading: visible placement now allows native view recreation without a readiness deadlock.
- Correct page backgrounds/appearance and keeping the active panel tab visible.
- Accessible numbered overflow selectors for many tabs, including duplicate titles/URLs.
- Serialized restoration/creation, latest-selection preservation, and no duplicate native page while awaiting delayed backend notifications.
- Loading/error state consistency and Retry using the original URL when an initial failed navigation has no committed page for `WKWebView.reload()`.
- Saved title/URL/order/selection retention across restart.
- Native eval/debug/LSP approvals show their actual JSON arguments instead of an opaque tool name; approval decisions were not silently weakened.

Native acceptance includes repeated creation/closure, nine restored tabs, duplicate-title selection, delayed loading, unavailable-localhost errors, corrected Retry, and a real generated Focus Board app. Do not confuse mock preview checks with these native checks.

### Titles, history, and image previews

- Explicit empty selection and unsent drafts survive late history/ensureThread events.
- Automatic titles recognize the first optimistic prompt while respecting manual renames and existing/custom titles.
- Internal metadata requests use ephemeral Codex threads and stay out of the user sidebar. One verified leaked helper session was archived, not destroyed.
- OMP can place title metadata before its session header. Listing/reading/forking/rewind paths now normalize that prefix in memory and preserve the original transcript.
- Broken persisted image previews are fixed: OMP may store `data: "blob:sha256:<hash>"`, which must be restored from the agent's isolated blob directory rather than treated as literal base64.
- Image restoration enforces a lowercase 64-character hash, MIME checks, canonical path containment, a 20 MiB limit, and SHA256 verification. Missing/invalid blobs become explicitly unavailable. Inline images remain unchanged, and source history is not rewritten.
- Live saved thumbnail and lightbox verification passed after restart. Do not remove the validation checks merely to make an image appear.

## 5. Signing / Keychain is no longer the old blocker

Repeated prompts were traced to provider-key partition entries tied to individual self-signed binary hashes. A stable designated requirement alone did not prove durable Keychain trust.

The user approved free Xcode Personal Team setup, signed into Xcode, and authorized creation of an Apple Development identity. Xcode is installed at `/Applications/Xcode.app`. The recorded certificate expiry is **October 30, 2027**; recheck actual identity validity before future signing changes.

The user separately chose Always Allow for:

1. Taucode's existing provider-key access.
2. Apple's `codesign` tool using the development signing private key.

Read-only metadata then showed a matching Apple `teamid:` partition on the provider item. Fresh launches and changed executable hashes completed provider requests without another approval step, with unchanged team trust and no new hash entry required. The acceptance record contains exact evidence and caveats.

- `apps/taucode/scripts/sign-dev.sh` uses an existing stable identity.
- It reads Git-ignored `apps/taucode/.signing-identity.local`; `TAUCODE_SIGNING_IDENTITY` is the explicit override.
- Do not replace this with ad hoc/self-signing, create new certificates unnecessarily, delete provider keys, reset app data/TCC, or script Keychain ACL/partition changes.
- Keep keys and signing secrets out of logs, commits, prompts, and handoffs.
- Free development signing is **not** Developer ID distribution/notarization. Another Mac, team changes, renewal, and distribution remain separate work.
- If a prompt reappears, diagnose which app/item/identity is involved and hand authentication to the user. Do not make the user approve repeated rebuilds without identifying the cause.

## 6. Computer Use: current implementation and invariants

The scoped reliability plan is complete: fix TextEdit's disabled Find recovery; exercise longer Calculator/TextEdit workflows while another app remains usable; show capture target/status and reliable Pause/Stop controls.

### Access and test boundaries

Authorized live targets are **Calculator** and **TextEdit using only the disposable fixture**. Existing permission to test these is not permission to capture arbitrary personal documents or add other apps. Expanded target access/screenshots require an appropriately specific user authorization.

- Background mode is enabled; foreground fallback is **off**.
- Main test conversation: Sol Account 2, Low, Full access.
- Planner/grounder: same configured account/model, with separate sidecar configuration.
- Fixture: `/Users/rolandleyco/taucode-dogfood-computer/Background QA.txt`.
- Baseline: `/tmp/taucode-textedit-baseline.txt`.
- Baseline/disk SHA256: `4cb1afec0fbdd78dcfe1fef13baa0883da9508e1eca000e754bdb884b3f2be50`.
- The full text starts **`ITaucode`**, contains 120 numbered sample lines, and ends `BACKGROUND_QA_OK`. The leading `I` is part of the established baseline; do not “clean it up.”
- Final state: fixture at the beginning, header and Line 001 visible, no Find or line-number dialog open; complete in-memory text and disk file match the baseline. No test Save was sent. TextEdit's existing Edited indicator is not proof of a new mutation.
- Previously verified Calculator long workflow ended at **325**. Treat that as last observed state, not an invariant if the user has since interacted.

### Targeting and user focus

Window selection prefers the target app's own focused usable window, excluding minimized/unreadable windows, before fallback. A tiny transient Calculator window previously caused failures while the user typed elsewhere; that is fixed and covered by live concurrency tests.

Snapshots retain native AX handles with snapshot/window identity. Document bounds are clipped to the visible window and scroll ancestors before grounding. Left clicks never substitute AXShowMenu. Exact target binding and policy evidence are rechecked around approvals and input.

Keyboard typing with a grounded field must match the actual focused AX element, not merely the same app/window. This prevents overwrite/Enter from reaching a document when the agent selected a Find field. The regression sends **zero input** when exact focus is wrong. Direct safe AX field writes can still work without keyboard delivery. Keep this distinction.

Already-held keys release through their original bound input during cleanup, even if Escape dismissed the field. New key-downs still require the current target checks. Do not weaken those checks to “fix” cleanup.

### Pause, Stop, progress, and latency

- Pause epochs invalidate old capture/planning/approval observations; Resume replans rather than sending a stale action.
- Control calls bind to a run ID; late responses cannot control a different run.
- Stop remains available while Pause is pending. Cleanup releases owned keys/buttons before slower overlay/focus work.
- Inline controls display target app/window and Observing / Planning / Acting / Waiting for approval / Waiting for the app phases, including during capture.
- Both OMP's `Computer Use` connector name and Codex's `taucode-computer` name render controls/results.
- Background mode has inline controls, **no giant floating HUD**. Foreground mode retains its native controls/global Stop handling.
- The sidecar's redundant separate reflection request was removed; each main prediction verifies the preceding action against a fresh screenshot.
- A matched Calculator comparison reduced prediction wait from 72.058 s to 47.516 s, but step counts differed. This is one observation, not a universal speed claim.
- Computer-use MCP delivery allows 3660 seconds while native limits remain enforced. OMP's timeout unit is milliseconds; Pi/Codex use seconds. Preserve conversion and cancellation/disconnect behavior.

### Pointer

The current native pointer is approximately 25 points tall, rounded/notched, with 42%-opacity neon-blue fill, an icy edge, lavender glow, and a fading click ripple. It holds roughly 2.5 seconds and fades by 3 seconds. It has no label, does not activate windows, ignores mouse input, and stays off occluding unrelated apps. Transparent Dock/system windows must not be treated as opaque occlusion rectangles; visibility uses AX hit ownership. Diagnostic display counts are supporting evidence, not proof that a human saw it. Native rendering preview and live user observations are recorded in acceptance notes.

### Disabled TextEdit Find recovery — solved for the reproduced state

Repro: TextEdit Window > Minimize, then AXRaise to restore. Find/Undo/Select All become disabled even though AXFocusedUIElement still identifies the document. Plain AXFocused=true refresh did not fix it.

The retained fix is narrowly scoped to a plain, unmodified, single left click on TextEdit's AXTextArea when the captured Command-F menu resolution is disabled. It posts the target-only AppKit window focus record used by Cua's `activate_without_raise_with(..., false)` and waits briefly. It does **not** defocus the user's app or call SetFrontProcess. WindowServer foreground and keyboard-focus readings must be available and unchanged before proceeding with the click. Existing action cleanup remains in force.

Two live minimize/restore cycles recovered Find. Both kept foreground and keyboard-focus pid 2726, independently identified as Safari, unchanged; no focus restorations occurred. The complete fixture stayed unchanged. Do not generalize this special case to all apps: earlier unconditional AppKit priming activated Notes without raising its window.

### Ambiguous AX completion — verify, never replay

macOS may return `-25204` (CannotComplete) after an AXPress **did happen**. Find Next moved the selection, and Done closed Find, despite this timeout. A pixel fallback can then click unrelated UI; it must not run.

Current behavior:

- Unsupported AX operations (`-25205`, `-25206`) may use existing fallback rules.
- Ambiguous completion is tagged `Action outcome uncertain: `.
- Release input/end action state; capture fresh evidence within existing time/step limits.
- Enter **read-only verification**. The planner may report Done only if the **entire requested task** is visibly complete, or Fail explaining remaining uncertainty/work.
- The host rejects every further click, edit, hotkey, or other input in that verification phase. It is not an automatic retry path.
- The original step keeps its error and is not mislabeled “not executed.” Verified completion has `verificationOnly:true`.
- Intermediate incomplete tasks still stop for review. Do not silently continue/replay an uncertain mutation.
- The Python worker explicitly forwards the `READ-ONLY VERIFICATION:` notice on subsequent turns. Without that fix, the action completed but the planner falsely said it had not received a timeout.

### Rejected proposals — correctable and bounded

Validation failures known to occur before input use `Action not executed: ` and bounded replanning. Existing limits are three consecutive/six total recoveries; stale-observation handling has its own bounded rules. Python ValueError/TypeError argument validation uses this path; provider errors remain distinct.

The scroll schema states integer clicks between -100 and 100. A real planner proposed +160; previously this safely rejected input but terminated the task. It now reports the bound, takes a fresh observation, and permits a corrected proposal. A live boundary test rejected +160 once, then completed with +100 and +30. Never conflate this with an uncertain native mutation.

### Persisted results

OMP may put structured failed tool JSON inside `error.message` prefixed by `Error:`. Parsing now unwraps those layers and loads durable run references. Compact persisted references do not need embedded thumbnails; the full run supplies them. Long errors must not truncate the reference JSON and make the recovery card disappear.

Recovery cards distinguish blocked, interrupted, and completed runs; rejected actions say Not executed, uncertain ones say Check result. Copy continuation request preserves action history/uncertainty and requires fresh inspection. It fills a clipboard request; it never auto-sends or blindly replays.

## 7. Live evidence worth knowing

Run JSON is stored under `~/Library/Application Support/dev.taucode.desktop/computer/runs/<id>.json`. These are real local runs, not mock screenshots. Do not dump base64 images or full private logs unnecessarily.

| Run ID | Evidence |
| --- | --- |
| `d448c0ca-e906-42de-9299-b9800d34de2e` | Calculator 123 × 45 = 5,535 while a concurrent taucode draft was typed and edited; draft intact, correct target window, no errors/restorations. |
| `f9a58252-7e33-4e8f-aa74-9f0eb5d90da5` | Longer Calculator chain: 125 × 8 = 1000; /4 = 250; +75 = 325; 14 native actions, independently checked. |
| `7385679d-7169-4fe1-b8e6-f9f0f27d92c3` | TextEdit background append; marker appeared after 120 sample lines; separate concurrent draft stayed in taucode. |
| `852e48ff-0832-45e5-92ed-c2c2421a8f66` | TextEdit About dialog opened/closed without touching the fixture. |
| `2411c34b-6d17-4c06-aaaa-e258f3f7debe` | Disposable Untitled window created, switched with fixture via Window menu, then only Untitled closed; concurrent draft retained. |
| `85e7aed9-9611-421f-9571-4816ca4b77b3` | Wrong-focus Find overwrite+Enter refused before input; full document unchanged. |
| `0992816a-4a64-42bd-82d7-f2390a181f79` | Direct AX write changed the Find field alone; document unchanged. |
| `e96f4341-4561-4dcc-b3fa-09196fe098ce` | First disabled-Find recovery passed; Done closed Find but revealed the AX timeout false failure. |
| `0b0bb1b9-b04f-4a06-bfa9-ef333969c3b2` | Second independently reproduced minimize/restore failure recovered; Find left visibly open, Safari front/key focus unchanged. |
| `be0c3f29-87eb-4977-906f-2c2a9e16541f` | Final timeout-verification build: Command-F, one Done click, -25204, fresh read-only confirmation of closed Find; planner correctly acknowledged timeout. |
| `9be34d8e-3486-40f8-90f6-5556c4851ab3` | Insert `BG_RELIABILITY_1031`, observe it, Undo once; full document restored, zero focus restorations. |
| `16ce1da3-2457-44b8-a000-fb77d014319e` | Edit > Find > Select Line dialog inspected without entering text, closed once; fixture restored unchanged. |
| `6e77b20f-c934-45e4-813d-b5f134e4c925` | Scroll reached Line 120/BACKGROUND_QA_OK; oversized +160 proposal exposed rejected-argument termination. This run itself was incomplete. |
| `b99e7e0f-f533-4785-bd59-15a59a411c55` | Final correction: +160 rejected before input, +100/+30 valid AX scrolls, header/Line 001 independently visible, baseline intact. |

Do not treat a run's prose alone as proof. Compare actual step path/error/recovered/verificationOnly/focusRestorations, fresh screenshot/AX state, and fixture integrity. Historical PIDs/window IDs are evidence, not stable identifiers to reuse.

## 8. Source map for likely next work

Paths below are relative to `apps/taucode` in either aligned checkout.

| Area | Files |
| --- | --- |
| Native runner, observation epochs, rejected/uncertain paths | `src-tauri/src/computer/runner.rs`, `tests.rs`, `control.rs`, `policy.rs` |
| Target/session binding and exact typing focus | `src-tauri/src/computer/background/mod.rs`, `ax_actions.rs`, `ax_tree.rs`, `windows.rs`, `menus.rs` |
| Target-only AppKit refresh / event routing | `src-tauri/src/computer/background/skylight.rs`, `delivery.rs`, `focus_guard.rs` |
| Cursor/native HUD | `src-tauri/src/computer/background/overlay.rs`, `src-tauri/src/computer/panels.rs` |
| Live bridge/status/sidecar | `src-tauri/src/computer/mod.rs`, `sidecar.rs` |
| Planner schema/argument rejection | `engines/computer/taucode_cua/actions.py`, `server.py` |
| Per-step planner messages | `engines/computer/taucode_cua/vendor/agent_s3/agents/worker.py` |
| Planner and native regressions | `engines/computer/tests/test_sidecar.py`, other Python tests, `src-tauri/src/computer/tests.rs`, `background/tests.rs` |
| Inline controls, saved results/recovery UI | `src/features/computer/ComputerRunItem.tsx`, `computer.ts`, `computer.css`, `computer.test.tsx` |
| OMP images/history/runtime | `src-tauri/src/shared/pi/images.rs`, `mod.rs`, `process.rs`, `profile.rs`, `tests.rs` |
| Auxiliary metadata/title requests | `src-tauri/src/shared/codex_aux_core.rs`, `src/features/threads/hooks/useThreadTitleAutogeneration.ts` |
| Connector deadlines | `src-tauri/src/shared/connectors.rs`, `shared/pi/profile.rs` |
| Native browser lifecycle | `src-tauri/src/browser/native/`, `src/features/browser/BrowserPane.tsx`, `NativeSlot.tsx` |
| Panel navigation | `src/features/panels/PanelTabList.tsx`, `SidePanel.tsx`, `DockWorkspace.tsx` |

Keep shared behavior in shared cores, app/daemon adapters thin, IPC contracts aligned, and existing design-system shells/tokens. Do not bulk-reformat large Rust files while making a small fix.

## 9. Current limits and recommended next work

The scoped Computer Use reliability pass is complete. The whole product is not declared finished or generally superior to another desktop app.

Recommended next task: **OMP-owned service visibility and schema ergonomics**.

A real Focus Board project was built at `/Users/rolandleyco/taucode-dogfood-focus-board`. Its OMP `focus-board-preview` service served `127.0.0.1:4173`, while Session rail showed Servers (0) and Background tasks (0). Native tool ownership/cleanup worked, but its service lifecycle/URLs were not reflected in the shared task UI. That is a concrete remaining product gap.

Suggested acceptance:

1. Reproduce with the isolated Focus Board fixture; first check whether its service is actually running. Old PID values and “left running” statements are historical—subsequent app quits can stop it.
2. Trace how OMP service events/results should enter shared background-task and preview state.
3. Show the owned service, actual lifecycle, and URL in the correct conversation's UI.
4. Make Open/Stop/Restart use the existing ownership model. Do not kill arbitrary port owners or expose the server beyond 127.0.0.1.
5. Reconcile the model-facing bash/service schema with service validation, especially optional/null defaults. The successful historical path wrote to `xd://bash` with `command: node server.mjs`, project cwd, `pty: true`, `name: focus-board-preview`, and readiness log/port/host/timeout; repeated direct bash attempts failed. Inspect the current contract before copying this shape.
6. Verify visible service creation, URL opening, stop, restart, app-exit cleanup, and preservation of files/drafts. Add regression coverage for delayed or duplicate lifecycle delivery.

Other useful follow-ups, separately scoped:

- More simultaneous user-typing tests for native **keyboard-delivery** routes. Calculator button clicks and TextEdit AX writes passed, but that is not universal concurrent-keyboard safety. `delivery::Scope` still has keyboard focus-lease behavior worth reviewing carefully.
- More apps/windows only after specific user authorization. Do not broaden the TextEdit AppKit workaround indiscriminately.
- Ambiguous intermediate actions can still require a reviewed continuation; any future automatic continuation needs stronger postcondition evidence and must never replay uncertain mutations.
- Native browser process-exit investigation: an earlier empty-tab exit was not reproduced; native creation failure/Retry has not been comprehensively fault-injected. Do not claim the old crash cause was found.
- A prior broader Pi test run failed `managed_preview_bridge_starts_logs_serves_and_stops` / `wait_url` at five seconds, including an isolated attempt. That failure was not solved by the Computer Use work. See actual logs/evidence before describing the entire Rust suite as green.
- Older broad frontend tests had failures under heavy load with isolated reruns passing; use bounded workers and report actual totals. This handoff records the current post-merge result below.
- Windows/macOS cross-platform behavior, VoiceOver/accessibility breadth, certificate renewal, release notarization, and power-loss durability remain beyond the scoped acceptance.
- Historical compact-theme-preview cropping and `/var/folders` symlink-path sandbox notes need fresh reproduction before being treated as current defects. Use the established non-symlink TMPDIR for reproducible native tests.

Failed recovery ideas not to repeat without new evidence:

- AXFocused=true on the document did not restore disabled Find.
- AXRaise alone restores the window but can leave the menu state broken.
- Disabled-menu keyboard fallback, bypassing the keyboard authentication envelope, reopening the same file, scrolling, or cancelling a menu did not establish recovery.
- Unconditional target AppKit priming caused activation problems in Notes.
- Extending AX timeouts while holding the input lock can undermine prompt Stop/key release. The retained solution verifies uncertain outcomes without replay, not a blanket timeout increase.

## 10. Build, signing, testing, and inspection

The currently verified running bundle is:

`/Users/rolandleyco/taucode-m1w/apps/taucode/src-tauri/target/debug/bundle/macos/taucode.app`

It was built from the latest application implementation, Apple Development-signed, strictly verified, relaunched, and tested live. The merge adds documentation and preserves the same application source; it does not require replacing that running bundle solely because Git gained a merge commit.

### Build in the existing worktree

First inspect status and the app's active runs. Quit an idle app through the supported computer-use UI before replacing its bundle. Preserve any user drafts/runs; do not kill an active task casually.

```sh
cd /Users/rolandleyco/taucode-m1w/apps/taucode
PATH="$HOME/.cargo/bin:$PATH" \
CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home \
TMPDIR=/Users/rolandleyco/taucode-m1w/.tmp \
npx tauri build --debug --bundles app
./scripts/sign-dev.sh
codesign --verify --deep --strict src-tauri/target/debug/bundle/macos/taucode.app
```

This location already has dependencies, build cache, the ignored local signing preference, and the trusted app path. If choosing another checkout/location, account for those differences deliberately rather than copying credentials or resetting permissions.

### Focused native/Python checks

```sh
cd /Users/rolandleyco/taucode-m1w/apps/taucode/src-tauri
PATH="$HOME/.cargo/bin:$PATH" \
CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home \
TMPDIR=/Users/rolandleyco/taucode-m1w/.tmp \
cargo test --lib computer:: -- --test-threads=2
```

```sh
cd /Users/rolandleyco/taucode-m1w/apps/taucode/engines/computer
.venv/bin/python -m pytest -q
```

Latest pre-merge passes for the retained implementation: **124 native Computer Use tests**, **133 Python tests**, signed debug build, strict codesign verification, and live acceptance above. These are separate suites, not a claim that every Rust module passed.

For frontend/Rust changes follow `apps/taucode/AGENTS.md`: typecheck always, relevant regression tests, full frontend tests for frontend behavior, cargo check for Rust. The merge verification also runs production frontend and documentation builds. Do not add meaningless tests for prose-only changes.

```sh
cd /Users/rolandleyco/taucode/apps/taucode
npm run typecheck
npm run build
npm test -- --maxWorkers=2
```

```sh
cd /Users/rolandleyco/taucode
npm --prefix docs run build
```

UI preview (`npm run preview:ui`, port 1421) and WebKit checks are useful for frontend layout, but cannot prove native browser placement, macOS permissions, Keychain trust, background focus, or actual provider execution.

### App data and diagnostics

- App data: `~/Library/Application Support/dev.taucode.desktop`.
- Live Computer Use status socket: `~/Library/Application Support/dev.taucode.desktop/run/computer.sock`.
- Saved Computer Use runs: `~/Library/Application Support/dev.taucode.desktop/computer/runs/`.
- Sidecar log: `~/Library/Application Support/dev.taucode.desktop/computer/logs/sidecar.log` and rotated backups.
- Visual evidence directory: `/Users/rolandleyco/.codex/visualizations/2026/10/28/01a18084-5833-7173-80ef-aac3635dfc85`.
- Latest build/test logs: `/tmp/taucode-rejected-input-{tests,python-tests,build}.log`, `/tmp/taucode-ax-verification-{tests,python-tests,final-build}.log`, `/tmp/taucode-window-primer-{tests,check,types,build}.log`.
- Merge logs: `/tmp/taucode-merge-main-{typecheck,build,frontend,cargo-check,docs}.log`.
- `/tmp` files and old screenshots may disappear. Git source/tests and the documented run IDs are more durable anchors. Revalidate paths before relying on them.

Read-only status inspection without dumping images/credentials:

```python
import json
import os
import socket

path = os.path.expanduser(
    "~/Library/Application Support/dev.taucode.desktop/run/computer.sock"
)
with socket.socket(socket.AF_UNIX) as connection:
    connection.settimeout(5)
    connection.connect(path)
    connection.sendall(b'{"action":"status"}\n')
    run = json.loads(connection.makefile().readline())["result"]

print({key: run.get(key) for key in ("id", "running", "phase", "summary")})
for step in run.get("steps", []):
    record = {
        key: step.get(key)
        for key in (
            "step", "action", "path", "recovered", "verificationOnly",
            "executionFallbackReason", "focusRestorations"
        )
    }
    record["error"] = (step.get("error") or "").split("\nSidecar")[0]
    print(record)
print((run.get("error") or "").split("\nSidecar")[0])
```

Use the configured computer-use tools for all GUI actions; do not bypass their restrictions with shell AppleScript/JXA/CGEvent synthesis. Read-only source/log/socket inspection is useful alongside UI verification. Refresh AX/screenshot state before acting, and do not reuse stale element indexes across navigation or screenshots.

If the Mac is locked, the tool cannot unlock it. Ask the user once with the concrete blocker, continue independent work, then recheck after their reply. SecurityAgent/authentication prompts require the user. Do not repeatedly claim a blocked test passed because an API returned a handle.

## 11. How to resume efficiently

1. Read this document and applicable AGENTS files, then inspect both checkout statuses and recent commits. Do not redo the merge already recorded here.
2. Read the current end of `docs/dogfood-acceptance.md`; distinguish completed evidence from older interrupted/pending paragraphs.
3. Verify which signed taucode bundle is running and whether any Computer Use/agent run is active before changing it.
4. Confirm the requested Sol Account 2/Low/Full access settings in the relevant test surface. Preserve the separate Computer Use allowlist, background mode, and foreground-fallback setting.
5. Start with the concrete OMP service-visibility gap unless the user steers to another issue. Use the disposable Focus Board project and local-only preview server.
6. Make small reproducible changes, add meaningful regression checks, build/sign, and verify in the actual app. Inspect both outputs and native logs; separate model mistakes, tool-schema mistakes, state/transport bugs, and real UI failures.
7. Report changes, exact tested scope, remaining limitations, and Git state. Avoid broad parity/security/performance claims from a few passing fixtures.
8. Keep the app usable for the user. Preserve unrelated files/drafts, avoid destructive cleanup, and never replay uncertain input automatically.

## 12. Merge verification result

Verified after the clean merge in `/Users/rolandleyco/taucode`:

| Check | Result |
| --- | --- |
| `npm run typecheck` | Passed |
| `npm run build` | Passed; Vite retains its large-chunk warning |
| `npm test -- --maxWorkers=2` | **1,888 tests / 226 files passed**, 83.77 seconds |
| `cargo check --all-targets --features custom-protocol` | Passed; existing compiler warnings remain |
| `npm --prefix docs run build` | Passed, including this handoff |
| Application tree comparison with `6a225fa` | Identical except the newer application README |
| Native Computer Use suite for that application implementation | **124 passed** before merge |
| Computer Use Python suite for that application implementation | **133 passed** before merge |
| Signed app / strict codesign / live workflows | Passed before merge for the same application implementation |

The merge introduced no application-source conflict resolutions; main's two
independent documentation commits were preserved. No fresh native app rebuild was
necessary solely for the documentation/merge commit. The existing verified signed
bundle remains the live build. Logs are listed above. The only unrelated untracked
files are the two explicitly preserved files described in section 1.

## 13. Commits included in this merge

The following exact history is provided for focused inspection/revert analysis. Do not reapply these commits; they are already ancestors of main.

- `c234787` — Fix native browser tabs stuck loading after eviction
- `c509ef6` — Restore native page backgrounds and keep active panel tabs visible
- `c6accfb` — Clarify task progress and preserve recovery states across conversations
- `a4f5798` — Improve conversation navigation and browser tab restoration
- `36049cb` — Avoid duplicate native pages while awaiting tab state
- `97bfed3` — Clear loading guidance when native navigation fails
- `6c2dadb` — Show native tool arguments in approval requests
- `f6455e1` — Retry failed native navigation using its original URL
- `43eea19` — Record completed native dogfood acceptance
- `368decc` — Replan computer actions after pause invalidates observations
- `867c0c8` — Allow computer-use runs to finish across engine MCP deadlines
- `8844208` — Stabilize background window selection and expose computer run controls
- `14fd484` — Keep background computer-use overlays out of other apps
- `dfc8048` — Improve computer-use progress, recovery and background input reliability
- `b1a3d50` — Recover invalid computer actions and render persisted OMP failures
- `5c41b42` — Restore background action cursor and recover changed approval targets
- `a029fc0` — Finish computer runs with the already verified observation
- `19eced0` — Ignore transparent system windows when showing the action cursor
- `6b4d658` — Record native cursor display verification
- `89763e4` — Polish the computer-use cursor and improve its visibility
- `f03ca64` — Match the reference cursor with translucent blue and lavender glow
- `a8fd52a` — Fix ambiguous computer input retries and preserve recovery cards
- `d20a75a` — Fix automatic titles and restore renamed OMP conversations
- `e5ef330` — Show Computer Use targets and harden run controls
- `12dce18` — Keep scrolled document grounding inside its visible viewport
- `71251de` — Keep background left clicks from opening context menus
- `9389f13` — Restore persisted OMP images for chat previews
- `155d823` — Record focus context for disabled background commands
- `fd5c69b` — Require exact field focus before background keyboard typing
- `001b5f7` — Recover TextEdit document state after background window restore
- `e789b22` — Verify ambiguous AX completion without replaying input
- `6a225fa` — Replan rejected computer-use arguments within bounded limits
