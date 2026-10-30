import { useRef, useState } from "react";
import { CircleAlert, RotateCcw, SlidersHorizontal } from "lucide-react";
import type { ConversationItem } from "@/types";

export function isTurnError(item: ConversationItem | undefined): boolean {
  return item?.kind === "message" && item.role === "assistant" && (
    item.isError === true || (item.id.endsWith("-assistant") && /^Turn failed(?: to start)?[:.]/.test(item.text))
  );
}

export function describeTurnError(message: string): { title: string; description: string; canRetry?: boolean } {
  if (/^(?:Turn failed(?: to start)?:\s*)?Pi process (?:exited\b|is closed\b)/i.test(message)) {
    return { title: "Agent stopped unexpectedly", description: "The agent process ended before finishing. Review any saved progress above, then send a follow-up to continue. Unfinished commands are not restarted automatically.", canRetry: false };
  }
  if (/^(?:Turn failed(?: to start)?:\s*)?Verification incomplete:/i.test(message)) {
    return { title: "Review needs follow-up", description: "One or more acceptance criteria remain unverified. Review the findings above, then ask for the missing checks or corrections.", canRetry: false };
  }
  if (/chatgpt_quota_paused|requests are paused|quota.{0,30}(?:exceeded|exhausted)|insufficient_quota/i.test(message)) {
    return { title: "Account usage limit reached", description: "Choose another model or account, or retry when this account’s allowance is available." };
  }
  if (/\b429\b|rate.?limit|too many requests/i.test(message)) {
    return { title: "Too many requests", description: "The provider is limiting requests. Wait a moment and retry, or choose another model." };
  }
  if (/\b401\b|invalid.{0,10}api.?key|authentication|unauthorized/i.test(message)) {
    return { title: "Provider access needs attention", description: "Check this provider’s credentials in Settings, or choose another model." };
  }
  if (/context.{0,20}(?:length|limit|window)|too many tokens/i.test(message)) {
    return { title: "Conversation is too long", description: "Compact this conversation or start a new one, then try again." };
  }
  if (/network|fetch failed|failed to fetch|connection|timed? ?out|timeout|ECONN/i.test(message)) {
    return { title: "Couldn’t reach the provider", description: "Check your connection and retry, or choose another model." };
  }
  return { title: "Request couldn’t complete", description: "Try again or choose another model. The original error is available below." };
}

export function TurnErrorCard({ message, onRetry, onChangeModel, disabled = false }: {
  message: string;
  onRetry?: () => Promise<boolean>;
  onChangeModel?: () => void;
  disabled?: boolean;
}) {
  const { title, description, canRetry = true } = describeTurnError(message);
  const retrying = useRef(false);
  const [pending, setPending] = useState(false);
  const [retryError, setRetryError] = useState(false);
  const retry = async () => {
    if (!onRetry || disabled || retrying.current) return;
    retrying.current = true;
    setPending(true);
    setRetryError(false);
    try { if (!await onRetry()) setRetryError(true); }
    catch { setRetryError(true); }
    finally { retrying.current = false; setPending(false); }
  };
  return <section className="turn-error-card" aria-label={title}>
    <div className="turn-error-heading"><CircleAlert size={18} aria-hidden /><strong>{title}</strong></div>
    <p>{description}</p>
    {canRetry && (onRetry || onChangeModel) && <div className="turn-error-actions">
      {onRetry && <button type="button" className="ghost" disabled={disabled || pending} onClick={() => void retry()} title="Resend the last request using the current model; keep existing work"><RotateCcw size={14} aria-hidden />{pending ? "Retrying…" : "Retry"}</button>}
      {onChangeModel && <button type="button" className="ghost" disabled={disabled || pending} onClick={onChangeModel}><SlidersHorizontal size={14} aria-hidden />Change model</button>}
    </div>}
    {retryError && <p role="status">Couldn’t restart the request. Your message is still available.</p>}
    <details className="turn-error-details"><summary>Error details</summary><pre>{message}</pre></details>
  </section>;
}
