// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useBrowserController } from "./useBrowserController";
import { subscribeAppServerEvents } from "@services/events";
import { interruptTurn } from "@services/tauri";
import type { AppServerEvent } from "@/types";
vi.mock("@services/events", () => ({subscribeAppServerEvents:vi.fn()}));
vi.mock("@services/tauri", () => ({interruptTurn:vi.fn().mockResolvedValue(undefined)}));
vi.mock("@utils/platformPaths", () => ({isMacPlatform:()=>true}));
let receive: (e: AppServerEvent) => void;
beforeEach(() => { localStorage.clear(); vi.clearAllMocks(); vi.mocked(subscribeAppServerEvents).mockImplementation(fn => {receive=fn; return vi.fn();}); });
afterEach(cleanup);
it("persists layout and interrupts the owning turn, including a different workspace", async () => {
  const hook = renderHook(useBrowserController);
  act(() => { window.dispatchEvent(new KeyboardEvent("keydown",{key:"b",metaKey:true,shiftKey:true})); });
  expect(hook.result.current.open).toBe(true); expect(localStorage.getItem("taucode.browser.open")).toBe("true");
  act(() => hook.result.current.setWidth(640)); expect(localStorage.getItem("taucode.browser.width")).toBe("640");
  act(() => receive({workspace_id:"other",message:{method:"item/started",params:{threadId:"thread",turnId:"turn",item:{id:"call",type:"mcpToolCall",server:"Browser"}}}}));
  expect(hook.result.current.agentActive).toBe(true);
  await act(() => hook.result.current.takeOver());
  expect(interruptTurn).toHaveBeenCalledWith("other","thread","turn");
  act(() => receive({workspace_id:"other",message:{method:"item/completed",params:{threadId:"thread",item:{id:"call"}}}}));
  expect(hook.result.current.agentActive).toBe(false);
  hook.unmount();
  const restored = renderHook(useBrowserController); expect(restored.result.current.open).toBe(true); expect(restored.result.current.width).toBe(640);
});
