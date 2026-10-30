# Development

The repository contains the desktop app, reusable Rust support crates, generated protocol types, and this standalone documentation site. Run app commands from `apps/taucode`; run documentation commands from the repository root with `--prefix docs`.

## Set up a checkout

Follow [installation](getting-started.md#build-from-source) for platform prerequisites, then:

```sh
git clone https://github.com/aiden0rchad/taucode.git
cd taucode/apps/taucode
npm ci
CARGO_NET_OFFLINE=false cargo fetch --locked --manifest-path src-tauri/Cargo.toml
npm run doctor:strict
npm run tauri:dev
```

Windows uses the PowerShell fetch setup and the `doctor:win` / `tauri:dev:win` scripts in the installation guide. The native app’s `src-tauri/.cargo/config.toml` is offline by default. Use the committed lockfiles and an explicit fetch when updating the cache.

Read the applicable `AGENTS.md` files before editing. The app's guide defines source-routing and validation rules.

## Daily commands

From `apps/taucode`:

| Command | Purpose |
| --- | --- |
| `npm run tauri:dev` | Native desktop development, including the strict prerequisite check. |
| `npm run preview:ui` | Credential-free browser preview of the real React UI. |
| `npm run typecheck` | TypeScript validation without emitting files. |
| `npm test` | Frontend tests using Vitest. |
| `npm run test -- <test-file>` | A focused frontend test. |
| `npm run build` | TypeScript and production frontend build. |
| `npm run lint` | ESLint, including design-system guardrails. |
| `npm run themes:docs` | Regenerate theme palettes/schema documentation after changing its source. |
| `npm run test:evals` | Offline evaluation fixture/reference checks. |
| `npm run tauri:build` | Stage runtimes/helpers and produce platform bundles. |

The frontend-only `npm run dev` is not equivalent to a working native app. The UI preview has its own mocked Tauri entry path.

## Rust checks

From `apps/taucode/src-tauri`:

```sh
cargo check --all-targets --features custom-protocol
cargo test
```

These use the local dependency cache. Missing dependencies require an explicit network-enabled fetch. Native Swift, Clang, CMake, and OS-library errors need the corresponding platform toolchain; do not treat them as TypeScript failures.

The support crates can be checked separately from the repository root:

```sh
cargo test --manifest-path crates/tau-mcp-proxy/Cargo.toml
cargo test --manifest-path crates/taucode-diag-mcp/Cargo.toml
```

Some live tests are ignored or require a native application, local socket, installed runtime, or real provider. Running offline unit tests does not establish cross-platform, provider, or desktop-control acceptance.

## Work on the UI without a provider

Start `npm run preview:ui` and open `http://localhost:1421`. The real app renders with typed fixture data, mocked IPC, and in-memory actions. It can show Code/Chat/Work, setup, themes, failures, and diagnostics without touching project files or contacting a model.

Use these URLs for repeatable manual checks:

```text
/?mode=code&thread=thread-rich&theme=dark
/?mode=chat&chat=chat-1&theme=light
/?mode=work&task=task-needs-input
/?setup=provider
/?settings=diagnostics
/?settings=appearance
```

The [preview harness reference](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/src/dev/uiPreview/README.md) lists the complete fixture controls and limitations. Keep screenshot captions explicit about sample data.

## Architecture rules

- Put cross-runtime backend behavior in `src-tauri/src/shared/`.
- Keep Tauri commands and daemon RPC handlers as thin adapters.
- Route frontend calls through `src/services/tauri.ts` and events through `src/services/events.ts`.
- Keep `src/App.tsx` as the composition root; use feature hooks for stateful orchestration.
- Reuse design-system primitives and tokens for shared shell surfaces.
- Preserve JSON-RPC method/payload contracts or update all adapters and tests together.

See [architecture](architecture.md) for paths and [the app agent guide](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/AGENTS.md) for the detailed contract.

## Protocol types

`packages/protocol/src/` contains generated Codex app-server types pinned to `codex-cli 0.164.0`. Do not hand-edit generated files.

Regeneration uses the matching CLI:

```sh
codex app-server generate-ts --experimental --out packages/protocol/src
```

Run that from the repository root only as part of an intentional protocol update, then verify affected adapters and contracts. The pinned schema version is not a claim that every optional runtime binary in a user's PATH matches it.

## Documentation development

The site uses VitePress, with the same basic documentation setup as Homelab Atlas. It has a separate dependency graph from the app:

```sh
npm --prefix docs ci
npm --prefix docs run dev
```

Use the local URL printed by VitePress. Production validation and preview:

```sh
npm --prefix docs run build
npm --prefix docs run preview
```

The build checks site links and generates the local search index and sitemap. The GitHub Pages base is `/taucode/`; verify assets and deep links under that prefix. The site has no analytics integration or external search service configured.

The page/sidebar configuration is `docs/.vitepress/config.mts`. Theme styling lives in `docs/.vitepress/theme/`; shared static images live in `docs/public/`. The README uses those same checked-in assets.

Engineering plans, specs, fixtures, and dated acceptance records remain in GitHub and are excluded from the published user-guide build. Link them through [engineering references](engineering.md). Do not turn historical implementation instructions into current product claims.

## CI and deployment

The **Documentation** workflow installs the locked docs package, builds the site, and deploys GitHub Pages from `main`. Pull requests build without deployment. Pages must use the GitHub Actions build source.

The **Windows x64** workflow builds the application installer and checks installation, bundled runtimes, and short startup. Those checks do not replace native feature testing.

## What to verify for a change

| Changed area | Relevant evidence |
| --- | --- |
| Docs | Site build, working links/assets, readable layout, source-accurate commands. |
| Frontend | Typecheck, targeted tests, production build, lint, keyboard/responsive review. |
| Shared Rust behavior | Cargo check/tests and app/daemon contract parity. |
| Engine or tool policy | Fake/fixture tests plus bounded real-runtime verification when needed. |
| Native browser/computer use | Relevant tests and native manual checks on the target OS. |
| Packaging | Runtime/resource/signature checks and the target platform bundle. |

State what was actually tested in a pull request. A preview screenshot, unit test, native smoke check, and real provider run establish different things.
