// @vitest-environment jsdom
import { render } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UpdateToast } from "./UpdateToast";
import type { UpdateState } from "../hooks/useUpdater";

it.each<UpdateState["stage"]>(["idle", "checking", "available", "downloading", "installing", "restarting", "latest", "error"])("renders no update UI for %s", stage => {
  const { container } = render(<UpdateToast state={{ stage, version: "1.2.3" }} onUpdate={vi.fn()} onDismiss={vi.fn()} postUpdateNotice={{ stage: "ready", version: "1.2.3", body: "Old release", htmlUrl: "https://example.com" }} />);
  expect(container.childElementCount).toBe(0);
});
