# M5m: fresh-install dogfood fixes

Found by the user setting up a fresh install, with screenshots. Fix all of these in
/Users/rolandleyco/taucode-m1w (taucode). Don't commit.

## 1. Wizard: the wordmark collides with the traffic lights
In the first-run takeover, "taucode / setup" sits at the window's top-left and overlaps
the macOS traffic-light buttons (`titleBarStyle: Overlay`).
- Make the top bar a proper drag region (`data-tauri-drag-region`) that's the same
  height as the app's title row.
- Place the wordmark after the traffic lights (left inset ≈ 78px on macOS, matching how
  the main window title row offsets for them; reuse that existing variable or offset if
  there is one), and vertically center it with the traffic lights.
- In the Settings-modal variant, keep the current placement.
- Test: the wordmark's left offset is at least the traffic-light inset on macOS.

## 2. Wizard: the background colors clash
The ambient blobs are currently a gray-blue (top-left) and a greenish tint
(bottom-right).
- Make the two blob colors and the base color tokens: `--setup-blob-1`,
  `--setup-blob-2`, `--setup-base`, plus an optional `--setup-blob-3` for a third
  glow.
- Default them for **dark appearances** to a dark blue / dark purple pair. Use these
  placeholder values; the reviewer will set the final palette:
  - `--setup-base: #0b0e1c`
  - `--setup-blob-1: #1e2a78`
  - `--setup-blob-2: #3b1d6e`
- For light appearances: soft periwinkle `#dfe3ff` / lavender `#ece0ff` on `#f7f7fb`.
- Themes may override these tokens (add them to the theme token allowlist under
  "Setup"). Neon and Matrix keep using their accent-derived colors through overrides.
- Keep the grid overlay and the slow drift.

## 3. Code screen: the project home uses an old reasoning dropdown
`WorkspaceHome.tsx` renders a plain low/medium/high dropdown for reasoning (and a
separate model select). Replace it with the **same combined model + reasoning chip and
slider popover** the thread composer uses (`ReasoningControl` / `ComposerShell`
pattern), including the heat animation, so the project home matches every other
composer. Remove the now-unused `reasoningOptions` / `onSelectEffort` dropdown
plumbing if nothing else uses it.
- Test: WorkspaceHome renders the combined chip, and opening it shows the slider.

## 4. Project home: agent instruction files beyond AGENTS.md
The panel only reads and writes `AGENTS.md`. Support **both `AGENTS.md` and
`CLAUDE.md`**, and also detect `GEMINI.md` and `.github/copilot-instructions.md`
read-only.
- Show tabs or a segmented control: `AGENTS.md` | `CLAUDE.md`, with the status of each
  (Found / Not found).
- Edit the selected file. Saving a not-found file creates it.
- Show detected-but-unsupported files as small "also found" chips that open in the
  editor app (the existing open-in flow).
- Default to whichever exists: AGENTS.md first, else CLAUDE.md.
- Backend: generalize `useWorkspaceAgentMd` and its Rust command to take a filename
  from an allowlist (no arbitrary paths, no `..`).
- Tests: the allowlist; reading and creating CLAUDE.md; the tab statuses.

## 5. Provider shows as a raw generated id
- The wizard creates providers with `id: "setup-<uuid>"`.
- The header ("pi · setup-14ac82be-7a1/Claude…"), the project home and the model chips
  display `providerId/modelId`.
- Fix:
  - **Display:** everywhere a model ref is shown to the user, show the **model display
    name**, with the **provider name** where disambiguation is needed (e.g. "Claude Opus
    5.5" in chips; tooltip "LiteLLM · Claude Opus 5.5"). The header engine label becomes
    "pi · Claude Opus 5.5". Make one helper (`formatModelRef(settings, ref)`) and use it
    everywhere a ref is rendered; find them all.
  - **IDs:** new providers from the wizard get a readable slug id derived from the name
    (`litellm`, `litellm-2` on collision), not a uuid. Don't migrate existing ids (they
    may be referenced by threads); the display fix covers them.
- Tests: the helper output; the wizard slug and collision handling.

## 6. "Ask Codex to do something…" placeholder
The project home composer says "Ask Codex…" even when the engine is omp/pi.
- Use the engine-aware placeholder the thread composer already uses ("Ask taucode to do
  something…", or the engine name if that's the existing pattern). Find all remaining
  hard-coded "Codex" user-facing strings outside codex-specific settings and fix them.

## 7. Codex-only calls error when codex isn't configured
The debug log shows `collaborationMode/list error` and `skills/list error`: "Add and
enable a codex-compatible provider and model in Settings".
- `useCollaborationModes` and `useSkills` call codex app-server methods even when the
  active engine isn't codex, or codex has no provider. Gate them: only call when the
  active/default engine is codex **and** codex is ready.
- Otherwise return empty results silently (no error log entries, no toasts).
- Tests: no call happens for pi/omp; the call happens for codex.

## 8. Non-git folder logs a git error
`could not find repository at '/Users/rolandleyco/tdarrflow'; class=Repository (6); code=NotFound (-3)`.
- A workspace that isn't a git repository must show the calm non-git state with **no
  error log entry**.
- Treat `NotFound` from repository discovery as "not a repo" (`null` status), not an
  error, in every git status/diff/log entry point. Keep logging real git errors.
- Test: a non-repo dir returns the non-git state and produces no error event.

## 9. Chat sidebar rows are cramped
In Chat mode the conversation rows (title + model line) have a tight selected
background: the title touches the top edge, and the model line is bold and the same
weight as the title.
- Rows: 6px 10px padding; the title is 13px regular; the model line is 11.5px muted
  (`--text-muted`), not bold; there's 2px between the lines; the time is
  right-aligned, muted, vertically centered.
- Selected/hover background: the row radius, inset 4px from the sidebar edges.
- Height: about 44px for two-line rows (not taller). Match the spacing feel of the Code
  sidebar rows.
- Test: the computed padding, and that the model line font-weight is 400.

## Verification
The command in `docs/specs/m3-computer-use.md`, with
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`, `$HOME/.cargo/bin` on PATH and
`TMPDIR=/Users/rolandleyco/taucode-m1w/.tmp` (the permitted tailscale exclusion), plus
`npm run build -- --mode uipreview`. No browser or computer use.
