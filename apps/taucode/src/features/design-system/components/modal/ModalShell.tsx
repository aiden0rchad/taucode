import { Squircle } from "@/features/design-system/components/squircle/Squircle";
import type { MouseEventHandler, ReactNode } from "react";
import { createPortal } from "react-dom";
import { joinClassNames } from "../classNames";

type ModalShellProps = {
  children: ReactNode;
  className?: string;
  cardClassName?: string;
  squircle?: boolean;
  onBackdropClick?: MouseEventHandler<HTMLDivElement>;
  ariaLabel?: string;
  ariaLabelledBy?: string;
  ariaDescribedBy?: string;
};

export function ModalShell({
  children,
  className,
  cardClassName,
  squircle = false,
  onBackdropClick,
  ariaLabel,
  ariaLabelledBy,
  ariaDescribedBy,
}: ModalShellProps) {
  const Card = squircle ? Squircle : "div";
  return createPortal(
    <div
      className={joinClassNames("ds-modal", className)}
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      aria-describedby={ariaDescribedBy}
    >
      <div className="ds-modal-backdrop" onClick={onBackdropClick} />
      <Card className={joinClassNames("ds-modal-card", cardClassName)}>{children}</Card>
    </div>, document.body
  );
}
