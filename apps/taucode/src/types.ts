export type EngineKind = string;
export type EngineProtocol = "pi-rpc" | "codex-app-server" | "acp";
export type EngineDefinition = { id: string; name: string; protocol: EngineProtocol; builtIn: boolean; command?: string; args?: string[]; env?: Record<string, string>; configDirEnv?: string; enabled: boolean; description?: string };
export type ModelRef = { providerId: string; modelId: string };
export type ProviderKind = "openai" | "anthropic" | "google" | "openrouter" | "groq" | "mistral" | "xai" | "deepseek" | "zai" | "ollama" | "lmstudio" | "openai-compatible";
export type ProviderAuth = { type: "none" } | { type: "apiKey" } | { type: "env"; var: string } | { type: "command"; command: string; args: string[]; timeoutMs?: number };
export type ReasoningCapability = { supported: boolean; levels: string[]; default: string | null; control: "effort" | "thinking-toggle" | "budget" | "adaptive" | "unknown"; source: "api" | "probe" | "heuristic" | "user"; detectedAt: number };
export type ProviderModel = { id: string; displayName?: string | null; enabled: boolean; contextWindow?: number | null; supportsTools?: boolean | null; supportsImages?: boolean | null; reasoning?: ReasoningCapability | null; detectedReasoning?: ReasoningCapability | null };
export type ProviderEntry = {
  id: string; name: string; kind: ProviderKind; baseUrl?: string | null;
  anthropicCompatible?: boolean;
  apiStyle?: "chat-completions" | "responses" | null; auth: ProviderAuth;
  models: ProviderModel[]; modelsFetchedAt?: number | null; enabled: boolean; extraHeaders?: Record<string, string>;
};
export type CommandRank = "colonel" | "major" | "enlisted";
export type AgentRole = { name: string; description: string; model: ModelRef | null; toolAccess: "read-only" | "full"; instructions: string; commandRank?: CommandRank | null };
export type AgentPolicy = {
  defaultEngine: EngineKind; defaultModel: ModelRef | null;
  subagents: { enabled: boolean; mode?: "custom" | "chain-of-command"; maxConcurrent: number; maxPerTurn: number; maxDepth: number; allowedModels: ModelRef[]; defaultModel: ModelRef | null; roles: AgentRole[] };
};
export type AgentPolicyOverride = Partial<Omit<AgentPolicy, "subagents">> & { subagents?: Partial<AgentPolicy["subagents"]> };
export type WorkspaceSettings = {
  mcpServers?: McpServerEntry[];
  agentPolicy?: AgentPolicyOverride | null;
  codegraphEnabled?: boolean | null;
  sidebarCollapsed: boolean;
  sortOrder?: number | null;
  groupId?: string | null;
  cloneSourceWorkspaceId?: string | null;
  gitRoot?: string | null;
  launchScript?: string | null;
  launchScripts?: LaunchScriptEntry[] | null;
  worktreeSetupScript?: string | null;
  worktreesFolder?: string | null;
};

export type LaunchScriptIconId =
  | "play"
  | "build"
  | "debug"
  | "wrench"
  | "terminal"
  | "code"
  | "server"
  | "database"
  | "package"
  | "test"
  | "lint"
  | "dev"
  | "git"
  | "config"
  | "logs";

export type LaunchScriptEntry = {
  id: string;
  script: string;
  icon: LaunchScriptIconId;
  label?: string | null;
};

export type WorkspaceGroup = {
  id: string;
  name: string;
  sortOrder?: number | null;
  copiesFolder?: string | null;
};

export type WorkspaceKind = "main" | "worktree" | "work";

export type WorktreeInfo = {
  branch: string;
};

export type WorkspaceInfo = {
  id: string;
  name: string;
  path: string;
  connected: boolean;
  kind?: WorkspaceKind;
  parentId?: string | null;
  worktree?: WorktreeInfo | null;
  settings: WorkspaceSettings;
};

export type AppServerEvent = {
  workspace_id: string;
  message: Record<string, unknown>;
};

export type TrayRecentThreadEntry = {
  workspaceId: string;
  workspaceLabel: string;
  threadId: string;
  threadLabel: string;
  updatedAt: number;
};

export type TraySessionUsage = {
  sessionLabel: string;
  weeklyLabel: string | null;
};

export type TrayOpenThreadPayload = {
  workspaceId: string;
  threadId: string;
};

export type Message = {
  id: string;
  role: "user" | "assistant";
  text: string;
};

export type CollabAgentRef = {
  threadId: string;
  nickname?: string;
  role?: string;
};

export type AgentNode = {
  id: string;
  agent: string;
  model?: string | null;
  task?: string;
  status: "pending" | "running" | "completed" | "failed" | "aborted";
  startedAt?: number | null;
  endedAt?: number | null;
  usage?: { input?: number; output?: number; cacheRead?: number; cacheWrite?: number; cost?: number | null } | null;
  lastMessage?: string;
  errorMessage?: string | null;
  children: AgentNode[];
};
export type SideOf = { threadId: string; workspaceId: string; atItemId: string | null };

export type CollabAgentStatus = CollabAgentRef & {
  status: string;
  message?: string;
  model?: string;
  usage?: { input: number; output: number; cost: number };
};

export type AgentMessageMetadata = Partial<Pick<
  import("@protocol/v2/ThreadItem").ThreadItem & { type: "agentMessage" },
  "phase" | "memoryCitation" | "delivery" | "questions"
>>;

export type ConversationItem =
  | (AgentMessageMetadata & {
      id: string;
      kind: "message";
      role: "user" | "assistant";
      text: string;
      images?: string[];
      isError?: boolean;
      retryMessage?: { text: string; images?: string[] } | null;
    })
  | {
      id: string;
      kind: "userInput";
      status: "answered";
      questions: {
        id: string;
        header: string;
        question: string;
        answers: string[];
      }[];
    }
  | { id: string; kind: "reasoning"; summary: string; content: string }
  | { id: string; kind: "diff"; title: string; diff: string; status?: string }
  | { id: string; kind: "review"; state: "started" | "completed"; text: string }
  | {
      id: string;
      kind: "explore";
      status: "exploring" | "explored";
      entries: { kind: "read" | "search" | "list" | "run"; label: string; detail?: string }[];
    }
  | {
      id: string;
      kind: "tool";
      toolType: string;
      title: string;
      detail: string;
      status?: string;
      output?: string;
      durationMs?: number | null;
      images?: string[];
      changes?: { path: string; kind?: string; diff?: string }[];
      collabSender?: CollabAgentRef;
      collabReceiver?: CollabAgentRef;
      collabReceivers?: CollabAgentRef[];
      collabStatuses?: CollabAgentStatus[];
      agentsTree?: AgentNode[];
      turnId?: string;
    };

export type ThreadSummary = {
  sideOf?: SideOf;
  engine?: EngineKind;
  capabilities?: { fork: boolean; rewind: boolean; steer?: boolean };
  modelProvider?: string;
  id: string;
  name: string;
  updatedAt: number;
  createdAt?: number;
  modelId?: string | null;
  effort?: string | null;
  isSubagent?: boolean;
  subagentNickname?: string | null;
  subagentRole?: string | null;
};

export type ThreadListSortKey = "created_at" | "updated_at";
export type ThreadListOrganizeMode =
  | "by_project"
  | "by_project_activity"
  | "threads_only";

export type ReviewTarget =
  | { type: "uncommittedChanges" }
  | { type: "baseBranch"; branch: string }
  | { type: "commit"; sha: string; title?: string }
  | { type: "custom"; instructions: string };

export type PullRequestReviewIntent =
  | "full"
  | "risks"
  | "tests"
  | "summary"
  | "question";

export type PullRequestReviewAction = {
  id: string;
  label: string;
  intent: PullRequestReviewIntent;
};

export type PullRequestSelectionLine = {
  type: "add" | "del" | "context";
  oldLine: number | null;
  newLine: number | null;
  text: string;
};

export type PullRequestSelectionRange = {
  path: string;
  status: string;
  start: number;
  end: number;
  lines: PullRequestSelectionLine[];
};

export type AccessMode = "read-only" | "current" | "full-access";
export type ServiceTier = "fast" | "flex";
export type BackendMode = "local" | "remote";
export type RemoteBackendProvider = "tcp";
export type RemoteBackendTarget = {
  id: string;
  name: string;
  provider: RemoteBackendProvider;
  host: string;
  token: string | null;
  lastConnectedAtMs?: number | null;
};
export type ThemePreference = string;
export type PersonalityPreference = "friendly" | "pragmatic";
export type FollowUpMessageBehavior = "queue" | "steer";
export type ComposerSendIntent = "default" | "queue" | "steer";
export type SendMessageResult = {
  status: "sent" | "blocked" | "steer_failed";
  threadId?: string;
};

export type ComposerEditorPreset = "default" | "helpful" | "smart";

export type ComposerEditorSettings = {
  preset: ComposerEditorPreset;
  expandFenceOnSpace: boolean;
  expandFenceOnEnter: boolean;
  fenceLanguageTags: boolean;
  fenceWrapSelection: boolean;
  autoWrapPasteMultiline: boolean;
  autoWrapPasteCodeLike: boolean;
  continueListOnShiftEnter: boolean;
};

export type OpenAppTarget = {
  id: string;
  label: string;
  kind: "app" | "command" | "finder";
  appName?: string | null;
  command?: string | null;
  args: string[];
};

export type McpCredential = { value: string } | { secret: true };
export type BrowserSettings = { engine?: "native" | "cdp"; privateBrowsing?: boolean; binary: string | null; allowedOrigins: string[]; blockedOrigins: string[]; approval: "auto" | "ask"; searchUrl: string };
export type McpServerEntry = {
  id: string; name?: string; enabled: boolean;
  transport?: { type: "stdio"; command: string; args: string[]; cwd?: string | null; env: Record<string, McpCredential> } | { type: "http"; url: string; headers: Record<string, McpCredential>; bearer?: { secret: true } | null } | null;
  toolAllowlist?: string[] | null; toolDenylist?: string[] | null; approval?: "auto" | "ask";
};
export type McpConnectionResult = { ok: boolean; latencyMs: number; tools?: { name: string; description?: string }[]; error?: string };

export type AppMode = "code" | "chat" | "work";
export type CodeEnvironment = { id: string; name: string; workspaceIds: string[]; engine: EngineKind; modelId: string | null };
export type AppSettings = {
  codeEnvironments?: CodeEnvironment[];
  setupComplete?: boolean;
  features?: Partial<import("@settings/features").Features>;
  diagnostics?: { level: "essential" | "standard" | "verbose"; retention: number; includeContentSnippets: boolean; exposeToAgents: boolean };
  engines?: EngineDefinition[];
  windowModes?: Record<string, AppMode>;
  chatDefaultModel?: ModelRef | null;
  computer?: import("@/features/computer/computer").ComputerSettings;
  browser?: BrowserSettings;
  mcpServers?: McpServerEntry[];
  providers?: ProviderEntry[];
  agentPolicy?: AgentPolicy;
  codegraphEnabled: boolean;
  codexBin: string | null;
  codexArgs: string | null;
  backendMode: BackendMode;
  remoteBackendProvider: RemoteBackendProvider;
  remoteBackendHost: string;
  remoteBackendToken: string | null;
  remoteBackends: RemoteBackendTarget[];
  activeRemoteBackendId: string | null;
  keepDaemonRunningAfterAppClose: boolean;
  defaultAccessMode: AccessMode;
  reviewDeliveryMode: "inline" | "detached";
  composerModelShortcut: string | null;
  composerAccessShortcut: string | null;
  composerReasoningShortcut: string | null;
  composerCollaborationShortcut: string | null;
  interruptShortcut: string | null;
  newAgentShortcut: string | null;
  newWorktreeAgentShortcut: string | null;
  newCloneAgentShortcut: string | null;
  archiveThreadShortcut: string | null;
  toggleProjectsSidebarShortcut: string | null;
  toggleGitSidebarShortcut: string | null;
  branchSwitcherShortcut: string | null;
  toggleDebugPanelShortcut: string | null;
  toggleTerminalShortcut: string | null;
  cycleAgentNextShortcut: string | null;
  cycleAgentPrevShortcut: string | null;
  cycleWorkspaceNextShortcut: string | null;
  cycleWorkspacePrevShortcut: string | null;
  lastComposerModelId: string | null;
  lastComposerReasoningEffort: string | null;
  uiScale: number;
  theme: ThemePreference;
  themeLight?: string;
  themeDark?: string;
  themeAnimations?: Record<string, boolean>;
  showMessageFilePath: boolean;
  chatHistoryScrollbackItems: number | null;
  threadTitleAutogenerationEnabled: boolean;
  automaticAppUpdateChecksEnabled: boolean;
  uiFontFamily: string;
  codeFontFamily: string;
  codeFontSize: number;
  notificationSoundsEnabled: boolean;
  systemNotificationsEnabled: boolean;
  subagentSystemNotificationsEnabled: boolean;
  splitChatDiffView: boolean;
  preloadGitDiffs: boolean;
  gitDiffIgnoreWhitespaceChanges: boolean;
  commitMessagePrompt: string;
  commitMessageModelId: string | null;
  collaborationModesEnabled: boolean;
  steerEnabled: boolean;
  followUpMessageBehavior: FollowUpMessageBehavior;
  composerFollowUpHintEnabled: boolean;
  pauseQueuedMessagesWhenResponseRequired: boolean;
  unifiedExecEnabled: boolean;

  personality: PersonalityPreference;
  dictationEnabled: boolean;
  dictationModelId: string;
  dictationPreferredLanguage: string | null;
  dictationHoldKey: string | null;
  composerEditorPreset: ComposerEditorPreset;
  composerFenceExpandOnSpace: boolean;
  composerFenceExpandOnEnter: boolean;
  composerFenceLanguageTags: boolean;
  composerFenceWrapSelection: boolean;
  composerFenceAutoWrapPasteMultiline: boolean;
  composerFenceAutoWrapPasteCodeLike: boolean;
  composerListContinuation: boolean;
  composerCodeBlockCopyUseModifier: boolean;
  workspaceGroups: WorkspaceGroup[];
  globalWorktreesFolder: string | null;
  openAppTargets: OpenAppTarget[];
  selectedOpenAppId: string;
};

export type CodexFeatureStage =
  | "under_development"
  | "beta"
  | "stable"
  | "deprecated"
  | "removed";

export type CodexFeature = {
  name: string;
  stage: CodexFeatureStage;
  enabled: boolean;
  defaultEnabled: boolean;
  displayName: string | null;
  description: string | null;
  announcement: string | null;
};

export type TcpDaemonState = "stopped" | "running" | "error";

export type TcpDaemonStatus = {
  state: TcpDaemonState;
  pid: number | null;
  startedAtMs: number | null;
  lastError: string | null;
  listenAddr: string | null;
};

export type TailscaleStatus = {
  installed: boolean;
  running: boolean;
  version: string | null;
  dnsName: string | null;
  hostName: string | null;
  tailnetName: string | null;
  ipv4: string[];
  ipv6: string[];
  suggestedRemoteHost: string | null;
  message: string;
};

export type TailscaleDaemonCommandPreview = {
  command: string;
  daemonPath: string;
  args: string[];
  tokenConfigured: boolean;
};

export type CodexDoctorResult = {
  ok: boolean;
  codexBin: string | null;
  version: string | null;
  appServerOk: boolean;
  details: string | null;
  path: string | null;
  nodeOk: boolean;
  nodeVersion: string | null;
  nodeDetails: string | null;
};

export type CodexUpdateMethod = "brew_formula" | "brew_cask" | "npm" | "unknown";

export type CodexUpdateResult = {
  ok: boolean;
  method: CodexUpdateMethod;
  package: string | null;
  beforeVersion: string | null;
  afterVersion: string | null;
  upgraded: boolean;
  output: string | null;
  details: string | null;
};

export type ApprovalRequest = {
  workspace_id: string;
  request_id: number | string;
  method: string;
  params: Record<string, unknown>;
};

export type RequestUserInputOption = import("@protocol/v2/ToolRequestUserInputOption").ToolRequestUserInputOption;

export type RequestUserInputQuestion = Pick<
  import("@protocol/v2/ToolRequestUserInputQuestion").ToolRequestUserInputQuestion,
  "id" | "header" | "question"
> & Partial<Pick<import("@protocol/v2/ToolRequestUserInputQuestion").ToolRequestUserInputQuestion,
  "isOther" | "isSecret" | "options"
>>;

export type RequestUserInputParams = {
  thread_id: string;
  turn_id: string;
  item_id: string;
  questions: RequestUserInputQuestion[];
};

export type RequestUserInputRequest = {
  workspace_id: string;
  request_id: number | string;
  params: RequestUserInputParams;
};

export type RequestUserInputAnswer = {
  answers: string[];
};

export type RequestUserInputResponse = {
  answers: Record<string, RequestUserInputAnswer>;
};

export type GitFileStatus = {
  path: string;
  status: string;
  additions: number;
  deletions: number;
};

export type GitFileDiff = {
  path: string;
  diff: string;
  oldLines?: string[];
  newLines?: string[];
  isBinary?: boolean;
  isImage?: boolean;
  oldImageData?: string | null;
  newImageData?: string | null;
  oldImageMime?: string | null;
  newImageMime?: string | null;
};

export type GitCommitDiff = {
  path: string;
  status: string;
  diff: string;
  oldLines?: string[];
  newLines?: string[];
  isBinary?: boolean;
  isImage?: boolean;
  oldImageData?: string | null;
  newImageData?: string | null;
  oldImageMime?: string | null;
  newImageMime?: string | null;
};

export type GitLogEntry = {
  sha: string;
  summary: string;
  author: string;
  timestamp: number;
};

export type GitLogResponse = {
  total: number;
  entries: GitLogEntry[];
  ahead: number;
  behind: number;
  aheadEntries: GitLogEntry[];
  behindEntries: GitLogEntry[];
  upstream: string | null;
};

export type GitHubIssue = {
  number: number;
  title: string;
  url: string;
  updatedAt: string;
};

export type GitHubIssuesResponse = {
  total: number;
  issues: GitHubIssue[];
};

export type GitHubUser = {
  login: string;
};

export type GitHubPullRequest = {
  number: number;
  title: string;
  url: string;
  updatedAt: string;
  createdAt: string;
  body: string;
  headRefName: string;
  baseRefName: string;
  isDraft: boolean;
  author: GitHubUser | null;
};

export type GitHubPullRequestsResponse = {
  total: number;
  pullRequests: GitHubPullRequest[];
};

export type GitHubPullRequestDiff = {
  path: string;
  status: string;
  diff: string;
};

export type GitHubPullRequestComment = {
  id: number;
  body: string;
  createdAt: string;
  url: string;
  author: GitHubUser | null;
};

export type TokenUsageBreakdown = {
  totalTokens: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
};

export type ThreadTokenUsage = {
  total: TokenUsageBreakdown;
  last: TokenUsageBreakdown;
  modelContextWindow: number | null;
};

export type LocalUsageDay = {
  day: string;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  totalTokens: number;
  agentTimeMs: number;
  agentRuns: number;
};

export type LocalUsageTotals = {
  last7DaysTokens: number;
  last30DaysTokens: number;
  averageDailyTokens: number;
  cacheHitRatePercent: number;
  peakDay: string | null;
  peakDayTokens: number;
};

export type LocalUsageModel = {
  model: string;
  tokens: number;
  sharePercent: number;
};

export type LocalUsageSnapshot = {
  updatedAt: number;
  days: LocalUsageDay[];
  totals: LocalUsageTotals;
  topModels: LocalUsageModel[];
};

export type TurnPlanStepStatus = "pending" | "inProgress" | "completed";

export type TurnPlanStep = {
  step: string;
  status: TurnPlanStepStatus;
};

export type TurnPlan = {
  turnId: string;
  explanation: string | null;
  steps: TurnPlanStep[];
};



export type CreditsSnapshot = {
  hasCredits: boolean;
  unlimited: boolean;
  balance: string | null;
};





export type QueuedMessage = {
  id: string;
  text: string;
  createdAt: number;
  images?: string[];

};



export type ModelOption = {
  reasoning?: ReasoningCapability | null;
  acpReasoning?: ReasoningCapability | null;
  providerId?: string;
  providerName?: string;
  modelId?: string;
  engines?: EngineKind[];
  id: string;
  model: string;
  displayName: string;
  description: string;
  supportedReasoningEfforts: { reasoningEffort: string; description: string }[];
  defaultReasoningEffort: string | null;
  isDefault: boolean;
};

export type CollaborationModeOption = {
  id: string;
  label: string;
  mode: string;
  model: string;
  reasoningEffort: string | null;
  developerInstructions: string | null;
  value: Record<string, unknown>;
};

export type SkillOption = {
  name: string;
  path: string;
  description?: string;
};



export type CustomPromptOption = {
  name: string;
  path: string;
  description?: string;
  argumentHint?: string;
  content: string;
  scope?: "workspace" | "global";
};

export type BranchInfo = {
  name: string;
  lastCommit: number;
};

export type DebugEntry = {
  id: string;
  timestamp: number;
  source: "client" | "server" | "event" | "stderr" | "error";
  label: string;
  payload?: unknown;
};

export type TerminalStatus = "idle" | "connecting" | "ready" | "error";

export type DictationModelState = "missing" | "downloading" | "ready" | "error";

export type DictationDownloadProgress = {
  totalBytes?: number | null;
  downloadedBytes: number;
};

export type DictationModelStatus = {
  state: DictationModelState;
  modelId: string;
  progress?: DictationDownloadProgress | null;
  error?: string | null;
  path?: string | null;
};

export type DictationSessionState = "idle" | "listening" | "processing";

export type DictationEvent =
  | { type: "state"; state: DictationSessionState }
  | { type: "level"; value: number }
  | { type: "transcript"; text: string }
  | { type: "error"; message: string }
  | { type: "canceled"; message: string };

export type DictationTranscript = {
  id: string;
  text: string;
};

export type Checkpoint = {
  id: string;
  commit: string | null;
  metadata: { threadId: string; turnId: string; userItemId: string; kind: "before" | "after" | "pre-rewind"; timestamp: string; engine: string; preview?: string };
  skipped: string[];
  warning: string | null;
  filesChanged: number;
};
export type CheckpointChange = { path: string; status: "modified" | "added since" | "deleted since"; diff: string };
export type CheckpointResponse = {
  checkpoints?: Checkpoint[];
  changes?: CheckpointChange[];
  conversation?: boolean;
  engine?: string;
  undoAvailable?: boolean;
  conversationChanged?: boolean;
  text?: string;
  error?: string | null;
};

export type WorkSpace = {
  id: string; name: string; path: string; kind: "work"; readOnlyFolders: string[];
  readOnly?: boolean;
};
export type WorkStatus = "queued" | "starting" | "stopping" | "running" | "needs-input" | "done" | "failed" | "stopped";
export type WorkRequest = {
  id: string; method: string;
  params: { reason?: string; command?: string; diffPreview?: string; questions?: { question: string; options?: { label: string }[] }[] };
};
export type WorkTask = {
  /** Optional Unix timestamps in seconds. */
  createdAt?: number; updatedAt?: number;
  importId?: string | null;
  pendingMessage?: string | null;
  effort?: string | null;
  id: string; spaceId: string; threadId: string | null; title: string; brief: string;
  attachments: string[]; model: string; accessMode: "ask" | "read-only" | "full-access"; status: WorkStatus;
  plan: TurnPlanStep[]; outputs: string[]; pending: WorkRequest[]; error: string | null;
};
export type WorkState = { spaces: WorkSpace[]; tasks: WorkTask[]; concurrency: number };
export type ImportEntry = {
  key: string; source: string; id: string; kind: string; title: string; project: string; cwd: string;
  archived: boolean; missingCwd: boolean; needsSecrets: boolean; warning: string | null;
};
export type ImportDetection = { entries: ImportEntry[]; warnings: string[] };
export type ImportLog = {
  id: string; created: { key: string; title: string; kind: string; threadId: string; workspaceId: string | null }[];
  skipped: string[]; warnings: string[]; cancelled: boolean; undone: boolean;
};
export type ImportProgress = { running: boolean; completed: number; total: number; phase?: string };
export type ImportedTranscript = {
  title: string; source: string; kind: string; cwd: string; readOnly: boolean; warning: string | null;
  messages: { role: string; text: string }[]; raw: unknown[] | null; attachments: string[];
};
