// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ApprovalRequest, WorkspaceInfo } from "../../../types";
import { ApprovalToasts } from "./ApprovalToasts";

const workspaces: WorkspaceInfo[] = [
  {
    id: "workspace-1",
    name: "Workspace One",
    path: "/tmp/workspace-1",
    connected: true,
    settings: { sidebarCollapsed: false },
  },
];

const approvals: ApprovalRequest[] = [
  {
    workspace_id: "workspace-1",
    request_id: 1,
    method: "codex/requestApproval/shell",
    params: { command: "echo one" },
  },
  {
    workspace_id: "workspace-1",
    request_id: 2,
    method: "codex/requestApproval/shell",
    params: { command: "echo two" },
  },
];

describe("ApprovalToasts", () => {
  it("shows readable details and actions without internal IDs or empty values", () => {
    const request: ApprovalRequest = { ...approvals[0], params: {
      reason: "Run the project tests", command: "npm test", cwd: "/project",
      diffPreview: "+ new line", approvalId: "secret-approval", itemId: "secret-item",
      threadId: "secret-thread", turnId: "secret-turn", grantRoot: "/secret-root",
      availableDecisions: ["accept", "acceptForSession", "decline", "cancel"],
      empty: null, absent: undefined, missing: "None", extra: "extra context",
    } };
    const { container, unmount } = render(<ApprovalToasts approvals={[request]} workspaces={workspaces}
      onDecision={vi.fn()} onRemember={vi.fn()} />);
    for (const text of ["Run a command", "Run the project tests", "npm test", "/project", "+ new line"]) {
      expect(screen.getByText(text)).toBeTruthy();
    }
    for (const text of ["secret-approval", "secret-item", "secret-thread", "secret-turn", "/secret-root", "None", "cancel"]) {
      expect(container.textContent).not.toContain(text);
    }
    for (const name of ["Decline", "Allow for session", "Always allow", "Approve (Enter)"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }
    expect(container.querySelector("details")?.open).toBe(false);
    expect(screen.getByText("More details")).toBeTruthy();
    expect(container.querySelector(".approval-toast-details .approval-toast-actions")).toBeNull();
    unmount();
  });

  it.each([
    ["item/fileChange/requestApproval", { path: "src/main.ts" }, "Change a file"],
    ["item/commandExecution/requestApproval", { tool: "mcp__server__read", reason: "Connector requests permission to call server · read" }, "Use a connector tool"],
  ])("labels %s approvals", (method, params, title) => {
    const { unmount } = render(<ApprovalToasts approvals={[{ ...approvals[0], method, params }]}
      workspaces={workspaces} onDecision={vi.fn()} />);
    expect(screen.getByText(title)).toBeTruthy();
    expect(screen.getByText("path" in params ? params.path : params.tool)).toBeTruthy();
    unmount();
  });

  it.each(["commandExecution", "fileChange"])("uses the shared approval controls for pi %s including session approval", (kind) => {
    const onDecision = vi.fn();
    const request: ApprovalRequest = { workspace_id: "workspace-1", request_id: "pi:thread:approval", method: `item/${kind}/requestApproval`, params: { command: "npm test", availableDecisions: ["accept", "acceptForSession", "decline", "cancel"] } };
    const { unmount } = render(<ApprovalToasts approvals={[request]} workspaces={workspaces} onDecision={onDecision} onRemember={vi.fn()} />);
    fireEvent.click(screen.getByText("Allow for session"));
    expect(onDecision).toHaveBeenCalledWith(request, "acceptForSession");
    expect(screen.queryByText("Always allow")).toBeNull();
    fireEvent.click(screen.getByText("Decline"));
    expect(onDecision).toHaveBeenCalledWith(request, "decline");
    unmount();
  });
  it("renders live-region semantics and handles Enter on primary request", () => {
    const onDecision = vi.fn();
    render(
      <ApprovalToasts approvals={approvals} workspaces={workspaces} onDecision={onDecision} />,
    );

    const region = screen.getByRole("region");
    expect(region.getAttribute("aria-live")).toBe("assertive");
    expect(screen.getAllByRole("alert")).toHaveLength(2);

    fireEvent.keyDown(window, { key: "Enter" });
    expect(onDecision).toHaveBeenCalledWith(approvals[1], "accept");
  });

  it("does not submit when an input is focused", () => {
    const onDecision = vi.fn();
    render(
      <ApprovalToasts approvals={approvals} workspaces={workspaces} onDecision={onDecision} />,
    );

    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onDecision).not.toHaveBeenCalled();
    document.body.removeChild(input);
  });
});
