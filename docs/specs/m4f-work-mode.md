# M4f: Work mode (Cowork-style agentic tasks for non-coding work)

## Goal

A **Work** mode, the third top-level mode next to Code and Chat (M4d adds the switcher).
In it the user hands taucode multi-step knowledge-work **tasks** on a folder they choose.
Examples:
- "organize my receipts folder by month and make a summary spreadsheet"
- "research X and write a 2-page brief"
- "turn these notes into a slide deck"

The agent plans, works in the background, shows progress and produces output files. Like
Claude Cowork / Codex work, but provider-agnostic and with taucode's safety model.

## Concepts

- **Space:** a user-selected folder (any folder, git or not) plus optional extra
  read-only folders. Stored like a workspace with `kind: "work"`; Code mode doesn't list
  it.
- **Task:** a thread in a space with:
  - a title
  - an initial brief and attachments (copied into `<space>/.taucode/uploads/` or a
    chosen inbox)
  - a status: queued, running, needs-input, done, failed, stopped
  - a plan checklist
  - an outputs list
  - optional schedule (out of scope for now; note only)

## Required

### 1. Engine profile

pi with a **work profile**:
- **Tools:** read/write/edit/ls/find/grep, bash (gated), the taucode browser connector,
  the taucode computer-use connector (when enabled in settings), user connectors, the
  subagents tool (policy applies).
- **System prompt addendum (work):**
  - you're doing knowledge work in this folder
  - plan first with a checklist (pi plan / todo tool, or a markdown checklist the
    translator turns into a `plan` item)
  - keep outputs in an `outputs/` subfolder unless told otherwise
  - ask before deleting or overwriting user files
  - summarize what was produced
- **Access:**
  - the default for Work tasks is `ask`
  - **regardless of mode, deleting or overwriting any pre-existing file outside
    `outputs/` requires approval**: a gate rule keyed on the workspace kind, implemented
    in `taucode-gate` policy (`lib/policy.ts`) plus tests
  - writes stay inside the space (existing workspace boundary)
- **Checkpoints:** M4b checkpoints apply (spaces get the shadow-git snapshots), so every
  task can be rewound.

### 2. Document skills (bundled)

- Ship a small skills pack under `apps/taucode/engines/work/skills/`, as pi skills
  (check pi 1.4.0's skills support and SKILL.md format; follow it) for:
  - **docx:** python-docx
  - **xlsx:** openpyxl
  - **pptx:** python-pptx
  - **pdf:** read with pypdf; write via reportlab, or HTML → PDF with the taucode
    browser's print
  - **charts:** matplotlib → PNG
- Python deps come from a new uv-managed engine `apps/taucode/engines/work/` (`pyproject`
  + `uv.lock`), installed like the `computer` engine (`engine_runtime.rs`).
- **The lockfile must be generated with network access by the human, before delegation:
  if `engines/work/uv.lock` doesn't exist when you start, create `pyproject.toml` with
  pinned versions, leave a clear TODO in your report, and make the skills degrade
  gracefully** (the skill tells the agent the library is unavailable).

### 3. UI (Work mode)

- **Sidebar:**
  - Spaces (add folder via picker; extra read-only folders)
  - under each space, Tasks grouped by status (Running, Needs input, Queued, Done)
- **New task composer:**
  - brief, attachments (drag and drop), space selector
  - model picker (registry), access mode
  - "Run now" / "Queue"
  - queue concurrency setting (default 2 tasks at once, global)
- **Task view:**
  - left: conversation/progress (reuses message rendering)
  - right: **Plan** (checklist live from plan items) and **Outputs** (files created or
    modified by the task: from fileChange items plus a folder diff at task end via M4b
    checkpoints), each with Open (default app) / Reveal in Finder / Quick Look preview
    for images/PDF
- **Background running:** tasks keep running when the window is hidden. Use the existing
  notification feature on done / needs-input / failed. Dock badge with the count of
  needs-input tasks.
- **Needs input:** approvals and questions bubble to a "Needs input" inbox at the top of
  the sidebar.
- **Templates:** 5 starter templates in the empty state (organize folder, research brief,
  spreadsheet from documents, slide deck from notes, clean up downloads; the last one is
  read-only first: propose, then approve).

### 4. Safety

- A task can't leave its space (except the extra read-only folders).
- The browser and computer-use connectors keep their own approval rules.
- Stopping a task interrupts it and marks it stopped.
- A "Stop all tasks" control.

## Tests

- **Gate policy:** delete/overwrite of pre-existing files outside `outputs/` needs
  approval in every access mode for `kind=work`; Code workspaces unchanged.
- **Rust:** work profile spawn args and prompt, queue concurrency, status transitions,
  outputs detection (fileChange plus checkpoint diff), notifications on transitions.
- **Frontend:** mode views, spaces/tasks sidebar grouping, composer queueing, plan and
  outputs panes, needs-input inbox, templates.
- **Skills:** each SKILL.md is valid per pi's format; the python helpers have unit tests
  that skip when deps are absent.

## Files

`apps/taucode/engines/work/**` (new), `engines/pi/extensions/lib/policy.ts` +
`taucode-gate.ts` (work rule only) + tests, `shared/pi/**` (work profile), workspace kind
in storage/types, `src/features/work/**` (new), app shell (mode switcher integration
with M4d), `src/services/tauri.ts`, `src/types.ts`, `engine_runtime.rs` (work engine).

## Constraints and verification

- No new crates or npm deps (Python deps only in `engines/work`). Offline;
  worktree-relative `CARGO_HOME`; verification as in M4a, plus `(cd engines/pi && npm
  test)`. Don't run `tauri build`.

## Out of scope

Scheduled/recurring tasks, VM sandboxing (Cowork runs a VM; taucode relies on the gate,
the workspace boundary and checkpoints), cloud sync.
