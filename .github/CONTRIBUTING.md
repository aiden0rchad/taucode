# Contributing to taucode

Focused fixes, reproducible reports, and documentation improvements are welcome. Start with the [README](../README.md), [development guide](../docs/development.md), and [architecture map](../docs/architecture.md).

## Report a problem

Use the issue forms. Include the app/build, OS and architecture, engine, provider type, reproduction steps, expected behavior, and actual error. Distinguish the offline UI preview from a native app run.

Remove credentials, private endpoints, personal files, and workspace details from logs and screenshots. Report vulnerabilities privately through [SECURITY.md](SECURITY.md).

## Make a change

1. Read the applicable `AGENTS.md` instructions.
2. Keep the patch focused and leave unrelated changes out.
3. Put shared backend behavior in shared cores and keep app/daemon adapters in sync.
4. Reuse the existing design system and registries.
5. Verify the changed behavior and document its limits.

Do not hand-edit generated protocol types. Preserve upstream copyright and third-party license notices.

## Checks

From `apps/taucode`:

```sh
npm ci
npm run typecheck
```

For frontend changes:

```sh
npm test
npm run build
npm run lint
```

For Rust changes, from `apps/taucode/src-tauri`:

```sh
cargo check --all-targets --features custom-protocol
cargo test
```

Native dependencies and the offline Cargo setup are covered in [installation](../docs/getting-started.md). Use focused tests when iterating, then run the required broader checks.

For documentation, from the repository root:

```sh
npm --prefix docs ci
npm --prefix docs run build
```

Inspect readable tables, links/assets, and narrow-screen layout. Label sample-data screenshots and distinguish previews from native/provider verification.

## Pull requests

Explain the concrete problem, resulting behavior, and relevant verification. State the platform/runtime actually tested and remaining gaps. Include a screenshot for visible UI changes and a regression check for nontrivial behavior.

Do not claim cross-platform parity, hard billing guarantees, complete sandboxing, or native acceptance from a unit test. Keep historical plans separate from current user guides.

Contributions use the repository's [MIT license](../LICENSE), subject to existing third-party licenses.
