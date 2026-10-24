// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from platform-macos/src/tools/menu_equivalent.rs; see NOTICE.
use super::super::policy::{keycode, Target};
use super::ax_actions::Element;
use std::time::{Duration, Instant};

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
}
impl<H> Default for Inventory<H> {
    fn default() -> Self {
        Self {
            titles: vec![],
            items: vec![],
            complete: false,
        }
    }
}
impl<H> Inventory<H> {
    pub fn resolve(&self, keys: &[String]) -> Option<&Item<H>> {
        if !self.complete {
            return None;
        }
        let combo = Combo::parse(keys)?;
        let mut matches = self
            .items
            .iter()
            .filter(|i| i.metadata.enabled && combo.matches(&i.metadata));
        let item = matches.next()?;
        matches.next().is_none().then_some(item)
    }
    pub fn observation(&self) -> serde_json::Value {
        serde_json::json!({"titles":self.titles,"complete":self.complete,"items":self.items.iter().filter(|i| i.metadata.enabled).take(40).map(|i| {
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
        ..Default::default()
    };
    let mut pending = vec![(root, Vec::<String>::new(), 0)];
    let mut visited = 0;
    while let Some((handle, mut path, depth)) = pending.pop() {
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
        if meta.role == "AXMenuItem" && meta.shortcut().is_some() {
            inventory.items.push(Item {
                handle: handle.clone(),
                path: path.join(" > "),
                metadata: meta,
            });
        }
        let remaining = cap.saturating_sub(visited + pending.len());
        let children = provider.children(&handle, remaining + 1);
        if children.len() > remaining {
            inventory.complete = false;
        }
        pending.extend(
            children
                .into_iter()
                .take(remaining)
                .rev()
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
pub fn read(pid: i32) -> Inventory<Element> {
    Element::app(pid)
        .and_then(|app| app.attr("AXMenuBar"))
        .map(|bar| walk(&Native, bar, 2000, Duration::from_secs(1)))
        .unwrap_or_default()
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
            assert!(inv.resolve(&keys(chord)).is_none());
        }
        assert!(inv.resolve(&keys(&["return"])).is_some());
        assert!(inv.resolve(&keys(&["shift", "return"])).is_none());
        fake.0[3].0.enabled = false;
        assert!(scan(&fake).resolve(&keys(&["cmd", "shift", "n"])).is_none());
        fake.0[3].0.enabled = true;
        fake.0.push((item, vec![]));
        fake.0[2].1.push(5);
        assert!(scan(&fake).resolve(&keys(&["cmd", "shift", "n"])).is_none());
        assert!(walk(&fake, 0, 4, Duration::from_secs(1))
            .resolve(&keys(&["cmd", "shift", "n"]))
            .is_none());
        assert!(walk(&fake, 0, 20, Duration::ZERO)
            .resolve(&keys(&["return"]))
            .is_none());
    }
}
