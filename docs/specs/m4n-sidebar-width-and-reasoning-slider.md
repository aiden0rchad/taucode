# M4n: consistent sidebar across modes; a reasoning slider driven by provider capabilities

## Part 1: the sidebar doesn't jump between modes

**Reproduced in the preview:**
- with a stored width of 340 (`localStorage codexmonitor.sidebarWidth=340`), Code's
  sidebar is 340px, but Chat/Work's is 280px
- the mode switcher's "Chat" button moves from x=122 to x=102

**Cause:**
- `useResizablePanels` (`src/features/layout/hooks/useResizablePanels.ts`) writes
  `--sidebar-width` onto Code's app root element (`appRef`)
- Chat/Work (`src/features/chat/chat.css` `.chat-layout`) read `var(--sidebar-width, 280px)`,
  where it's undefined, so they fall back to 280
- they also have no resize handle, and use a different breakpoint width (220px under
  900px)

**Required:**
- One sidebar width state for the whole window: lift the sidebar width (state,
  persistence key, clamp, `MIN 220` / `MAX 420` / `DEFAULT 280`) into a shared
  hook/provider used by all modes, writing `--sidebar-width` on `document.documentElement`
  (or the common shell root).
- Chat and Work render the same resize handle (same component and behaviour as Code's),
  and resizing in any mode updates all modes.
- Identical sidebar chrome geometry in all modes: the same top inset (traffic lights), the
  same padding, the mode switcher at the same x/y and size, the same footer height.
  Remove Chat's separate 220px breakpoint, or make all modes use the same responsive
  rule.
- Collapse state (if Code supports collapsing the sidebar) is also shared.
- **Tests:**
  - the shared hook persists and clamps
  - all three modes read the same width
  - Chat/Work include the resize handle
  - a CSS test: no mode hardcodes a sidebar width other than through `--sidebar-width`

## Part 2: reasoning slider with detected capabilities

**Goal:** the composer shows a **reasoning slider** with only the levels the selected
model actually supports, detected from the provider API when the user runs **Test & fetch
models** (or refreshes). It's hidden for non-reasoning models. The chosen level is
applied to the next turn.

### Capability model (`types.rs` + TS)

Extend `ProviderModel` with:

```
reasoning: {
  supported: bool,
  levels: string[],      // pi level names in order: subset of off, minimal, low, medium, high, xhigh, max
  default: string|null,
  control: "effort" | "thinking-toggle" | "budget" | "adaptive" | "unknown",
  source: "api" | "probe" | "heuristic" | "user",
  detectedAt: number
} | null
```

Older stored data without the field means unknown, so detection runs on the next fetch.

### Detection per provider kind

Run it in the existing fetch-models code path (`providers.rs` / `shared/providers.rs`).
Keys stay in the keychain and child env, as today.

- **LiteLLM proxies** (any OpenAI-compatible base URL whose `/v1/model/info` returns
  data; try it, and use it when present):
  - use `model_info.supports_reasoning`, `model_info.supported_reasoning_efforts` (map
    `none` → `off`; the others map 1:1), and `supported_openai_params`
    (`reasoning_effort` → control `effort`; `thinking` → control `thinking-toggle`, still
    with levels if efforts are listed)
  - `supports_adaptive_thinking` with no efforts → control `adaptive`, levels
    `low, medium, high`
  - **Verified live on the user's lab:**
    - `GPT 6 Astra` gives `[low, medium, high, xhigh, max]`
    - `GLM 5.3` gives `[low, high, max]` with param `thinking`
    - `Claude Sonnet 5.5` gives adaptive with param `reasoning_effort`
- **OpenRouter** `/api/v1/models`: `supported_parameters` includes `reasoning` (or
  `include_reasoning`) → supported, levels `off, low, medium, high` (plus
  `minimal` / `xhigh` only if the API exposes them).
- **Ollama** `/api/show` per model: `capabilities` includes `thinking` → supported. For
  `gpt-oss*` the levels are `low, medium, high`; otherwise `thinking-toggle` with
  `off, high`.
- **Google Gemini** `models.list`: a `thinking` boolean on the model → supported, levels
  `off`/`low`/`medium`/`high` (budget-mapped by pi).
- **Anthropic and OpenAI official APIs** (no capability fields): fall back to a small,
  documented **heuristic table** by model family, with `source: "heuristic"`. Keep it in
  one file with tests.
- **Probe (optional button "Detect reasoning" per model in Settings):**
  - send a minimal request (max_tokens 1, one short message) with the provider's effort
    param at `low`
  - 2xx → supported (`source: "probe"`); a 400 naming the param → unsupported
  - never automatic, because it costs a request
- **User override in Settings → Providers → model row:** toggle supported, pick the
  allowed levels (`source: "user"`), and reset to detected.

### Engines

- **pi** (Code / Chat / Work):
  - when writing pi's `models.json` (`shared/pi/config.rs`), set `reasoning: true` and a
    `thinkingLevelMap` with every pi level → provider value for supported levels and
    `null` for unsupported ones; plus `thinkingFormat` when known (LiteLLM
    `reasoning_effort` → `"openai"`, OpenRouter → `"openrouter"`, the `thinking` param on
    z.ai-style models → `"zai"`, otherwise leave pi's auto-detect)
  - apply the chosen level with pi RPC `set_thinking_level` before sending the turn (see
    pi docs: `engines/pi/node_modules/@earendil-works/pi-coding-agent/docs/rpc.md`,
    `models-configuration.md`); `get_available_thinking_levels` must agree with the
    detected levels (assert in a test with the fake pi)
- **codex:** map the level to `model_reasoning_effort` per turn (the existing effort
  plumbing), clamped to supported levels.
- **claude (ACP):** if the adapter advertises an effort/config option for the session
  (check `dist/acp-agent.js` for set_config_option / effort), map it; otherwise hide the
  slider for claude threads with a tooltip.

### UI

- **Composer (Code / Chat / Work, the shared `ComposerShell`):**
  - a compact **reasoning control** to the left of the model chip: a brain icon plus the
    level label
  - click opens a popover with a **discrete slider**: stops are exactly the model's
    `levels`, labelled (Off · Minimal · Low · Medium · High · XHigh · Max), keyboard
    accessible (arrow keys), with a source note ("from provider API" / "estimated" /
    "set manually")
  - hidden when `supported` is false; shows "?" with a "Detect in Settings" link when
    unknown
- Replace the existing Code "medium" effort dropdown with this control (the same
  semantics).
- **Persistence:** the level is per thread (and remembered per model as the default for
  new threads). Switching to a model that lacks the current level clamps to the nearest
  supported one and tells the user via the chip label.
- **Settings → Providers:** show the detected levels per model as small pills, with the
  override and "Detect reasoning" controls.
- **Preview fixtures:** models with different level sets (5 levels, 3 levels,
  toggle-only, none, unknown), and a thread using each.

### Tests

- **Rust:** detection parsers for LiteLLM (use a fixture copied from the real response
  shape: `supports_reasoning`, `supported_reasoning_efforts`,
  `supported_openai_params`, `supports_adaptive_thinking`), OpenRouter, Ollama and
  Gemini; the heuristic table; probe classification (2xx / 400 / other); the
  `models.json` `thinkingLevelMap` / `thinkingFormat` output; `set_thinking_level` sent
  before a turn; clamping on model switch; serde migration of old provider data.
- **Frontend:** the slider stops equal the levels, keyboard nav, hidden/unknown states,
  the per-thread level, the clamp notice, Settings pills and override.

## Files

`src-tauri/src/{types.rs,providers.rs,shared/providers.rs,shared/pi/**,shared/codex_core.rs,shared/acp/**}`,
`src/features/{layout,chat,work,composer,settings,models}/**`, `src/dev/uiPreview/**`,
`src/types.ts`, `src/services/tauri.ts`.

## Constraints and verification

- No new deps. Offline (use fixtures; live detection is verified by the reviewer).
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust
  parallelism; the known sandbox-only tailscale test may be excluded), plus
  `npm run build -- --mode uipreview`.
- Don't run `tauri build`.
