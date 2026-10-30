# Credits & third-party notices

taucode brings together an existing desktop foundation, agent runtimes, protocol adapters, and native/document tooling. These projects and assets retain their own licenses and attribution requirements.

## Desktop foundation

**[CodexMonitor](https://github.com/Dimillian/CodexMonitor)** by **Thomas Ricouard** is the MIT-licensed application taucode is derived from. Its original copyright and license are preserved in the repository's [root license](https://github.com/aiden0rchad/taucode/blob/main/LICENSE) and [app license](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/LICENSE).

The MIT license for taucode does not relicense third-party libraries, model weights, logos, or separately installed tools.

## Main building blocks

| Project or component | Role in taucode |
| --- | --- |
| Tauri and its plugins | Desktop shell, IPC, dialogs, native integration, and bundling. |
| React, TypeScript, and Vite | Frontend application and build tooling. |
| pi and oh-my-pi | Agent runtimes and supported protocol/extension paths. |
| Codex | Optional app-server engine and generated protocol types. |
| Claude Code ACP adapter | Optional ACP runtime integration. |
| Agent-S3 | Vendored planning code for the computer-use sidecar. |
| rmcp | Rust MCP transport/client/server support. |
| Playwright MCP / Chromium | Advanced browser automation path. |
| CodeGraph | Optional separately installed indexed code intelligence. |
| xterm.js / portable-pty | Terminal UI and PTY management. |
| Whisper / whisper-rs | Dictation support. |
| Node.js / npm / uv | Bundled runtime installation and execution support. |
| python-docx, openpyxl, python-pptx, pypdf, reportlab, matplotlib | Work-mode document and chart tooling. |
| VitePress | This documentation site's navigation, rendering, and local search. |

Use the committed package/Cargo/uv lockfiles and the original packages' license files for the exact dependency versions and terms. This overview is not a complete generated software bill of materials.

## Vendored source and runtime notices

The vendored Agent-S3 source keeps its [license](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/engines/computer/taucode_cua/vendor/agent_s3/LICENSE). The pi extension directory includes its own [license](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/engines/pi/extensions/LICENSE).

Packaging stages the standalone Node license and npm's included notices. uv is shipped with its MIT/Apache license files. Downloaded engines and native packages retain the notices provided by their distributions. See [packaging](packaging.md) for the resource layout and checks.

## Icons, marks, and model names

Third-party editor/app icons and material file icons remain the property of their respective owners and are used to identify integrations. Their presence does not imply affiliation or endorsement.

The taucode documentation mark and banner are repository-native SVG artwork. Screenshot images render the actual app through its offline preview with sample data; they are not private-session captures.

Model names, API-provider names, and runtime names identify compatibility targets. Downloaded model weights have their own terms, which are independent of the application license.

## Correcting attribution

If an attribution is missing or inaccurate, open a documentation issue with the relevant file/package and its authoritative license reference. Do not include private credentials or proprietary material in the report.
