import { useFileLinkOpener } from "@/features/messages/hooks/useFileLinkOpener";
import type { EditableInstructionFile, InstructionFile } from "@services/tauri";
import { useFeatureSettings, useFeatures } from "@settings/features";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {

  CustomPromptOption,
  DictationSessionState,
  DictationTranscript,
  ModelOption,
  SkillOption,
  WorkspaceInfo,
} from "../../../types";
import { ComposerInput } from "../../composer/components/ComposerInput";
import { useComposerImages } from "../../composer/hooks/useComposerImages";
import { useComposerAutocompleteState } from "../../composer/hooks/useComposerAutocompleteState";
import { usePromptHistory } from "../../composer/hooks/usePromptHistory";
import type {
  WorkspaceHomeRun,
  WorkspaceHomeRunInstance,
  WorkspaceRunMode,
} from "../hooks/useWorkspaceHome";
import { computeDictationInsertion } from "../../../utils/dictation";
import { isComposingEvent } from "../../../utils/keys";
import { FileEditorCard } from "../../shared/components/FileEditorCard";
import { WorkspaceHomeRunControls } from "./WorkspaceHomeRunControls";
import { WorkspaceHomeHistory } from "./WorkspaceHomeHistory";
import { WorkspaceHomeGitInitBanner } from "./WorkspaceHomeGitInitBanner";
import { buildIconPath } from "./workspaceHomeHelpers";
import { useWorkspaceHomeSuggestionsStyle } from "../hooks/useWorkspaceHomeSuggestionsStyle";
import type { ThreadStatusById } from "../../../utils/threadStatus";

type WorkspaceHomeProps = {
  imageAttachments?: Pick<ReturnType<typeof useComposerImages>, "activeImages" | "attachImages" | "pickImages" | "removeImage" | "clearActiveImages">;
  workspace: WorkspaceInfo;
  showGitInitBanner: boolean;
  initGitRepoLoading: boolean;
  onInitGitRepo: () => void | Promise<void>;
  runs: WorkspaceHomeRun[];
  recentThreadInstances: WorkspaceHomeRunInstance[];
  recentThreadsUpdatedAt: number | null;
  prompt: string;
  onPromptChange: (value: string) => void;
  onStartRun: (images?: string[]) => Promise<boolean>;
  runMode: WorkspaceRunMode;
  onRunModeChange: (mode: WorkspaceRunMode) => void;
  models: ModelOption[];
  selectedModelId: string | null;
  onSelectModel: (modelId: string) => void;
  modelSelections: Record<string, number>;
  onToggleModel: (modelId: string) => void;
  onModelCountChange: (modelId: string, count: number) => void;
  collaborationModes: { id: string; label: string }[];
  selectedCollaborationModeId: string | null;
  onSelectCollaborationMode: (id: string | null) => void;
  engine?: import("@/types").EngineKind;
  selectedEffort: string | null;
  onSelectEffort: (effort: string) => void;

  error: string | null;
  isSubmitting: boolean;
  activeWorkspaceId: string | null;
  activeThreadId: string | null;
  threadStatusById: ThreadStatusById;
  onSelectInstance: (workspaceId: string, threadId: string) => void;
  skills: SkillOption[];


  prompts: CustomPromptOption[];
  files: string[];
  dictationEnabled: boolean;
  dictationState: DictationSessionState;
  dictationLevel: number;
  onToggleDictation: () => void;
  onCancelDictation?: () => void;
  onOpenDictationSettings: () => void;
  dictationError: string | null;
  onDismissDictationError: () => void;
  dictationHint: string | null;
  onDismissDictationHint: () => void;
  dictationTranscript: DictationTranscript | null;
  onDictationTranscriptHandled: (id: string) => void;
  textareaRef?: RefObject<HTMLTextAreaElement | null>;
  onFileAutocompleteActiveChange?: (active: boolean) => void;
  agentMdFilename: EditableInstructionFile;
  agentMdStatuses: Record<EditableInstructionFile, boolean>;
  agentMdDetected: InstructionFile[];
  onAgentMdSelect: (file: EditableInstructionFile) => void;
  agentMdContent: string;
  agentMdExists: boolean;
  agentMdTruncated: boolean;
  agentMdLoading: boolean;
  agentMdSaving: boolean;
  agentMdError: string | null;
  agentMdDirty: boolean;
  onAgentMdChange: (value: string) => void;
  onAgentMdRefresh: () => void;
  onAgentMdSave: () => void;
};

export function WorkspaceHome({
  imageAttachments,
  workspace,
  showGitInitBanner,
  initGitRepoLoading,
  onInitGitRepo,
  runs,
  recentThreadInstances,
  recentThreadsUpdatedAt,
  prompt,
  onPromptChange,
  onStartRun,
  runMode,
  onRunModeChange,
  models,
  selectedModelId,
  onSelectModel,
  modelSelections,
  onToggleModel,
  onModelCountChange,
  collaborationModes,
  selectedCollaborationModeId,
  onSelectCollaborationMode,
  engine,
  selectedEffort,
  onSelectEffort,

  error,
  isSubmitting,
  activeWorkspaceId,
  activeThreadId,
  threadStatusById,
  onSelectInstance,
  skills,


  prompts,
  files,
  dictationEnabled,
  dictationState,
  dictationLevel,
  onToggleDictation,
  onCancelDictation,
  onOpenDictationSettings,
  dictationError,
  onDismissDictationError,
  dictationHint,
  onDismissDictationHint,
  dictationTranscript,
  onDictationTranscriptHandled,
  textareaRef: textareaRefProp,
  onFileAutocompleteActiveChange,
  agentMdFilename, agentMdStatuses, agentMdDetected, onAgentMdSelect,
  agentMdContent,
  agentMdExists,
  agentMdTruncated,
  agentMdLoading,
  agentMdSaving,
  agentMdError,
  agentMdDirty,
  onAgentMdChange,
  onAgentMdRefresh,
  onAgentMdSave,
}: WorkspaceHomeProps) {
  const settings = useFeatureSettings();
  const { openFileLink } = useFileLinkOpener(workspace.path, settings?.openAppTargets ?? [], settings?.selectedOpenAppId ?? "");
  if (!useFeatures().git) showGitInitBanner = false;
  const [showIcon, setShowIcon] = useState(true);
  const [selectionStart, setSelectionStart] = useState<number | null>(null);
  const iconPath = useMemo(() => buildIconPath(workspace.path), [workspace.path]);
  const iconSrc = useMemo(() => convertFileSrc(iconPath), [iconPath]);
  const fallbackTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const textareaRef = textareaRefProp ?? fallbackTextareaRef;
  const localImages = useComposerImages({
    activeThreadId: null,
    activeWorkspaceId: workspace.id,
  });
  const { activeImages, attachImages, pickImages, removeImage, clearActiveImages } = imageAttachments ?? localImages;

  const {
    isAutocompleteOpen,
    autocompleteMatches,
    autocompleteAnchorIndex,
    highlightIndex,
    setHighlightIndex,
    applyAutocomplete,
    handleInputKeyDown,
    handleTextChange,
    handleSelectionChange,
    fileTriggerActive,
  } = useComposerAutocompleteState({
    text: prompt,
    selectionStart,
    disabled: isSubmitting,

    skills,

    prompts,
    files,
    textareaRef,
    setText: onPromptChange,
    setSelectionStart,
  });

  const suggestionsStyle = useWorkspaceHomeSuggestionsStyle({
    isAutocompleteOpen,
    autocompleteAnchorIndex,
    selectionStart,
    prompt,
    textareaRef,
  });

  useEffect(() => {
    onFileAutocompleteActiveChange?.(fileTriggerActive);
  }, [fileTriggerActive, onFileAutocompleteActiveChange]);

  const {
    handleHistoryKeyDown,
    handleHistoryTextChange,
    recordHistory,
    resetHistoryNavigation,
  } = usePromptHistory({
    historyKey: workspace.id,
    text: prompt,
    hasAttachments: activeImages.length > 0,
    disabled: isSubmitting,
    isAutocompleteOpen,
    textareaRef,
    setText: onPromptChange,
    setSelectionStart,
  });

  const handleTextChangeWithHistory = (next: string, cursor: number | null) => {
    handleHistoryTextChange(next);
    handleTextChange(next, cursor);
  };

  const isDictationBusy = dictationState !== "idle";

  useEffect(() => {
    setShowIcon(true);
  }, [workspace.id]);

  useEffect(() => {
    if (!dictationTranscript) {
      return;
    }
    const textToInsert = dictationTranscript.text.trim();
    if (!textToInsert) {
      onDictationTranscriptHandled(dictationTranscript.id);
      return;
    }

    const textarea = textareaRef.current;
    const start = textarea?.selectionStart ?? selectionStart ?? prompt.length;
    const end = textarea?.selectionEnd ?? start;
    const { nextText, nextCursor } = computeDictationInsertion(
      prompt,
      textToInsert,
      start,
      end,
    );

    onPromptChange(nextText);
    resetHistoryNavigation();

    requestAnimationFrame(() => {
      if (!textareaRef.current) {
        return;
      }
      textareaRef.current.focus();
      textareaRef.current.setSelectionRange(nextCursor, nextCursor);
      setSelectionStart(nextCursor);
    });

    onDictationTranscriptHandled(dictationTranscript.id);
  }, [
    dictationTranscript,
    onDictationTranscriptHandled,
    onPromptChange,
    prompt,
    resetHistoryNavigation,
    selectionStart,
    textareaRef,
  ]);

  const handleRunSubmit = async () => {
    if (!prompt.trim() && activeImages.length === 0) {
      return;
    }
    if (isDictationBusy) {
      return;
    }

    const trimmed = prompt.trim();
    const didStart = await onStartRun(activeImages);
    if (didStart) {
      if (trimmed) {
        recordHistory(trimmed);
      }
      resetHistoryNavigation();
      clearActiveImages();
    }
  };

  const handleComposerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (isComposingEvent(event)) {
      return;
    }

    handleHistoryKeyDown(event);
    if (event.defaultPrevented) {
      return;
    }

    handleInputKeyDown(event);
    if (event.defaultPrevented) {
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      if (isDictationBusy) {
        event.preventDefault();
        return;
      }
      event.preventDefault();
      void handleRunSubmit();
    }
  };

  const agentMdStatus = agentMdLoading
    ? "Loading…"
    : agentMdSaving
      ? "Saving…"
      : agentMdExists
        ? ""
        : "Not found";
  const agentMdMetaParts: string[] = [];
  if (agentMdStatus) {
    agentMdMetaParts.push(agentMdStatus);
  }
  if (agentMdTruncated) {
    agentMdMetaParts.push("Truncated");
  }
  const agentMdMeta = agentMdMetaParts.join(" · ");
  const agentMdSaveLabel = agentMdExists ? "Save" : "Create";
  const agentMdSaveDisabled = agentMdLoading || agentMdSaving || agentMdTruncated || (agentMdExists && !agentMdDirty);
  const agentMdRefreshDisabled = agentMdLoading || agentMdSaving;

  return (
    <div className="workspace-home">
      <div className="workspace-home-hero">
        {showIcon && (
          <img
            className="workspace-home-icon"
            src={iconSrc}
            alt=""
            onError={() => setShowIcon(false)}
          />
        )}
        <div>
          <div className="workspace-home-title">{workspace.name}</div>
          <div className="workspace-home-path">{workspace.path}</div>
        </div>
      </div>

      {showGitInitBanner && (
        <WorkspaceHomeGitInitBanner
          isLoading={initGitRepoLoading}
          onInitGitRepo={onInitGitRepo}
        />
      )}

      <div className="workspace-home-composer">
        <div className="composer">
          <ComposerInput
            engine={engine ?? workspace.settings.agentPolicy?.defaultEngine ?? settings?.agentPolicy?.defaultEngine ?? "pi"}
            text={prompt}
            disabled={isSubmitting}
            sendLabel="Send"
            canStop={false}
            canSend={prompt.trim().length > 0 || activeImages.length > 0}
            isProcessing={isSubmitting}
            onStop={() => {}}
            onSend={() => {
              void handleRunSubmit();
            }}
            dictationState={dictationState}
            dictationLevel={dictationLevel}
            dictationEnabled={dictationEnabled}
            onToggleDictation={onToggleDictation}
            onCancelDictation={onCancelDictation}
            onOpenDictationSettings={onOpenDictationSettings}
            dictationError={dictationError}
            onDismissDictationError={onDismissDictationError}
            dictationHint={dictationHint}
            onDismissDictationHint={onDismissDictationHint}
            attachments={activeImages}
            onAddAttachment={() => {
              void pickImages();
            }}
            onAttachImages={attachImages}
            onRemoveAttachment={removeImage}
            onTextChange={handleTextChangeWithHistory}
            onSelectionChange={handleSelectionChange}
            onKeyDown={handleComposerKeyDown}
            isExpanded={false}
            onToggleExpand={undefined}
            textareaRef={textareaRef}
            suggestionsOpen={isAutocompleteOpen}
            suggestions={autocompleteMatches}
            highlightIndex={highlightIndex}
            onHighlightIndex={setHighlightIndex}
            onSelectSuggestion={applyAutocomplete}
            suggestionsStyle={suggestionsStyle}
          />
        </div>
        {error && <div className="workspace-home-error">{error}</div>}
      </div>

      <WorkspaceHomeRunControls
        workspaceKind={workspace.kind}
        runMode={runMode}
        onRunModeChange={onRunModeChange}
        models={models}
        selectedModelId={selectedModelId}
        onSelectModel={onSelectModel}
        modelSelections={modelSelections}
        onToggleModel={onToggleModel}
        onModelCountChange={onModelCountChange}
        collaborationModes={collaborationModes}
        selectedCollaborationModeId={selectedCollaborationModeId}
        onSelectCollaborationMode={onSelectCollaborationMode}
        engine={engine}
        selectedEffort={selectedEffort}
        onSelectEffort={onSelectEffort}
        isSubmitting={isSubmitting}
      />

      <div className="workspace-home-agent">
        <div className="workspace-home-instruction-tabs" role="group" aria-label="Instruction files">
          {(["AGENTS.md", "CLAUDE.md"] as const).map(file => <button type="button" className="ghost" key={file} aria-pressed={agentMdFilename === file} onClick={() => onAgentMdSelect(file)}>{file} · {agentMdStatuses[file] ? "Found" : "Not found"}</button>)}
        </div>
        {!!agentMdDetected.length && <div className="workspace-home-also-found">Also found: {agentMdDetected.map(file => <button type="button" className="ghost composer-chip" key={file} onClick={() => { void openFileLink({ path: file, line: null, column: null }); }}>{file}</button>)}</div>}
        {agentMdTruncated && (
          <div className="workspace-home-agent-warning">
            Showing the first part of a large file.
          </div>
        )}
        <FileEditorCard
          title={agentMdFilename}
          meta={agentMdMeta}
          error={agentMdError}
          value={agentMdContent}
          placeholder="Add workspace instructions for the agent…"
          disabled={agentMdLoading}
          refreshDisabled={agentMdRefreshDisabled}
          saveDisabled={agentMdSaveDisabled}
          saveLabel={agentMdSaveLabel}
          onChange={onAgentMdChange}
          onRefresh={onAgentMdRefresh}
          onSave={onAgentMdSave}
          classNames={{
            container: "workspace-home-agent-card",
            header: "workspace-home-section-header",
            title: "workspace-home-section-title",
            actions: "workspace-home-section-actions",
            meta: "workspace-home-section-meta",
            iconButton: "ghost workspace-home-icon-button",
            error: "workspace-home-error",
            textarea: "workspace-home-agent-textarea",
            help: "workspace-home-section-meta",
          }}
        />
      </div>

      <WorkspaceHomeHistory
        runs={runs}
        recentThreadInstances={recentThreadInstances}
        recentThreadsUpdatedAt={recentThreadsUpdatedAt}
        activeWorkspaceId={activeWorkspaceId}
        activeThreadId={activeThreadId}
        threadStatusById={threadStatusById}
        onSelectInstance={onSelectInstance}
      />
    </div>
  );
}
