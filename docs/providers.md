# Providers & models

A **provider** is the service or local server that runs your AI requests. A **model** is the particular AI you choose from that provider. Start with one provider and one model; you can add more later.

If this is your first setup, follow [installation steps 2–4](getting-started.md#_2-connect-one-provider-and-model). The [glossary](glossary.md) explains API keys, models, and engines in plain language.

## What to choose first

Use a provider you already have API access to. For a cloud provider, obtain an API key from its own API dashboard and review its pricing. For a local provider, the model server must already be running.

An **API key** is a secret credential; enter it in provider settings, not in your chat. A **base URL** is the server address; keep the preset's default unless your provider or server setup tells you otherwise.

You do not need to configure environment variables, auth commands, gateways, or role-specific models to make a first request.

## Add a provider

1. Open **Settings → Providers → Add provider**.
2. Choose a provider type and give it a recognizable name and ID.
3. Use its default endpoint or enter your server's base URL.
4. Configure authentication, save, then choose **Test & fetch models**.
5. Enable the models you want available in pickers.
6. In **Settings → Agents**, choose a default model. Role-specific models are optional advanced settings.

Use separate provider entries when two accounts or gateways need different credentials. A model reference combines the provider entry and model ID, so two identically named models on different providers remain distinct.

## Check that it works

1. Confirm that **Test & fetch models** returns a model list.
2. Enable one model, then select it in a new Chat.
3. Send a short message without attachments.

**Expected result:** a complete text answer. Loading a model list only checks discovery; sending a message checks that the provider can actually run that model for you.

| Problem | First thing to check |
| --- | --- |
| No models appear | Provider address, API key, connection error, and whether the provider is enabled. |
| A model appears in settings but not the picker | Whether that model is enabled. |
| 401 or 403 | Whether the key and account have access to the selected service/model. |
| Quota or rate-limit error | The provider's allowance or billing settings; repeated retries may not help. |
| Local server cannot be reached | Whether the model server is running on the expected computer and address. |

The rest of this page is the detailed reference for additional providers, authentication methods, and capabilities.

## Supported provider types

| Type | Default base URL | Notes |
| --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | Engine support depends on the endpoint capabilities. |
| Anthropic | `https://api.anthropic.com/v1` | Uses Anthropic authentication and model discovery. |
| Google | `https://generativelanguage.googleapis.com/v1beta` | Uses Google's API-key header and paginated model discovery. |
| OpenRouter | `https://openrouter.ai/api/v1` | Availability and pricing depend on the upstream model. |
| Groq | `https://api.groq.com/openai/v1` | Choose a model suitable for the requested tools. |
| Mistral | `https://api.mistral.ai/v1` | Capabilities vary by model. |
| xAI | `https://api.x.ai/v1` | Uses bearer authentication. |
| DeepSeek | `https://api.deepseek.com` | Model capabilities depend on the service. |
| Z.ai | `https://api.z.ai/api/paas/v4` | Check the endpoint for your service plan. |
| Ollama | `http://localhost:11434/v1` | The model server must already be running. |
| LM Studio | `http://localhost:1234/v1` | Start its local API server and load a model. |
| OpenAI compatible | User supplied | A self-hosted server or gateway must implement the API the selected engine needs. |

These are provider presets, not a promise that every model works with every engine or feature. A server that supports chat completions may not support the Responses API required by a Codex route.

## Authentication

taucode supports three provider authentication methods:

| Method | Use it when | Behavior |
| --- | --- | --- |
| API key | You have a persistent provider key | The value goes to the OS credential store, rather than the settings JSON. macOS uses Keychain. |
| Environment variable | An existing launch environment supplies the key | The variable name is configured; the value must exist in the app's environment. GUI-launched apps may not inherit your shell's variables. |
| Command | A trusted local helper supplies a short-lived credential | The helper's stdout supplies the credential. Treat the helper as executable code you trust. |

Keys are passed to engine children through the managed environment. Generated provider files reference environment variable names instead of embedding key values.

For a local service that does not require authentication, use its appropriate local preset and configuration. Do not paste a cloud key into a local endpoint simply to satisfy a placeholder field.

There are no consumer-account sign-in flows. Existing ChatGPT, Claude, or other subscriptions are not connected to taucode. Use the provider's API credentials or a local server.

## Local servers and gateways

For Ollama or LM Studio, start the service first, load a model, and verify the endpoint from the same computer that runs the engine. `localhost` refers to that computer: when an engine runs on a remote daemon, it refers to the daemon host.

For a gateway, choose **OpenAI compatible** and enter its actual base URL, including the API prefix if required. Avoid embedding a username, password, or token in the URL. Supply credentials through the authentication controls.

Start with a short plain-text request, then test tool use. Streaming, image inputs, tool schemas, context windows, and reasoning controls are separate compatibility questions. A small local model may answer chat questions while failing at structured agent tool calls.

## Model discovery and manual entries

**Test & fetch models** requests the provider's model catalog. Refreshing preserves existing enable flags and manually added models. If a server does not expose a usable model list, add its exact model ID manually in the provider editor.

The display name is for people; the model ID sent to the API must match the server. Do not infer a model ID from a marketing name or change its capitalization arbitrarily.

If discovery succeeds but inference fails, inspect the request error. Common causes include account access, a model not loaded locally, a missing API route, a quota limit, or an unsupported tool/image format.

## Reasoning controls

Available reasoning levels come from the model/provider capability information and explicit overrides. taucode can detect compatible gateway metadata and Ollama thinking capabilities; an explicit probe makes a small inference request.

A successful low-effort probe proves that request worked. It does not validate every reasoning level. Prefer the levels the model advertises and test a representative task before relying on them.

## Defaults, roles, and conversation models

The app's agent policy sets a default model. Project policies and Code conversation teams can override relevant settings. A role can use an explicitly selected provider/model or inherit the configured fallback.

Chat has an optional independent default model. Chat and Work run on dedicated pi profiles even if Code uses a different engine. Changing a default does not convert existing conversation history into another engine's format.

See [agents](agents.md) for role precedence and [engines](engines.md) for engine capabilities.

## Usage and cost

Usage is based on what the provider reports. If pricing is missing, taucode shows an unknown cost instead of treating it as zero. A local endpoint can also have real operating costs even when it reports no token price.

Team token allowances and evaluation caps are operational controls, not a guarantee about the final provider bill. Already running requests can complete after the threshold is crossed.

## Credential and connection problems

- **401/403:** verify the key, endpoint, provider type, and model access.
- **Model missing:** refresh discovery, check enable flags, or add its exact ID manually.
- **Environment key missing:** launch from the configured environment or use stored credentials.
- **Command timed out:** run the helper yourself; keep it in a local, available folder. Cloud-synced scripts can stall while downloading.
- **macOS Keychain prompts repeatedly:** keep the development signing identity stable and read [Keychain troubleshooting](troubleshooting.md#macos-keychain-prompts).

Do not put keys, auth-helper output, or full request headers in issues. The [privacy guide](privacy.md) explains storage and sharing boundaries.
