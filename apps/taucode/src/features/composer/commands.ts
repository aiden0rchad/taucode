import type { AutocompleteItem } from "./hooks/useComposerAutocomplete";

const taskInstructions = {
  plan: "Plan this task before making changes. Explain the approach, key steps, and how to verify it. Wait for my approval before implementing.",
  goal: "Work toward this goal. Verify the result and report what is complete or blocked.",
};

function taskDraft(command: keyof typeof taskInstructions, body = "") {
  return `${command === "plan" ? "Plan" : "Goal"}: ${body}\n\n${taskInstructions[command]}`;
}

export const basicCommands: AutocompleteItem[] = [
  { id: "sidechat", label: "sidechat", description: "Explore a tangent beside this conversation" },
  { id: "plan", label: "plan", description: "Draft a plan before making changes", insertText: taskDraft("plan"), cursorOffset: 6, replaceTrigger: true },
  { id: "goal", label: "goal", description: "Start a task with a clear outcome", insertText: taskDraft("goal"), cursorOffset: 6, replaceTrigger: true },
  { id: "new", label: "new", description: "Start a fresh conversation" },
  { id: "compact", label: "compact", description: "Summarize this conversation to free context" },
].map(command => ({ ...command, group: "Slash" }));

/** Also handle commands pasted or typed with arguments, without selecting a suggestion. */
export function expandTaskCommand(text: string): { text: string; needsInput: boolean } | null {
  const match = /^\/(plan|goal)(?:\s+([\s\S]*))?$/i.exec(text.trim());
  if (!match) return null;
  const body = (match[2] ?? "").trim();
  return { text: taskDraft(match[1].toLowerCase() as keyof typeof taskInstructions, body), needsInput: !body };
}
