# Agents & Chain of Command

taucode separates model selection from delegation policy. You can keep one model responsible for a task, allow bounded custom delegation, or configure a hierarchy for Code work.

## Choose a delegation mode

Open **Settings → Agents → Defaults**:

| Mode | Behavior |
| --- | --- |
| Single model | Disables subagent delegation for the supported profile. |
| Custom delegation | Uses your configured roles and subagent policy without the command hierarchy. |
| Chain of Command | Enables Code's commander/coordinator/worker/reviewer hierarchy and its verification rules. |

Chain of Command is supported by the compatible pi/OMP Code paths. Unsupported engines reject new turns with a readable error. Chat and Work use a single-model profile when inheriting this mode; they do not become command trees.

The [Engines guide](engines.md) explains custom-runtime trust. A compatible protocol does not prove an arbitrary executable will honor every delegated policy.

## Configure roles and models

Assign a provider/model to a role when you want an explicit choice. Model resolution is:

1. The configured role model.
2. The default subagent model.
3. The parent model.

Only enabled registry models are eligible, and configured model allowlists still apply. In Chain of Command, a child request cannot override the model assigned to its role. Selecting a local or cheaper model must be explicit; inheritance does not promise lower cost.

The command hierarchy keeps canonical core ranks. Children cannot promote themselves to commander. Named specialist roles can participate under the allowed hierarchy; legacy custom roles without a rank remain outside that tree.

## What verification means

Chain of Command gates a successful result on actual reviewer evidence. A parent's statement that a reviewer approved the work is not interchangeable with a completed reviewer's transcript and outcome.

This improves the evidence available to you. It does not establish that a reviewer is independent in every practical sense, that a model cannot miss a bug, or that the result is correct. Review tests and important changes yourself.

## App, project, and conversation policy

App defaults are inherited by projects unless overridden. Existing Code conversations on pi/OMP can have their own saved team override.

The composer's five-star toggle and team editor label whether you are editing **This conversation**, **Project defaults**, or **App defaults**. Confirm that label before saving.

For a conversation, you can override role/model choices, delegation mode, concurrency/depth/per-turn limits, and run time. **Use project team** removes that conversation's override. New conversations inherit project defaults.

The project's daily team token allowance stays project-wide; a conversation cannot raise or disable it. Forks and side chats copy applicable conversation overrides. A side-chat model picker does not edit its parent's team.

Active descendants inherit a snapshot of the root's policy. Edits to another conversation or project do not change a running team's snapshot; the next turn refreshes relevant settings.

## Limits and their scope

| Limit | Scope and behavior |
| --- | --- |
| Maximum concurrency | Active child model work in the root command tree; the commander is excluded. Coordinators can lend a slot while waiting. |
| Maximum children per turn | Shared reservations across the command tree; a descendant cannot create a fresh root allowance. |
| Maximum depth | Bounds delegation depth; does not count as permission to access broader files or tools. |
| Run time | A root run and descendants share one absolute deadline. Zero disables this limit. |
| Daily team tokens | Project-wide reported usage for eligible Code Chain of Command work; resets at 00:00 UTC. Zero disables it. |

Custom delegation retains its own per-delegator behavior rather than inheriting every Chain of Command rule.

Token accounting persists across processes and restarts and uses provider-reported input, output, and cache usage. Historical activity from before the accounting feature is not backfilled. Missing or unavailable accounting stops capped runs instead of silently assuming zero usage.

::: warning Reported usage is not a hard billing cap
Requests already in flight can finish after a token threshold is reached. Unknown model prices remain unknown. Use provider-side budgets as well when you need a billing boundary.
:::

## Follow the actual agent work

Open the **Agents** panel to inspect child status and transcripts. Use the tree to distinguish a running worker, a reviewer, a failed child, and completed work. A nested child becoming idle does not prove the root has finished all verification.

Changing an engine or policy does not retroactively rerun previous work. If a child fails, inspect its error and partial changes before asking the parent to continue.

## Interrupted work

The verified pi/OMP recovery paths preserve checkpointed root output and delegation transcripts after an unexpected engine stop. Owned POSIX processes are stopped on the verified paths, and unfinished commands are not automatically restarted.

The most recent unflushed output can still be lost. Windows cleanup, other engine paths, and power-loss durability need separate verification. Continuing with a follow-up should account for the retained files and completed actions.

## A useful first team

Choose a small task with a deterministic check, assign one implementation role and one review role, and keep concurrency low. Ask the reviewer to run or inspect a specific check and report evidence. Compare that result with single-model execution before expanding the team.

[Evals](evals.md) helps compare engine/model configurations, but one successful example does not establish a general quality or cost advantage.
