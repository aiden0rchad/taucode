import { useFeatures } from "@settings/features";
import type { ReactNode } from "react";
import { MessageSquarePlus } from "lucide-react";
import { IconButton } from "@/features/design-system/components/IconButton";
import { openFeedback } from "@/features/diagnostics/client";

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
      <div className="actions">{actionsNode ?? null}{features.diagnostics && <IconButton aria-label="Log an issue or idea" title="Log an issue or idea" onClick={() => openFeedback()}><MessageSquarePlus aria-hidden /></IconButton>}</div>
    </div>
  );
}
