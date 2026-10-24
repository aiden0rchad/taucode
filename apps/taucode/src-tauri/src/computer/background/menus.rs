// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from platform-macos/src/tools/menu_equivalent.rs; see NOTICE.
use super::super::policy::{keycode, Target};
use super::ax_actions::Element;
use std::time::{Duration, Instant};

pub const MAX_MENU_ELEMENTS: usize = 60;

#[derive(Debug, serde::Serialize, PartialEq)]
pub struct Diagnostic {
    pub reason: &'static str,
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Combo {
    character: String,
    virtual_key: u16,
    modifiers: u8,
}
impl Combo {
    fn parse(keys: &[String]) -> Option<Self> {
        let mut modifiers = 8; // AX: Command is implicit; bit 3 means no Command.
        let mut base = None;
        for key in keys {
            match keycode(key)? {
                55 => modifiers &= !8,
                56 => modifiers |= 1,
                58 => modifiers |= 2,
                59 => modifiers |= 4,
                code => {
                    if base.is_some() {
                        return None;
                    }
                    base = Some((key, code));
                }
            }
        }
        let (key, virtual_key) = base?;
        Some(Self {
            character: if key.chars().count() == 1 {
                key.to_uppercase()
            } else {
                String::new()
            },
            virtual_key,
            modifiers,
        })
    }
    fn matches(&self, item: &Metadata) -> bool {
        item.modifiers == Some(self.modifiers)
            && if self.character.is_empty() {
                item.virtual_key == Some(self.virtual_key)
            } else {
                item.character.to_uppercase() == self.character
            }
    }
}
#[derive(Default, Clone)]
pub struct Metadata {
    pub title: String,
    role: String,
    pub enabled: bool,
    character: String,
    modifiers: Option<u8>,
    virtual_key: Option<u16>,
}
impl Metadata {
    fn shortcut(&self) -> Option<Vec<String>> {
        let modifiers = self.modifiers.filter(|m| *m <= 15)?;
        let base = if self.character.is_empty() {
            [
                "return",
                "tab",
                "space",
                "backspace",
                "escape",
                "delete",
                "home",
                "end",
                "pageup",
                "pagedown",
                "left",
                "right",
                "up",
                "down",
                "f1",
                "f2",
                "f3",
                "f4",
                "f5",
                "f6",
                "f7",
                "f8",
                "f9",
                "f10",
                "f11",
                "f12",
            ]
            .into_iter()
            .find(|key| keycode(key) == self.virtual_key)?
            .to_owned()
        } else {
            self.character.to_lowercase()
        };
        let mut keys = vec![];
        if modifiers & 8 == 0 {
            keys.push("command".into());
        }
        for (bit, name) in [(1, "shift"), (2, "option"), (4, "control")] {
            if modifiers & bit != 0 {
                keys.push(name.into());
            }
        }
        keys.push(base);
        Some(keys)
    }
}
pub trait Provider {
    type Handle: Clone;
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn read(&self, handle: &Self::Handle) -> Metadata;
    fn children(&self, handle: &Self::Handle, limit: usize) -> Vec<Self::Handle>;
}
pub struct Item<H> {
    pub handle: H,
    pub path: String,
    pub metadata: Metadata,
}
impl<H> Item<H> {
    pub fn node(&self) -> super::ax_tree::Node {
        super::ax_tree::Node {
            role: "AXMenuItem".into(),
            label: self.path.clone(),
            value: self.metadata.shortcut().unwrap_or_default().join("+"),
            actions: vec!["AXPress".into()],
            enabled: self.metadata.enabled,
            ..Default::default()
        }
    }
    pub fn target(&self) -> Target {
        Target {
            role: "AXMenuItem".into(),
            label: self.path.clone(),
            resolved_hotkey: true,
            ..Default::default()
        }
    }
}
pub struct Inventory<H> {
    pub titles: Vec<String>,
    pub items: Vec<Item<H>>,
    complete: bool,
    available: bool,
}
impl<H> Default for Inventory<H> {
    fn default() -> Self {
        Self {
            titles: vec![],
            items: vec![],
            complete: false,
            available: false,
        }
    }
}
impl<H> Inventory<H> {
    pub fn resolve(&self, keys: &[String]) -> Result<&Item<H>, Diagnostic> {
        let fail = |reason, candidates| Diagnostic { reason, candidates };
        if !self.complete {
            return Err(fail(
                if self.available {
                    "timeout"
                } else {
                    "menubar_unavailable"
                },
                vec![],
            ));
        }
        let combo = Combo::parse(keys).ok_or_else(|| fail("no_match", vec![]))?;
        let matches: Vec<_> = self
            .items
            .iter()
            .filter(|i| combo.matches(&i.metadata))
            .collect();
        let enabled: Vec<_> = matches.iter().filter(|i| i.metadata.enabled).collect();
        match enabled.as_slice() {
            [item] => Ok(item),
            [] if matches.is_empty() => Err(fail("no_match", vec![])),
            [] => Err(fail(
                "disabled",
                matches.iter().map(|i| i.path.clone()).collect(),
            )),
            _ => Err(fail(
                "ambiguous",
                enabled.iter().map(|i| i.path.clone()).collect(),
            )),
        }
    }
    pub fn elements(&self) -> impl Iterator<Item = &Item<H>> {
        self.items
            .iter()
            .filter(|i| i.metadata.enabled)
            .take(MAX_MENU_ELEMENTS)
    }
    pub fn observation(&self) -> serde_json::Value {
        serde_json::json!({"titles":self.titles,"complete":self.complete,"items":self.elements().map(|i| {
            serde_json::json!({"title":i.path.chars().take(256).collect::<String>(),"keys":i.metadata.shortcut()})
        }).collect::<Vec<_>>()})
    }
}
// Read the entire bounded tree before resolving: a first match may be ambiguous.
fn walk<P: Provider>(
    provider: &P,
    root: P::Handle,
    cap: usize,
    budget: Duration,
) -> Inventory<P::Handle> {
    let deadline = provider.now() + budget;
    let mut inventory = Inventory {
        complete: true,
        available: true,
        ..Default::default()
    };
    let mut pending = std::collections::VecDeque::from([(root, Vec::<String>::new(), 0)]);
    let mut visited = 0;
    while let Some((handle, mut path, depth)) = pending.pop_front() {
        if visited >= cap || provider.now() >= deadline || depth > 25 {
            inventory.complete = false;
            break;
        }
        visited += 1;
        let meta = provider.read(&handle);
        if meta.role.is_empty() {
            inventory.complete = false;
        }
        if meta.role == "AXMenuBarItem" {
            // The system Apple menu does not belong to the target's commands.
            if meta.title == "Apple" || meta.title == "" {
                continue;
            }
            inventory.titles.push(meta.title.clone());
        }
        if !meta.title.is_empty() && meta.role != "AXMenu" {
            path.push(meta.title.clone());
        }
        if meta.role == "AXMenuItem" && !meta.title.is_empty() {
            inventory.items.push(Item {
                handle: handle.clone(),
                path: path.join(" > "),
                metadata: meta,
            });
        }
        let remaining = cap.saturating_sub(visited + pending.len());
        let mut children = provider.children(&handle, remaining + 1);
        if depth == 0 {
            // File commands must precede large application/Services submenus.
            children.sort_by_cached_key(|h| match provider.read(h).title.as_str() {
                "File" => 0,
                "View" | "Go" | "Window" | "Help" => 1,
                _ => 2,
            });
        }
        if children.len() > remaining {
            inventory.complete = false;
        }
        pending.extend(
            children
                .into_iter()
                .take(remaining)
                .map(|h| (h, path.clone(), depth + 1)),
        );
    }
    if provider.now() >= deadline {
        inventory.complete = false;
    }
    inventory
}
struct Native;
impl Provider for Native {
    type Handle = Element;
    fn read(&self, e: &Element) -> Metadata {
        let role = e.text("AXRole").unwrap_or_default();
        if role != "AXMenuItem" {
            return Metadata {
                title: e.text("AXTitle").unwrap_or_default(),
                role,
                ..Default::default()
            };
        }
        Metadata {
            title: e.text("AXTitle").unwrap_or_default(),
            role,
            enabled: e.text("AXEnabled").as_deref() == Some("1"),
            character: e.text("AXMenuItemCmdChar").unwrap_or_default(),
            modifiers: e
                .text("AXMenuItemCmdModifiers")
                .and_then(|s| s.parse().ok()),
            virtual_key: e
                .text("AXMenuItemCmdVirtualKey")
                .and_then(|s| s.parse().ok()),
        }
    }
    fn children(&self, e: &Element, limit: usize) -> Vec<Element> {
        e.children("AXChildren", limit)
    }
}
pub fn path(element: &Element) -> Option<String> {
    let mut labels = vec![];
    let mut current = Some(element.clone());
    for _ in 0..26 {
        let e = current?;
        let role = e.text("AXRole")?;
        if role == "AXMenuBar" {
            labels.reverse();
            return Some(labels.join(" > "));
        }
        if matches!(role.as_str(), "AXMenuBarItem" | "AXMenuItem") {
            let title = e.text("AXTitle")?;
            if !title.is_empty() {
                labels.push(title);
            }
        }
        current = e.attr("AXParent");
    }
    None
}
pub fn read(pid: i32) -> Inventory<Element> {
    let Some(app) = Element::app(pid) else {
        return Inventory::default();
    };
    match app.attr_result("AXMenuBar") {
        Ok(bar) => walk(&Native, bar, 2000, Duration::from_secs(1)),
        Err(error) => Inventory {
            available: error.contains("ax_error(-25204)"),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Vec<(Metadata, Vec<usize>)>);
    impl Provider for Fake {
        type Handle = usize;
        fn read(&self, h: &usize) -> Metadata {
            self.0[*h].0.clone()
        }
        fn children(&self, h: &usize, limit: usize) -> Vec<usize> {
            self.0[*h].1.iter().take(limit).copied().collect()
        }
    }
    fn keys(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn large_application_menu_cannot_starve_file_commands() {
        let meta = |role: &str, title: &str| Metadata {
            role: role.into(),
            title: title.into(),
            enabled: true,
            ..Default::default()
        };
        let mut fake = Fake(vec![
            (meta("AXMenuBar", ""), vec![1, 2]),
            (meta("AXMenuBarItem", "Finder"), vec![3]),
            (meta("AXMenuBarItem", "File"), vec![4]),
            (meta("AXMenu", ""), (6..2006).collect()),
            (meta("AXMenu", ""), vec![5]),
            (
                Metadata {
                    character: "N".into(),
                    modifiers: Some(1),
                    ..meta("AXMenuItem", "New Folder")
                },
                vec![],
            ),
        ]);
        fake.0
            .extend((6..2006).map(|_| (meta("AXMenuItem", "Service"), vec![])));
        let bounded = walk(&fake, 0, 100, Duration::from_secs(1));
        assert!(bounded.items.iter().any(|i| i.path == "File > New Folder"));
        assert_eq!(
            bounded
                .resolve(&keys(&["cmd", "shift", "n"]))
                .err()
                .unwrap()
                .reason,
            "timeout"
        );
        let complete = walk(&fake, 0, 3000, Duration::from_secs(1));
        assert_eq!(
            complete
                .resolve(&keys(&["cmd", "shift", "n"]))
                .unwrap()
                .path,
            "File > New Folder"
        );
        assert_eq!(
            walk(&fake, 0, 3000, Duration::ZERO)
                .resolve(&keys(&["cmd", "n"]))
                .err()
                .unwrap()
                .reason,
            "timeout"
        );
    }

    #[test]
    fn enabled_menu_elements_include_shortcutless_items_with_cap_and_snapshot_binding() {
        let inv = Inventory {
            titles: vec!["File".into()],
            complete: true,
            available: true,
            items: (0..100)
                .map(|n| Item {
                    handle: n,
                    path: format!("File > Item {n}"),
                    metadata: Metadata {
                        title: format!("Item {n}"),
                        enabled: n != 0,
                        ..Default::default()
                    },
                })
                .collect(),
        };
        assert_eq!(inv.elements().count(), MAX_MENU_ELEMENTS);
        let mut snapshot = super::super::ax_tree::Snapshot::default();
        snapshot.id = 7;
        snapshot.binding = Some((42, 3));
        for item in inv.elements() {
            snapshot.append_menu(item.node(), item.handle);
        }
        let (node, _) = snapshot.resolve_bound(7, 0, 42, 3, |_| true).unwrap();
        assert_eq!(node.label, "File > Item 1");
        assert_eq!(node.role, "AXMenuItem");
        assert_eq!(node.actions, ["AXPress"]);
        assert!(node.enabled);
        assert_eq!(node.value, "");
        assert!(snapshot.resolve(6, 0).is_err());
    }
    #[test]
    fn diagnostic_reasons_and_shift_shortcut_are_explicit() {
        let mut inv: Inventory<usize> = Inventory::default();
        let chord = keys(&["command", "shift", "n"]);
        assert_eq!(
            inv.resolve(&chord).err().unwrap().reason,
            "menubar_unavailable"
        );
        inv.available = true;
        assert_eq!(inv.resolve(&chord).err().unwrap().reason, "timeout");
        inv.complete = true;
        assert_eq!(inv.resolve(&chord).err().unwrap().reason, "no_match");
        inv.items.push(Item {
            handle: 0,
            path: "File > New Folder".into(),
            metadata: Metadata {
                title: "New Folder".into(),
                character: "N".into(),
                modifiers: Some(1),
                ..Default::default()
            },
        });
        let failure = inv.resolve(&chord).err().unwrap();
        assert_eq!(failure.reason, "disabled");
        assert_eq!(failure.candidates, ["File > New Folder"]);
        inv.items[0].metadata.enabled = true;
        for character in ["N", "n"] {
            inv.items[0].metadata.character = character.into();
            assert_eq!(inv.resolve(&chord).unwrap().path, "File > New Folder");
        }
        inv.items.push(Item {
            handle: 1,
            path: "Other > New Folder".into(),
            metadata: inv.items[0].metadata.clone(),
        });
        let failure = inv.resolve(&chord).err().unwrap();
        assert_eq!(failure.reason, "ambiguous");
        assert_eq!(
            failure.candidates,
            ["File > New Folder", "Other > New Folder"]
        );
    }

    #[test]
    fn fake_tree_matches_char_modifiers_virtual_keys_and_rejects_ambiguity() {
        let item = Metadata {
            title: "New Folder".into(),
            role: "AXMenuItem".into(),
            character: "N".into(),
            modifiers: Some(1),
            enabled: true,
            ..Default::default()
        };
        let mut fake = Fake(vec![
            (
                Metadata {
                    role: "AXMenuBar".into(),
                    ..Default::default()
                },
                vec![1],
            ),
            (
                Metadata {
                    title: "File".into(),
                    role: "AXMenuBarItem".into(),
                    ..Default::default()
                },
                vec![2],
            ),
            (
                Metadata {
                    role: "AXMenu".into(),
                    ..Default::default()
                },
                vec![3, 4],
            ),
            (item.clone(), vec![]),
            (
                Metadata {
                    title: "Enter".into(),
                    character: String::new(),
                    modifiers: Some(8),
                    virtual_key: Some(36),
                    ..item.clone()
                },
                vec![],
            ),
        ]);
        let scan = |f: &Fake| walk(f, 0, 20, Duration::from_secs(1));
        let inv = scan(&fake);
        assert_eq!(inv.observation()["titles"], serde_json::json!(["File"]));
        assert_eq!(
            inv.observation()["items"][0]["keys"],
            serde_json::json!(["command", "shift", "n"])
        );
        assert_eq!(
            inv.resolve(&keys(&["cmd", "shift", "n"])).unwrap().path,
            "File > New Folder"
        );
        for chord in [
            &["cmd", "n"][..],
            &["cmd", "alt", "n"],
            &["ctrl", "shift", "n"],
            &["cmd", "n", "q"],
        ] {
            assert!(inv.resolve(&keys(chord)).is_err());
        }
        assert!(inv.resolve(&keys(&["return"])).is_ok());
        assert!(inv.resolve(&keys(&["shift", "return"])).is_err());
        fake.0[3].0.enabled = false;
        assert!(scan(&fake).resolve(&keys(&["cmd", "shift", "n"])).is_err());
        fake.0[3].0.enabled = true;
        fake.0.push((item, vec![]));
        fake.0[2].1.push(5);
        assert!(scan(&fake).resolve(&keys(&["cmd", "shift", "n"])).is_err());
        assert!(walk(&fake, 0, 4, Duration::from_secs(1))
            .resolve(&keys(&["cmd", "shift", "n"]))
            .is_err());
        assert!(walk(&fake, 0, 20, Duration::ZERO)
            .resolve(&keys(&["return"]))
            .is_err());
    }
}
