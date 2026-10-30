# Remote daemon & iOS

taucode includes a standalone daemon and control CLI so a supported client can work with projects on another host. This is an advanced, experimental path; it is separate from the normal local desktop workflow.

The desktop app is not required to remain open when the standalone daemon owns the work. The daemon host itself must stay running and reachable.

## What runs where

| Component | Location |
| --- | --- |
| Client UI | The desktop or experimental mobile client you use. |
| Project files | The daemon host's filesystem. |
| Agent and connector processes | The daemon host. |
| Provider endpoint `localhost` | The daemon host, not the client phone/computer. |
| App-data state | The daemon's configured data directory. |
| macOS native browser | Requires a desktop host; daemon-only use should select Chromium. |

Remote mode does not synchronize an arbitrary folder from your client to the host. Paths must make sense on the machine actually running the task.

## Prefer a private connection

The documented remote path is token-authenticated TCP. Treat the daemon as privileged access to the host's configured workspace/tools. Do not publish its listener directly to the internet or assume the token encrypts the transport.

Use a trusted private network or an encrypted private overlay such as an existing Tailscale tailnet. Limit which peers can reach the host and keep the token out of URLs, issue reports, and screenshots.

## Desktop-assisted setup

1. Prepare the engine and providers on the host.
2. Open **Settings → Server** on the desktop host.
3. Set a remote backend token and start the mobile-access daemon.
4. If using Tailscale, ensure both devices are connected to the same authorized tailnet.
5. Use the host/port supplied by the helper; the documented default port is `4732`.
6. In the client, enter the host/port and matching token.
7. Use **Connect & test**, then verify a small read-only task.

A successful connection test establishes reachability/authentication. It does not verify every engine, provider, native tool, or client feature.

## Build and use the standalone tools

From `apps/taucode/src-tauri`:

```sh
cargo build --bin codex_monitor_daemon --bin codex_monitor_daemonctl
./target/debug/codex_monitor_daemonctl status
./target/debug/codex_monitor_daemonctl start
```

The existing binary names retain underscores. The control CLI can use the saved settings and provides status/start/stop/command-preview operations:

```sh
./target/debug/codex_monitor_daemonctl command-preview
./target/debug/codex_monitor_daemonctl stop
```

Review command-preview output locally; it can reflect private host configuration. Available overrides include `--data-dir`, `--listen`, `--token`, `--daemon-path`, and `--json`. Do not paste a literal token into shared shell history or a public issue.

The app bundle also contains helper binaries; its exact installed path is platform-specific. See [packaging](packaging.md) for the macOS bundle layout.

## Experimental iOS path

The inherited mobile layout and remote backend flow are present, but iOS is work in progress. Terminal and dictation are unavailable in the documented mobile build. Do not infer Code/Chat/Work or desktop-tool parity from a working connection.

Building for iOS needs Xcode, Apple signing setup, and the appropriate Rust target. From a development Mac:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

From `apps/taucode`, the existing scripts provide simulator/device flows:

```sh
./scripts/build_run_ios.sh
./scripts/build_run_ios_device.sh --list-devices
```

The [mobile runbook](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/docs/mobile-ios-tailscale-blueprint.md) contains the inherited transport/signing details. Some upstream names remain in that engineering reference. Use current taucode build identifiers and your own signing configuration.

## Troubleshooting remote mode

- Confirm the daemon process is still running and its host is awake.
- Confirm host/port, token, and listener match the client settings.
- Check the firewall/private-network policy without broadening access unnecessarily.
- Confirm providers and connector executables are configured on the daemon host.
- Use Chromium for daemon-only browser work on macOS.
- Check data-directory selection if settings or workspaces appear missing.

Use [troubleshooting](troubleshooting.md) for provider/runtime failures and [privacy](privacy.md) for the transport and storage boundaries.
