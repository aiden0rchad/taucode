import { useCallback, useEffect, useState } from "react";
import type { DebugEntry, WorkspaceInfo } from "@/types";
import { readAgentMd, writeAgentMd, type EditableInstructionFile, type InstructionFile } from "@services/tauri";
import { useFileEditor } from "@/features/shared/hooks/useFileEditor";

function useInstructionFile(workspaceId: string | null, filename: EditableInstructionFile) {
  const read = useCallback(() => workspaceId ? readAgentMd(workspaceId, filename) : Promise.resolve({ exists: false, content: "", truncated: false }), [workspaceId, filename]);
  const write = useCallback(async (content: string) => { if (workspaceId) await writeAgentMd(workspaceId, content, filename); }, [workspaceId, filename]);
  return useFileEditor({ key: workspaceId ? `${workspaceId}:${filename}` : null, read, write, readErrorTitle: `Couldn’t load ${filename}`, writeErrorTitle: `Couldn’t save ${filename}` });
}

export function useWorkspaceAgentMd({ activeWorkspace, filename, onDebug }: {
  activeWorkspace: WorkspaceInfo | null;
  filename?: EditableInstructionFile;
  onDebug?: (entry: DebugEntry) => void;
}) {
  const workspaceId = activeWorkspace?.id ?? null;
  const agents = useInstructionFile(workspaceId, "AGENTS.md");
  const claude = useInstructionFile(workspaceId, "CLAUDE.md");
  const [selection, setSelection] = useState<{ workspaceId: string | null; filename: EditableInstructionFile }>();
  const [detected, setDetected] = useState<InstructionFile[]>([]);
  useEffect(() => {
    setDetected([]);
    if (!workspaceId) return;
    let active = true;
    const files = ["GEMINI.md", ".github/copilot-instructions.md"] as const;
    void Promise.all(files.map(async file => (await readAgentMd(workspaceId, file)).exists ? file : null))
      .then(found => { if (active) setDetected(found.filter((file): file is typeof files[number] => file !== null)); })
      .catch(error => { if (active) onDebug?.({ id: `${Date.now()}-instructions-read`, timestamp: Date.now(), source: "error", label: "instructions/read error", payload: String(error) }); });
    return () => { active = false; };
  }, [workspaceId, onDebug]);
  const selectedFile = filename ?? (selection?.workspaceId === workspaceId ? selection.filename : !agents.isLoading && !claude.isLoading && !agents.exists && claude.exists ? "CLAUDE.md" : "AGENTS.md");
  return {
    ...(selectedFile === "AGENTS.md" ? agents : claude),
    selectedFile,
    selectFile: (file: EditableInstructionFile) => setSelection({ workspaceId, filename: file }),
    fileStatuses: { "AGENTS.md": agents.exists, "CLAUDE.md": claude.exists },
    detected,
  };
}
