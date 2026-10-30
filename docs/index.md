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
      text: New here? Start here
      link: /start-here
    - theme: alt
      text: Make your first file
      link: /first-task
    - theme: alt
      text: View on GitHub
      link: https://github.com/aiden0rchad/taucode
features:
  - title: Code in context
    details: Ask questions about a coding project, review file changes, and work with its code in one place.
    link: /code
  - title: Just have a conversation
    details: Ask questions, discuss text or images you attach, and keep a separate chat history.
    link: /chat
  - title: Turn a brief into files
    details: Choose a folder, describe what you need, and follow the task through to its finished files.
    link: /work
  - title: Bring your own models
    details: Connect an AI service or an existing local model server. Start with one provider and one model.
    link: /providers
  - title: Choose the engine
    details: Start with pi, the default background program that manages conversations and tools. More options are available later.
    link: /engines
  - title: Inspect and recover
    details: Check what the agent did, review permission requests, and restore eligible files from saved checkpoints.
    link: /checkpoints
---

<div class="home-preview">

![Code mode with a project tree, conversation, plan, and session rail](/screenshots/code.png)

<figcaption>The real interface, rendered with the offline preview's sample data.</figcaption>

</div>

## A guide for each step

**First time using AI tools?** Begin with [New to taucode?](start-here.md), follow [installation](getting-started.md), then make a file with the [first Work task](first-task.md). Look up unfamiliar words in the [glossary](glossary.md).

| You want to… | Start here |
| --- | --- |
| Learn the basics without assuming coding knowledge | [Start here](start-here.md) |
| Follow a complete, no-coding example | [Your first Work task](first-task.md) |
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
