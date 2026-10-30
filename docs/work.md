# Work · spaces, tasks & outputs

Work turns a folder and a brief into a task with a plan, visible progress, approvals, and output files. It is intended for document and knowledge work as well as folder-based automation.

![Work task with its conversation and supporting panel](/screenshots/work.png)

*Offline preview with sample data.*

## Spaces and tasks

A **space** is a folder you choose. It can be a Git repository or an ordinary directory. Optional extra folders provide read-only reference material.

A **task** is a conversation inside that space with a brief, selected model, access settings, status, plan, and outputs. Code projects and Work spaces are separate views; switching modes does not convert a project into a space.

Work uses the dedicated **pi work profile**. The optional OMP engine is not the Work runtime. Chain of Command is a Code feature; Work uses a single-model profile when inheriting that policy, while supported custom delegation is a separate setting.

## Create a task

1. Choose **Work → Add space** and select a test folder.
2. Create a task in that space.
3. Describe the deliverable, relevant source files, destination, and constraints.
4. Choose a configured model and inspect the access policy.
5. Select **Run now** or **Queue**.

For example:

```text
Read the Markdown notes in this folder. Create a two-page research brief and a
summary spreadsheet under outputs/. Cite the source filenames, mark uncertain
claims, and leave the original files unchanged.
```

Start with a small representative input. A model's ability to produce valid files depends on the model, available tools, runtime installation, and the source material.

## Task states

| State | Meaning and next action |
| --- | --- |
| Queued | Waiting for a task slot. Review the brief before it starts. |
| Running | The agent is working; inspect its transcript and plan. |
| Needs input | An approval or question needs attention. Open the task to respond. |
| Done | The agent finished its turn. Review the actual output files. |
| Failed | Read the error and retained progress before following up. |
| Stopped | Execution was interrupted. Existing edits and files remain. |

Task concurrency controls how many queued tasks can run together. Background work continues while the window is hidden, while the app/backend remains running. It is not a scheduled cloud service; quitting the runtime or losing the host can interrupt it.

## Files and approvals

The Work profile directs deliverables into `outputs/` unless told otherwise. It treats deleting or overwriting pre-existing files outside that output area as approval-sensitive, including in broader access modes.

Keep source material and deliverables in separate folders. A clear brief helps, but a sentence in a prompt is not a substitute for access controls or a backup. General shell execution and trusted custom tools do not become a complete OS sandbox merely because they run in a Work space.

Browser, computer, and third-party connectors retain their own approval policies. Approving a task's file access does not imply permission to send messages, upload files, or interact with unrelated apps.

## Bundled document skills

The Work runtime includes a small skill pack and locked Python dependencies for:

| Deliverable | Tooling used by the bundled skills |
| --- | --- |
| Word documents | `python-docx` |
| Spreadsheets | `openpyxl` |
| Presentations | `python-pptx` |
| PDFs | `pypdf` and `reportlab` |
| Charts | `matplotlib` |

The runtime is prepared on demand. Its first installation needs internet access and disk space. The presence of a skill does not guarantee professional layout, numerical correctness, source accuracy, or compatibility with every input file. Open and inspect the exported artifact in the app you intend to use.

## Inspect the plan and outputs

The task conversation shows the agent's work and tool results. The Task panel collects the plan and reported files; output detection combines file-change events and the task's checkpoint comparison where available.

Use the file controls to open or reveal outputs. Keep an eye on paths: a file mentioned in prose is not proof that it exists or that it is the latest revision. Verify important documents and spreadsheet formulas directly.

## Previews and background processes

The Previews panel lists development servers or background commands started through the supported task tooling. It exposes Open, Stop, and Restart controls and the detected local address where available.

Stopping a preview process does not delete its source files. Stopping the task is a separate action. A local preview URL is reachable only where networking and process lifetime allow; it is not a published website.

## Stop, recover, and rewind

Use **Stop task** for one task or **Stop all tasks** for the current running work. Review partial outputs before retrying. A new follow-up should account for files already created.

Checkpoints can support selective file rewind for a space, with the same exclusions and size limits as Code. Rewind cannot undo a website submission, an email, or state changed inside another desktop application. See [checkpoints](checkpoints.md).

## Good first tasks

- Summarize a few notes into one Markdown report.
- Extract a small known table into a spreadsheet and compare it with the source.
- Turn an outline into a short slide deck, then inspect its layout.
- Propose a folder reorganization before authorizing any moves or deletions.

For sensitive files, read [privacy & trust](privacy.md) before choosing a provider or enabling browser/computer tools.
