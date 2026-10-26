import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { joinClassNames } from "./classNames";
import "./icon-button.css";

export type IconButtonProps = ComponentPropsWithoutRef<"button"> & {
  "aria-label": string;
  size?: "sm" | "md" | "lg";
  variant?: "ghost" | "subtle" | "active";
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton({ size = "md", variant = "ghost", className, type = "button", ...props }, ref) {
    return <button {...props} ref={ref} type={type} data-icon-button={size}
      className={joinClassNames("ds-icon-button", `ds-icon-button--${variant}`, className)} />;
  },
);
