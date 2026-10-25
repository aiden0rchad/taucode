# M4k: browser UI preview harness (dev only)

## Goal

Render the real taucode React UI in a plain browser with `npm run preview:ui`, using
**mocked Tauri IPC and fixture data**, so UI work can be checked visually without building
the desktop app.

Today `npm run dev` in a browser crashes immediately:
`getCurrentWindow` → `Cannot read properties of undefined (reading 'metadata')` from
`useUiScaleShortcuts.ts`, because there's no Tauri runtime.

## Required

1. **Entry and mode**
   - `npm run preview:ui` runs Vite in a custom mode (`--mode uipreview`, port 1421,
     strictPort).
   - In that mode only, before the app mounts, a dev module
     `src/dev/uiPreview/index.ts` installs the mocks: `mockWindows("main")` and
     `mockIPC(handler)` from `@tauri-apps/api/mocks` (already part of
     `@tauri-apps/api`; no new deps), plus event `listen`/`emit` support as needed.
   - Guard it with `import.meta.env.MODE === "uipreview"` and a dynamic import, so the
     production bundle (`npm run build`) contains none of it. Add a test or build check
     asserting that the dist doesn't include `uiPreview`.
2. **IPC mock handler**
   - Answers every command the UI calls at startup and in the three modes with realistic
     fixture data, and returns sensible empty defaults for anything unknown. Log unknown
     commands once to the console, and never throw.
   - Fixtures (`src/dev/uiPreview/fixtures.ts`):
     - app settings with 2 providers and a few models
     - 2 workspaces: one git with a branch, one non-git
     - **Code:** 4 threads, one of them with a rich conversation: user/assistant
       messages, a command execution with output, a file change with diff, a plan, a
       `collabAgentToolCall` with an agents tree of 2 nodes, a side chat, checkpoints
     - **Chat:** 5 chats across Today / Previous 7 days / Older; one open with a few
       markdown messages including a code block
     - **Work:** 2 spaces, tasks in each status (running with a plan and outputs,
       needs-input with a pending approval, queued, done)
     - import detection results
   - Mutating calls (send, create, rename) update in-memory state, so clicking around
     works.
3. **URL state for screenshots:** query params select the starting state, e.g.
   `?mode=code&thread=<id>`, `?mode=chat&chat=<id>`, `?mode=work&task=<id>`,
   `?settings=display`, `?theme=dark|light`, so specific screens can be opened directly.
4. **Docs:** a short `src/dev/uiPreview/README.md` on running it and adding fixtures.

## Don't change

No visual or style changes in this task. No behaviour changes to the real app.

## Tests

- vitest: the mock handler returns non-throwing defaults for unknown commands; the
  fixtures conform to the TS types (typecheck).
- The production build excludes the preview module.

## Constraints and verification

- No new deps. Offline.
- Verification: `cd apps/taucode && npm run typecheck && npm test && npm run build && npm run lint`.
  You can't open a browser (the sandbox blocks localhost listeners), so make sure the
  preview entry at least compiles. The human reviewer will view it.
- Don't run `tauri build`.
