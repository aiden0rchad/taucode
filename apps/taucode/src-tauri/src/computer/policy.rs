use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub(crate) use crate::types::{
    ComputerAccess as Access, ComputerSettings as Settings, ConfirmTyping,
};

pub fn validate(s: &Settings) -> Result<(), String> {
    if !matches!(s.mode.as_str(), "background" | "foreground") {
        return Err("Invalid computer mode".into());
    }
    if !matches!(s.grounding_mode.as_str(), "elements" | "pixels" | "ui-tars") {
        return Err("Invalid grounding mode".into());
    }
    if !(1..=500).contains(&s.max_steps)
        || !(1..=3600).contains(&s.max_seconds)
        || !(1..=8192).contains(&s.grounding_width)
        || !(1..=8192).contains(&s.grounding_height)
        || s.shortcut
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .is_err()
        || s.allowed_apps
            .iter()
            .map(|a| &a.bundle_id)
            .chain(&s.denied_apps)
            .any(|a| a.is_empty() || a.contains(char::is_whitespace) || a.contains('\0'))
    {
        return Err("Invalid computer limits, bundle IDs or kill-switch shortcut".into());
    }
    if s.enabled && (s.planner.is_none() || s.grounding.is_none()) {
        return Err("Select planner and grounding models".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Click {
        x: f64,
        y: f64,
        button: String,
        clicks: u32,
        hold_keys: Vec<String>,
    },
    Type {
        text: String,
        x: Option<f64>,
        y: Option<f64>,
        overwrite: bool,
        enter: bool,
    },
    Scroll {
        x: f64,
        y: f64,
        clicks: i32,
        shift: bool,
    },
    Drag {
        from: [f64; 2],
        to: [f64; 2],
        hold_keys: Vec<String>,
    },
    Hotkey {
        keys: Vec<String>,
    },
    HoldAndPress {
        hold: Vec<String>,
        press: Vec<String>,
    },
    Open {
        app_or_file: String,
    },
    SwitchApp {
        app: String,
    },
    Wait {
        seconds: f64,
    },
    Done {
        summary: String,
    },
    Fail {
        reason: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prediction {
    pub action: Value,
    pub reflection: String,
    pub plan_text: String,
    #[serde(default)]
    pub grounding: Vec<Value>,
}
#[derive(Clone, Copy, Debug)]
pub struct Display {
    pub origin: [f64; 2],
    pub width: f64,
    pub height: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
}
pub fn map_point(p: [f64; 2], space: [f64; 2], display: Display) -> Result<[f64; 2], String> {
    if space.iter().chain(p.iter()).any(|n| !n.is_finite())
        || space.iter().any(|n| *n <= 0.)
        || p.iter().zip(space).any(|(n, s)| *n < 0. || *n >= s)
        || display.origin.iter().any(|n| !n.is_finite())
        || !display.width.is_finite()
        || !display.height.is_finite()
        || display.width <= 0.
        || display.height <= 0.
    {
        return Err("Coordinates outside display".into());
    }
    Ok([
        display.origin[0] + p[0] * display.width / space[0],
        display.origin[1] + p[1] * display.height / space[1],
    ])
}
impl Prediction {
    pub fn element_target(&self) -> Result<Option<(u64, usize)>, String> {
        match (
            self.action.get("snapshot_id"),
            self.action.get("element_index"),
        ) {
            (None, None) => Ok(None),
            (Some(id), Some(index))
                if matches!(
                    self.action["kind"].as_str(),
                    Some("click" | "type" | "scroll")
                ) =>
            {
                let id = id
                    .as_u64()
                    .filter(|n| *n > 0)
                    .ok_or("Invalid snapshot id")?;
                let index = index
                    .as_u64()
                    .filter(|n| *n < 250)
                    .ok_or("Invalid element index")?;
                Ok(Some((id, index as usize)))
            }
            _ => Err("Invalid element target".into()),
        }
    }
    pub fn validated(&self, display: Display) -> Result<Action, String> {
        self.element_target()?;
        let mut value = self.action.clone();
        let obj = value.as_object_mut().ok_or("Expected action object")?;
        obj.remove("snapshot_id");
        obj.remove("element_index");
        if let Some(target) = obj.remove("target") {
            if target
                .as_str()
                .is_none_or(|s| s.len() > 32768 || s.contains('\0'))
            {
                return Err("Invalid action target".into());
            }
        } else if obj.get("kind").and_then(Value::as_str) == Some("click") {
            return Err("Missing click target".into());
        }
        let mut space = [0.; 2];
        for (i, key) in ["grounding_width", "grounding_height"].iter().enumerate() {
            space[i] = obj
                .remove(*key)
                .and_then(|v| v.as_f64())
                .filter(|n| n.is_finite() && *n > 0. && *n <= 8192.)
                .ok_or("Missing grounding dimensions")?;
        }
        let mut action: Action =
            serde_json::from_value(value).map_err(|e| format!("Invalid action: {e}"))?;
        match &mut action {
            Action::Click { x, y, .. } | Action::Scroll { x, y, .. } => {
                [*x, *y] = map_point([*x, *y], space, display)?;
            }
            Action::Type { x, y, .. } => match (*x, *y) {
                (Some(a), Some(b)) => {
                    let p = map_point([a, b], space, display)?;
                    *x = Some(p[0]);
                    *y = Some(p[1]);
                }
                (None, None) => (),
                _ => return Err("Incomplete type coordinates".into()),
            },
            Action::Drag { from, to, .. } => {
                *from = map_point(*from, space, display)?;
                *to = map_point(*to, space, display)?;
            }
            _ => (),
        }
        let key_lists = match &action {
            Action::Click { hold_keys, .. } | Action::Drag { hold_keys, .. } => vec![hold_keys],
            Action::Hotkey { keys } => vec![keys],
            Action::HoldAndPress { hold, press } => vec![hold, press],
            _ => vec![],
        };
        if key_lists
            .iter()
            .any(|list| list.len() > 16 || list.iter().any(|k| keycode(k).is_none()))
        {
            return Err("Unknown key or excessive key count".into());
        }
        match &action {
            Action::Click { button, clicks, .. }
                if !["left", "right", "middle"].contains(&button.as_str())
                    || !(1..=3).contains(clicks) =>
            {
                return Err("Invalid click".into())
            }
            Action::Scroll { clicks, .. } if clicks.unsigned_abs() > 100 => {
                return Err("Invalid scroll".into())
            }
            Action::Wait { seconds } if !seconds.is_finite() || !(0.0..=10.).contains(seconds) => {
                return Err("Invalid wait".into())
            }
            Action::Type { text, .. } if text.len() > 32768 || text.contains('\0') => {
                return Err("Invalid text".into())
            }
            _ => (),
        }
        Ok(action)
    }
}
pub const ALWAYS_DENIED: &[&str] = &[
    "com.agilebits.onepassword7",
    "com.agilebits.onepassword",
    "com.agilebits.onepassword-osx",
    "com.1password.1password",
    "com.bitwarden.desktop",
    "com.lastpass.LastPass",
    "com.dashlane.Dashlane",
    "com.apple.keychainaccess",
    "com.apple.Passwords",
    "com.keepassxc.keepassxc",
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "com.chase",
    "com.chase.sig.android",
    "com.bankofamerica.BofA",
    "com.wf.wellsfargomobile",
    "com.revolut.revolut",
    "com.paypal.PPClient",
];
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    pub app: String,
    pub secure: bool,
}
// Host AX evidence only; never populate this from planner text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Target {
    pub role: String,
    pub label: String,
    pub secure: bool,
    pub window_title: String,
    pub selected_rows: String,
    pub resolved_hotkey: bool,
}
pub const SYSTEM_SETTINGS: &str = "com.apple.systempreferences";
pub fn access(app: &str, s: &Settings) -> Access {
    if app.eq_ignore_ascii_case(SYSTEM_SETTINGS)
        || s.allowed_apps
            .iter()
            .any(|a| a.bundle_id == app && a.access == Access::ReadOnly)
    {
        Access::ReadOnly
    } else {
        Access::Full
    }
}
pub fn secure_field(role: &str, subrole: &str) -> bool {
    role.to_ascii_lowercase().contains("secure") || subrole.to_ascii_lowercase().contains("secure")
}
pub fn denied_pane(title: &str, sidebar: &str) -> Option<&'static str> {
    let text = format!("{title} {sidebar}")
        .to_lowercase()
        .replace('&', "and");
    [
        "Privacy & Security",
        "Users & Groups",
        "Passwords",
        "Login Password",
        "Lock Screen",
        "Touch ID & Password",
        "Wallet & Apple Pay",
        "Login Items & Extensions",
        "Sharing",
        "Network",
    ]
    .into_iter()
    .find(|pane| text.contains(&pane.to_lowercase().replace('&', "and")))
    .or_else(|| {
        (text.contains("battery") && text.contains("options")).then_some("Battery → Options")
    })
}
pub fn check_pane(app: &str, target: &Target) -> Result<(), String> {
    if app.eq_ignore_ascii_case(SYSTEM_SETTINGS) {
        if let Some(pane) = denied_pane(&target.window_title, &target.selected_rows) {
            return Err(format!(
                "System Settings read-only access: pane blocked: {pane}"
            ));
        }
    }
    Ok(())
}
fn sensitive_field(label: &str) -> bool {
    label
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|word| {
            [
                "password", "passcode", "pin", "otp", "card", "cvv", "ssn", "iban", "routing",
            ]
            .iter()
            .any(|s| word.eq_ignore_ascii_case(s))
        })
}
fn safe_hotkey(keys: &[String]) -> bool {
    let Some(mut keys) = keys.iter().map(|k| keycode(k)).collect::<Option<Vec<_>>>() else {
        return false;
    };
    keys.sort_unstable();
    matches!(
        keys.as_slice(),
        [0, 55]
            | [3, 55]
            | [6, 55]
            | [6, 55, 56]
            | [8, 55]
            | [9, 55]
            | [45, 55]
            | [48]
            | [53]
            | [123]
            | [124]
            | [125]
            | [126]
    )
}
fn risky_hotkey(keys: &[String]) -> bool {
    let codes: Vec<_> = keys.iter().filter_map(|k| keycode(k)).collect();
    codes.contains(&55)
        && codes
            .iter()
            .any(|k| matches!(k, 1 | 12 | 13 | 48 | 51 | 53 | 117))
}
fn return_key(keys: &[String]) -> bool {
    keys.len() == 1 && keycode(&keys[0]) == Some(36)
}
#[derive(Debug, PartialEq)]
pub enum Decision {
    Allow,
    Confirm(&'static str),
    Deny(String),
}
pub fn app_allowed(app: &str, s: &Settings, apps: &[String]) -> bool {
    !app.is_empty()
        && app != "dev.taucode.desktop"
        && !ALWAYS_DENIED.iter().any(|a| a.eq_ignore_ascii_case(app))
        && !s.denied_apps.iter().any(|a| a.eq_ignore_ascii_case(app))
        && s.allowed_apps.iter().any(|a| a.bundle_id == app)
        && apps.iter().any(|a| a == app)
}
pub fn decide(
    a: &Action,
    description: &str,
    c: &Context,
    target_info: &Target,
    s: &Settings,
    apps: &[String],
    steps: u32,
    elapsed: Duration,
) -> Decision {
    if steps >= s.max_steps || elapsed >= Duration::from_secs(s.max_seconds) {
        return Decision::Deny("Run limit reached".into());
    }
    let bootstrap = s.mode == "foreground"
        && c.app == "dev.taucode.desktop"
        && matches!(a, Action::Open { .. } | Action::SwitchApp { .. });
    if !bootstrap && !app_allowed(&c.app, s, apps) {
        return Decision::Deny(format!("App blocked: {}", c.app));
    }
    let target = match a {
        Action::Open { app_or_file } => Some(app_or_file),
        Action::SwitchApp { app } => Some(app),
        _ => None,
    };
    if target
        .is_some_and(|app| !app_allowed(app, s, apps) || (s.mode == "background" && app != &c.app))
    {
        return Decision::Deny("Target app is not allowed; use an allowed bundle ID".into());
    }
    if let Err(reason) = check_pane(&c.app, target_info) {
        return Decision::Deny(reason);
    }
    if c.app.eq_ignore_ascii_case(SYSTEM_SETTINGS) && matches!(a, Action::Click { .. }) {
        if let Some(pane) = denied_pane(
            &format!("{} {}", target_info.label, description),
            &format!("{} {}", target_info.window_title, target_info.selected_rows),
        ) {
            return Decision::Deny(format!(
                "System Settings read-only access: navigation to pane blocked: {pane}"
            ));
        }
    }
    if access(&c.app, s) == Access::ReadOnly && !bootstrap {
        let allowed = match a {
            Action::Click {
                button,
                clicks,
                hold_keys,
                ..
            } => {
                button == "left"
                    && *clicks == 1
                    && hold_keys.is_empty()
                    && matches!(
                        target_info.role.as_str(),
                        "AXButton"
                            | "AXRow"
                            | "AXCell"
                            | "AXOutlineRow"
                            | "AXLink"
                            | "AXTab"
                            | "AXMenuItem"
                            | "AXDisclosureTriangle"
                            | "AXStaticText"
                    )
                    && (target_info.role != "AXMenuItem"
                        || matches!(
                            target_info.label.split(" > ").next(),
                            Some("View" | "Go" | "Window" | "Help")
                        ))
                    && !risky_target(description)
                    && !risky_target(&target_info.label)
            }
            Action::Scroll { .. }
            | Action::Open { .. }
            | Action::SwitchApp { .. }
            | Action::Wait { .. }
            | Action::Done { .. }
            | Action::Fail { .. } => true,
            _ => false,
        };
        if !allowed {
            return Decision::Deny("App has read-only access: only navigation clicks, scrolling and opening the app are allowed".into());
        }
    }
    if let Action::Click { hold_keys, .. } | Action::Drag { hold_keys, .. } = a {
        if hold_keys
            .iter()
            .any(|k| !matches!(keycode(k), Some(55 | 56 | 58 | 59)))
        {
            return Decision::Confirm("Confirm keyboard presses held during pointer action");
        }
    }
    if matches!(a, Action::Type { .. }) && (c.secure || target_info.secure) {
        return Decision::Confirm("Typing into a secure text field");
    }
    if matches!(a, Action::Click { .. })
        && (risky_target(description) || risky_target(&target_info.label))
    {
        return Decision::Confirm("Potential submit, send, purchase or destructive click");
    }
    if matches!(
        a,
        Action::Type { .. } | Action::Hotkey { .. } | Action::HoldAndPress { .. }
    ) {
        if s.confirm_typing == ConfirmTyping::Always {
            return Decision::Confirm("Confirm typing or keyboard action");
        }
        if sensitive_field(&target_info.label) {
            return Decision::Confirm("Typing or keyboard action in a sensitive field");
        }
        if c.secure || target_info.secure {
            return Decision::Confirm("Typing or keyboard action in a secure field");
        }
        if let Action::Hotkey { keys } = a {
            if risky_hotkey(keys) {
                return Decision::Confirm("Potentially unsafe keyboard shortcut");
            }
            if target_info.resolved_hotkey && target_info.role == "AXMenuItem" {
                return if risky_target(&target_info.label) {
                    Decision::Confirm("Potential submit, send, purchase or destructive menu item")
                } else {
                    Decision::Allow
                };
            }
        }
        let allow_return = s.confirm_typing == ConfirmTyping::RiskyOnly
            && !c.secure
            && !target_info.secure
            && matches!(
                target_info.role.as_str(),
                "AXTextField" | "AXTextArea" | "AXComboBox"
            );
        let safe = match a {
            Action::Type { enter, text, .. } => {
                (allow_return || (!enter && !text.contains(['\r', '\n'])))
                    && matches!(
                        target_info.role.as_str(),
                        "AXTextField" | "AXTextArea" | "AXComboBox"
                    )
            }
            Action::Hotkey { keys } => {
                (safe_hotkey(keys) || (allow_return && return_key(keys)))
                    && !target_info.secure
                    && !c.secure
            }
            Action::HoldAndPress { hold, press } => {
                !press.is_empty()
                    && press.iter().all(|key| {
                        let mut keys = hold.clone();
                        keys.push(key.clone());
                        safe_hotkey(&keys) || (allow_return && return_key(&keys))
                    })
                    && !target_info.secure
                    && !c.secure
            }
            _ => false,
        };
        if !safe {
            return Decision::Confirm(
                "Confirm typing, Enter or potentially unsafe keyboard action",
            );
        }
    }
    Decision::Allow
}

#[cfg(unix)]
fn risky_target(target: &str) -> bool {
    // POSIX word boundaries, using the already-linked native regex API.
    let Ok(target) = std::ffi::CString::new(target) else {
        return true;
    };
    unsafe {
        let mut regex = std::mem::MaybeUninit::<libc::regex_t>::uninit();
        if libc::regcomp(regex.as_mut_ptr(), c"(^|[^[:alnum:]_])(submit|send|buy|purchase|pay|checkout|confirm|order|delete|transfer|remove|erase|unsubscribe|publish|post)([^[:alnum:]_]|$)".as_ptr(), libc::REG_EXTENDED | libc::REG_ICASE | libc::REG_NOSUB) != 0 {
            return true;
        }
        let mut regex = regex.assume_init();
        let matched =
            libc::regexec(&regex, target.as_ptr(), 0, std::ptr::null_mut(), 0) != libc::REG_NOMATCH;
        libc::regfree(&mut regex);
        matched
    }
}

#[cfg(not(unix))]
fn risky_target(_: &str) -> bool {
    true // Computer Use is unsupported here; never allow an unchecked click.
}
pub fn keycode(key: &str) -> Option<u16> {
    Some(match key.to_ascii_lowercase().as_str() {
        "a" => 0,
        "s" => 1,
        "d" => 2,
        "f" => 3,
        "h" => 4,
        "g" => 5,
        "z" => 6,
        "x" => 7,
        "c" => 8,
        "v" => 9,
        "b" => 11,
        "q" => 12,
        "w" => 13,
        "e" => 14,
        "r" => 15,
        "y" => 16,
        "t" => 17,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "6" => 22,
        "5" => 23,
        "=" => 24,
        "9" => 25,
        "7" => 26,
        "-" => 27,
        "8" => 28,
        "0" => 29,
        "]" => 30,
        "o" => 31,
        "u" => 32,
        "[" => 33,
        "i" => 34,
        "p" => 35,
        "enter" | "return" => 36,
        "l" => 37,
        "j" => 38,
        "'" => 39,
        "k" => 40,
        ";" => 41,
        "\\" => 42,
        "," => 43,
        "/" => 44,
        "n" => 45,
        "m" => 46,
        "." => 47,
        "tab" => 48,
        "space" => 49,
        "`" => 50,
        "backspace" => 51,
        "escape" | "esc" => 53,
        "command" | "cmd" | "super" => 55,
        "shift" => 56,
        "alt" | "option" => 58,
        "ctrl" | "control" => 59,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        "home" => 115,
        "end" => 119,
        "pageup" => 116,
        "pagedown" => 121,
        "delete" => 117,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        _ => return None,
    })
}

#[cfg(test)]
mod refinements {
    use super::*;

    fn settings() -> Settings {
        Settings {
            allowed_apps: vec!["com.apple.TextEdit".into(), SYSTEM_SETTINGS.into()],
            ..Default::default()
        }
    }
    fn field(role: &str, label: &str) -> Target {
        Target {
            role: role.into(),
            label: label.into(),
            ..Default::default()
        }
    }
    fn decision(action: &Action, target: &Target, settings: &Settings, app: &str) -> Decision {
        decide(
            action,
            "ordinary target",
            &Context {
                app: app.into(),
                secure: false,
            },
            target,
            settings,
            &settings.app_ids(),
            0,
            Duration::ZERO,
        )
    }
    fn typing(enter: bool) -> Action {
        Action::Type {
            text: "hello".into(),
            x: None,
            y: None,
            overwrite: false,
            enter,
        }
    }
    fn click() -> Action {
        Action::Click {
            x: 0.,
            y: 0.,
            button: "left".into(),
            clicks: 1,
            hold_keys: vec![],
        }
    }
    fn keys(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|k| (*k).into()).collect()
    }
    fn confirmed(d: Decision) {
        assert!(matches!(d, Decision::Confirm(_)), "{d:?}");
    }

    #[test]
    fn menu_clicks_obey_read_only_roots_risky_titles_and_pane_denylist() {
        let mut s = settings();
        for title in ["File > Delete", "Account > Pay", "Message > Send"] {
            confirmed(decision(
                &click(),
                &field("AXMenuItem", title),
                &s,
                "com.apple.TextEdit",
            ));
        }
        s.allowed_apps[0].access = Access::ReadOnly;
        for menu in ["View", "Go", "Window", "Help"] {
            assert_eq!(
                decision(
                    &click(),
                    &field("AXMenuItem", &format!("{menu} > Next")),
                    &s,
                    "com.apple.TextEdit"
                ),
                Decision::Allow
            );
        }
        for title in [
            "File > New Folder",
            "Edit > Copy",
            "TextEdit > About",
            "Next",
        ] {
            let d = decision(
                &click(),
                &field("AXMenuItem", title),
                &s,
                "com.apple.TextEdit",
            );
            assert!(matches!(d, Decision::Deny(reason) if reason.contains("read-only")));
        }
        for role in ["AXRow", "AXOutlineRow", "AXMenuItem"] {
            let label = if role == "AXMenuItem" {
                "View > General"
            } else {
                "General"
            };
            assert_eq!(
                decision(&click(), &field(role, label), &s, SYSTEM_SETTINGS),
                Decision::Allow
            );
            denied(decision(
                &click(),
                &field(role, "Privacy & Security"),
                &s,
                SYSTEM_SETTINGS,
            ));
        }
    }

    #[test]
    fn resolved_hotkeys_use_menu_policy_with_risky_shortcut_precedence() {
        let mut s = settings();
        let ordinary = Target {
            resolved_hotkey: true,
            ..field("AXMenuItem", "File > New Folder")
        };
        let action = Action::Hotkey {
            keys: keys(&["cmd", "shift", "n"]),
        };
        confirmed(decide(
            &action,
            "New Folder",
            &Context {
                app: "com.apple.TextEdit".into(),
                secure: true,
            },
            &ordinary,
            &s,
            &s.app_ids(),
            0,
            Duration::ZERO,
        ));
        assert_eq!(
            decision(&action, &ordinary, &s, "com.apple.TextEdit"),
            Decision::Allow
        );
        // A focused menu item alone is not evidence that this shortcut resolved.
        confirmed(decision(
            &action,
            &field("AXMenuItem", "File > New Folder"),
            &s,
            "com.apple.TextEdit",
        ));
        confirmed(decision(
            &action,
            &field("AXTextArea", "Document"),
            &s,
            "com.apple.TextEdit",
        ));
        for label in ["File > Delete", "Send > Message", "Account > Pay"] {
            confirmed(decision(
                &action,
                &Target {
                    resolved_hotkey: true,
                    ..field("AXMenuItem", label)
                },
                &s,
                "com.apple.TextEdit",
            ));
        }
        for chord in [
            &["cmd", "q"][..],
            &["cmd", "w"],
            &["cmd", "backspace"],
            &["cmd", "shift", "delete"],
            &["cmd", "s"],
            &["ctrl", "cmd", "q"],
        ] {
            confirmed(decision(
                &Action::Hotkey { keys: keys(chord) },
                &ordinary,
                &s,
                "com.apple.TextEdit",
            ));
        }
        s.confirm_typing = ConfirmTyping::Always;
        confirmed(decision(&action, &ordinary, &s, "com.apple.TextEdit"));
        s.confirm_typing = ConfirmTyping::Sensitive;
        s.allowed_apps[0].access = Access::ReadOnly;
        denied(decision(&action, &ordinary, &s, "com.apple.TextEdit"));
        denied(decision(&action, &ordinary, &s, SYSTEM_SETTINGS));
    }

    #[test]
    fn risky_only_return_decision_table() {
        let mut s = settings();
        s.confirm_typing = ConfirmTyping::RiskyOnly;
        assert_eq!(
            serde_json::to_value(&s).unwrap()["confirmTyping"],
            "risky-only"
        );
        assert_eq!(
            serde_json::from_value::<Settings>(serde_json::json!({"confirmTyping":"risky-only"}))
                .unwrap()
                .confirm_typing,
            ConfirmTyping::RiskyOnly
        );
        for role in [
            "AXTextField",
            "AXTextArea",
            "AXComboBox",
            "AXButton",
            "AXSecureTextField",
            "",
        ] {
            for sensitive in [false, true] {
                for secure in [false, true] {
                    let target = Target {
                        secure,
                        ..field(role, if sensitive { "Password" } else { "Document" })
                    };
                    for action in [
                        typing(true),
                        Action::Hotkey {
                            keys: keys(&["return"]),
                        },
                        Action::Hotkey {
                            keys: keys(&["enter"]),
                        },
                        Action::HoldAndPress {
                            hold: vec![],
                            press: keys(&["return"]),
                        },
                    ] {
                        let result = decision(&action, &target, &s, "com.apple.TextEdit");
                        if !secure
                            && !sensitive
                            && matches!(role, "AXTextField" | "AXTextArea" | "AXComboBox")
                        {
                            assert_eq!(result, Decision::Allow, "{role}");
                        } else {
                            confirmed(result);
                        }
                    }
                }
            }
        }
        confirmed(decision(
            &Action::Hotkey {
                keys: keys(&["cmd", "w"]),
            },
            &field("AXTextArea", "Document"),
            &s,
            "com.apple.TextEdit",
        ));
        confirmed(decision(
            &click(),
            &field("AXButton", "Send"),
            &s,
            "com.apple.TextEdit",
        ));
        s.allowed_apps[0].access = Access::ReadOnly;
        denied(decision(
            &typing(true),
            &field("AXTextArea", "Document"),
            &s,
            "com.apple.TextEdit",
        ));
    }
    fn denied(d: Decision) {
        assert!(
            matches!(d, Decision::Deny(ref reason) if reason.contains("read-only")),
            "{d:?}"
        );
    }

    #[test]
    fn typing_modes_roles_secure_fields_and_enter() {
        let mut s = settings();
        for role in ["AXTextField", "AXTextArea", "AXComboBox"] {
            let target = field(role, "Document");
            assert_eq!(
                decision(&typing(false), &target, &s, "com.apple.TextEdit"),
                Decision::Allow
            );
            confirmed(decision(&typing(true), &target, &s, "com.apple.TextEdit"));
            for text in ["hello\nworld", "hello\rworld"] {
                let mut action = typing(false);
                if let Action::Type { text: value, .. } = &mut action {
                    *value = text.into();
                }
                confirmed(decision(&action, &target, &s, "com.apple.TextEdit"));
            }
            s.confirm_typing = ConfirmTyping::Always;
            confirmed(decision(&typing(false), &target, &s, "com.apple.TextEdit"));
            s.confirm_typing = ConfirmTyping::Sensitive;
        }
        for (role, subrole) in [
            ("AXSecureTextField", ""),
            ("AXTextField", "AXSecureTextField"),
            ("AXTextField", "AXSecureEntry"),
        ] {
            assert!(secure_field(role, subrole));
            let target = Target {
                secure: secure_field(role, subrole),
                ..field(role, "Document")
            };
            for mode in [ConfirmTyping::Always, ConfirmTyping::Sensitive] {
                s.confirm_typing = mode;
                confirmed(decision(&typing(false), &target, &s, "com.apple.TextEdit"));
            }
        }
        assert!(!secure_field("AXTextField", "AXSearchField"));
        confirmed(decision(
            &typing(false),
            &Target::default(),
            &s,
            "com.apple.TextEdit",
        ));
    }

    #[test]
    fn sensitive_metadata_uses_whole_words_case_insensitively() {
        let s = settings();
        for word in [
            "password", "passcode", "pin", "otp", "card", "cvv", "ssn", "iban", "routing",
        ] {
            for label in [
                word.to_uppercase(),
                format!("Enter ({word}) here"),
                format!("account-{word}"),
            ] {
                confirmed(decision(
                    &typing(false),
                    &field("AXTextField", &label),
                    &s,
                    "com.apple.TextEdit",
                ));
            }
        }
        for label in [
            "spinning",
            "postcard",
            "routingNumber",
            "passwordless",
            "card_number",
            "Document",
        ] {
            assert_eq!(
                decision(
                    &typing(false),
                    &field("AXTextField", label),
                    &s,
                    "com.apple.TextEdit"
                ),
                Decision::Allow,
                "{label}"
            );
        }
    }

    #[test]
    fn keyboard_decision_table_including_aliases_and_held_keys() {
        let mut s = settings();
        let target = field("AXTextArea", "Document");
        for held in [
            &["enter"][..],
            &["return"],
            &["cmd", "delete"],
            &["cmd", "q"],
        ] {
            let mut action = click();
            if let Action::Click { hold_keys, .. } = &mut action {
                *hold_keys = keys(held);
            }
            confirmed(decision(&action, &target, &s, "com.apple.TextEdit"));
            confirmed(decision(
                &Action::Drag {
                    from: [0., 0.],
                    to: [1., 1.],
                    hold_keys: keys(held),
                },
                &target,
                &s,
                "com.apple.TextEdit",
            ));
        }
        let safe: &[&[&str]] = &[
            &["cmd", "n"],
            &["command", "f"],
            &["super", "a"],
            &["cmd", "c"],
            &["cmd", "v"],
            &["cmd", "z"],
            &["cmd", "shift", "z"],
            &["left"],
            &["right"],
            &["up"],
            &["down"],
            &["tab"],
            &["escape"],
        ];
        let risky: &[&[&str]] = &[
            &["cmd", "q"],
            &["cmd", "w"],
            &["cmd", "backspace"],
            &["cmd", "option", "backspace"],
            &["cmd", "shift", "backspace"],
            &["cmd", "option", "esc"],
            &["cmd", "s"],
            &["ctrl", "cmd", "q"],
            &["cmd", "delete"],
            &["cmd", "shift", "delete"],
            &["cmd", "alt", "delete"],
            &["command", "tab"],
            &["enter"],
            &["return"],
            &["shift", "return"],
            &["f5"],
            &["a"],
            &[],
            &["cmd", "n", "q"],
        ];
        for (table, allow) in [(safe, true), (risky, false)] {
            for chord in table {
                let mut actions = vec![Action::Hotkey { keys: keys(chord) }];
                if let Some((press, hold)) = chord.split_last() {
                    actions.push(Action::HoldAndPress {
                        hold: keys(hold),
                        press: keys(&[*press]),
                    });
                }
                for action in actions {
                    s.confirm_typing = ConfirmTyping::Sensitive;
                    let result = decision(&action, &target, &s, "com.apple.TextEdit");
                    if allow {
                        assert_eq!(result, Decision::Allow, "{chord:?}");
                    } else {
                        confirmed(result);
                    }
                    s.confirm_typing = ConfirmTyping::Always;
                    confirmed(decision(&action, &target, &s, "com.apple.TextEdit"));
                }
            }
        }
        s.confirm_typing = ConfirmTyping::Sensitive;
        confirmed(decision(
            &Action::HoldAndPress {
                hold: keys(&["cmd"]),
                press: keys(&["n", "q"]),
            },
            &target,
            &s,
            "com.apple.TextEdit",
        ));
    }

    #[test]
    fn read_only_role_and_action_table() {
        let mut s = settings();
        s.allowed_apps[0].access = Access::ReadOnly;
        for role in [
            "AXButton",
            "AXRow",
            "AXCell",
            "AXOutlineRow",
            "AXLink",
            "AXTab",
            "AXMenuItem",
            "AXDisclosureTriangle",
            "AXStaticText",
        ] {
            assert_eq!(
                decision(
                    &click(),
                    &field(
                        role,
                        if role == "AXMenuItem" {
                            "View > General"
                        } else {
                            "General"
                        }
                    ),
                    &s,
                    "com.apple.TextEdit"
                ),
                Decision::Allow,
                "{role}"
            );
            for word in [
                "submit",
                "send",
                "buy",
                "purchase",
                "pay",
                "checkout",
                "confirm",
                "order",
                "delete",
                "transfer",
                "remove",
                "erase",
                "unsubscribe",
                "publish",
                "post",
            ] {
                denied(decision(
                    &click(),
                    &field(role, word),
                    &s,
                    "com.apple.TextEdit",
                ));
            }
        }
        for role in [
            "AXCheckBox",
            "AXSwitch",
            "AXRadioButton",
            "AXSlider",
            "AXIncrementor",
            "AXPopUpButton",
            "AXComboBox",
            "AXTextField",
            "AXTextArea",
            "AXGroup",
            "",
        ] {
            denied(decision(
                &click(),
                &field(role, "General"),
                &s,
                "com.apple.TextEdit",
            ));
        }
        let target = field("AXTextArea", "Document");
        for action in [
            typing(false),
            typing(true),
            Action::Type {
                text: "replace".into(),
                x: None,
                y: None,
                overwrite: true,
                enter: false,
            },
            Action::Hotkey {
                keys: keys(&["cmd", "c"]),
            },
            Action::HoldAndPress {
                hold: keys(&["cmd"]),
                press: keys(&["n"]),
            },
            Action::Drag {
                from: [0., 0.],
                to: [1., 1.],
                hold_keys: vec![],
            },
        ] {
            denied(decision(&action, &target, &s, "com.apple.TextEdit"));
        }
        for action in [
            Action::Scroll {
                x: 0.,
                y: 0.,
                clicks: 1,
                shift: false,
            },
            Action::Open {
                app_or_file: "com.apple.TextEdit".into(),
            },
            Action::SwitchApp {
                app: "com.apple.TextEdit".into(),
            },
        ] {
            assert_eq!(
                decision(&action, &target, &s, "com.apple.TextEdit"),
                Decision::Allow
            );
        }
    }

    #[test]
    fn system_settings_force_read_only_and_deny_panes_by_title_sidebar_and_destination() {
        let mut s = settings();
        s.allowed_apps[1].access = Access::Full;
        denied(decision(
            &typing(false),
            &field("AXTextField", "Search"),
            &s,
            SYSTEM_SETTINGS,
        ));
        assert_eq!(
            decision(&click(), &field("AXRow", "General"), &s, SYSTEM_SETTINGS),
            Decision::Allow
        );
        for pane in [
            "Privacy & Security",
            "Users & Groups",
            "Passwords",
            "Login Password",
            "Lock Screen",
            "Touch ID & Password",
            "Wallet & Apple Pay",
            "Login Items & Extensions",
            "Sharing",
            "Network",
            "Battery → Options",
        ] {
            for target in [
                Target {
                    window_title: pane.into(),
                    ..field("AXButton", "General")
                },
                Target {
                    selected_rows: pane.into(),
                    ..field("AXButton", "General")
                },
                field("AXRow", pane),
            ] {
                denied(decision(&click(), &target, &s, SYSTEM_SETTINGS));
            }
        }
        let battery = Target {
            selected_rows: "Battery".into(),
            window_title: "System Settings".into(),
            ..field("AXButton", "Options…")
        };
        denied(decision(&click(), &battery, &s, SYSTEM_SETTINGS));
        assert_eq!(denied_pane("Options", "Battery"), Some("Battery → Options"));
        assert_eq!(denied_pane("Battery", "Battery"), None);
        assert_eq!(
            denied_pane("privacy and security", ""),
            Some("Privacy & Security")
        );
        for app in ALWAYS_DENIED {
            s.allowed_apps.push((*app).into());
            assert!(!app_allowed(app, &s, &s.app_ids()));
        }
    }

    #[test]
    fn settings_migrate_strings_and_reject_invalid_modes_or_access() {
        let s: Settings = serde_json::from_value(serde_json::json!({"allowedApps": ["com.apple.TextEdit", {"bundleId": "com.apple.Preview", "access": "read-only"}, {"bundleId": SYSTEM_SETTINGS, "access": "full"}]})).unwrap();
        assert_eq!(s.confirm_typing, ConfirmTyping::Sensitive);
        assert_eq!(s.allowed_apps[0].access, Access::Full);
        assert_eq!(s.allowed_apps[1].access, Access::ReadOnly);
        assert_eq!(s.allowed_apps[2].access, Access::ReadOnly);
        let value = serde_json::to_value(&s).unwrap();
        assert_eq!(
            value["allowedApps"][0],
            serde_json::json!({"bundleId":"com.apple.TextEdit","access":"full"})
        );
        assert_eq!(value["confirmTyping"], "sensitive");
        for invalid in [
            serde_json::json!({"confirmTyping":"never"}),
            serde_json::json!({"allowedApps":[{"bundleId":"app","access":"all"}]}),
            serde_json::json!({"allowedApps":[{"bundleId":"app"}]}),
        ] {
            assert!(serde_json::from_value::<Settings>(invalid).is_err());
        }
    }
}
