// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { useSidebarDensity } from "./useSidebarDensity";

afterEach(() => { cleanup(); localStorage.clear(); delete document.documentElement.dataset.sidebarDensity; });
it("defaults to compact and persists density across mounts and subscribers", () => {
  const first = renderHook(useSidebarDensity);
  const second = renderHook(useSidebarDensity);
  expect(first.result.current[0]).toBe("compact");
  act(() => first.result.current[1]("comfortable"));
  expect(second.result.current[0]).toBe("comfortable");
  expect(document.documentElement.dataset.sidebarDensity).toBe("comfortable");
  first.unmount(); second.unmount();
  const reloaded = renderHook(useSidebarDensity);
  expect(reloaded.result.current[0]).toBe("comfortable");
  act(() => reloaded.result.current[1]("invalid"));
  expect(reloaded.result.current[0]).toBe("compact");
});
