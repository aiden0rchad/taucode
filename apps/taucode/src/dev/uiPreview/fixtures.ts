import type { ThreadItem } from "@protocol/v2/ThreadItem";
import type { AgentNode, AppSettings, Checkpoint, EngineKind, ImportDetection, SideOf, WorkState, WorkspaceInfo } from "@/types";
import type { Chat } from "@/features/chat/chat";
import { buildDefaultSettings } from "@settings/hooks/useAppSettings";

export type PreviewItem = ThreadItem & { agentsTree?: AgentNode[] };
export type PreviewThread = {
  id: string; workspaceId: string; cwd: string; name: string; preview: string;
  model: string; modelProvider: string; engine: EngineKind; createdAt: number; updatedAt: number;
  capabilities: { fork: boolean; rewind: boolean; steer: boolean };
  sideOf?: SideOf;
  turns: { id: string; status: "completed"; items: PreviewItem[] }[];
};
export const diff = "diff --git a/src/greeting.ts b/src/greeting.ts\n--- a/src/greeting.ts\n+++ b/src/greeting.ts\n@@ -1 +1 @@\n-export const greeting = 'Hello';\n+export const greeting = 'Hello, world!';\n";
export const userMessage = (id: string, text: string): PreviewItem => ({ type: "userMessage", id, clientId: null, content: [{ type: "text", text, text_elements: [] }] });
export const assistantMessage = (id: string, text: string): PreviewItem => ({ type: "agentMessage", id, text, phase: null, memoryCitation: null, delivery: null, questions: null });
export function makeThread(id: string, workspaceId: string, title: string, items: PreviewItem[] = [], now = Date.now()): PreviewThread {
  return {
    id, workspaceId, cwd: workspaceId === "workspace-notes" ? "/preview/notes" : "/preview/atlas",
    name: title, preview: title, model: "gpt-5", modelProvider: "openai", engine: "pi",
    createdAt: Math.floor(now / 1000), updatedAt: Math.floor(now / 1000),
    capabilities: { fork: true, rewind: true, steer: true },
    turns: items.length ? [{ id: `${id}-turn`, status: "completed", items }] : [],
  };
}

export function createFixtures(now = Date.now()) {
  const settings: AppSettings = {
    ...buildDefaultSettings(), theme: "dark", automaticAppUpdateChecksEnabled: false,
    notificationSoundsEnabled: false, systemNotificationsEnabled: false, subagentSystemNotificationsEnabled: false,
    windowModes: { main: "code" }, chatDefaultModel: { providerId: "openai", modelId: "gpt-5" },
    providers: [
      { id: "openai", name: "OpenAI", kind: "openai", enabled: true, auth: { type: "none" }, models: [
        { id: "gpt-5", displayName: "GPT-5", enabled: true, contextWindow: 200000, supportsTools: true, supportsImages: true },
        { id: "gpt-5-mini", displayName: "GPT-5 mini", enabled: true, contextWindow: 128000, supportsTools: true, supportsImages: true },
      ] },
      { id: "anthropic", name: "Anthropic", kind: "anthropic", enabled: true, auth: { type: "none" }, models: [
        { id: "claude-sonnet-4", displayName: "Claude Sonnet 4", enabled: true, contextWindow: 200000, supportsTools: true, supportsImages: true },
      ] },
    ],
    agentPolicy: { defaultEngine: "pi", defaultModel: { providerId: "openai", modelId: "gpt-5" }, subagents: {
      enabled: true, maxConcurrent: 2, maxPerTurn: 4, maxDepth: 2, allowedModels: [], defaultModel: null, roles: [],
    } },
  };
  const workspaces: WorkspaceInfo[] = [
    { id: "workspace-atlas", name: "Atlas", path: "/preview/atlas", connected: true, kind: "main", settings: { sidebarCollapsed: false, gitRoot: "/preview/atlas" } },
    { id: "workspace-notes", name: "Notes", path: "/preview/notes", connected: true, kind: "main", settings: { sidebarCollapsed: false } },
  ];
  const agentsTree: AgentNode[] = [{ id: "agent-review", agent: "Reviewer", model: "openai/gpt-5-mini", task: "Review the greeting change", status: "completed", lastMessage: "The change is correct and covered by tests.", children: [
    { id: "agent-tests", agent: "Test runner", model: "openai/gpt-5-mini", task: "Run focused tests", status: "completed", lastMessage: "All 8 tests pass.", children: [] },
  ] }];
  const richItems: PreviewItem[] = [
    userMessage("code-user", "Update the greeting, run the tests, and ask a reviewer to check the change."),
    assistantMessage("code-intro", "I’ll update the greeting and verify the result.\n\nThe public API stays the same."),
    { type: "plan", id: "code-plan", text: "- [x] Inspect the greeting\n- [x] Update the message\n- [x] Run tests and review" },
    { type: "commandExecution", id: "code-command", command: "npm test -- greeting", cwd: "/preview/atlas", pluginId: null, scriptPath: null, processId: null, source: "agent", status: "completed", commandActions: [], aggregatedOutput: "✓ greeting.test.ts (8 tests)\nTest Files  1 passed\nTests       8 passed\nDuration    218ms", exitCode: 0, durationMs: 218 },
    { type: "fileChange", id: "code-file", status: "completed", changes: [{ path: "src/greeting.ts", kind: { type: "update", move_path: null }, diff }] },
    { type: "collabAgentToolCall", id: "code-collab", tool: "spawnAgent", status: "completed", senderThreadId: "thread-rich", receiverThreadIds: ["agent-review"], prompt: "Review the greeting change and test coverage.", model: "openai/gpt-5-mini", reasoningEffort: null, agentsStates: { "agent-review": { status: "completed", message: "Looks good. All tests pass." } }, agentsTree },
    assistantMessage("code-done", "Updated `src/greeting.ts`. All **8 tests pass**, and the reviewer found no issues.\n\n```ts\nexport const greeting = 'Hello, world!';\n```"),
  ];
  const threads = [
    makeThread("thread-rich", "workspace-atlas", "Polish the welcome message", richItems, now),
    makeThread("thread-api", "workspace-atlas", "Sketch the API client", [userMessage("api-user", "Outline a small API client."), assistantMessage("api-answer", "Use a single fetch wrapper with typed responses and explicit error handling.")], now - 3600000),
    makeThread("thread-notes", "workspace-notes", "Organize project notes", [userMessage("notes-user", "Help organize these notes."), assistantMessage("notes-answer", "Group the notes into **Ideas**, **Decisions**, and **Next steps**.")], now - 86400000),
    { ...makeThread("thread-side", "workspace-atlas", "Why keep the greeting constant?", [userMessage("side-user", "Why use a constant here?"), assistantMessage("side-answer", "It gives the interface and tests one shared source of truth.")], now - 60000), sideOf: { threadId: "thread-rich", workspaceId: "workspace-atlas", atItemId: "code-done" } },
  ];
  const chats: Chat[] = ["Plan a weekend in Kyoto", "Explain async iterators", "Ideas for a team offsite", "A better morning routine", "Notes on good documentation"].map((title, i) => ({
    id: `chat-${i + 1}`, model: "gpt-5", modelProvider: "openai", createdAt: Math.floor(now / 1000) - [0, 0, 2, 5, 20][i] * 86400,
    updatedAt: Math.floor(now / 1000) - [0, 0, 2, 5, 20][i] * 86400, preview: title,
    chat: { title, pinned: false, deletedAt: null, web: false },
  }));
  const chatThreads = chats.map(chat => makeThread(chat.id, "chats", chat.chat.title!, [
    userMessage(`${chat.id}-user`, chat.chat.title!),
    assistantMessage(`${chat.id}-answer`, "## A starting point\n\nKeep the plan simple and leave room to explore.\n\n- Pick one priority for each day.\n- Save a few ideas for later.\n\n```js\nconst priorities = ['explore', 'learn', 'rest'];\npriorities.forEach(item => console.log(item));\n```"),
    userMessage(`${chat.id}-followup`, "Can you make that more flexible?"),
    assistantMessage(`${chat.id}-final`, "Absolutely. Treat these as **options**, and choose what fits your energy that day."),
  ], chat.updatedAt * 1000));
  const work: WorkState = { concurrency: 2, spaces: [
    { id: "space-studio", name: "Studio", path: "/preview/studio", kind: "work", readOnlyFolders: ["/preview/reference"] },
    { id: "space-research", name: "Research", path: "/preview/research", kind: "work", readOnlyFolders: [] },
  ], tasks: (["running", "needs-input", "queued", "done"] as const).map((status, i) => ({
    id: `task-${status}`, spaceId: i % 2 ? "space-research" : "space-studio", threadId: status === "queued" ? null : `work-thread-${i}`,
    title: ["Prepare the launch brief", "Organize research sources", "Draft the weekly update", "Summarize customer interviews"][i],
    brief: "Review the notes, identify the key findings, and write a concise report in outputs/.", attachments: ["notes.md"], model: "openai/gpt-5", accessMode: "ask",
    status, error: null,
    plan: status === "queued" ? [] : [{ step: "Read source notes", status: "completed" }, { step: "Write the brief", status: status === "done" ? "completed" : "inProgress" }, { step: "Check sources", status: status === "done" ? "completed" : "pending" }],
    outputs: ["running", "done"].includes(status) ? ["outputs/brief.md", "outputs/sources.csv"] : [],
    pending: status === "needs-input" ? [{ id: "approval-organize", method: "item/commandExecution/requestApproval", params: { reason: "Move the source notes into a dated archive?", command: "mv notes.md archive/notes.md" } }] : [],
  })) };
  const workThreads = work.tasks.filter(t => t.threadId).map(task => makeThread(task.threadId!, task.spaceId, task.title, [
    assistantMessage(`${task.id}-progress`, "I’ve reviewed the notes and identified three themes. The draft is ready in **outputs/brief.md**."),
  ], now));
  const checkpoints: Checkpoint[] = (["before", "after"] as const).map(kind => ({
    id: `checkpoint-${kind}`, commit: kind === "before" ? "a1b2c3d" : "d4e5f6a", metadata: { threadId: "thread-rich", turnId: "thread-rich-turn", userItemId: "code-user", kind, timestamp: new Date(now - (kind === "before" ? 60000 : 0)).toISOString(), engine: "pi", preview: "Update the greeting" }, skipped: [], warning: null, filesChanged: 1,
  }));
  const imports: ImportDetection = { entries: [
    { key: "codex:preview", source: "codex", id: "import-code", kind: "code", title: "Explore the design system", project: "Atlas", cwd: "/preview/atlas", archived: false, missingCwd: false, needsSecrets: false, warning: null },
    { key: "cowork:preview", source: "cowork", id: "import-work", kind: "work", title: "Quarterly research", project: "Research", cwd: "/preview/research", archived: false, missingCwd: false, needsSecrets: false, warning: null },
  ], warnings: [] };
  return { settings, workspaces, threads, chats, chatThreads, work, workThreads, checkpoints, imports };
}
