// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { AppSettings, DictationEvent } from "@/types";
import { ComposerDictation } from "./ComposerDictation";
import { cancelDictation, startDictation, stopDictation } from "@services/tauri";

let emit: (event: DictationEvent) => void;
vi.mock("@services/events", () => ({
  subscribeDictationEvents: (listener: typeof emit) => { emit = listener; return () => {}; },
  subscribeDictationDownload: () => () => {},
}));
vi.mock("@services/tauri", () => ({
  getDictationModelStatus: vi.fn().mockResolvedValue({ state: "ready" }),
  requestDictationPermission: vi.fn().mockResolvedValue(true),
  startDictation: vi.fn().mockResolvedValue("listening"),
  stopDictation: vi.fn().mockResolvedValue("processing"),
  cancelDictation: vi.fn().mockResolvedValue("idle"),
}));
beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);
const settings = { dictationEnabled: true, dictationModelId: "small", dictationPreferredLanguage: "en", dictationHoldKey: "" } as AppSettings;

it("uses the existing dictation controller for start, stop, cancel, and one-time transcript delivery", async () => {
  const onTranscript = vi.fn(), onBusyChange = vi.fn(), onError = vi.fn();
  render(<ComposerDictation settings={settings} disabled={false} onTranscript={onTranscript} onBusyChange={onBusyChange} onError={onError} onOpenSettings={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Start dictation" }));
  expect(startDictation).toHaveBeenCalledWith("en");
  act(() => emit({ type: "state", state: "listening" }));
  expect(onBusyChange).toHaveBeenLastCalledWith(true);
  fireEvent.click(screen.getByRole("button", { name: "Stop dictation" }));
  expect(stopDictation).toHaveBeenCalledOnce();
  act(() => emit({ type: "state", state: "processing" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel transcription" }));
  expect(cancelDictation).toHaveBeenCalledOnce();
  act(() => { emit({ type: "transcript", text: "Dictated text" }); emit({ type: "state", state: "idle" }); });
  await waitFor(() => expect(onTranscript).toHaveBeenCalledExactlyOnceWith("Dictated text"));
  expect(onBusyChange).toHaveBeenLastCalledWith(false);
  act(() => emit({ type: "error", message: "Microphone unavailable" }));
  expect(onError).toHaveBeenCalledWith("Microphone unavailable");
});

it("offers settings when disabled and cancels capture on unmount", async () => {
  const onOpenSettings = vi.fn();
  const props = { disabled: false, onTranscript: vi.fn(), onBusyChange: vi.fn(), onError: vi.fn(), onOpenSettings };
  const { rerender, unmount } = render(<ComposerDictation {...props} settings={{ ...settings, dictationEnabled: false }} />);
  fireEvent.click(screen.getByRole("button", { name: "Open dictation settings" }));
  expect(onOpenSettings).toHaveBeenCalledOnce();
  rerender(<ComposerDictation {...props} settings={settings} />);
  await screen.findByRole("button", { name: "Start dictation" });
  act(() => emit({ type: "state", state: "listening" }));
  unmount();
  expect(cancelDictation).toHaveBeenCalledOnce();
});
