# Security policy

taucode is in early development and has not had an independent security audit. No long-term supported release line is declared.

## Report privately

Use GitHub's [private vulnerability report](https://github.com/aiden0rchad/taucode/security/advisories/new). Do not open a public issue with an exploit, credentials, private data, or details that would put users at immediate risk.

Include the build/commit, OS, engine/connector, expected boundary, minimal reproduction, and impact. Use disposable test data and placeholders. Never send real provider keys or private customer files.

If private reporting is temporarily unavailable, open an issue requesting a private channel without disclosing the vulnerability itself.

## Scope

Relevant boundaries include credential handling, daemon authentication, frontend IPC isolation, approval enforcement, workspace paths, browser/computer input, and untrusted tool content.

Custom engines and stdio servers are trusted executables. Full-access shell execution and remote MCP tools are not a universal OS sandbox. See [privacy and trust](../docs/privacy.md).

Allow time for assessment and a coordinated fix before publishing details. Response and fix timelines depend on severity and maintainer availability; no guaranteed window is promised.
