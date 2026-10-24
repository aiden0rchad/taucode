# M3d: computer-use policy refinements (usable but safe)

## Why

Live testing of harder tasks needs typing (Notes, TextEdit) and read-only System Settings.
Today `computer/policy.rs`:
- asks for approval on **every** Type/Hotkey/HoldAndPress, so an agent can't fill a note
  without the user clicking approve per keystroke
- **hard-denies** System Settings (and Terminal) entirely

## Required

1. **Typing confirmation setting**, `computer.confirmTyping`: `"always"` | `"sensitive"`
   (default **`"sensitive"`**).
   - **sensitive:**
     - typing (AX set value / selected-text insert / background unicode typing) into a
       non-secure text field of an allowed, full-access app → allow
     - still **confirm**:
       - secure fields (`AXSecureTextField`, or a subrole marking it secure)
       - any `type` with `enter: true`, and any Return/Enter press
       - risky hotkeys: ⌘Q, ⌘W, ⌘⌫, ⌘⌥⌫, ⌘⇧⌫, ⌘⌥Esc, ⌘S on an unsaved new document, ⌃⌘Q,
         anything with ⌘ plus Delete/Backspace, Tab-cycling apps (⌘Tab)
       - typing into a field whose label/placeholder/identifier matches
         password/passcode/pin/otp/card/cvv/ssn/iban/routing (whole words, case-insensitive)
     - allow a small safe hotkey set: ⌘N, ⌘F, ⌘A, ⌘C, ⌘V, ⌘Z, ⌘⇧Z, arrows, Tab, Escape;
       document the list in the settings UI
   - **always:** current behaviour.
2. **Per-app access level.** `allowedApps` entries become
   `{ bundleId, access: "full" | "read-only" }`; migrate plain strings to `"full"`.
   - **read-only:**
     - allow: click/press on navigation elements (AXButton, AXRow, AXCell, AXOutlineRow,
       AXLink, AXTab, AXMenuItem, AXDisclosureTriangle, AXStaticText), scroll, and opening
       the app
     - **deny**:
       - type/hotkey/hold_and_press
       - AX set value
       - pressing AXCheckBox / AXSwitch / AXRadioButton / AXSlider / AXIncrementor /
         AXPopUpButton / AXComboBox / AXTextField / AXTextArea
       - any click whose target matches the risky words
       - drag
     - The reason string names the access level so the planner can adapt.
3. **System Settings becomes read-only, never full.**
   - Remove `com.apple.systempreferences` from the hard denylist. It's always treated as
     read-only, even if a user configures full (UI shows it locked to read-only).
   - Add a **pane denylist**: Privacy & Security, Users & Groups, Passwords, Login Password,
     Lock Screen, Touch ID & Password, Wallet & Apple Pay, Login Items & Extensions, Sharing,
     Network, Battery → Options. Detect the current pane from the window title and the
     selected sidebar row (AX).
   - Navigating **to** a denied pane (clicking its sidebar row) and acting **inside** one are
     both denied, with a clear reason. Reading a screenshot of an allowed pane is fine.
   - Terminal and the other hard-denied apps stay denied.
4. **Settings UI:** the `confirmTyping` selector with an explanation; per-app access level in
   the allowed-apps editor; a note on System Settings' forced read-only and its pane
   denylist.
5. **Tests:** decision tables covering every rule above (typing sensitive vs always, secure
   fields, Enter, each risky hotkey, sensitive-label fields, the safe hotkeys, read-only
   allow/deny by role, System Settings forced read-only, pane denylist by title/sidebar,
   migration of plain-string allowlists).

## Constraints and verification

Offline; no new dependencies; always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. Same verification command as
`m3-computer-use.md`. Don't run `tauri build`. No weakening of the other M3 rules (no eval,
the sidecar never executes input, approvals UI-only, kill switch, Terminal denied).
