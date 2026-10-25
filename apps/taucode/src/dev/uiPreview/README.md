# Browser UI preview

From `apps/taucode`, run `npm run preview:ui`, then open
<http://127.0.0.1:1421>. This renders the real React app with Tauri window, IPC,
and event mocks. No desktop build, provider credentials, or new dependencies are
needed. The port is strict: an occupied port fails instead of selecting another.

Direct screenshot URLs:

- `/?mode=code&thread=home` — Code home dashboard.
- `/?mode=code&thread=thread-empty` — new Code thread and starter pills.
- `/?mode=chat&chat=new` or `/?mode=work&task=new` — empty states.
- `/?mode=chat&chat=new&composer=attachments` — image and file attachment chips.
- `/?mode=chat&chat=new&dictation=ready` or `/?mode=work&task=new&dictation=ready` — available dictation controls.
- `/?mode=chat&chat=chat-streaming` — responding state and Stop control.
- `/?mode=code&thread=thread-rich` — messages, plan, command output, diff, agents.
- `/?mode=code&thread=thread-notes` — the non-git workspace.
- `/?mode=chat&chat=chat-1` — markdown conversation and dated chat groups.
- `/?mode=work&task=task-running` — plan and outputs.
- `/?mode=work&task=task-needs-input` — pending approval.
- `/?mode=work&task=task-queued` or `task-done` — other task states.
- `/?settings=appearance&theme=light` — appearance settings; `theme=dark` also works. All built-in IDs and custom-forest are supported; the Appearance gallery includes an invalid-file fixture.

Missing or invalid IDs select the first fixture. Settings URLs open Code mode.
All feature flags default to on, as in the app. Use
`?features=-reasoningEffects,-dictation,-git` to preview disabled features.
Prefix a feature with `+` (or `%2B`) to enable it; entries apply left to right,
unknown names are ignored, and disabling Git also disables worktrees.
Feature overrides take precedence over `dictation=ready`.
Selections apply once per page load, so subsequent navigation remains interactive.
Use `?mode=code&thread=thread-rich&sidechat=thread-side` for a populated side-chat panel tab.
Every settings section is available through `?settings=<section>`; Models & Agents includes Defaults, Subagents, Roles and Engines.

Use the Agents panel, Side chat list, and Rewind action in `thread-rich` to inspect
the additional fixtures. Settings → Import → Detect local sources shows mock imports.

Sending, creating, renaming, pinning, and task approvals update in-memory state.
Reloading resets it. File dialogs are cancelled; native OS actions and real engines
are unavailable. Imports only produce a preview log. Nothing touches project files
or contacts a model provider. Sentry is disabled in this mode.

Add typed data in `fixtures.ts`; each handler instance gets fresh fixtures.
Use protocol `ThreadItem` shapes for conversation items, and app types for settings,
workspaces, tasks, imports, and checkpoints. Add IPC commands/actions in `handler.ts`
using the response shapes consumed by `src/services/tauri.ts`. Unknown commands log
once and return an empty default. Sending publishes the same app-server events the
real UI subscribes to.

Verification:

```sh
npm run typecheck && npm test && npm run build && npm run lint
```

`npm run build` fails if preview modules or their marker appear in a production
chunk. To compile the preview entry without opening a listener:

```sh
npx --no-install vite build --mode uipreview --outDir node_modules/.tmp/ui-preview-dist
```

In a sandbox that cannot write to the system temp directory, create
`node_modules/.tmp` and set `TMPDIR="$PWD/node_modules/.tmp"` for verification.
