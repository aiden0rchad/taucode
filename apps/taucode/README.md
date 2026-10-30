# taucode desktop app

The application source for [taucode](../../README.md), a desktop workspace for AI coding, conversations, and document work, built on CodexMonitor.

**[Documentation](https://aiden0rchad.github.io/taucode/)** · **[Installation](../../docs/getting-started.md)** · **[Development](../../docs/development.md)** · **[Builds & limitations](../../docs/releases.md)**

## Run locally

Install Node.js 22.19+ with npm, stable Rust, Git, CMake, and the platform prerequisites in the installation guide. From this directory:

```sh
npm ci
CARGO_NET_OFFLINE=false cargo fetch --locked --manifest-path src-tauri/Cargo.toml
npm run tauri:dev
```

On Windows, follow the PowerShell setup in the installation guide and use `npm run tauri:dev:win`.

pi is the default engine. Add API credentials or a local model server through Settings → Providers, then prepare the runtime in Settings → Engines. Codex and CodeGraph are optional, separately installed integrations. Chat and Work use dedicated pi profiles.

## Preview and validate

```sh
npm run preview:ui
npm run typecheck
npm test
npm run build
```

The preview runs at `http://localhost:1421` with sample data and mocked native APIs. It makes no model requests and cannot perform real tasks. See the [preview harness reference](src/dev/uiPreview/README.md).

For Rust checks, use `cargo check --all-targets --features custom-protocol` and `cargo test` from `src-tauri`. For bundles, use `npm run tauri:build` or the Windows `:win` variant. Read [macOS packaging](../../docs/packaging.md) before distribution builds.

## Source navigation

- [Architecture](../../docs/architecture.md) and [development](../../docs/development.md).
- [Codebase map](docs/codebase-map.md) and [agent contract](AGENTS.md).
- [Remote daemon and iOS](../../docs/remote.md).
- [Privacy, storage, and trust](../../docs/privacy.md).
- [Contributing](../../.github/CONTRIBUTING.md) and [security](../../.github/SECURITY.md).

The original CodexMonitor MIT notice is retained in [LICENSE](LICENSE). See [third-party notices](../../docs/THIRD_PARTY_NOTICES.md).
