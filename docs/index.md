---
layout: home
title: Your models. Your workspace.
hero:
  name: taucode
  text: Your models.<br>Your workspace.
  tagline: Code, conversations, and document work in one desktop app. Choose your provider, pick an engine, and keep the work together.
  image:
    src: /taucode-mark.svg
    alt: taucode mark
  actions:
    - theme: brand
      text: Get started
      link: /getting-started
    - theme: alt
      text: Explore the guides
      link: /code
    - theme: alt
      text: View on GitHub
      link: https://github.com/aiden0rchad/taucode
features:
  - title: Code in context
    details: Projects, conversations, Git, diffs, terminals, side chats, and agent teams in the same workspace.
    link: /code
  - title: Just have a conversation
    details: A dedicated Chat profile with separate history, attachments, and optional restricted web access.
    link: /chat
  - title: Turn a brief into files
    details: Work on a folder with a plan, task queue, approval requests, and a visible list of outputs.
    link: /work
  - title: Bring your own models
    details: Cloud APIs, Ollama, LM Studio, and compatible gateways. Provider configuration belongs to you.
    link: /providers
  - title: Choose the engine
    details: pi, oh-my-pi, optional Codex, Claude Code through ACP, or a compatible custom executable.
    link: /engines
  - title: Inspect and recover
    details: See tool activity, review approvals, rewind checkpointed files, and compare engines with Evals.
    link: /checkpoints
---

<div class="home-preview">

![Code mode with a project tree, conversation, plan, and session rail](/screenshots/code.png)

<figcaption>The real interface, rendered with the offline preview's sample data.</figcaption>

</div>

## A guide for each step

| You want to… | Start here |
| --- | --- |
| Install taucode and run a first task | [Installation & first run](getting-started.md) |
| Connect a cloud API or local server | [Providers & models](providers.md) |
| Understand models, engines, and connectors | [FAQ](faq.md#what-is-the-difference-between-a-provider-a-model-an-engine-and-a-connector) |
| Work with more than one agent | [Agents & Chain of Command](agents.md) |
| Add tools or code intelligence | [MCP connectors & CodeGraph](connectors.md) |
| Understand what can leave your computer | [Privacy, storage & trust](privacy.md) |
| Fix a failed request or installation | [Troubleshooting](troubleshooting.md) |
| Build, test, or contribute | [Development](development.md) |

## Read this before installing

taucode is in **early development**. macOS has the broadest live verification; Windows has an experimental installer and build/startup checks. Linux and iOS need more validation. The [builds and platform status](releases.md) page explains the current evidence and limitations.

The app is built on [CodexMonitor](https://github.com/Dimillian/CodexMonitor). It uses your provider credentials and can make real file, shell, browser, or desktop changes according to the features and permissions you enable. Keep backups and read the [privacy and trust guide](privacy.md).
