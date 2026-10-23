import type { PostUpdateNoticeState, UpdateState } from "../hooks/useUpdater";

type UpdateToastProps = {
  state: UpdateState;
  onUpdate: () => void;
  onDismiss: () => void;
  postUpdateNotice?: PostUpdateNoticeState;
  onDismissPostUpdateNotice?: () => void;
};

// Updates remain unavailable until taucode has its own release channel.
export function UpdateToast(_props: UpdateToastProps) {
  return null;
}
