import { useFeatures } from "@settings/features";
import type { ReactNode } from "react";

type MainTopbarProps = {
  leftNode: ReactNode;
  actionsNode?: ReactNode;
  className?: string;
};

export function MainTopbar({ leftNode, actionsNode, className }: MainTopbarProps) {
  const features = useFeatures();
  const classNames = ["main-topbar", className].filter(Boolean).join(" ");
  return (
    <div className={classNames} data-tauri-drag-region>
      <div className="main-topbar-left">{leftNode}</div>
      <div className="actions">{actionsNode ?? null}{features.diagnostics && <button type="button" className="ghost" aria-label="Command palette" onClick={() => window.dispatchEvent(new Event("taucode:diagnostic-commands"))}>Commands</button>}</div>
    </div>
  );
}
