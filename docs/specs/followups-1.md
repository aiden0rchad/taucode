# Follow-ups batch 1 (found in M1–M3 reviews and live tests)

Each item is small and independent. Done = all implemented with tests, and the verification
command passes.

1. **Packaged engine paths.**
   - The pi runtime (`engines/pi`), `no-accounts.mjs`, `engines/browser` and the computer
     sidecar (`engines/computer`) are located via `env!("CARGO_MANIFEST_DIR")`, which only
     works on the dev machine.
   - Add them as Tauri bundle **resources** and resolve them at runtime from the resource
     dir (`app.path().resource_dir()`, or the daemon's equivalent next to its executable).
   - Fall back to `CARGO_MANIFEST_DIR` only in debug builds.
   - `node_modules` and `.venv` are created at first run into app data, from the bundled
     lockfiles (pi/browser: `npm ci --omit=dev` with system node ≥ 22.19; computer: `uv sync`),
     with progress shown in Settings. If node or uv is missing, show a clear setup message.
   - Test path resolution in both modes.
2. **pi auth guard** (`engines/pi/no-accounts.mjs`): also intercept `fs.promises.readFile` /
   `writeFile` and `fs.readFile` / `writeFile` (callback) for `auth.json`, with the same
   semantics. Test it with `node --test`.
3. **Parallel history paging (codex):** in `load_thread_history_page`, fetch
   `thread/items/list` for the page's turns with bounded concurrency (4), preserving order.
   Test with a fake session that records concurrency.
4. **pi compaction on tiny sessions:** "Nothing to compact (session too small)" shows as an
   informational notice ("Nothing to compact yet"), not an error toast or a failed item.
5. **MCP proxy progress:** forward upstream `notifications/progress` for a call back to the
   client with the client's original progress token (`crates/tau-mcp-proxy`). Test it with
   the fake upstream.
6. **Provider auth commands:**
   - make the timeout 15 s by default, configurable per provider (`auth.timeoutMs`)
   - on timeout, the error says: "The auth command took longer than N s. If the script lives
     in an iCloud-synced folder (Desktop/Documents), move it or read the key file directly."
   - apply to every place that runs auth commands: Test & fetch, the engines' env setup and the
     computer sidecar keys

## Constraints

Offline; no new dependencies; always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. No accounts or logins. **Don't touch
`crates/tau-mcp-proxy` except for item 5.** Don't run `tauri build`.

## Verification

Same command as `m3-computer-use.md`.
