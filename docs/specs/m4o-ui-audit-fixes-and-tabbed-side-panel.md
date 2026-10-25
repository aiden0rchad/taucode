# M4o: UI audit fixes, a Codex-style title bar, and a tabbed side panel (tools + browser)

## Context

The reviewer audited every screen in the preview harness (`npm run preview:ui`, port 1421)
at 1136px, 1440px and 960px, in dark and light, with a DOM checker for overlaps, clipped
or off-screen controls, unlabelled controls and font sizes, plus screenshots. The user
supplied Codex desktop screenshots as the reference for the title bar and the right-side
panel. Keep the current visual language (M4m tokens, shared composer, flat rows); **don't
rename CSS variables**. The undefined-var test must keep passing.

## Part A: Codex-style title bar and chrome

The reference: one ~40px top row across the whole window, containing:
- **left:** the macOS traffic lights (overlay title bar; keep `trafficLightPosition`),
  then back / forward arrows (navigate thread history), then the **sidebar toggle**
- **centre-left:** the current thread / chat / task title, with a small icon (folder for
  Code threads)
- **right:** a `⋯` overflow menu (thread actions: rename, copy, export, archive), the
  **side-panel toggle**, and a "new window/tab" style icon if one applies

**Required:**
1. Move the sidebar toggle out of the main header into this row, right after the traffic
   lights (with a fixed inset that clears them).
   - The empty band above the mode switcher and the empty top-left area when the
     sidebar is collapsed (both circled by the user) disappear.
   - The sidebar content (mode switcher etc.) starts directly under the title row.
2. The title row is the drag region (`data-tauri-drag-region` on its empty parts; buttons
   aren't draggable).
3. Below the title row, the main header keeps only context chips and actions that belong
   to the thread (engine chip, branch chip, Run, Open in editor, Side chat).
   - It must **never overlap at any width**: measured at 1136px, the branch chip overlapped
     "Set launch script", and the title "Atlas" disappeared.
   - Use `flex-wrap: nowrap`, a shrinking title with ellipsis, chips with min-width and
     ellipsis, and below a width threshold move the lower-priority actions (Open in
     editor, terminal toggle, copy) into the `⋯` menu.
4. Remove the stale "**Codex**" label from the 960px tablet icon rail (use "taucode" /
   mode names) and check every UI string for "Codex" outside the codex-engine context.

## Part B: tabbed side panel (tools and browser in one place, like Codex desktop)

Replace today's right panel (icon tabs for git/files/prompts/agents plus a "Diff" button,
which overflows: "Diff" was cut off and "Git panel view" / "Discard changes" sat
off-screen) **and** the separate browser dock with **one tabbed side panel**.

- **Tab strip at the top of the panel:**
  - tabs like a browser's (icon + title + ×), a "+" button for a new tab, horizontally
    scrollable when crowded
  - the expand/collapse control at the right end
- **Tab kinds:**
  - **Changes** (the existing git/diff panel)
  - **Files**
  - **Terminal** (the existing terminal panel)
  - **Agents** (the M4a agents tree)
  - **Prompts**
  - **Side chat** (opens the M4a side chat in a tab instead of the split)
  - **Browser pages:** the existing taucode browser view, one tab per page, with an
    address bar (back / forward / reload, URL/search field, download and ⋯ icons)
- **New tab page** (shown for "+", and when the panel opens without tabs):
  - a "Tools" grid of two-column rows, each with an icon, a label and a right-aligned
    shortcut: Changes ⌃⇧G, Terminal ⌃`, Files ⌘P, Side chat ⌥⌘S, Agents, New page
    (browser); reuse existing shortcuts where they exist and don't clash
  - a "Suggested" row of up to 4 recently visited sites from the taucode browser history,
    with a favicon (if available locally) and a title
  - an address bar at the top: typing a URL opens a browser tab
- **Header:** the title-bar side-panel toggle opens and closes the panel. The globe
  button in the header goes away (the browser lives in the panel); keep a "Browser"
  entry in the `⋯` menu and a shortcut that opens a new browser tab.
- **Persistence:** open tabs and the active tab persist per thread (Code) / chat / task.
  The panel width uses the existing right-panel resize logic, with a sane minimum (≥ 320)
  so its tab strip never overflows.
- **All modes:**
  - Chat and Work get the panel too (Browser, Side chat where it applies, Files for Work
    spaces)
  - Work's Plan/Outputs pane becomes a "Task" tab in that panel, open by default on a
    task
- **Behaviour is unchanged:** existing panel contents, browser approvals,
  connectors and shortcuts all work as today.

## Part C: bugs and polish from the audit

1. **Conversation column:** centre the message column and composer with a max-width of
   **780px** and at least 32px side padding.
   - Today the column hugs the left edge (17px) when the sidebar is collapsed.
   - The tool-call group, plan card and code blocks align to the same column edges.
2. **Plan-ready card hidden under the composer:** at 960px its buttons are covered by the
   composer. The card must be in normal flow above the composer and fully visible:
   scroll the conversation to keep it in view, and never let it sit behind the composer
   (a z-index/scroll-padding fix: the message list needs `scroll-padding-bottom` /
   padding equal to the composer's height).
3. **Minimum text size:** nothing below **11px**.
   - Today sidebar meta (`13 conversations · Updated`), engine/model chips (`pi`,
     `gpt-5`), times (`now`, `1m`), labels (`Branch`, `Repository root`), status words
     (`running`, `queued`) and badges render at 10px, and the `src` badge at 8.8px.
   - Raise the meta/caption token to 11.5px and badges to 11px; keep the hierarchy via
     colour and weight.
4. **Overlaps:**
   - **Chat/Work sidebars:** the "Search chats/tasks" control overlaps "New chat/task" and
     a second "Search conversations" control. There must be one search control: the
     magnifier toggles an inline field that replaces the row instead of overlaying it.
   - **Work sidebar:** the task list overlaps the footer ("Organize research sources" ×
     "Open settings"). The list must scroll inside its own area above the footer.
   - **Work:** the "Concurrent tasks" menu overlaps list rows. It should be a popover
     (portal) and closed by default.
5. **Accessibility:**
   - workspace rows, thread rows and diff rows are clickable `div`s; make them buttons or
     `role="button"` with `tabIndex=0`, Enter/Space handlers and accessible names
   - add `aria-label`s to the composer textarea, the commit message, the plan-change
     textarea, the settings toggles (`.settings-toggle` buttons have no name) and the
     unnamed `INPUT`
   - visible focus rings everywhere (the existing token)
6. **Settings:**
   - the nav order puts Import and Computer Use first: reorder to Providers, Models &
     Agents, Connectors, Composer, Display & Sound, Shortcuts, Dictation, Git, Open in,
     Projects, Environments, Browser, Computer Use, Server, Import, About (group with
     small section labels: Core / Workspace / Tools / Advanced)
   - the preview deep link `?settings=providers` opened Display & Sound: make
     `?settings=<section>` select that section
   - the settings modal uses the same type scale and minimum sizes
7. **Light theme:** the selected sidebar row in Chat is barely distinguishable. Raise the
   selected-row fill contrast to at least 1.25:1 against the sidebar in light (and check
   dark).

## Tests

- Title row: sidebar toggle position, drag region, no main-header overlap. A layout test
  computes the header children's widths for 960/1136/1440 container widths with mocked
  rects (or a pure layout function) and asserts that overflowing actions move into `⋯`.
- Side panel:
  - tab open/close/reorder persistence
  - the new tab page lists the tools with shortcuts
  - an address-bar URL opens a browser tab
  - the old right-panel icon strip and the floating/globe browser entry are gone
  - Work's task tab is open by default
- Min font size: a CSS scan for `font-size` values below 11px (px or rem-equivalent) in
  `src/**/*.css`.
- Accessibility: rows are focusable buttons; the labelled controls listed above have
  names.
- Settings order and the deep link.
- Preview fixtures cover browser tabs, a terminal tab and the new tab page.

## Files

`apps/taucode/src/**` (layout, app shell, title bar, header, sidebar, right panel /
browser / terminal / git / files / agents / sidechats placements, chat, work, settings,
styles, dev/uiPreview), `src-tauri/tauri.conf.json` only if title bar settings need it.
No backend behaviour changes (browser history for "Suggested" may use the existing
browser data API; if none exists, read the last visited URLs that the frontend already
keeps, or leave Suggested empty with a placeholder, and report).

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust parallelism;
  the known sandbox-only tailscale test may be excluded), plus
  `npm run build -- --mode uipreview`.
- Don't run `tauri build`. Don't try to open a browser or use computer use; the reviewer
  checks visuals.
