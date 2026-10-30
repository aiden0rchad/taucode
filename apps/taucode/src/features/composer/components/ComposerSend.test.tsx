/** @vitest-environment jsdom */
import { assertRowAlignment } from "@/test/rowAlignment.mjs";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { isMobilePlatform } from "../../../utils/platformPaths";
import { Composer } from "./Composer";
import type {


  ComposerSendIntent,
  FollowUpMessageBehavior,
} from "../../../types";

vi.mock("../../../services/dragDrop", () => ({
  subscribeWindowDragDrop: vi.fn(() => () => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: async () => [],
  convertFileSrc: (path: string) => `tauri://${path}`,
}));

vi.mock("../../../utils/platformPaths", async () => {
  const actual = await vi.importActual<typeof import("../../../utils/platformPaths")>(
    "../../../utils/platformPaths",
  );
  return {
    ...actual,
    isMobilePlatform: vi.fn(() => false),
  };
});

type HarnessProps = {
  isEmpty?: boolean;
  engine?: import("@/types").EngineKind;
  onSend: (
    text: string,
    images: string[],
    submitIntent?: ComposerSendIntent,
  ) => void;

  isProcessing?: boolean;
  followUpMessageBehavior?: FollowUpMessageBehavior;
  steerAvailable?: boolean;
  selectedServiceTier?: "fast" | "flex" | null;
  onSideChat?: () => void;
  sideChatDisabledReason?: string;
};

function ComposerHarness({
  isEmpty,
  engine,
  onSend,

  isProcessing = false,
  followUpMessageBehavior = "queue",
  steerAvailable = false,
  selectedServiceTier = null,
  onSideChat,
  sideChatDisabledReason,
}: HarnessProps) {
  const [draftText, setDraftText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  return (
    <Composer
      isEmpty={isEmpty}
      engine={engine}
      onSend={onSend}
      onSideChat={onSideChat}
      sideChatDisabledReason={sideChatDisabledReason}
      onStop={() => {}}
      canStop={false}
      isProcessing={isProcessing}

      steerAvailable={steerAvailable}
      followUpMessageBehavior={followUpMessageBehavior}
      composerFollowUpHintEnabled={true}
      collaborationModes={[]}
      selectedCollaborationModeId={null}
      onSelectCollaborationMode={() => {}}
      models={[]}
      selectedModelId={null}
      onSelectModel={() => {}}
      reasoningOptions={[]}
      selectedEffort={null}
      onSelectEffort={() => {}}
      selectedServiceTier={selectedServiceTier}
      reasoningSupported={false}
      accessMode="current"
      onSelectAccessMode={() => {}}
      skills={[]}

      prompts={[]}
      files={[]}
      draftText={draftText}
      onDraftChange={setDraftText}
      textareaRef={textareaRef}
      dictationEnabled={false}
    />
  );
}

describe("Composer send triggers", () => {
  it("keeps access on the left and one model control before mic and send", () => {
    const { container } = render(<ComposerHarness onSend={vi.fn()} />);
    expect(screen.getByRole("combobox", { name: "Access mode" }).closest(".composer-shell-left")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Select model · ?" }).closest(".composer-shell-actions")).toBeTruthy();
    expect(container.querySelectorAll(".reasoning-control")).toHaveLength(1);
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(container.querySelector(".composer-select-wrap--effort")).toBeNull();
    expect(container.querySelectorAll(".composer-shell-actions button")).toHaveLength(3);
  });
  it("opens Code tools and inserts a file mention", () => {
    render(<ComposerHarness onSend={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    expect(screen.getByRole("button", { name: "Attach images" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Mention a file" }));
    expect(screen.getByRole("textbox")).toHaveProperty("value", "@");
    expect(screen.queryByRole("dialog", { name: "Attachments and tools" })).toBeNull();
  });
  it.each([['pi', 'taucode'], ['codex', 'Codex'], ['claude', 'Claude Code']] as const)("names the %s engine in the placeholder", (engine, label) => {
    render(<ComposerHarness onSend={vi.fn()} engine={engine} />);
    expect(screen.getByPlaceholderText(`Ask ${label} to do something… Type / for commands`)).toBeTruthy();
  });
  afterEach(() => {
    assertRowAlignment(document.body);
    cleanup();
    vi.mocked(isMobilePlatform).mockReturnValue(false);
    vi.restoreAllMocks();
  });

  it("sends once on Enter", () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} />);

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "hello world" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("hello world", [], "default");
  });

  it("filters slash commands and inserts a plan without sending it", () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} onSideChat={vi.fn()} />);
    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "/pl", selectionStart: 3 } });
    expect(screen.getByRole("option", { name: /\/plan/ })).toBeTruthy();
    fireEvent.keyDown(textarea, { key: "Tab" });
    expect(textarea).toHaveProperty("value", expect.stringContaining("Wait for my approval before implementing."));
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onSend).not.toHaveBeenCalled();
  });

  it("expands a typed goal with arguments on Send", () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "/goal Make a timer" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(onSend).toHaveBeenCalledWith(expect.stringMatching(/^Goal: Make a timer\n\nWork toward this goal/), [], "default");
  });

  it("turns a bare goal into a draft with the caret ready for the objective", async () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "/goal" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(screen.getByRole("textbox")).toHaveProperty("value", expect.stringMatching(/^Goal: \n\n/));
    expect(onSend).not.toHaveBeenCalled();
    await waitFor(() => expect((screen.getByRole("textbox") as HTMLTextAreaElement).selectionStart).toBe(6));
  });

  it("opens a side chat locally instead of sending the command to the model", () => {
    const onSend = vi.fn();
    const onSideChat = vi.fn();
    render(<ComposerHarness onSend={onSend} onSideChat={onSideChat} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "/sidechat " } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(onSideChat).toHaveBeenCalledOnce();
    expect(onSend).not.toHaveBeenCalled();
    expect(screen.getByRole("textbox")).toHaveProperty("value", "");
  });

  it("explains an unavailable side chat without losing the draft", () => {
    const onSend = vi.fn();
    const onSideChat = vi.fn();
    render(<ComposerHarness onSend={onSend} onSideChat={onSideChat} sideChatDisabledReason="Forking is unavailable" />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "/sidechat " } });
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter" });
    expect(screen.getByRole("status").textContent).toBe("Forking is unavailable");
    expect(screen.getByRole("textbox")).toHaveProperty("value", "/sidechat ");
    expect(onSideChat).not.toHaveBeenCalled();
    expect(onSend).not.toHaveBeenCalled();
  });

  it("supports Escape and arrow selection without submitting the draft", () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} onSideChat={vi.fn()} />);
    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "/", selectionStart: 1 } });
    fireEvent.keyDown(textarea, { key: "ArrowDown" });
    expect(within(screen.getByRole("listbox")).getAllByRole("option")[1].getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(textarea, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(textarea).toHaveProperty("value", "/");
    expect(onSend).not.toHaveBeenCalled();
  });

  it.each([{ engine: "pi", isEmpty: true }, { engine: "claude", isEmpty: false }])("hides unavailable compaction for $engine (empty=$isEmpty)", ({ engine, isEmpty }) => {
    render(<ComposerHarness onSend={vi.fn()} engine={engine} isEmpty={isEmpty} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "/", selectionStart: 1 } });
    expect(screen.queryByRole("option", { name: /\/compact/ })).toBeNull();
    expect(screen.getByRole("option", { name: /\/plan/ })).toBeTruthy();
  });

  it("sends once on send-button click", () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} />);

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "from button" } });
    fireEvent.click(screen.getByLabelText("Send"));

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("from button", [], "default");
  });

  it("shows the fast-mode indicator when enabled", () => {
    const onSend = vi.fn();
    render(<ComposerHarness onSend={onSend} selectedServiceTier="fast" />);

    expect(screen.getByLabelText("Fast mode enabled")).toBeTruthy();
  });

  it("blurs the textarea after Enter send on mobile", () => {
    vi.mocked(isMobilePlatform).mockReturnValue(true);
    const onSend = vi.fn();
    const blurSpy = vi.spyOn(HTMLTextAreaElement.prototype, "blur");
    render(<ComposerHarness onSend={onSend} />);

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "dismiss keyboard" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith(
      "dismiss keyboard",
      [],
      "default",
    );
    expect(blurSpy).toHaveBeenCalledTimes(1);
  });



  it("uses queue by default while processing when follow-up behavior is queue", () => {
    const onSend = vi.fn();
    render(
      <ComposerHarness
        onSend={onSend}
        isProcessing={true}
        followUpMessageBehavior="queue"
        steerAvailable={true}
      />,
    );

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "queue this" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("queue this", [], "queue");
  });

  it("uses opposite follow-up behavior on Shift+Ctrl+Enter while processing", () => {
    const onSend = vi.fn();
    render(
      <ComposerHarness
        onSend={onSend}
        isProcessing={true}
        followUpMessageBehavior="queue"
        steerAvailable={true}
      />,
    );

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "steer this" } });
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: true, ctrlKey: true });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("steer this", [], "steer");
  });

  it("falls back to queue when steer is selected but unavailable", () => {
    const onSend = vi.fn();
    render(
      <ComposerHarness
        onSend={onSend}
        isProcessing={true}
        followUpMessageBehavior="steer"
        steerAvailable={false}
      />,
    );

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "queue fallback" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    expect(
      screen.getByText(
        "Default: Queue (Steer unavailable). Both Enter and Shift+Ctrl+Enter will queue this message.",
      ),
    ).toBeTruthy();
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith("queue fallback", [], "queue");
  });

  it("treats Shift+Ctrl+Enter like normal send when not processing", () => {
    const onSend = vi.fn();
    render(
      <ComposerHarness
        onSend={onSend}
        isProcessing={false}
        followUpMessageBehavior="queue"
        steerAvailable={true}
      />,
    );

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "normal shortcut send" } });
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: true, ctrlKey: true });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith(
      "normal shortcut send",
      [],
      "default",
    );
  });

  it("does not queue on Tab while processing", () => {
    const onSend = vi.fn();
    render(
      <ComposerHarness
        onSend={onSend}
        isProcessing={true}
        followUpMessageBehavior="queue"
        steerAvailable={true}
      />,
    );

    const textarea = screen.getByRole("textbox");
    fireEvent.change(textarea, { target: { value: "tab no send" } });
    fireEvent.keyDown(textarea, { key: "Tab" });

    expect(onSend).not.toHaveBeenCalled();
  });
});
