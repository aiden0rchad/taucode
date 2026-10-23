import { useEffect, useState } from "react";
import { computerRequest } from "@services/tauri";
import type { ComputerRun } from "./computer";
export function ComputerRunItem({ run: reference }: { run: ComputerRun }) {
  const [loaded, setLoaded] = useState<ComputerRun | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    if (reference.stored && reference.id) void computerRequest<ComputerRun>({ action: "runLog", id: reference.id }).then(run => { if (active) setLoaded(run); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [reference.id, reference.stored]);
  const run = loaded?.id === reference.id && loaded ? loaded : reference;
  return <section aria-label="Computer run" className="computer-run">
    {error && <p role="alert">{error}</p>}
    {run.instruction && <p>{run.instruction}</p>}
    <ol>{run.steps.map(step => <li key={step.step}>
      <details><summary>Step {step.step}: {String(step.action.kind ?? "action")}</summary>
        {step.thumbnail && <img width={240} src={`data:image/png;base64,${step.thumbnail}`} alt={`Screen before step ${step.step}`} />}
        <pre>{JSON.stringify(step.action, null, 2)}</pre>
        {step.reflection && <p>{step.reflection}</p>}
        {step.error && <p role="alert">{step.error}</p>}
      </details>
    </li>)}</ol>
    {run.summary && <p>{run.summary}</p>}{run.error && <p role="alert">{run.error}</p>}
  </section>;
}
