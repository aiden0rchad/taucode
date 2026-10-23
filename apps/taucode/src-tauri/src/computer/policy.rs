use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub(crate) use crate::types::ComputerSettings as Settings;

pub fn validate(s: &Settings) -> Result<(), String> {
    if !matches!(s.mode.as_str(), "background" | "foreground") {
        return Err("Invalid computer mode".into());
    }
    if !matches!(s.grounding_mode.as_str(), "pixels" | "ui-tars") {
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
    pub fn validated(&self, display: Display) -> Result<Action, String> {
        let mut value = self.action.clone();
        let obj = value.as_object_mut().ok_or("Expected action object")?;
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
    "com.apple.systempreferences",
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
        && s.allowed_apps.iter().any(|a| a == app)
        && apps.iter().any(|a| a == app)
}
pub fn decide(
    a: &Action,
    description: &str,
    c: &Context,
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
    if matches!(a, Action::Type { .. }) && c.secure {
        return Decision::Confirm("Typing into a secure text field");
    }
    if matches!(a, Action::Click { .. }) && risky_target(description) {
        return Decision::Confirm("Potential submit, send, purchase or destructive click");
    }
    // A target field can change focus, and keyboard shortcuts can submit without a click.
    if matches!(
        a,
        Action::Type { .. } | Action::Hotkey { .. } | Action::HoldAndPress { .. }
    ) {
        return Decision::Confirm("Confirm typing or keyboard action");
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
