# M3c: element-first computer use (AX tree grounding), pixels as fallback

## Why

A live test on the user's Mac, with Calculator behind Finder:

- taucode's pipeline grounds to **pixels**, then calls `AXUIElementCopyElementAtPosition`.
  That fails in the background (`-25204`) and falls back to SkyLight event injection, which
  Calculator only partly accepts, while focus-guard restorations fight on every step.
  Result: "2" never registered.
- Driving the same window through trycua's CuaDriver by **Accessibility element token**
  (`get_window_state` → `click {pid, element_token}` → `AXPress`) computed **12 × 7 = 84** in
  the background. Finder stayed frontmost and the cursor never moved: all 6 presses reported
  `route: accessibility`.

So taucode's computer use must be **element-first**.

## Reference (MIT, port; don't depend)

`/Users/rolandleyco/taucode-refs/cua`, commit `ac7a167431329ae04a09dbb7332360cb4e5f4d61`:

- `libs/cua-driver/rust/crates/platform-macos/src/ax/`: AX bindings and tree walking
- `.../src/tools/get_window_state.rs` (or wherever the tool lives; search
  `get_window_state`): snapshot walk, `elements` (index, role, label, value, actions, frame,
  parent_index, depth), element tokens, snapshot invalidation, timeouts and caps
- `.../src/tools/click.rs`, `type_text.rs`, `set_value.rs` (search the names): the AX action
  paths (AXPress, show_menu/pick, AXSetValue, kAXSelectedText insertion)
- docs: `docs/content/docs/cua-driver/reference/mcp-tools/click-and-type.mdx` and
  `apps-windows-and-screen.mdx`

Keep the MIT headers; extend `computer/background/NOTICE`. **No new crates.**

## Required

1. **AX snapshot** (`computer/background/ax_tree.rs`):
   - walk the target window's AX tree with a time budget and element cap, without
     activating or raising the window
   - produce `elements[]`: `index`, `role`, `label` (title / description / help, in the
     reference's order), `value`, `actions`, `frame` (window-local), `parent_index`, `depth`,
     plus a stable in-run handle to re-resolve the AXUIElement for actions
   - snapshot ids invalidate older handles
2. **Element grounding.** A new default grounding mode, `"elements"`, ahead of `"pixels"` and
   `"ui-tars"`:
   - send the sidecar the action description from the planner plus a compact numbered
     element list (role, label, value, actions; truncate long values) and the window
     screenshot
   - the grounding model returns **one element index** or `none`; parse strictly
   - add a sidecar method / ACI variant: the structured action carries `element_index`
     (plus `x`/`y` from the element frame centre for the overlay)
   - when the grounding model returns `none`, or the window has no usable elements
     (canvas / web content without AX), fall back to pixel grounding for that action
3. **Execution:**
   - `click` on an element exposing AXPress → `AXUIElementPerformAction(AXPress)`
   - `type` with overwrite into an element with a settable AXValue → set the value; otherwise
     insert via kAXSelectedText into the element, or into the pid's focused element
   - `scroll` → the reference's AX scroll path if present, else background pixel scroll
   - re-snapshot after each action, so the next planner step sees fresh state
   - record `path: "ax-element"` / `"pixel-skylight"` / `"foreground"` per step
   - keep the policy check on the **element label/role** (risky words, secure text fields
     from `AXSecureTextField`) as well as the description
4. **Planner context:** include a short summary of key elements (role + label, at most ~40,
   interactive first) in the planner's per-step observation text, so it plans in element
   terms. Keep the screenshot.
5. **No regressions:** background mode, focus guard, overlay, HUD, approvals, kill switch and
   all M3 safety rules stay. The pixel paths remain as fallback.

## Tests

- AX tree walk against a fake AX provider: caps, timeouts, handle invalidation, label
  precedence
- element-grounding prompt building and strict parsing (index / none / garbage)
- execution routing (AXPress, set value, selected-text insert, fallback to pixel)
- policy on element labels; secure-field detection
- planner observation summary formatting
- the sidecar: new grounding mode tests

## Verification

Same command as `m3-computer-use.md`. Offline; no new crates; always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. Don't run `tauri build`.
