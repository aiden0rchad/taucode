# Computer use & permissions

This feature lets an agent look at and operate desktop applications. It is optional: the [first Chat message](getting-started.md#_4-send-a-first-chat-message) and [notes-to-summary exercise](first-task.md) do not need it.

For a first setup, leave it until you have a task that specifically needs another app. **Screen Recording** allows capture; **Accessibility** allows the supported inspection/input paths. These are macOS permissions you grant yourself. A model can make mistakes, so begin with a harmless test and learn the Stop/takeover controls before using important files.

Computer use lets a model inspect and interact with supported desktop apps. It is an **experimental macOS feature** with explicit model, permission, app-selection, and approval requirements.

Do not assume the Windows or Linux build has equivalent native desktop control. The [platform status](releases.md) describes the current verification boundary.

## How it works

The Rust host owns capture, target validation, policy checks, and input. A Python sidecar uses the vendored Agent-S3 planner and a selected grounding model to propose structured actions. The sidecar does not directly execute arbitrary screen or keyboard code.

An action is still a real action in the selected app. A successful click can send a message, alter a document, or change remote state if the target app allows it. Model proposals need the same care as any other automation.

## Prepare the feature

1. Enable **Computer use** in Settings → Features.
2. Prepare the computer runtime when requested; its first installation needs internet access.
3. In **Settings → Computer Use**, select appropriate planner and grounding models from your provider registry.
4. Review the app restrictions and approval settings.
5. Check **Screen Recording** and **Accessibility** in macOS System Settings.
6. Quit and reopen taucode after changing Screen Recording access when macOS requires it.

The first-run checklist can preflight permissions and open the relevant privacy panes. It does not silently grant permissions. The computer-use settings can request access; macOS makes the final grant decision.

## Choose planner and grounding models

The planner decides what action to attempt. Grounding maps that action to an observed target. Both need capabilities compatible with the requested input/output format.

| Grounding mode | Purpose |
| --- | --- |
| Pixels | Uses the dimensions of an aspect-preserving screenshot, capped at 1920 pixels on its long side. |
| UI-TARS | Uses the configured supported image dimensions and specialized grounding prompt. |

When the planner names a target by the exact label of one accessibility element, grounding uses that element without calling the grounding model. For runs of clicks on buttons that stay on screen, such as a calculator keypad, the planner can propose one click sequence. Each click is still its own step, with a fresh capture and the usual checks, but only the first needs a planner call. The sequence stops, and the planner takes over, if a step is not executed cleanly or the next label no longer matches exactly one element.

In a run that targets Notes or Reminders, the planner can search notes or reminders by title and text through the app's scripting interface instead of its search field, which background input reaches poorly or not at all. The search sends no input, and the count and up to 20 titles go to the planner with its next screenshot. The first search in each app makes macOS ask whether taucode may control it.

Keyboard steps wait up to 5 seconds for you to stop typing before sending keys, so your keystrokes cannot land in the target app. If you are still typing after that, the step is not sent and the planner tries again.

Do not assume a model that handles ordinary image chat can reliably ground desktop controls. Test a harmless, bounded task and inspect the actual target/result.

Each planning step has a time limit, **Planning seconds per step** in Settings → Computer Use (default 120, range 10–600). It covers the provider request and its retries. Raise it for slow providers or local models; the run's **Maximum seconds** still bounds the whole run.

Screenshots and relevant app context can be sent to the selected provider. A remote planner or grounding model makes this an external data transfer; see [privacy](privacy.md).

## Foreground and background interaction

Supported macOS paths use accessibility elements where possible, with policy checks around focus, target identity, and fallback input. Background operation has live verification for bounded tasks on the development Mac; applications differ in their accessibility behavior.

Some menus, hosted views, secure fields, or transient controls may not support the same interaction path. Missing targets and stale observations should produce a fresh observation or a failure, rather than being treated as evidence that input landed correctly.

Background typing and shortcuts briefly route the Mac's keyboard focus to the target window. While you are typing, taucode holds its keyboard input until you pause, so your keystrokes are not sent to the target app. If you press a key during taucode's own keyboard input, it stops sending input, does not replay anything, and only inspects the app to report what is visibly complete.

## App access

**Settings → Computer Use → App access** has two modes:

- **Any app** (default): a run can use any installed app that is not blocked.
- **Only listed apps**: a run can use only the bundle IDs in the allowed list.

Computer Use does not ask for approval by default. Turn on **Ask for approval** to be asked, like Codex desktop, before a run uses a new app (*Allow Computer Use to use "Notes"?* with **Decline**, **Allow for this session** until taucode quits, or **Always allow**) and before risky actions such as send, buy or delete clicks, Return, and unsafe shortcuts. With it on, always-allowed apps are listed in settings, where you can remove them, and browsers, messaging and mail apps, Contacts, Calendar, Finder and System Settings show a prompt-injection warning in the app prompt.

With approvals off, Computer Use still never types into password, card or other sensitive fields; it refuses those actions.

Computer Use works in the background. Some fields only take keyboard focus in the app in front, such as the search field in Notes. Turn on **Allow foreground fallback** to let Computer Use briefly bring the app forward and use the real mouse and keyboard for that step, then return you to your app. With **Ask for approval** on, it asks first. With approval off, the setting is your consent.

Some apps are never allowed, whatever you approve: password managers (1Password, Bitwarden, LastPass, Dashlane, NordPass, KeePassXC, Passwords), Keychain Access, terminals (Terminal, iTerm2, Warp, Ghostty, WezTerm, kitty, Rio, Wave, Termius), known banking apps, the macOS password and Touch ID prompts, and taucode itself. Add more under **Blocked app bundle IDs**.

An agent names apps by name (`Calculator`, `Find My`) or bundle ID (`com.apple.calculator`). In ask mode the name is looked up among installed apps; in listed mode a name that matches more than one allowed app is refused.

Keep sensitive apps and documents out of the allowed scope. App restrictions reduce the target set; they do not make an inaccurate model accurate.

## Approval and takeover

With **Ask for approval** on, inspect pending actions before approving them. The approval prompt appears in the taucode window as soon as an action needs approval, even while taucode is behind other windows, and a system notification is sent too. Time spent waiting for approval counts toward the run's **Maximum seconds**. Policies can distinguish reading, typing, activation, menus, and higher-risk operations. Features and access policies can deny actions even when the model requests them.

Use **Stop** or the available takeover control as soon as the automation is doing something unexpected. Input already delivered cannot be recalled. A checkpoint of the project folder cannot undo changes in another app.

Start with a clearly bounded test:

```text
Use Calculator to calculate 12 × 7. Report the visible result and stop.
Do not interact with any other app.
```

This is a useful smoke check, not a broad capability or safety certification.

## Permission and signing issues

macOS associates privacy and Keychain access with app identity. Ad-hoc signing and identity changes can lead to repeated permission prompts after rebuilding. Development signing can use a stable existing identity; see [macOS packaging](packaging.md).

Screen Recording and Accessibility are separate TCC grants. A signing entitlement alone does not grant either. Provider Keychain approval is a third, separate concern.

If permission appears granted but capture fails, confirm the exact build is listed, quit/reopen after the grant, and test a simple allowed app. Do not repeatedly broaden permissions to compensate for a model or target-selection error.

## Current limitations

- Native live verification is macOS-specific and does not cover every app or OS version.
- Background behavior depends on target app accessibility support.
- Model inference, image handling, and grounding can fail independently.
- A prediction uses one bounded deadline across planning, correction, grounding, and retries; an upstream delay can exhaust it.
- Lock-screen or otherwise disallowed targets are refused on the verified paths.
- Stop limits future work; it does not reverse an already completed operation.

Use the [browser tools](browser.md) for browser-only work when they provide a clearer, narrower interaction surface.
