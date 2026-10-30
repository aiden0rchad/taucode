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

Do not assume a model that handles ordinary image chat can reliably ground desktop controls. Test a harmless, bounded task and inspect the actual target/result.

Screenshots and relevant app context can be sent to the selected provider. A remote planner or grounding model makes this an external data transfer; see [privacy](privacy.md).

## Foreground and background interaction

Supported macOS paths use accessibility elements where possible, with policy checks around focus, target identity, and fallback input. Background operation has live verification for bounded tasks on the development Mac; applications differ in their accessibility behavior.

Some menus, hosted views, secure fields, or transient controls may not support the same interaction path. Missing targets and stale observations should produce a fresh observation or a failure, rather than being treated as evidence that input landed correctly.

Keep sensitive apps and documents out of the allowed scope. App restrictions reduce the target set; they do not make an inaccurate model accurate.

## Approval and takeover

Inspect pending actions before approving them. Policies can distinguish reading, typing, activation, menus, and higher-risk operations. Features and access policies can deny actions even when the model requests them.

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
