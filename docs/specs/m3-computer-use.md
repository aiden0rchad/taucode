# M3: computer use (Agent-S3 planner, Rust eyes and hands)

## Goal

Any engine can operate native macOS apps through a built-in MCP connector
`taucode-computer` (the reserved id from M1-E):

- **Agent-S3** (`gui-agents==0.3.2`, Apache-2.0) only *plans*: screenshot in → one structured
  action out.
- **Rust** captures the screen (excluding taucode's own windows), checks policy, gets
  approvals, and injects input.
- Models come from the user's provider registry: a vision **planner** model and a
  **grounding** model (UI-TARS-1.5-7B recommended). No accounts.

Done = the verification command passes. Live tests need macOS permissions, which the user
grants; they're run outside your sandbox.

## Non-negotiable safety rules

1. **Never `eval`/`exec` model output.** Agent-S's `create_pyautogui_code`
   (`gui_agents/s3/utils/common_utils.py`) does `eval(code)` on the plan. Replace it in our
   sidecar with an `ast`-based parser:
   - accept exactly one call `agent.<method>(...)`
   - `method` must be in the allowlist (`click`, `type`, `scroll`, `drag_and_drop`,
     `hotkey`, `hold_and_press`, `open`, `switch_applications`, `wait`, `done`, `fail`)
   - all arguments must be literals (`ast.literal_eval`)
   - anything else → a `fail` action carrying the reason, fed back to the planner
2. **Structured actions only.** Subclass `OSWorldACI` so each method returns a JSON action:
   - `{kind:"click", x, y, button, clicks, hold_keys}`, `{kind:"type", text, x?, y?, overwrite, enter}`
   - `{kind:"scroll", x, y, clicks, shift}`, `{kind:"drag", from, to, hold_keys}`
   - `{kind:"hotkey", keys}`, `{kind:"hold_and_press", hold, press}`
   - `{kind:"open", app_or_file}`, `{kind:"switch_app", app}`, `{kind:"wait", seconds}`
   - `{kind:"done", summary}`, `{kind:"fail", reason}`

   Grounding (description → x,y) still runs inside the sidecar. Coordinates are returned in
   grounding space, with the grounding width/height. Disable `call_code_agent`,
   `set_cell_values`, and anything needing OCR/paddle (`highlight_text_span`).
3. **The sidecar never touches the screen, mouse or keyboard.** pyautogui must not be
   imported for execution: stub the import in the sidecar, or keep only what grounding needs.
   It receives PNG bytes and returns JSON.
4. **Kill switch:** a global shortcut (default ⌃⌥⌘.) stops a run within 100 ms and
   releases any held keys/buttons. A floating always-on-top **HUD** (excluded from capture)
   shows the current step, Pause and Stop.
5. **Policy:**
   - per-run app allowlist, and a denylist that always applies (password managers, Keychain
     Access, System Settings privacy/security panes, Terminal running sudo, banking apps by
     bundle id)
   - "confirm before typing into secure text fields" (AX `AXSecureTextField`)
   - "confirm submit/send/buy-like clicks", using the planner's action description +
     keywords
   - max steps, max wall time
   - approvals use the existing approval UI

## Components

1. **Sidecar** `apps/taucode/engines/computer/` (Python, managed by `uv`):
   - **Vendor, don't depend.** `gui-agents`' declared dependencies drag in PaddlePaddle/OCR
     (uv can't lock it without them; verified). Copy only the Agent-S3 modules the planner
     and grounding path need from `/Users/rolandleyco/taucode-refs/Agent-S/gui_agents/s3`
     (about 3.8k lines, Apache-2.0) into `taucode_cua/vendor/agent_s3/`:
     `agents/{agent_s,worker,grounding}.py`, `core/{engine,mllm,module}.py`,
     `memory/procedural_memory.py`, `utils/{common_utils,formatters}.py` (plus `bbon/` only
     if worker needs it). Keep the Apache-2.0 LICENSE and add a NOTICE with the upstream
     commit. Patch the vendored code to:
     - remove `eval` in `create_pyautogui_code`, replaced by the safe parser below
     - drop the pyautogui, cv2, pytesseract and subprocess/code-agent paths
     - rewrite imports to the vendored package path
   - `pyproject.toml` (Python 3.12) depends only on `numpy`, `pillow`, `backoff`, `openai`,
     `anthropic` and `google-genai` (+ `pytest` dev). Lock with uv; the lock must contain
     no paddle, opencv, pyautogui or pytesseract.
   - `taucode_cua/server.py`: a stdio JSON-RPC process (not MCP; the Rust host is its only
     client) with methods `configure(planner, grounding, platform)`, `reset()`,
     `predict(instruction, screenshot_png_b64, screen_size)` →
     `{ action, reflection, plan_text }`, and `health()`
   - planner/grounding engine params are built from registry providers:
     - kind → Agent-S `engine_type`: openai→openai, anthropic→anthropic, google→gemini,
       openrouter→open_router, deepseek→deepseek, ollama→ollama
     - everything OpenAI-compatible (lmstudio, groq, xai, mistral, zai, openai-compatible)
       → `openai` with `base_url`
     - keys arrive via env vars set by the host, never args or files
   - the venv lives at `<app data>/engines/computer/.venv`, created on first enable with a
     progress UI; it's pre-created in the sandbox for tests
2. **Rust `src-tauri/src/computer/`:**
   - **capture:** ScreenCaptureKit with a content filter excluding all taucode windows (by
     window id); the main display first, with Retina scale handled
   - **input:** CGEvent mouse/keyboard (unicode typing for `type`), with key mapping for
     hotkeys
   - **coordinate mapping** from grounding space → display points
   - **loop:** capture → `predict` → validate (policy, denylist, frontmost app via
     NSWorkspace) → approve if needed → execute → emit progress → repeat until done, fail,
     limits or Stop
   - **step log:** screenshot thumbnail, action, reflection; persisted per thread item
   - **permissions:** `CGPreflightScreenCaptureAccess` / `AXIsProcessTrusted` checks, with
     onboarding that deep-links to the right System Settings pane. **Never** try to change
     permissions programmatically.
3. **MCP connector `taucode-computer`:** served by the taucode executable
   (`--tau-computer-mcp`, the same pattern as the proxy and browser), talking to the running
   app over the local socket.
   - Tool `computer_run_task(instruction, apps?: [bundle ids], max_steps?)`: long-running,
     streams MCP progress notifications per step, returns a summary plus the final
     screenshot.
   - Tool `computer_screenshot()` (read-only).
   - Mutating approval applies per the policy above, not per MCP call.
4. **UI:**
   - Settings → Computer Use: enable, planner model and grounding model pickers (from the
     registry; grounding needs width/height presets for UI-TARS-1.5-7B 1920×1080 and
     UI-TARS-72B 1000×1000), allow/deny lists, limits, kill-switch shortcut, permission status
     with "Open System Settings" buttons
   - thread item rendering of runs as a step list with thumbnails
   - the HUD window

## Dependencies

Pre-added and pre-fetched by the orchestrator before you start:
`tauri-plugin-global-shortcut`, `screencapturekit`, `core-graphics`, and the objc2 crates
already present. Don't add others.

## Tests

- **Python** (`pytest` or `unittest`, in the venv): the ast parser (allowlist, literal-only,
  rejects attribute chains / names / calls in args / multiple statements), JSON actions from
  each ACI method, engine-param mapping, no pyautogui execution path.
- **Rust:**
  - coordinate mapping incl. Retina and grounding sizes
  - policy decisions (denylist, secure fields, buy-like detection, limits)
  - kill-switch state machine (held keys released)
  - the loop against a fake sidecar and fake capture/input traits (no real screen or input
    in tests)
  - connector config generation
- **vitest:** settings section, permission states, run item rendering, HUD controls.

## Constraints

Offline sandbox; the venv and Rust deps are pre-installed. Always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. Tests must not capture the screen
or move the mouse. Follow `apps/taucode/AGENTS.md`. Don't touch
protocol/fixtures/crates/docs/agent configs.

## Out of scope

Windows/Linux capture and input, multi-display selection UI, Agent-S4, a local UI-TARS
model server (the user points a provider at one).
