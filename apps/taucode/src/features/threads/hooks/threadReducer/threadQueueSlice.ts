import type { ThreadAction, ThreadState } from "../useThreadsReducer";

export function reduceThreadQueue(state: ThreadState, action: ThreadAction): ThreadState {
  switch (action.type) {
    case "addApproval": {
      const exists = state.approvals.some(
        (item) =>
          item.request_id === action.approval.request_id &&
          item.workspace_id === action.approval.workspace_id,
      );
      if (exists) {
        return state;
      }
      return { ...state, approvals: [...state.approvals, action.approval] };
    }
    case "removeApproval":
      return {
        ...state,
        approvals: state.approvals.filter(
          (item) =>
            item.request_id !== action.requestId ||
            item.workspace_id !== action.workspaceId,
        ),
      };
    case "removeThreadApprovals":
      return {
        ...state,
        approvals: state.approvals.filter(
          (item) =>
            item.workspace_id !== action.workspaceId ||
            item.params.threadId !== action.threadId ||
            (item.params.turnId != null && action.turnId != null &&
              item.params.turnId !== action.turnId),
        ),
      };
    case "addUserInputRequest": {
      const exists = state.userInputRequests.some(
        (item) =>
          item.request_id === action.request.request_id &&
          item.workspace_id === action.request.workspace_id,
      );
      if (exists) {
        return state;
      }
      return {
        ...state,
        userInputRequests: [...state.userInputRequests, action.request],
      };
    }
    case "removeUserInputRequest":
      return {
        ...state,
        userInputRequests: state.userInputRequests.filter(
          (item) =>
            item.request_id !== action.requestId ||
            item.workspace_id !== action.workspaceId,
        ),
      };
    default:
      return state;
  }
}
