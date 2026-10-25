import { useState } from "react";
import type { WorkTask } from "@/types";
import { workOutput } from "@services/tauri";
export function WorkFiles({ spaceId, tasks }: { spaceId: string; tasks: WorkTask[] }) {
  const [error, setError] = useState("");
  const files = [...new Map(tasks.filter(t => t.spaceId === spaceId).flatMap(t => t.outputs.map(path => [path, t.id] as const))).entries()];
  return <section aria-label="Space files"><h2>Files</h2><p>Files created or modified by tasks in this space.</p>{error && <p role="alert">{error}</p>}{files.map(([path, taskId]) => <button className="work-file" key={path} onClick={() => { void workOutput(taskId, path, "open").catch(e => setError(String(e))); }}>{path}</button>)}{!files.length && <p>No task files yet.</p>}</section>;
}
