import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => { ctx.ui.setStatus("taucode-work", "ready"); });
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

