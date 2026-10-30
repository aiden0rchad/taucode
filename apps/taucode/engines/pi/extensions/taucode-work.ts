import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => { ctx.ui.setStatus("taucode-work", "ready"); });
  pi.registerTool({
    name: "preview_server",
    label: "Preview server",
    description: "Manage a preview server for this Work task. Start the server with a foreground command bound to 127.0.0.1 (no &, nohup, or daemon mode). Taucode keeps it running after the turn and provides logs and Stop/Restart controls. Use list to get status, output, and detected URLs; a printed URL does not prove readiness. Stop accepts the server id. Commands run in the space folder with the bundled Work Python available. Only the parent task may use this tool.",
    parameters: Type.Object({
      action: Type.Union([Type.Literal("start"), Type.Literal("list"), Type.Literal("stop")]),
      command: Type.Optional(Type.String({ minLength: 1, maxLength: 32768 })),
      label: Type.Optional(Type.String({ maxLength: 256 })),
      id: Type.Optional(Type.String({ minLength: 1 })),
    }),
    async execute(_id, params, signal, _onUpdate, ctx) {
      if (!ctx.hasUI || process.env.TAUCODE_SUBAGENT_DEPTH) throw new Error("Ask the parent task to manage preview servers.");
      if (signal?.aborted) throw new Error("Preview request cancelled");
      const response = await ctx.ui.select("taucode:preview:" + JSON.stringify(params), ["Continue"], { signal });
      if (response === undefined || signal?.aborted) throw new Error("Preview request cancelled");
      const result = JSON.parse(response);
      if (result.error) throw new Error(result.error);
      return { content: [{ type: "text", text: JSON.stringify(result.tasks) }], details: { tasks: result.tasks } };
    },
  });
  pi.registerTool({
    name: "ask_user",
    label: "Ask user",
    description: "Pause a Work task and ask for missing information or a decision. The question appears in the Needs input inbox. This does not grant permission to execute tools; their approval rules still apply.",
    parameters: Type.Object({
      question: Type.String({ minLength: 1, maxLength: 4000 }),
      choices: Type.Optional(Type.Array(Type.String({ minLength: 1, maxLength: 200 }), { minItems: 2, maxItems: 8 })),
    }),
    async execute(_id, params, signal, _onUpdate, ctx) {
      if (!ctx.hasUI || process.env.TAUCODE_SUBAGENT_DEPTH) throw new Error("Ask the parent task to request user input.");
      const answer = params.choices
        ? await ctx.ui.select(params.question, params.choices, { signal })
        : await ctx.ui.input(params.question, undefined, { signal });
      return { content: [{ type: "text", text: answer ?? "User input cancelled. Do not assume approval." }], details: { cancelled: answer === undefined } };
    },
  });
}

