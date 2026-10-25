// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SettingsBrowserSection } from "./SettingsBrowserSection";
import { browserRequest } from "@services/tauri";
import type { AppSettings } from "@/types";
vi.mock("@services/tauri", () => ({engineRuntime: vi.fn().mockResolvedValue([{ engine: "browser", state: "ready", message: "Ready" }]), browserRequest:vi.fn().mockResolvedValue(undefined)}));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it("saves browser settings and requires confirmation before deleting the profile", async () => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(<SettingsBrowserSection appSettings={{} as AppSettings} onUpdateAppSettings={save} />);
  fireEvent.change(screen.getByLabelText("Browser binary"),{target:{value:"/custom/chrome"}});
  fireEvent.change(screen.getByLabelText(/Allowed origins/),{target:{value:"https://one.test\nhttps://two.test"}});
  fireEvent.change(screen.getByLabelText(/Blocked origins/),{target:{value:"https://blocked.test"}});
  fireEvent.change(screen.getByLabelText("Browser tool approval"),{target:{value:"auto"}});
  fireEvent.click(screen.getByText("Save browser settings"));
  await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({browser:expect.objectContaining({binary:"/custom/chrome",allowedOrigins:["https://one.test","https://two.test"],blockedOrigins:["https://blocked.test"],approval:"auto"})})));
  await screen.findByText(/Saved\./);
  fireEvent.click(screen.getByText("Clear browsing data")); expect(browserRequest).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Cancel")); expect(browserRequest).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Clear browsing data")); fireEvent.click(screen.getByText("Delete browsing data"));
  await waitFor(() => expect(browserRequest).toHaveBeenCalledWith({action:"clear",confirmed:true}));
});
