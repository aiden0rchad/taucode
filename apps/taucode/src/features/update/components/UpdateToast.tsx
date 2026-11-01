import type { PostUpdateNoticeState, UpdateState } from "../hooks/useUpdater";
import { UpdateDownload } from "./UpdateDownload";
import { ReleaseNotes } from "./ReleaseNotes";
import {
  ToastActions,
  ToastBody,
  ToastCard,
  ToastHeader,
  ToastTitle,
  ToastViewport,
} from "../../design-system/components/toast/ToastPrimitives";

type UpdateToastProps = {
  state: UpdateState;
  onDismiss: () => void;
  postUpdateNotice?: PostUpdateNoticeState;
  onDismissPostUpdateNotice?: () => void;
};

const messages: Partial<Record<UpdateState["stage"], string>> = {
  checking: "Checking for updates…",
  latest: "You're up to date.",
  noReleases: "No releases published yet.",
};

export function UpdateToast({ state, onDismiss }: UpdateToastProps) {
  if (state.stage === "idle") return null;
  const message = state.stage === "unavailable" ? state.error ?? "Couldn't check for updates." : messages[state.stage];
  return (
    <ToastViewport className="update-toasts" role="region" ariaLive="polite">
      <ToastCard className="update-toast" role="status">
        <ToastHeader className="update-toast-header">
          <ToastTitle className="update-toast-title">Update</ToastTitle>
          {state.version ? <div className="update-toast-version">v{state.version}</div> : null}
        </ToastHeader>
        {state.stage === "available" ? (
          <>
            <ToastBody className="update-toast-body">
              taucode {state.version} is available. You have {__APP_VERSION__}.
            </ToastBody>
            {state.result?.notes ? <ReleaseNotes className="update-toast-notes" notes={state.result.notes} /> : null}
            <ToastActions className="update-toast-actions">
              <button className="secondary" onClick={onDismiss}>Later</button>
              <UpdateDownload key={state.version} version={state.version ?? ""} available={!!state.result?.downloadUrl} />
            </ToastActions>
          </>
        ) : (
          <div className="update-toast-inline">
            <ToastBody className="update-toast-body update-toast-body-inline">{message}</ToastBody>
            {state.stage !== "checking" ? <button className="secondary" onClick={onDismiss}>Dismiss</button> : null}
          </div>
        )}
      </ToastCard>
    </ToastViewport>
  );
}
