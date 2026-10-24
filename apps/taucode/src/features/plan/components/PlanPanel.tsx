import { CheckCircle2, Circle, LoaderCircle } from "lucide-react";
import type { TurnPlan } from "../../../types";

type PlanPanelProps = {
  plan: TurnPlan | null;
  isProcessing: boolean;
};

function formatProgress(plan: TurnPlan) {
  const total = plan.steps.length;
  if (!total) {
    return "";
  }
  const completed = plan.steps.filter((step) => step.status === "completed").length;
  return `${completed}/${total}`;
}

export function PlanPanel({ plan, isProcessing }: PlanPanelProps) {
  const progress = plan ? formatProgress(plan) : "";
  const steps = plan?.steps ?? [];
  const showEmpty = !steps.length && !plan?.explanation;
  const emptyLabel = isProcessing ? "Waiting on a plan..." : "No active plan.";

  return (
    <aside className="plan-panel">
      <div className="plan-header">
        <span>Plan</span>
        {progress && <span className="plan-progress">{progress}</span>}
      </div>
      {steps.length > 0 && <progress className="plan-progress" aria-label="Plan completion" max={steps.length} value={steps.filter(step => step.status === "completed").length} />}
      {plan?.explanation && (
        <div className="plan-explanation">{plan.explanation}</div>
      )}
      {showEmpty ? (
        <div className="plan-empty">{emptyLabel}</div>
      ) : (
        <ol className="plan-list">
          {steps.map((step, index) => (
            <li key={`${step.step}-${index}`} className={`plan-step ${step.status}`}>
              <span className="plan-step-status" aria-hidden>
                {step.status === "completed" ? <CheckCircle2 /> : step.status === "inProgress" ? <LoaderCircle /> : <Circle />}
              </span>
              <span className="plan-step-text">{step.step}</span>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}
