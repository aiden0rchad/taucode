import { useEffect, useState, type KeyboardEvent } from "react";
import type { ThreadItem } from "@protocol/v2/ThreadItem";
import type { CommandExecutionRequestApprovalResponse } from "@protocol/v2/CommandExecutionRequestApprovalResponse";
import type { FileChangeRequestApprovalResponse } from "@protocol/v2/FileChangeRequestApprovalResponse";
import {
  engineStart,
  onExit,
  onNotification,
  onServerRequest,
  onStderr,
  serverRequestRespond,
  threadFork,
  threadStart,
  turnInterrupt,
  turnStart,
  type ServerNotification,
  type ServerRequest,
} from "./engine";
import "./App.css";

type ApprovalRequest =
  | Extract<ServerRequest, { method: "item/commandExecution/requestApproval" }>
  | Extract<ServerRequest, { method: "item/fileChange/requestApproval" }>;

interface ActiveThread {
  id: string;
  modelProvider: string;
}

interface RawEvent {
  kind: string;
  payload: unknown;
}

function isApprovalRequest(request: ServerRequest): request is ApprovalRequest {
  return (
    request.method === "item/commandExecution/requestApproval" ||
    request.method === "item/fileChange/requestApproval"
  );
}

function userText(item: Extract<ThreadItem, { type: "userMessage" }>): string {
  return item.content
    .map((input) => (input.type === "text" ? input.text : `[${input.type}]`))
    .join("\n");
}

function App() {
  const [cwd, setCwd] = useState("");
  const [model, setModel] = useState("");
  const [provider, setProvider] = useState("");
  const [engineStatus, setEngineStatus] = useState("starting");
  const [thread, setThread] = useState<ActiveThread | null>(null);
  const [turnId, setTurnId] = useState<string | null>(null);
  const [turnStatus, setTurnStatus] = useState("idle");
  const [running, setRunning] = useState(false);
  const [composer, setComposer] = useState("");
  const [items, setItems] = useState<ThreadItem[]>([]);
  const [commandOutput, setCommandOutput] = useState<Record<string, string>>({});
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);
  const [rawEvents, setRawEvents] = useState<RawEvent[]>([]);
  const [error, setError] = useState("");

  function log(kind: string, payload: unknown) {
    setRawEvents((current) => [...current, { kind, payload }].slice(-500));
  }

  function upsertItem(item: ThreadItem) {
    setItems((current) => {
      const withoutOptimistic =
        item.type === "userMessage"
          ? current.filter(
              (candidate) =>
                !(
                  candidate.id.startsWith("local-") &&
                  candidate.type === "userMessage" &&
                  userText(candidate) === userText(item)
                ),
            )
          : current;
      const index = withoutOptimistic.findIndex(
        (candidate) => candidate.id === item.id,
      );
      if (index < 0) return [...withoutOptimistic, item];
      return withoutOptimistic.map((candidate, candidateIndex) =>
        candidateIndex === index ? item : candidate,
      );
    });
  }

  useEffect(() => {
    let disposed = false;
    let unlisten: Array<() => void> = [];

    function handleNotification(notification: ServerNotification) {
      log("notification", notification);
      switch (notification.method) {
        case "item/started":
        case "item/completed":
          upsertItem(notification.params.item);
          break;
        case "item/agentMessage/delta":
          setItems((current) => {
            let found = false;
            const next = current.map((item) => {
              if (
                item.id !== notification.params.itemId ||
                item.type !== "agentMessage"
              ) {
                return item;
              }
              found = true;
              return { ...item, text: item.text + notification.params.delta };
            });
            return found
              ? next
              : [
                  ...next,
                  {
                    type: "agentMessage",
                    id: notification.params.itemId,
                    text: notification.params.delta,
                    phase: null,
                    memoryCitation: null,
                    delivery: null,
                    questions: null,
                  },
                ];
          });
          break;
        case "item/commandExecution/outputDelta":
          setCommandOutput((current) => ({
            ...current,
            [notification.params.itemId]:
              (current[notification.params.itemId] ?? "") +
              notification.params.delta,
          }));
          break;
        case "turn/started":
          setTurnId(notification.params.turn.id);
          setTurnStatus(notification.params.turn.status);
          setRunning(true);
          break;
        case "turn/completed":
          setTurnId(notification.params.turn.id);
          setTurnStatus(notification.params.turn.status);
          setRunning(false);
          if (notification.params.turn.error) {
            setError(JSON.stringify(notification.params.turn.error));
          }
          break;
        case "error":
          setError(JSON.stringify(notification.params));
          break;
      }
    }

    async function connect() {
      try {
        const listeners = await Promise.all([
          onNotification(handleNotification),
          onServerRequest((request) => {
            log("server-request", request);
            if (isApprovalRequest(request)) {
              setApprovals((current) => [...current, request]);
            } else {
              void serverRequestRespond(request.id, undefined, {
                code: -32601,
                message: "Method not found",
              }).catch((reason: unknown) => setError(String(reason)));
            }
          }),
          onStderr((line) => log("stderr", line)),
          onExit((exit) => {
            log("exit", exit);
            setEngineStatus(`exited (${exit.code ?? "signal"})`);
            setRunning(false);
          }),
        ]);
        if (disposed) {
          listeners.forEach((stop) => stop());
          return;
        }
        unlisten = listeners;
        const info = await engineStart();
        if (!disposed) {
          setCwd((current) => current || info.home);
          setEngineStatus("ready");
        }
      } catch (reason) {
        if (!disposed) {
          setEngineStatus("error");
          setError(String(reason));
        }
      }
    }

    void connect();
    return () => {
      disposed = true;
      unlisten.forEach((stop) => stop());
    };
  }, []);

  async function startNewThread() {
    setError("");
    try {
      const response = await threadStart({
        cwd,
        model: model || null,
        modelProvider: provider || null,
      });
      setThread({
        id: response.thread.id,
        modelProvider: response.modelProvider,
      });
      setItems([]);
      setCommandOutput({});
      setApprovals([]);
      setTurnId(null);
      setTurnStatus("idle");
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function forkCurrentThread() {
    if (!thread) return;
    setError("");
    try {
      const response = await threadFork({
        threadId: thread.id,
        model: model || null,
        modelProvider: provider || null,
      });
      setThread({
        id: response.thread.id,
        modelProvider: response.modelProvider,
      });
      setTurnId(null);
      setTurnStatus("forked");
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function sendPrompt() {
    const text = composer.trim();
    if (!thread || !text || running) return;
    setError("");
    setComposer("");
    setRunning(true);
    setTurnStatus("starting");
    upsertItem({
      type: "userMessage",
      id: `local-${Date.now()}`,
      clientId: null,
      content: [{ type: "text", text, text_elements: [] }],
    });
    try {
      const response = await turnStart(thread.id, text);
      setTurnId(response.turn.id);
      setTurnStatus(response.turn.status);
    } catch (reason) {
      setRunning(false);
      setError(String(reason));
    }
  }

  async function stopTurn() {
    if (!thread || !turnId) return;
    try {
      await turnInterrupt(thread.id, turnId);
      setTurnStatus("interrupting");
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function answerApproval(request: ApprovalRequest, approved: boolean) {
    try {
      if (request.method === "item/commandExecution/requestApproval") {
        const response: CommandExecutionRequestApprovalResponse = {
          decision: approved ? "accept" : "decline",
        };
        await serverRequestRespond(request.id, response);
      } else {
        const response: FileChangeRequestApprovalResponse = {
          decision: approved ? "accept" : "decline",
        };
        await serverRequestRespond(request.id, response);
      }
      setApprovals((current) =>
        current.filter((candidate) => String(candidate.id) !== String(request.id)),
      );
    } catch (reason) {
      setError(String(reason));
    }
  }

  function composerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void sendPrompt();
    }
  }

  const itemIds = new Set(items.map((item) => item.id));
  const unmatchedApprovals = approvals.filter(
    (approval) => !itemIds.has(approval.params.itemId),
  );

  return (
    <main className="app-shell">
      <header className="topbar">
        <label className="folder-field">
          <span>Folder</span>
          <input value={cwd} onChange={(event) => setCwd(event.target.value)} />
        </label>
        <label>
          <span>Model</span>
          <input
            value={model}
            placeholder="server default"
            onChange={(event) => setModel(event.target.value)}
          />
        </label>
        <label>
          <span>Provider</span>
          <input
            value={provider}
            placeholder="server default"
            onChange={(event) => setProvider(event.target.value)}
          />
        </label>
        <button
          type="button"
          disabled={!cwd || engineStatus !== "ready"}
          onClick={() => void startNewThread()}
        >
          Start thread
        </button>
        <button
          type="button"
          disabled={!thread || running}
          onClick={() => void forkCurrentThread()}
        >
          Fork to provider
        </button>
        <span className={`status ${engineStatus}`}>{engineStatus}</span>
      </header>

      <section className="session-line">
        <span>{thread ? `Thread ${thread.id}` : "No thread"}</span>
        {thread && <span>Provider: {thread.modelProvider}</span>}
        <span>Turn: {turnStatus}</span>
      </section>

      {error && <div className="error">{error}</div>}

      <section className="transcript" aria-live="polite">
        {items.length === 0 && (
          <p className="empty">Start a thread, then send a prompt.</p>
        )}
        {items.map((item) => (
          <ItemCard
            key={item.id}
            item={item}
            output={commandOutput[item.id] ?? ""}
            approvals={approvals.filter(
              (approval) => approval.params.itemId === item.id,
            )}
            onAnswer={answerApproval}
          />
        ))}
        {unmatchedApprovals.map((approval) => (
          <ApprovalCard
            key={String(approval.id)}
            request={approval}
            onAnswer={answerApproval}
          />
        ))}
      </section>

      <section className="composer">
        <textarea
          value={composer}
          rows={4}
          placeholder={thread ? "Message Codex…" : "Start a thread first"}
          disabled={!thread}
          onChange={(event) => setComposer(event.target.value)}
          onKeyDown={composerKeyDown}
        />
        <div className="composer-actions">
          <span>⌘ Enter to send</span>
          {running && (
            <button type="button" className="danger" onClick={() => void stopTurn()}>
              Stop
            </button>
          )}
          <button
            type="button"
            disabled={!thread || !composer.trim() || running}
            onClick={() => void sendPrompt()}
          >
            Send
          </button>
        </div>
      </section>

      <details className="event-log">
        <summary>Raw event log ({rawEvents.length}/500)</summary>
        <pre>
          {rawEvents
            .map((event) => `${event.kind} ${JSON.stringify(event.payload)}`)
            .join("\n")}
        </pre>
      </details>
    </main>
  );
}

function ItemCard({
  item,
  output,
  approvals,
  onAnswer,
}: {
  item: ThreadItem;
  output: string;
  approvals: ApprovalRequest[];
  onAnswer: (request: ApprovalRequest, approved: boolean) => Promise<void>;
}) {
  let body;
  switch (item.type) {
    case "userMessage":
      body = <p>{userText(item)}</p>;
      break;
    case "agentMessage":
      body = <p className="agent-text">{item.text}</p>;
      break;
    case "reasoning":
      body = (
        <details>
          <summary>{item.summary.join(" ") || "Reasoning"}</summary>
          <pre>{item.content.join("\n")}</pre>
        </details>
      );
      break;
    case "commandExecution":
      body = (
        <>
          <code>{item.command}</code>
          <pre>{item.aggregatedOutput ?? output}</pre>
          {item.exitCode !== null && <span>Exit code: {item.exitCode}</span>}
        </>
      );
      break;
    case "fileChange":
      body = (
        <ul>
          {item.changes.map((change) => (
            <li key={change.path}>{change.path}</li>
          ))}
        </ul>
      );
      break;
    default:
      body = (
        <details>
          <summary>{item.type}</summary>
          <pre>{JSON.stringify(item, null, 2)}</pre>
        </details>
      );
  }

  return (
    <article className={`item ${item.type}`}>
      <small>{item.type}</small>
      {body}
      {approvals.map((approval) => (
        <ApprovalCard
          key={String(approval.id)}
          request={approval}
          onAnswer={onAnswer}
        />
      ))}
    </article>
  );
}

function ApprovalCard({
  request,
  onAnswer,
}: {
  request: ApprovalRequest;
  onAnswer: (request: ApprovalRequest, approved: boolean) => Promise<void>;
}) {
  const command =
    request.method === "item/commandExecution/requestApproval"
      ? request.params.command
      : null;
  return (
    <aside className="approval">
      <strong>Approval required</strong>
      {command && <code>{command}</code>}
      {request.params.reason && <p>{request.params.reason}</p>}
      <div>
        <button type="button" onClick={() => void onAnswer(request, true)}>
          Approve
        </button>
        <button
          type="button"
          className="danger"
          onClick={() => void onAnswer(request, false)}
        >
          Deny
        </button>
      </div>
    </aside>
  );
}

export default App;
