# New to taucode? Start here

taucode is an app where you can talk to AI, ask it to help with code, or give it a folder of files to work on. This guide explains the choices you will see before you start changing settings.

You do not need to understand agent protocols, use several AI models, or write code to follow the [first Work task](first-task.md). Installing this early test version can still involve extra steps; the [installation guide](getting-started.md) explains them by operating system.

## 1. Choose what you want to do

The three buttons at the top of the sidebar select a mode:

| Choose | When you want to… | A first request to try |
| --- | --- | --- |
| **Chat** | Ask a question or discuss text you provide. | “Explain this paragraph in simpler words.” |
| **Work** | Read files in a chosen folder and create a document or other output. | “Summarize these notes into a file.” |
| **Code** | Understand or change a software project. | “Explain how this project is organized.” |

Start with **Chat** to check that your model connection works. Then try **Work** if you want the app to create files. Code is useful when you have a coding project.

## 2. Understand the three setup choices

You will see the words **provider**, **model**, and **engine**. They describe different parts of the setup:

| Word | Plain-language meaning | What you choose |
| --- | --- | --- |
| **Provider** | The service or local server that answers AI requests. | An API service you have access to, or an existing local model server. |
| **Model** | The particular AI you want that provider to run. | One available model from the list taucode fetches. |
| **Engine** | The background program that manages the conversation and tool use. | Start with the default engine, **pi**. |

In practice: **connect one provider → enable one model → prepare pi → send a message**. You can explore other engines or agent teams later.

### What is an API key?

An API key is a secret credential that lets an app use your provider account. Obtain it through your chosen provider's own API dashboard and enter it in **Settings → Providers**. Treat it like a password.

API access can have its own bill. A ChatGPT or Claude consumer subscription does not connect to taucode. Read your provider's API pricing and limits before using it.

An existing local model server is another option, but setting one up is a separate task. The [provider guide](providers.md) covers connecting one that is already running.

## 3. Install and connect

Follow [Installation & first run](getting-started.md) to:

1. Download the test installer for your computer, or use the source-build instructions if needed.
2. Open taucode and add your provider.
3. Enable a model and prepare the default pi engine.
4. Send a simple message in Chat.

**You are ready to continue when:** Chat displays a complete answer to your test message. If the model list loads but messages fail, check the [connection checklist](providers.md#check-that-it-works).

## 4. Make your first file

The [first Work task](first-task.md) uses a small practice folder and made-up meeting notes. It walks through every step, from creating the input file to opening the finished summary.

You will learn what a **space**, **task**, **approval**, and **output** mean while using them. Keep important personal files out of the practice folder.

## 5. Know what to check

Before sending a task, check the selected folder, model, and access setting. **Ask** can pause for approval; read what the app is asking to do before continuing.

After a task finishes, open the actual output and compare it with your request. AI can make mistakes even when the status says Done. **Stop** interrupts work, but it does not automatically undo changes already made.

The selected provider can receive your messages and relevant file content. Read [privacy in plain language](privacy.md#the-short-version) before using sensitive material.

## How to read these guides

- **Settings → Providers** means open Settings, then choose Providers.
- Text such as **Run now** names a button or setting in the app.
- `notes.txt` is a filename. `outputs/` is a folder name; the slash means “folder.”
- A block labeled `text` contains an example request or file content you can copy.
- A block labeled `sh` or `powershell` contains terminal commands. Those appear in the developer setup sections; they are not messages to send to the AI.
- “Expected result” tells you how to check that a step worked before moving on.

Each main guide begins with the everyday steps. Its later sections explain limits and technical details when you need them.

## Pick your next guide

| If you want to… | Open |
| --- | --- |
| Install the app | [Installation & first run](getting-started.md) |
| Follow a complete example without coding | [Your first Work task](first-task.md) |
| Understand an unfamiliar word | [Plain-language glossary](glossary.md) |
| Write a better request | [Writing a useful brief](work.md#write-a-useful-brief) |
| Fix something that failed | [First things to try](troubleshooting.md#first-things-to-try) |
| Explore the detailed features | [Code](code.md), [Chat](chat.md), or [Work](work.md) |
