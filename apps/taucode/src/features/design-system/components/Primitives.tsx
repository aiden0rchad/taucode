import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";

export function Button({ variant = "secondary", size = "sm", className = "", type = "button", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" | "danger"; size?: "sm" | "md" }) {
  return <button type={type} className={`ui-button ui-button-${variant} ui-button-${size} ${className}`} {...props} />;
}
export function Select({ className = "", ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={`ui-select ${className}`} {...props} />;
}
export function TextArea({ className = "", ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={`ui-textarea ${className}`} {...props} />;
}
export function Card({ className = "", ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`ui-card ${className}`} {...props} />;
}
export function EmptyState({ eyebrow, title, subtitle, suggestions, children }: { eyebrow: string; title: string; subtitle: string; suggestions: ReactNode; children?: ReactNode }) {
  return <section className="ui-empty-state"><span className="chat-eyebrow">{eyebrow}</span><h1>{title}</h1><p>{subtitle}</p><div className="ui-suggestions">{suggestions}</div>{children}</section>;
}
export function ListRow({ active, className = "", ...props }: HTMLAttributes<HTMLDivElement> & { active?: boolean }) {
  return <div className={`ui-list-row${active ? " is-active" : ""} ${className}`} {...props} />;
}
