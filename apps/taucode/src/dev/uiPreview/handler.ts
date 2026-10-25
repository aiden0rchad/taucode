import { createDiagnosticsFixture } from "./diagnostics";
import { builtinIds, parseTheme, type ThemeFile, type ThemeManifest } from "@/features/themes/validate";
import customTheme from "@/features/themes/fixtures/valid-custom.json";
import { defaultFeatures, type Features } from '@settings/features';
import type { BrowserState } from "@/features/browser/browser";
import type { AppSettings, ImportLog, WorkTask, WorkspaceInfo } from "@/types";
import { assistantMessage, createFixtures, diff, makeThread, userMessage, type PreviewThread } from "./fixtures";

type Params = Record<string, unknown>;
type Publish = (workspaceId: string, method: string, params: Params) => void;
const record = (value: unknown): Params => value && typeof value === "object" && !Array.isArray(value) ? value as Params : {};

export function createPreviewHandler(publish: Publish = () => {}, search = "") {
  const state = createFixtures();
  const query = new URLSearchParams(search);
  const diagnostics = createDiagnosticsFixture(query.get("diagnostics") === "empty");
  const mode = query.get("mode");
  if (query.get("dictation") === "ready") { state.settings.dictationEnabled = true; state.settings.features = { ...state.settings.features, dictation: true }; }
  for (const entry of (query.get('features') ?? '').split(',')) {
    // URLSearchParams decodes a literal '+' as a space; accept it and %2B.
    const match = /^([-+ ])([a-zA-Z]+)$/.exec(entry);
    if (match && Object.prototype.hasOwnProperty.call(defaultFeatures, match[2])) state.settings.features = { ...state.settings.features, [match[2] as keyof Features]: match[1] !== '-' };
  }
  state.settings.windowModes = { main: query.has("settings") ? "code" : mode === "chat" || mode === "work" ? mode : "code" };
  const theme = query.get("theme");
  if (theme && (builtinIds.includes(theme) || theme === "system" || theme === customTheme.id)) state.settings.theme = theme;
  let themeFiles: ThemeFile[] = [{ file: customTheme.id+".taucode-theme.json", manifest: customTheme as ThemeManifest }, { file: "invalid-token.taucode-theme.json", error: "Unknown token: --arbitrary" }];
  const warned = new Set<string>();
  const logs: ImportLog[] = [];
  let sequence = 0;
  const browser: BrowserState = { tabs: query.get("panel") === "browser" ? [{ id: "preview-page", title: "Example", url: "https://example.com/", loading: false }] : [], active: query.get("panel") === "browser" ? "preview-page" : "" };
  const browserChannels = new Map<string, { channel: unknown; index: number }>();
  const broadcastBrowser = () => {
    for (const entry of browserChannels.values()) {
      const channel = record(entry.channel);
      if (typeof channel.onmessage === "function") channel.onmessage(structuredClone(browser));
      else {
        const callback = typeof channel.id === "number" ? channel.id : Number(String(entry.channel).replace("__CHANNEL__:", ""));
        const internals = (window as unknown as { __TAURI_INTERNALS__: { runCallback: (id: number, value: unknown) => void } }).__TAURI_INTERNALS__;
        if (Number.isFinite(callback)) internals.runCallback(callback, { message: structuredClone(browser), index: entry.index++ });
      }
    }
  };
  const id = (prefix: string) => `${prefix}-preview-${++sequence}`;
  const page = (data: unknown[]) => ({ result: { data, nextCursor: null } });
  const response = (thread?: PreviewThread) => ({ result: { thread: thread ?? makeThread("missing", "", "New conversation") } });
  const findThread = (threadId: unknown) => [...state.threads, ...state.chatThreads, ...state.workThreads].find(t => t.id === threadId);
  const unknown = (command: string) => {
    if (!warned.has(command)) { warned.add(command); console.warn(`[uiPreview] Unhandled IPC: ${command}`); }
    return /^(list_|get_.*s$)/.test(command) ? [] : null;
  };
  const send = (thread: PreviewThread | undefined, text: string) => {
    if (!thread) return { result: { turn: { id: "missing", status: "completed" } } };
    const turn = { id: id("turn"), status: "completed" as const, items: [
      userMessage(id("user"), text), assistantMessage(id("assistant"), `Preview response: **${text || "Message received"}**\n\nThis conversation is stored in memory. No provider was contacted.`),
    ] };
    thread.turns.push(turn);
    thread.updatedAt = Math.floor(Date.now() / 1000);
    // Deliver after the invoke resolves, as the real asynchronous backend does.
    setTimeout(() => {
      publish(thread.workspaceId, "turn/started", { threadId: thread.id, turn: { id: turn.id } });
      for (const item of turn.items) publish(thread.workspaceId, "item/completed", { threadId: thread.id, turnId: turn.id, item });
      publish(thread.workspaceId, "turn/completed", { threadId: thread.id, turn });
    }, 30);
    return { result: { turn: { id: turn.id, status: "inProgress" } } };
  };
  function chat(action: string, params: Params) {
    const entry = state.chats.find(c => c.id === params.threadId);
    const thread = state.chatThreads.find(t => t.id === params.threadId);
    switch (action) {
      case "list": return state.chats;
      case "read": return response(thread);
      case "start":
      case "fork": {
        const created = makeThread(id("chat"), "chats", "New chat");
        if (action === "fork" && thread) created.turns = structuredClone(thread.turns);
        const model = String(params.model || "openai/gpt-5");
        created.modelProvider = model.split("/")[0];
        created.model = model.slice(model.indexOf("/") + 1);
        state.chatThreads.push(created);
        state.chats.unshift({ ...created, chat: { title: null, pinned: false, deletedAt: null } });
        return response(created);
      }
      case "send": {
        if (entry) entry.updatedAt = Math.floor(Date.now() / 1000);
        const input = Array.isArray(params.input) ? params.input : [];
        return send(thread, input.map(part => String(record(part).text ?? "")).filter(Boolean).join("\n"));
      }
      case "rename": if (entry) { entry.chat.title = String(params.title ?? "New chat"); if (thread) thread.name = entry.chat.title; } return null;
      case "pin": if (entry) entry.chat.pinned = Boolean(params.pinned); return null;
      case "web": if (entry) entry.chat.web = Boolean(params.web); return null;
      case "trash": case "delete": if (entry) entry.chat.deletedAt = Math.floor(Date.now() / 1000); return null;
      case "restore": if (entry) entry.chat.deletedAt = null; return null;
      case "stop":
        if (entry) { entry.running = false; publish("chats", "turn/completed", { threadId: entry.id }); }
        return null;
      default: return unknown(`chat_request:${action}`);
    }
  }
  function work(action: string, params: Params) {
    const task = state.work.tasks.find(t => t.id === params.taskId);
    switch (action) {
      case "list": return state.work;
      case "read": return response(findThread(task?.threadId));
      case "add-space": {
        const path = String(params.path ?? "/preview/new-space");
        const space = { id: id("space"), name: String(params.name || path.split("/").pop()), path, kind: "work" as const, readOnlyFolders: Array.isArray(params.readOnlyFolders) ? params.readOnlyFolders.map(String) : [] };
        state.work.spaces.push(space); return space;
      }
      case "create":
      case "continue": {
        const brief = String(params.brief ?? task?.brief ?? "New task");
        const created: WorkTask = { id: id("task"), spaceId: String(params.spaceId ?? state.work.spaces[0].id), threadId: null, title: brief.slice(0, 64), brief, attachments: Array.isArray(params.attachments) ? params.attachments.map(String) : [], model: String(params.model ?? "openai/gpt-5"), accessMode: params.accessMode === "full-access" || params.accessMode === "read-only" ? params.accessMode : "ask", status: params.runNow ? "running" : "queued", plan: [], outputs: [], pending: [], error: null };
        if (created.status === "running") {
          created.threadId = id("work-thread");
          state.workThreads.push(makeThread(created.threadId, created.spaceId, created.title, [assistantMessage(id("progress"), "I’m reviewing the brief and preparing a plan.")]));
          created.plan = [{ step: "Review the brief", status: "inProgress" }, { step: "Prepare outputs", status: "pending" }];
        }
        state.work.tasks.unshift(created); return created;
      }
      case "rename": if (task) task.title = String(params.title ?? task.title); return task ?? null;
      case "respond": if (task) { task.pending = []; task.status = record(params.answer).decision === "decline" ? "stopped" : "running"; } break;
      case "stop": if (task) task.status = "stopped"; break;
      case "stop-all": state.work.tasks.forEach(t => { if (["running", "needs-input", "queued"].includes(t.status)) { t.status = "stopped"; t.pending = []; } }); break;
      case "concurrency": state.work.concurrency = Math.min(8, Math.max(1, Number(params.count) || 2)); break;
      case "checkpoint": return checkpoint(record(params.request));
      default: return unknown(`work_request:${action}`);
    }
    publish("work", "work/updated", {});
    return task ?? null;
  }
  function checkpoint(params: Params) {
    if (params.action === "list") return { checkpoints: state.checkpoints.filter(c => c.metadata.threadId === params.threadId), conversation: true, engine: "pi", undoAvailable: false };
    if (params.action === "diff") return { changes: [{ path: "src/greeting.ts", status: "modified", diff }], conversation: true };
    return { checkpoints: [], conversation: true, conversationChanged: false, undoAvailable: false };
  }
  function handle(command: string, args: Params): unknown {
    const params = record(args.params);
    switch (command) {
      case "get_app_settings": return state.settings;
      case "update_app_settings": state.settings = { ...state.settings, ...record(args.settings) } as AppSettings; return state.settings;
      case "set_window_mode": if (["code", "chat", "work"].includes(String(args.mode))) state.settings.windowModes = { main: args.mode as "code" | "chat" | "work" }; return null;
      case "list_workspaces": return state.workspaces;
      case "add_workspace": {
        const path = String(args.path ?? "/preview/project");
        const workspace: WorkspaceInfo = { id: id("workspace"), path, name: path.split("/").pop() || "Project", connected: true, kind: "main", settings: { sidebarCollapsed: false } };
        state.workspaces.push(workspace); return workspace;
      }
      case "update_workspace_settings": {
        const workspace = state.workspaces.find(w => w.id === args.id);
        if (workspace) workspace.settings = { ...workspace.settings, ...record(args.settings) };
        return workspace ?? state.workspaces[0];
      }
      case "themes_list": return themeFiles;
      case "window_set_material": case "themes_open_folder": case "themes_reveal_file": case "themes_export": return null;
      case "themes_save": {
        const manifest = parseTheme(JSON.stringify(args.manifest));
        if (builtinIds.includes(manifest.id) || manifest.id === "system") throw new Error("Built-in theme ids are reserved");
        if (!args.replace && themeFiles.some(f => f.manifest?.id === manifest.id)) throw new Error("Theme id already exists");
        themeFiles = [...themeFiles.filter(f => f.manifest?.id !== manifest.id), { file:manifest.id+".taucode-theme.json", manifest }];
        return null;
      }
      case "themes_delete": themeFiles = themeFiles.filter(f => f.manifest?.id !== args.id); return null;
      case "remove_workspace": state.workspaces = state.workspaces.filter(w => w.id !== args.id); return null;
      case "list_threads": return page(state.threads.filter(t => t.workspaceId === args.workspaceId));
      case "read_thread":
      case "resume_thread": return response(findThread(args.threadId));
      case "list_thread_history": return { turns: [], nextCursor: null };
      case "start_thread":
      case "fork_thread":
      case "create_side_chat": {
        const parent = findThread(args.threadId);
        const thread = makeThread(id("thread"), String(args.workspaceId), command === "create_side_chat" ? "Side chat" : "New conversation");
        thread.cwd = state.workspaces.find(w => w.id === args.workspaceId)?.path ?? thread.cwd;
        if (command === "fork_thread" && parent) thread.turns = structuredClone(parent.turns);
        if (command === "create_side_chat" && parent) thread.sideOf = { workspaceId: parent.workspaceId, threadId: parent.id, atItemId: typeof args.atItemId === "string" ? args.atItemId : null };
        state.threads.unshift(thread); return response(thread);
      }
      case "set_thread_name": { const thread = findThread(args.threadId); if (thread) { thread.name = String(args.name); thread.preview = thread.name; } return null; }
      case "archive_thread": state.threads = state.threads.filter(t => t.id !== args.threadId); return null;
      case "send_user_message": return send(findThread(args.threadId), String(args.text ?? ""));
      case "chat_request": return chat(String(args.action), params);
      case "work_request": return work(String(args.action), params);
      case "checkpoint_request": return checkpoint(params);
      case "get_subagent_transcript": return { messages: [assistantMessage("review-result", "All tests pass. No issues found.")], truncated: false };
      case "model_list": return page(state.settings.providers?.flatMap(p => p.models.map(m => ({ id: `${p.id}/${m.id}`, model: `${p.id}/${m.id}`, displayName: m.displayName, description: p.name, isDefault: m.id === "gpt-5", reasoning: m.reasoning, providerId: p.id, providerName: p.name, modelId: m.id, engines: ["pi", "codex", "claude"], supportedReasoningEfforts: m.reasoning?.levels.map(level => ({ reasoningEffort: level, description: level })) ?? [], defaultReasoningEffort: m.reasoning?.default ?? null }))) ?? []);
      case "get_config_model": return { model: "openai/gpt-5" };
      case "engine_test": return { engine: record(args.definition).id, state: "ready", version: "1.2.3", message: "Protocol handshake passed · v1.2.3 (preview)" };
      case "engine_runtime": return ["pi", "omp", "codex", "claude-acp", "browser", "computer"].map(engine => ({ engine, state: "ready", message: engine === "omp" ? "Ready v18.12.6" : "Ready v1.4.0" }));
      case "get_git_status": {
        const files = args.workspaceId === "workspace-atlas" ? [{ path: "src/greeting.ts", status: "M", additions: 1, deletions: 1 }] : [];
        return { branchName: files.length ? "feature/welcome" : "", files, stagedFiles: [], unstagedFiles: files, totalAdditions: files.length, totalDeletions: files.length };
      }
      case "get_git_diffs": return args.workspaceId === "workspace-atlas" ? [{ path: "src/greeting.ts", diff, status: "M" }] : [];
      case "list_git_roots": return args.workspaceId === "workspace-atlas" ? ["/preview/atlas"] : [];
      case "list_git_branches": return [{ name: "feature/welcome", lastCommit: 0 }, { name: "main", lastCommit: 0 }];
      case "get_git_remote": return args.workspaceId === "workspace-atlas" ? "https://example.com/atlas.git" : null;
      case "get_git_log": return { entries: [], total: 0, ahead: 0, behind: 0 };
      case "list_workspace_files": return ["README.md", "src/greeting.ts", "src/greeting.test.ts"];
      case "read_workspace_file": return { content: "export const greeting = 'Hello, world!';\n", truncated: false };
      case "file_read": return { content: "", exists: true, path: "/preview/config.toml" };
      case "get_codex_config_path": return "/preview/config.toml";
      case "get_agents_settings": return { agents: [], features: {}, configPath: "/preview/config.toml" };
      case "skills_list": return { result: { data: [{ cwd: "/preview/atlas", skills: [], errors: [] }] } };
      case "prompts_list": return [];
      case "list_mcp_server_status":
      case "experimental_feature_list":
      case "collaboration_mode_list": return page([]);
      case "connector_request": return params.action === "list" || params.action === "secrets" ? [] : { ok: true, tools: [] };
      case "computer_request": return { running: false, approval: null, steps: [], error: null, permissions: { screen: true, accessibility: true } };
      case "computer_request_permissions": return { screen: false, accessibility: false, unsupported: true };
      case "browser_subscribe": { const subscription = id("browser"); browserChannels.set(subscription, { channel: args.channel, index: 0 }); broadcastBrowser(); return subscription; }
      case "browser_unsubscribe": browserChannels.delete(String(args.id)); return null;
      case "browser_request": {
        if (params.action === "new") { browser.active = id("page"); browser.tabs.push({ id: browser.active, title: "New page", url: "about:blank", loading: false }); }
        if (params.action === "activate") browser.active = String(params.id);
        if (params.action === "close") { browser.tabs = browser.tabs.filter(t => t.id !== params.id); if (browser.active === params.id) browser.active = browser.tabs[0]?.id ?? ""; }
        if (params.action === "navigate") { const tab = browser.tabs.find(t => t.id === browser.active); if (tab) { tab.url = String(params.url); tab.title = tab.url === "about:blank" ? "New page" : new URL(tab.url).hostname; } }
        broadcastBrowser(); return null;
      }
      case "terminal_open": return { id: args.terminalId };
      case "terminal_write": case "terminal_resize": case "terminal_close": return null;
      case "diagnostics_request": return diagnostics(String(args.action), params);
      case "provider_has_api_key": return false;
      case "provider_test_and_fetch": return { ok: true, latencyMs: 12, models: state.settings.providers?.find(p => p.id === args.id)?.models ?? [] };
      case "local_usage_snapshot": return { updatedAt: Date.now(), days: [], topModels: [], totals: { last7DaysTokens: 0, last30DaysTokens: 0, averageDailyTokens: 0, cacheHitRatePercent: 0, peakDay: null, peakDayTokens: 0 } };
      case "import_request": {
        switch (args.action) {
          case "detect": case "export": return state.imports;
          case "logs": return logs;
          case "status": return { running: false, completed: 0, total: 0 };
          case "run": { const log: ImportLog = { id: id("import"), created: [], skipped: [], warnings: ["Preview only: no files were imported."], cancelled: false, undone: false }; logs.push(log); return log; }
          case "undo": { const log = logs.find(l => l.id === params.logId); if (log) log.undone = true; return log ?? null; }
          case "cancel": return null;
          default: return unknown(`import_request:${String(args.action)}`);
        }
      }
      case "codegraph_status": case "codegraph_rebuild": return { state: "ready", detail: "Preview index ready" };
      case "is_mobile_runtime": case "is_macos_debug_build": return false;
      case "tailscale_status": return { installed: false, running: false, version: null, dnsName: null, hostName: null, tailnetName: null, ipv4: [], ipv6: [], suggestedRemoteHost: null, message: "Unavailable in browser preview" };
      case "tailscale_daemon_status": return { state: "stopped", pid: null, startedAtMs: null, lastError: null, listenAddr: null };
      case "tailscale_daemon_command_preview": return { command: "", daemonPath: "", args: [], tokenConfigured: false };
      case "is_workspace_path_dir": return true;
      case "app_build_type": return "debug";
      case "get_open_app_icon": return null;
      case "worktree_setup_status": return { hasScript: false, ran: true };
      case "get_github_issues": return { issues: [], total: 0 };
      case "get_github_pull_requests": return { pullRequests: [], total: 0 };
      case "get_git_commit_diff": case "get_github_pull_request_diff": case "get_github_pull_request_comments": return [];
      case "dictation_request_permission": return true;
      case "dictation_model_status": return { state: query.get("dictation") === "ready" ? "ready" : "missing", progress: null, error: null };
      case "plugin:window|scale_factor": return 1;
      case "plugin:window|inner_size": case "plugin:window|outer_size": return { width: 1440, height: 1000 };
      case "plugin:window|inner_position": case "plugin:window|outer_position": return { x: 0, y: 0 };
      case "plugin:window|is_maximized": case "plugin:window|is_fullscreen": return false;
      case "plugin:window|is_focused": case "plugin:window|is_visible": return true;
      case "plugin:app|version": return "0.0.0-preview";
      case "plugin:app|name": return "taucode preview";
      case "plugin:notification|is_permission_granted": return false;
      case "plugin:dialog|open": case "plugin:dialog|save": case "plugin:updater|check": return null;
      case "connect_workspace": case "set_workspace_runtime_codex_args": case "set_thread_access_mode":
      case "thread_live_subscribe": case "thread_live_unsubscribe": case "turn_interrupt":
      case "menu_set_accelerators": case "set_tray_recent_threads": case "set_tray_session_usage":
      case "work_output": case "provider_set_api_key": case "provider_delete_api_key":
      case "plugin:window|set_title": case "plugin:window|set_theme": case "plugin:window|set_effects":
      case "plugin:webview|set_webview_zoom": return null;
      default: return unknown(command);
    }
  }
  return {
    state,
    handler(command: string, payload?: unknown): unknown {
      if (command === "get_git_status" && record(payload).workspaceId === "workspace-notes") {
        return Promise.reject(new Error("could not find repository at '/preview/notes'; class=Repository (6); code=NotFound (-3)"));
      }
      try { return structuredClone(handle(command, record(payload))); }
      catch { return unknown(command); }
    },
  };
}
