# Engines and features

Settings → Engines selects the default runtime from enabled, ready engines and
enables engines for project and composer pickers. Each row groups its icon, name,
protocol and status together. Install/Retry appears only when needed; installation
shows progress and Cancel. The row menu provides default selection, runtime checks
(for built-ins), protocol tests (for custom engines), details, and custom Edit/Delete.
Deletion requires confirmation and keeps conversation files. Built-ins are pi (default), oh-my-pi, Codex and Claude Code (ACP).
Existing threads retain their engine ID. Disabled engines are hidden from pickers;
their threads open read-only with an enable notice. Unknown IDs resolve to the
current enabled default with a notice.

Oh-my-pi uses the shared pi-RPC adapter, with its own profile and storage. Enabling
it or selecting Install prepares the bundled lockfile on demand. Installation is
approximately 1.1 GB and can be cancelled. It requires Bun >= 1.3.14; the bundled
manifest pins Bun 1.4.4 and omp 18.12.6. Only manifests and support files ship as
resources, never node_modules. `engines/omp/no-accounts.mjs` is explicitly listed in
`tauri.conf.json`; the install path calls `copy_tree`, which copies the guard into
app-data alongside the manifests while excluding dependency directories. Work remains on pi because its work extension has
not been validated under omp.

## Add a custom engine

Choose Add engine in the engine group header. The modal accepts a name, ACP/pi-RPC
protocol, executable (with Browse), one argument per line, optional non-secret
environment key/value rows, and an optional config-directory variable.
The command must resolve through PATH or be an executable file. Use Test before
saving: ACP receives `initialize`; pi-RPC receives `get_state` in `--mode rpc`.
The handshake has a 15-second timeout and its process is stopped afterwards. Test
shows the reported version when available, or a failure with up to 4 KiB of stderr.

Examples:

| Engine | Protocol | Command | Args | Config-directory variable |
| --- | --- | --- | --- | --- |
| Gemini CLI | ACP | `gemini` | `--experimental-acp` | Agent-specific, if supported |
| pi fork | pi-RPC | `/path/to/pi-fork` | Leave empty | `PI_CODING_AGENT_DIR` |

These examples require an installed agent with the selected protocol. A pi fork
must also support pi's extension API and the taucode approval extension handshake.
ACP agents must implement session creation/loading, prompting and permission
requests. Routing uses the protocol; adding a compatible agent needs no Rust change.

Custom definitions persist in `AppSettings.engines`. Engine IDs are stable strings.
Environment keys containing KEY, TOKEN, SECRET or PASSWORD are rejected with
“use a provider instead”. Loader overrides, PATH/HOME overrides, OMP_AUTH_* and
OTEL_* overrides are also rejected. Provider credentials come from the provider
registry via environment or its existing command mechanism, not engine definitions.

## Isolation and enforcement

Each runtime gets an environment allowlist and isolated HOME/configuration.
Pi profiles use `<data>/<engine-id>/agent/<workspace-id>` and separate session
storage; ACP profiles use their own engine/workspace directory. The optional
config-directory variable points into that isolated directory. Project files remain
available to the agent according to the thread's access policy.

Pi's models.json and omp's models.yml are generated from the same registry and
reasoning capabilities. They contain environment variable names, never API keys.
Omp's preload intercepts its SQLite auth_credentials store: credential reads return
empty, and API-key/OAuth writes fail. No login UI is forwarded. OMP_AUTH_BROKER_URL,
OMP_AUTH_BROKER_TOKEN and OTEL exporter variables are not inherited.

Omp network checks found in the installed source:

- `main.ts` checks releases through `startup.checkUpdate`; disabled, together with
  the setup wizard.
- The plugin marketplace schedules automatic updates; `marketplace.autoUpdate=false`.
- OTLP export is controlled by `telemetry.otlpExportEnabled=false`, plus
  `OTEL_SDK_DISABLED=true`.
- Model discovery and other fetches are restricted by the preload to configured
  provider and enabled HTTP MCP connector origins. Unconfigured origins fail.

Omp loads taucode's approval and subagent extensions. Bash/edit/write and MCP calls
use the existing access/connector policies. LSP, debugger and Python/JS eval are
available when supported by the installed runtime; each native call requires an
explicit interactive approval, and is denied in read-only threads and subagents.
Native omp task/browser/computer/login tools are excluded. Taucode supplies its
own bounded subagents and browser/computer MCP tools. Project MCP configuration,
external rules, skills and extensions are disabled in the omp profile.

For ACP, taucode filters supplied MCP servers, sends feature restrictions in session
metadata, and denies permission requests for disabled native capabilities. Arbitrary
custom executables are trusted programs: ACP metadata is advisory unless the agent
honors it, and these controls are not an OS sandbox for custom programs or their
subprocesses. Custom agents must respect the protocol's permission boundary and
must not implement independent account login or ambient credential discovery.

## Feature switches

Settings → Features defaults every switch on for existing installations. Controls
and subscriptions update live; engine tool changes take effect on new turns/sessions
without terminating an active turn. Code always remains available. Git off implies
worktrees off, while checkpoints use their own store and remain independent.
Codex's original experimental flags are available through the Codex row menu and
the Codex flags navigation item under Engines.

The feature accessor filters connector delivery for pi, omp, ACP and Codex. Browser
and computer helpers also reject startup when disabled. Subagents off sets their
concurrency to zero; terminal off prevents new PTYs; Work off stops queued tasks
from starting. Reasoning effects off preserves the slider without animation.

## Checks

The standard verification in `docs/specs/m3-computer-use.md`, plus
`npm run build -- --mode uipreview`, covers migration, registry resolution, custom
handshakes, feature gating and existing engine behavior. Rust tests also launch real
omp with an offline streaming transport and exercise its auth/network preload.

With installed omp and localhost socket access, run from `apps/taucode/src-tauri`:

```sh
cargo test live_omp_streams_from_registry_stub -- --ignored --nocapture
```

That separate test uses an actual localhost OpenAI-compatible SSE server and checks
prompt → streamed delta → completed turn through taucode's adapter.
