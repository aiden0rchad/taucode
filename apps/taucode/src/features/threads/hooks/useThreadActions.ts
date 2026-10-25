import { useCallback, useRef, useState } from "react";
import type { Dispatch, MutableRefObject } from "react";
import type {
  DebugEntry,
  ThreadListSortKey,
  ThreadSummary,
  WorkspaceInfo,
} from "@/types";
import {
  archiveThread as archiveThreadService,
  forkThread as forkThreadService,
  listThreads as listThreadsService,
  listThreadHistory,
  listWorkspaces as listWorkspacesService,
  resumeThread as resumeThreadService,
  startThread as startThreadService,
} from "@services/tauri";
import {
  getThreadTimestamp,
  buildItemsFromThread,
} from "@utils/threadItems";
import { extractThreadCodexMetadata } from "@threads/utils/threadCodexMetadata";
import {
  buildThreadSummaryFromThread,
  extractEngineCapabilities,
  extractThreadFromResponse,
} from "@threads/utils/threadSummary";
import { asString } from "@threads/utils/threadNormalize";
import { engineCapabilities } from "@threads/utils/engineCapabilities";
import {
  getParentThreadIdFromThread,
  shouldHideSubagentThreadFromSidebar,
} from "@threads/utils/threadRpc";
import { saveThreadActivity } from "@threads/utils/threadStorage";
import {
  buildResumeHydrationPlan,
  buildWorkspacePathLookup,
  buildWorkspaceThreadListState,
  getThreadListNextCursor,
  resolveWorkspaceIdForThreadPath,
} from "@threads/utils/threadActionHelpers";
import type { ThreadAction, ThreadState } from "./useThreadsReducer";

const THREAD_LIST_TARGET_COUNT = 20;
const THREAD_LIST_PAGE_SIZE = 100;
const THREAD_LIST_MAX_PAGES_OLDER = 6;
const THREAD_LIST_MAX_PAGES_DEFAULT = 6;
const THREAD_LIST_CURSOR_PAGE_START = "__codex_monitor_page_start__";

type UseThreadActionsOptions = {
  engine?: import("@/types").EngineKind;
  model?: string | null;
  dispatch: Dispatch<ThreadAction>;
  itemsByThread: ThreadState["itemsByThread"];
  threadsByWorkspace: ThreadState["threadsByWorkspace"];
  activeThreadIdByWorkspace: ThreadState["activeThreadIdByWorkspace"];
  activeTurnIdByThread: ThreadState["activeTurnIdByThread"];
  threadParentById: ThreadState["threadParentById"];
  threadListCursorByWorkspace: ThreadState["threadListCursorByWorkspace"];
  threadStatusById: ThreadState["threadStatusById"];
  threadSortKey: ThreadListSortKey;
  onDebug?: (entry: DebugEntry) => void;
  getCustomName: (workspaceId: string, threadId: string) => string | undefined;
  threadActivityRef: MutableRefObject<Record<string, Record<string, number>>>;
  loadedThreadsRef: MutableRefObject<Record<string, boolean>>;
  replaceOnResumeRef: MutableRefObject<Record<string, boolean>>;
  applyCollabThreadLinksFromThread: (
    workspaceId: string,
    threadId: string,
    thread: Record<string, unknown>,
  ) => void;
  updateThreadParent: (parentId: string, childIds: string[]) => void;
  onSubagentThreadDetected: (workspaceId: string, threadId: string) => void;
  onThreadCodexMetadataDetected?: (
    workspaceId: string,
    threadId: string,
    metadata: { modelId: string | null; effort: string | null },
  ) => void;
};

export function useThreadActions({
  engine, model,
  dispatch,
  itemsByThread,
  threadsByWorkspace,
  activeThreadIdByWorkspace,
  activeTurnIdByThread,
  threadParentById,
  threadListCursorByWorkspace,
  threadStatusById,
  threadSortKey,
  onDebug,
  getCustomName,
  threadActivityRef,
  loadedThreadsRef,
  replaceOnResumeRef,
  applyCollabThreadLinksFromThread,
  updateThreadParent,
  onSubagentThreadDetected,
  onThreadCodexMetadataDetected,
}: UseThreadActionsOptions) {
  const resumeInFlightByThreadRef = useRef<Record<string, number>>({});
  const [historyCursorByThread, setHistoryCursorByThread] = useState<Record<string, string | null>>({});
  const [historyLoadingByThread, setHistoryLoadingByThread] = useState<Record<string, boolean>>({});
  const [historyErrorByThread, setHistoryErrorByThread] = useState<Record<string, string | null>>({});
  const historyInFlight = useRef(new Set<string>());
  const itemsRef = useRef(itemsByThread);
  itemsRef.current = itemsByThread;

  const loadOlderHistory = useCallback(async (workspaceId: string, threadId: string) => {
    const cursor = historyCursorByThread[threadId];
    if (!cursor || historyInFlight.current.has(threadId)) return;
    historyInFlight.current.add(threadId);
    setHistoryLoadingByThread((previous) => ({ ...previous, [threadId]: true }));
    setHistoryErrorByThread((previous) => ({ ...previous, [threadId]: null }));
    const snapshot = itemsRef.current[threadId] ?? [];
    try {
      const page = await listThreadHistory(workspaceId, threadId, cursor);
      if (page.nextCursor === cursor) throw new Error("Repeated history cursor");
      dispatch({ type: "prependThreadHistory", threadId, items: buildItemsFromThread({ turns: page.data }), snapshot });
      setHistoryCursorByThread((previous) => ({ ...previous, [threadId]: page.nextCursor }));
    } catch (error) {
      setHistoryErrorByThread((previous) => ({ ...previous, [threadId]: String(error) }));
    } finally {
      historyInFlight.current.delete(threadId);
      setHistoryLoadingByThread((previous) => ({ ...previous, [threadId]: false }));
    }
  }, [dispatch, historyCursorByThread]);
  const threadStatusByIdRef = useRef(threadStatusById);
  const activeTurnIdByThreadRef = useRef(activeTurnIdByThread);
  threadStatusByIdRef.current = threadStatusById;
  activeTurnIdByThreadRef.current = activeTurnIdByThread;

  const applyThreadMetadata = useCallback(
    (
      workspaceId: string,
      threadId: string,
      thread: Record<string, unknown>,
      options?: { notifySubagent?: boolean },
    ) => {
      if (thread.sideOf) dispatch({ type: "mergeThreadSummary", workspaceId, threadId, patch: { sideOf: thread.sideOf as import("@/types").SideOf, isSubagent: false } });
      const codexMetadata = extractThreadCodexMetadata(thread);
      if (codexMetadata.modelId || codexMetadata.effort) {
        onThreadCodexMetadataDetected?.(workspaceId, threadId, codexMetadata);
      }
      const sourceParentId = getParentThreadIdFromThread(thread);
      if (sourceParentId) {
        updateThreadParent(sourceParentId, [threadId]);
        if (options?.notifySubagent && !thread.sideOf) {
          onSubagentThreadDetected(workspaceId, threadId);
        }
      }
    },
    [
      dispatch,
      onSubagentThreadDetected,
      onThreadCodexMetadataDetected,
      updateThreadParent,
    ],
  );

  const dispatchPreviewMessage = useCallback(
    (threadId: string, text: string, timestamp: number) => {
      dispatch({
        type: "setLastAgentMessage",
        threadId,
        text,
        timestamp,
      });
    },
    [dispatch],
  );

  const extractThreadId = useCallback(
    (response: Record<string, unknown> | null | undefined) => {
      const thread = extractThreadFromResponse(response);
      return String(thread?.id ?? "");
    },
    [],
  );

  const startThreadForWorkspace = useCallback(
    async (workspaceId: string, options?: { activate?: boolean }) => {
      const shouldActivate = options?.activate !== false;
      onDebug?.({
        id: `${Date.now()}-client-thread-start`,
        timestamp: Date.now(),
        source: "client",
        label: "thread/start",
        payload: { workspaceId },
      });
      try {
        const response = await (engine || model ? startThreadService(workspaceId, engine, model) : startThreadService(workspaceId));
        onDebug?.({
          id: `${Date.now()}-server-thread-start`,
          timestamp: Date.now(),
          source: "server",
          label: "thread/start response",
          payload: response,
        });
        const threadId = extractThreadId(response);
        if (threadId) {
          dispatch({ type: "ensureThread", workspaceId, threadId });
          const thread = extractThreadFromResponse(response);
          dispatch({ type: "mergeThreadSummary", workspaceId, threadId, patch: { ...extractEngineCapabilities(thread), engine: String(thread?.engine ?? "codex"), modelProvider: String(thread?.modelProvider ?? "") } });
          if (shouldActivate) {
            dispatch({ type: "setActiveThreadId", workspaceId, threadId });
          }
          loadedThreadsRef.current[threadId] = true;
          return threadId;
        }
        return null;
      } catch (error) {
        onDebug?.({
          id: `${Date.now()}-client-thread-start-error`,
          timestamp: Date.now(),
          source: "error",
          label: "thread/start error",
          payload: error instanceof Error ? error.message : String(error),
        });
        throw error;
      }
    },
    [dispatch, engine, model, extractThreadId, loadedThreadsRef, onDebug],
  );

  const resumeThreadForWorkspace = useCallback(
    async (
      workspaceId: string,
      threadId: string,
      force = false,
      replaceLocal = false,
    ) => {
      if (!threadId) {
        return null;
      }
      if (!force && loadedThreadsRef.current[threadId]) {
        return threadId;
      }
      const status = threadStatusByIdRef.current[threadId];
      if (status?.isProcessing && loadedThreadsRef.current[threadId] && !force) {
        onDebug?.({
          id: `${Date.now()}-client-thread-resume-skipped`,
          timestamp: Date.now(),
          source: "client",
          label: "thread/resume skipped",
          payload: { workspaceId, threadId, reason: "active-turn" },
        });
        return threadId;
      }
      onDebug?.({
        id: `${Date.now()}-client-thread-resume`,
        timestamp: Date.now(),
        source: "client",
        label: "thread/resume",
        payload: { workspaceId, threadId },
      });
      const inFlightCount =
        (resumeInFlightByThreadRef.current[threadId] ?? 0) + 1;
      resumeInFlightByThreadRef.current[threadId] = inFlightCount;
      if (inFlightCount === 1) {
        dispatch({ type: "setThreadResumeLoading", threadId, isLoading: true });
      }
      const snapshot = itemsRef.current[threadId] ?? [];
      try {
        const response =
          (await resumeThreadService(workspaceId, threadId)) as
            | Record<string, unknown>
            | null;
        onDebug?.({
          id: `${Date.now()}-server-thread-resume`,
          timestamp: Date.now(),
          source: "server",
          label: "thread/resume response",
          payload: response,
        });
        const thread = extractThreadFromResponse(response);
        if (thread) {
          const isPaginated = "historyCursor" in thread;
          const shouldReplace = replaceLocal || replaceOnResumeRef.current[threadId] === true;
          if (isPaginated) {
            setHistoryCursorByThread((previous) => ({ ...previous, [threadId]: !shouldReplace && previous[threadId] !== undefined
              ? previous[threadId] : typeof thread.historyCursor === "string" ? thread.historyCursor : null }));
            dispatch({ type: "prependThreadHistory", threadId, items: buildItemsFromThread(thread), snapshot, recent: true, replace: shouldReplace });
          }
          dispatch({ type: "ensureThread", workspaceId, threadId });
          applyThreadMetadata(workspaceId, threadId, thread, {
            notifySubagent: true,
          });
          applyCollabThreadLinksFromThread(workspaceId, threadId, thread);
          const localItems = itemsRef.current[threadId] ?? [];
          if (shouldReplace) {
            replaceOnResumeRef.current[threadId] = false;
          }
          const hydrationPlan = buildResumeHydrationPlan({
            thread,
            workspaceId,
            threadId,
            replaceLocal: shouldReplace,
            localItems,
            localStatus: threadStatusByIdRef.current[threadId],
            localActiveTurnId: activeTurnIdByThreadRef.current[threadId] ?? null,
            getCustomName,
          });
          if (!hydrationPlan.shouldHydrate) {
            loadedThreadsRef.current[threadId] = true;
            return threadId;
          }
          if (hydrationPlan.keepLocalProcessing) {
            onDebug?.({
              id: `${Date.now()}-client-thread-resume-keep-processing`,
              timestamp: Date.now(),
              source: "client",
              label: "thread/resume keep-processing",
              payload: { workspaceId, threadId },
            });
          }
          dispatch({
            type: "markProcessing",
            threadId,
            isProcessing: hydrationPlan.shouldMarkProcessing,
            timestamp: hydrationPlan.processingTimestamp,
          });
          dispatch({
            type: "setActiveTurnId",
            threadId,
            turnId: hydrationPlan.resumedActiveTurnId,
          });
          dispatch({
            type: "markReviewing",
            threadId,
            isReviewing: hydrationPlan.reviewing,
          });
          if (!isPaginated && hydrationPlan.mergedItems.length > 0) {
            dispatch({
              type: "setThreadItems",
              threadId,
              items: hydrationPlan.mergedItems,
            });
          }
          if (hydrationPlan.threadName) {
            dispatch({
              type: "setThreadName",
              workspaceId,
              threadId,
              name: hydrationPlan.threadName,
            });
          }
          if (
            hydrationPlan.lastMessageText &&
            hydrationPlan.lastMessageTimestamp !== null
          ) {
            dispatchPreviewMessage(
              threadId,
              hydrationPlan.lastMessageText,
              hydrationPlan.lastMessageTimestamp,
            );
          }
        }
        loadedThreadsRef.current[threadId] = true;
        return threadId;
      } catch (error) {
        onDebug?.({
          id: `${Date.now()}-client-thread-resume-error`,
          timestamp: Date.now(),
          source: "error",
          label: "thread/resume error",
          payload: error instanceof Error ? error.message : String(error),
        });
        return null;
      } finally {
        const nextCount = Math.max(
          0,
          (resumeInFlightByThreadRef.current[threadId] ?? 1) - 1,
        );
        if (nextCount === 0) {
          delete resumeInFlightByThreadRef.current[threadId];
          dispatch({ type: "setThreadResumeLoading", threadId, isLoading: false });
        } else {
          resumeInFlightByThreadRef.current[threadId] = nextCount;
        }
      }
    },
    [
      applyThreadMetadata,
      applyCollabThreadLinksFromThread,
      dispatchPreviewMessage,
      dispatch,
      getCustomName,
      loadedThreadsRef,
      onDebug,
      replaceOnResumeRef,
    ],
  );

  const forkThreadForWorkspace = useCallback(
    async (
      workspaceId: string,
      threadId: string,
      options?: { activate?: boolean; model?: string },
    ) => {
      if (!threadId) {
        return null;
      }
      const parent = threadsByWorkspace[workspaceId]?.find(t => t.id === threadId);
      if (!engineCapabilities(parent?.engine ?? "codex", parent?.capabilities).forkLatest) {
        onDebug?.({ id: `${Date.now()}-client-thread-fork-error`, timestamp: Date.now(), source: "error", label: "thread/fork error", payload: "Fork is unsupported for this engine" });
        return null;
      }
      const shouldActivate = options?.activate !== false;
      onDebug?.({
        id: `${Date.now()}-client-thread-fork`,
        timestamp: Date.now(),
        source: "client",
        label: "thread/fork",
        payload: { workspaceId, threadId },
      });
      try {
        const response = await (options?.model ? forkThreadService(workspaceId, threadId, options.model) : forkThreadService(workspaceId, threadId));
        onDebug?.({
          id: `${Date.now()}-server-thread-fork`,
          timestamp: Date.now(),
          source: "server",
          label: "thread/fork response",
          payload: response,
        });
        const forkedThreadId = extractThreadId(response);
        if (!forkedThreadId) {
          return null;
        }
        dispatch({ type: "ensureThread", workspaceId, threadId: forkedThreadId });
        if (shouldActivate) {
          dispatch({
            type: "setActiveThreadId",
            workspaceId,
            threadId: forkedThreadId,
          });
        }
        loadedThreadsRef.current[forkedThreadId] = false;
        await resumeThreadForWorkspace(workspaceId, forkedThreadId, true, true);
        return forkedThreadId;
      } catch (error) {
        onDebug?.({
          id: `${Date.now()}-client-thread-fork-error`,
          timestamp: Date.now(),
          source: "error",
          label: "thread/fork error",
          payload: error instanceof Error ? error.message : String(error),
        });
        return null;
      }
    },
    [
      dispatch,
      extractThreadId,
      loadedThreadsRef,
      onDebug,
      resumeThreadForWorkspace,
      threadsByWorkspace,
    ],
  );

  const refreshThread = useCallback(
    async (workspaceId: string, threadId: string) => {
      if (!threadId) {
        return null;
      }
      replaceOnResumeRef.current[threadId] = true;
      return resumeThreadForWorkspace(workspaceId, threadId, true, true);
    },
    [replaceOnResumeRef, resumeThreadForWorkspace],
  );

  const resetWorkspaceThreads = useCallback(
    (workspaceId: string) => {
      const threadIds = new Set<string>();
      const list = threadsByWorkspace[workspaceId] ?? [];
      list.forEach((thread) => threadIds.add(thread.id));
      const activeThread = activeThreadIdByWorkspace[workspaceId];
      if (activeThread) {
        threadIds.add(activeThread);
      }
      threadIds.forEach((threadId) => {
        loadedThreadsRef.current[threadId] = false;
      });
    },
    [activeThreadIdByWorkspace, loadedThreadsRef, threadsByWorkspace],
  );

  const buildThreadSummary = useCallback(
    (
      workspaceId: string,
      thread: Record<string, unknown>,
      fallbackIndex: number,
    ): ThreadSummary | null =>
      buildThreadSummaryFromThread({
        workspaceId,
        thread,
        fallbackIndex,
        getCustomName,
      }),
    [getCustomName],
  );

  const listThreadsForWorkspaces = useCallback(
    async (
      workspaces: WorkspaceInfo[],
      options?: {
        preserveState?: boolean;
        sortKey?: ThreadListSortKey;
        maxPages?: number;
      },
    ) => {
      const targets = workspaces.filter((workspace) => workspace.id);
      if (targets.length === 0) {
        return;
      }
      const preserveState = options?.preserveState ?? false;
      const requestedSortKey = options?.sortKey ?? threadSortKey;
      const maxPages = Math.max(1, options?.maxPages ?? THREAD_LIST_MAX_PAGES_DEFAULT);
      if (!preserveState) {
        targets.forEach((workspace) => {
          dispatch({
            type: "setThreadListLoading",
            workspaceId: workspace.id,
            isLoading: true,
          });
          dispatch({
            type: "setThreadListCursor",
            workspaceId: workspace.id,
            cursor: null,
          });
        });
      }
      onDebug?.({
        id: `${Date.now()}-client-thread-list`,
        timestamp: Date.now(),
        source: "client",
        label: "thread/list",
        payload: {
          workspaceIds: targets.map((workspace) => workspace.id),
          preserveState,
          maxPages,
        },
      });
      try {
        const requester = targets.find((workspace) => workspace.connected) ?? targets[0];
        const matchingThreadsByWorkspace: Record<string, Record<string, unknown>[]> = {};
        let workspacePathLookup = buildWorkspacePathLookup(targets);
        const targetWorkspaceIds = new Set(targets.map((workspace) => workspace.id));
        try {
          const knownWorkspaces = await listWorkspacesService();
          if (knownWorkspaces.length > 0) {
            workspacePathLookup = buildWorkspacePathLookup([
              ...targets,
              ...knownWorkspaces,
            ]);
          }
        } catch {
          workspacePathLookup = buildWorkspacePathLookup(targets);
        }
        const uniqueThreadIdsByWorkspace: Record<string, Set<string>> = {};
        const resumeCursorByWorkspace: Record<string, string | null> = {};
        targets.forEach((workspace) => {
          matchingThreadsByWorkspace[workspace.id] = [];
          uniqueThreadIdsByWorkspace[workspace.id] = new Set<string>();
          resumeCursorByWorkspace[workspace.id] = null;
        });
        let pagesFetched = 0;
        let cursor: string | null = null;
        do {
          const pageCursor = cursor;
          pagesFetched += 1;
          const response =
            (await listThreadsService(
              requester.id,
              cursor,
              THREAD_LIST_PAGE_SIZE,
              requestedSortKey,
            )) as Record<string, unknown>;
          onDebug?.({
            id: `${Date.now()}-server-thread-list`,
            timestamp: Date.now(),
            source: "server",
            label: "thread/list response",
            payload: response,
          });
          const result = (response.result ?? response) as Record<string, unknown>;
          const data = Array.isArray(result?.data)
            ? (result.data as Record<string, unknown>[])
            : [];
          const nextCursor = getThreadListNextCursor(result);
          data.forEach((thread) => {
            const workspaceId = resolveWorkspaceIdForThreadPath(
              String(thread?.cwd ?? ""),
              workspacePathLookup,
              targetWorkspaceIds,
            );
            if (!workspaceId) {
              return;
            }
            const threadId = String(thread?.id ?? "");
            if (threadId && shouldHideSubagentThreadFromSidebar(thread.source)) {
              dispatch({ type: "hideThread", workspaceId, threadId });
              return;
            }
            matchingThreadsByWorkspace[workspaceId]?.push(thread);
            if (!threadId) {
              return;
            }
            const uniqueThreadIds = uniqueThreadIdsByWorkspace[workspaceId];
            if (!uniqueThreadIds || uniqueThreadIds.has(threadId)) {
              return;
            }
            uniqueThreadIds.add(threadId);
            if (
              uniqueThreadIds.size > THREAD_LIST_TARGET_COUNT &&
              resumeCursorByWorkspace[workspaceId] === null
            ) {
              resumeCursorByWorkspace[workspaceId] =
                pageCursor ?? THREAD_LIST_CURSOR_PAGE_START;
            }
          });
          cursor = nextCursor;
          if (pagesFetched >= maxPages) {
            break;
          }
        } while (cursor);

        const nextThreadActivity = { ...threadActivityRef.current };
        let didChangeAnyActivity = false;
        targets.forEach((workspace) => {
          const matchingThreads = matchingThreadsByWorkspace[workspace.id] ?? [];
          const activityByThread = nextThreadActivity[workspace.id] ?? {};
          const threadListState = buildWorkspaceThreadListState({
            workspaceId: workspace.id,
            matchingThreads,
            activityByThread,
            requestedSortKey,
            buildThreadSummary,
            activeThreadId: activeThreadIdByWorkspace[workspace.id],
            existingThreadIds: (threadsByWorkspace[workspace.id] ?? []).map(
              (thread) => thread.id,
            ),
            threadStatusById,
            threadParentById,
            threadListTargetCount: THREAD_LIST_TARGET_COUNT,
          });
          threadListState.uniqueThreads.forEach((thread) => {
            const threadId = String(thread?.id ?? "");
            if (!threadId) {
              return;
            }
            applyThreadMetadata(workspace.id, threadId, thread, {
              notifySubagent: true,
            });
          });
          if (threadListState.didChangeActivity) {
            nextThreadActivity[workspace.id] = threadListState.nextActivityByThread;
            didChangeAnyActivity = true;
          }
          dispatch({
            type: "setThreads",
            workspaceId: workspace.id,
            threads: threadListState.summaries,
            sortKey: requestedSortKey,
            preserveAnchors: true,
          });
          dispatch({
            type: "setThreadListCursor",
            workspaceId: workspace.id,
            cursor: resumeCursorByWorkspace[workspace.id] ?? cursor,
          });
          threadListState.previewUpdates.forEach(({ threadId, text, timestamp }) => {
            dispatchPreviewMessage(threadId, text, timestamp);
          });
        });
        if (didChangeAnyActivity) {
          threadActivityRef.current = nextThreadActivity;
          saveThreadActivity(nextThreadActivity);
        }
      } catch (error) {
        onDebug?.({
          id: `${Date.now()}-client-thread-list-error`,
          timestamp: Date.now(),
          source: "error",
          label: "thread/list error",
          payload: error instanceof Error ? error.message : String(error),
        });
      } finally {
        if (!preserveState) {
          targets.forEach((workspace) => {
            dispatch({
              type: "setThreadListLoading",
              workspaceId: workspace.id,
              isLoading: false,
            });
          });
        }
      }
    },
    [
      applyThreadMetadata,
      buildThreadSummary,
      dispatchPreviewMessage,
      dispatch,
      onDebug,
      activeThreadIdByWorkspace,
      threadParentById,
      threadActivityRef,
      threadStatusById,
      threadSortKey,
      threadsByWorkspace,
    ],
  );

  const listThreadsForWorkspace = useCallback(
    async (
      workspace: WorkspaceInfo,
      options?: {
        preserveState?: boolean;
        sortKey?: ThreadListSortKey;
        maxPages?: number;
      },
    ) => {
      await listThreadsForWorkspaces([workspace], options);
    },
    [listThreadsForWorkspaces],
  );

  const loadOlderThreadsForWorkspace = useCallback(
    async (workspace: WorkspaceInfo) => {
      const requestedSortKey = threadSortKey;
      const cursorValue = threadListCursorByWorkspace[workspace.id] ?? null;
      if (!cursorValue) {
        return;
      }
      const nextCursor =
        cursorValue === THREAD_LIST_CURSOR_PAGE_START ? null : cursorValue;
      let workspacePathLookup = buildWorkspacePathLookup([workspace]);
      const allowedWorkspaceIds = new Set([workspace.id]);
      const existing = threadsByWorkspace[workspace.id] ?? [];
      dispatch({
        type: "setThreadListPaging",
        workspaceId: workspace.id,
        isLoading: true,
      });
      onDebug?.({
        id: `${Date.now()}-client-thread-list-older`,
        timestamp: Date.now(),
        source: "client",
        label: "thread/list older",
        payload: { workspaceId: workspace.id, cursor: cursorValue },
      });
      try {
        try {
          const knownWorkspaces = await listWorkspacesService();
          if (knownWorkspaces.length > 0) {
            workspacePathLookup = buildWorkspacePathLookup([
              workspace,
              ...knownWorkspaces,
            ]);
          }
        } catch {
          workspacePathLookup = buildWorkspacePathLookup([workspace]);
        }
        const matchingThreads: Record<string, unknown>[] = [];
        const maxPagesWithoutMatch = THREAD_LIST_MAX_PAGES_OLDER;
        let pagesFetched = 0;
        let cursor: string | null = nextCursor;
        do {
          pagesFetched += 1;
          const response =
            (await listThreadsService(
              workspace.id,
              cursor,
              THREAD_LIST_PAGE_SIZE,
              requestedSortKey,
            )) as Record<string, unknown>;
          onDebug?.({
            id: `${Date.now()}-server-thread-list-older`,
            timestamp: Date.now(),
            source: "server",
            label: "thread/list older response",
            payload: response,
          });
          const result = (response.result ?? response) as Record<string, unknown>;
          const data = Array.isArray(result?.data)
            ? (result.data as Record<string, unknown>[])
            : [];
          const next = getThreadListNextCursor(result);
          matchingThreads.push(
            ...data.filter(
              (thread) => {
                const workspaceId = resolveWorkspaceIdForThreadPath(
                  String(thread?.cwd ?? ""),
                  workspacePathLookup,
                  allowedWorkspaceIds,
                );
                if (workspaceId !== workspace.id) {
                  return false;
                }
                const threadId = String(thread?.id ?? "");
                if (threadId && shouldHideSubagentThreadFromSidebar(thread.source)) {
                  dispatch({ type: "hideThread", workspaceId, threadId });
                  return false;
                }
                return true;
              },
            ),
          );
          cursor = next;
          if (matchingThreads.length === 0 && pagesFetched >= maxPagesWithoutMatch) {
            break;
          }
          if (pagesFetched >= THREAD_LIST_MAX_PAGES_OLDER) {
            break;
          }
        } while (cursor && matchingThreads.length < THREAD_LIST_TARGET_COUNT);

        const existingIds = new Set(existing.map((thread) => thread.id));
        const additions: ThreadSummary[] = [];
        matchingThreads.forEach((thread) => {
          const id = String(thread?.id ?? "");
          if (!id || existingIds.has(id)) {
            return;
          }
          applyThreadMetadata(workspace.id, id, thread);
          const summary = buildThreadSummary(
            workspace.id,
            thread,
            existing.length + additions.length,
          );
          if (!summary) {
            return;
          }
          additions.push(summary);
          existingIds.add(id);
        });

        if (additions.length > 0) {
          dispatch({
            type: "setThreads",
            workspaceId: workspace.id,
            threads: [...existing, ...additions],
            sortKey: requestedSortKey,
          });
        }
        dispatch({
          type: "setThreadListCursor",
          workspaceId: workspace.id,
          cursor,
        });
        matchingThreads.forEach((thread) => {
          const threadId = String(thread?.id ?? "");
          const preview = asString(thread?.preview ?? "").trim();
          if (!threadId || !preview) {
            return;
          }
          dispatch({
            type: "setLastAgentMessage",
            threadId,
            text: preview,
            timestamp: getThreadTimestamp(thread),
          });
        });
      } catch (error) {
        onDebug?.({
          id: `${Date.now()}-client-thread-list-older-error`,
          timestamp: Date.now(),
          source: "error",
          label: "thread/list older error",
          payload: error instanceof Error ? error.message : String(error),
        });
      } finally {
        dispatch({
          type: "setThreadListPaging",
          workspaceId: workspace.id,
          isLoading: false,
        });
      }
    },
    [
      applyThreadMetadata,
      buildThreadSummary,
      dispatch,
      onDebug,
      threadListCursorByWorkspace,
      threadsByWorkspace,
      threadSortKey,
    ],
  );

  const archiveThread = useCallback(
    async (workspaceId: string, threadId: string) => {
      try {
        await archiveThreadService(workspaceId, threadId);
      } catch (error) {
        onDebug?.({
          id: `${Date.now()}-client-thread-archive-error`,
          timestamp: Date.now(),
          source: "error",
          label: "thread/archive error",
          payload: error instanceof Error ? error.message : String(error),
        });
      }
    },
    [onDebug],
  );

  return {
    historyCursorByThread,
    historyLoadingByThread,
    historyErrorByThread,
    loadOlderHistory,
    startThreadForWorkspace,
    forkThreadForWorkspace,
    resumeThreadForWorkspace,
    refreshThread,
    resetWorkspaceThreads,
    listThreadsForWorkspaces,
    listThreadsForWorkspace,
    loadOlderThreadsForWorkspace,
    archiveThread,
  };
}
