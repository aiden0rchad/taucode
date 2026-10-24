// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Ported from ax/{tree,snapshot}.rs and tools/get_window_state.rs; see NOTICE.
use super::{ax_actions::Element, Window};
use serde::Serialize;
use std::time::{Duration, Instant};

pub const MAX_ELEMENTS: usize = 250;

fn secure_field(role: &str, subrole: Option<&str>) -> bool {
    role == "AXSecureTextField" || subrole == Some("AXSecureTextField")
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Node {
    pub index: usize,
    pub role: String,
    pub label: String,
    pub value: String,
    pub actions: Vec<String>,
    pub frame: Option<[f64; 4]>,
    pub parent_index: Option<usize>,
    pub depth: usize,
    pub secure: bool,
    pub enabled: bool,
    pub value_settable: bool,
}

pub fn label(title: Option<String>, description: Option<String>, help: Option<String>) -> String {
    [title, description, help]
        .into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .unwrap_or_default()
}

pub trait Provider {
    type Handle;
    fn read(&self, handle: &Self::Handle) -> Node;
    fn children(&self, handle: &Self::Handle, limit: usize) -> Vec<Self::Handle>;
}

pub struct Snapshot<H> {
    pub id: u64,
    pub elements: Vec<Node>,
    handles: Vec<H>,
}
impl<H> Default for Snapshot<H> {
    fn default() -> Self {
        Self {
            id: 0,
            elements: vec![],
            handles: vec![],
        }
    }
}
impl<H> Snapshot<H> {
    // tools/scroll.rs: native AXTextArea's scroll-area buttons, forward then reverse.
    pub fn scroll_button(&self, index: usize, reverse: bool) -> Option<usize> {
        let node = self.elements.get(index)?;
        if node.role != "AXTextArea" {
            return None;
        }
        let parent = node.parent_index?;
        let area = self.elements.get(parent)?;
        if area.role != "AXScrollArea" {
            return None;
        }
        self.elements
            .iter()
            .skip(parent + 1)
            .take_while(|n| n.depth > area.depth)
            .filter(|n| {
                n.depth <= area.depth + 4
                    && n.role == "AXButton"
                    && n.actions.iter().any(|a| a == "AXPress")
            })
            .take(4)
            .nth(usize::from(reverse))
            .map(|n| n.index)
    }
    pub fn resolve(&self, id: u64, index: usize) -> Result<(&Node, &H), String> {
        if id != self.id || id == 0 {
            return Err("Stale AX snapshot; capture again".into());
        }
        self.elements
            .get(index)
            .zip(self.handles.get(index))
            .ok_or("Unknown AX element".into())
    }
    pub fn replace(
        &mut self,
        provider: &impl Provider<Handle = H>,
        root: Option<H>,
        cap: usize,
        budget: Duration,
    ) {
        self.id = self.id.checked_add(1).expect("AX snapshot id exhausted");
        self.elements.clear();
        self.handles.clear();
        let deadline = Instant::now() + budget;
        let mut pending: Vec<_> = root.into_iter().map(|h| (h, None, 0)).collect();
        while self.elements.len() < cap && Instant::now() < deadline {
            let Some((handle, parent, depth)) = pending.pop() else {
                break;
            };
            let mut node = provider.read(&handle);
            let index = self.elements.len();
            node.index = index;
            node.parent_index = parent;
            node.depth = depth;
            if depth < 25 && Instant::now() < deadline {
                let remaining = cap.saturating_sub(index + 1 + pending.len());
                pending.extend(
                    provider
                        .children(&handle, remaining)
                        .into_iter()
                        .take(remaining)
                        .rev()
                        .map(|h| (h, Some(index), depth + 1)),
                );
            }
            self.elements.push(node);
            self.handles.push(handle);
        }
    }
}

pub struct Native<'a>(pub &'a Window);
impl Provider for Native<'_> {
    type Handle = Element;
    fn read(&self, e: &Element) -> Node {
        let role = e.text("AXRole").unwrap_or_default();
        let secure = secure_field(&role, e.text("AXSubrole").as_deref());
        Node {
            label: label(e.text("AXTitle"), e.text("AXDescription"), e.text("AXHelp")),
            value: if secure {
                String::new()
            } else {
                e.text("AXValue")
                    .unwrap_or_default()
                    .chars()
                    .take(512)
                    .collect()
            },
            actions: e.actions(),
            frame: e
                .frame()
                .map(|[x, y, w, h]| [x - self.0.origin[0], y - self.0.origin[1], w, h]),
            enabled: e.enabled(),
            value_settable: e.settable("AXValue"),
            role,
            secure,
            ..Node::default()
        }
    }
    fn children(&self, e: &Element, limit: usize) -> Vec<Element> {
        e.children("AXChildren", limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Duration);
    impl Provider for Fake {
        type Handle = usize;
        fn read(&self, _: &usize) -> Node {
            std::thread::sleep(self.0);
            Node::default()
        }
        fn children(&self, h: &usize, limit: usize) -> Vec<usize> {
            (h + 1..h + 4).take(limit).collect()
        }
    }
    #[test]
    fn bounded_walk_and_stale_handles() {
        let mut s = Snapshot::default();
        s.replace(&Fake(Duration::ZERO), Some(0), 5, Duration::from_secs(1));
        assert_eq!(s.elements.len(), 5);
        assert_eq!(s.elements[1].parent_index, Some(0));
        assert_eq!(s.elements[1].depth, 1);
        let old = s.id;
        assert!(s.resolve(old, 4).is_ok());
        assert!(s.resolve(old, 5).is_err());
        s.replace(
            &Fake(Duration::from_millis(10)),
            Some(0),
            100,
            Duration::from_millis(1),
        );
        assert_eq!(s.elements.len(), 1);
        assert!(s.resolve(old, 0).is_err());
        s.replace(&Fake(Duration::ZERO), None, 100, Duration::ZERO);
        assert!(s.elements.is_empty());
    }
    #[test]
    fn label_precedence() {
        assert_eq!(
            label(
                Some("title".into()),
                Some("description".into()),
                Some("help".into())
            ),
            "title"
        );
        assert_eq!(
            label(Some(" ".into()), Some("2".into()), Some("help".into())),
            "2"
        );
        assert_eq!(label(None, Some("".into()), Some("help".into())), "help");
    }

    #[test]
    fn secure_roles_and_native_scroll_buttons() {
        assert!(secure_field("AXSecureTextField", None));
        assert!(secure_field("AXTextField", Some("AXSecureTextField")));
        assert!(!secure_field("AXTextField", Some("AXSearchField")));
        let mut s: Snapshot<usize> = Snapshot::default();
        s.elements = vec![
            Node {
                index: 0,
                role: "AXScrollArea".into(),
                ..Node::default()
            },
            Node {
                index: 1,
                role: "AXTextArea".into(),
                depth: 1,
                parent_index: Some(0),
                ..Node::default()
            },
            Node {
                index: 2,
                role: "AXButton".into(),
                depth: 1,
                parent_index: Some(0),
                actions: vec!["AXPress".into()],
                ..Node::default()
            },
            Node {
                index: 3,
                role: "AXButton".into(),
                depth: 1,
                parent_index: Some(0),
                actions: vec!["AXPress".into()],
                ..Node::default()
            },
        ];
        assert_eq!(s.scroll_button(1, false), Some(2));
        assert_eq!(s.scroll_button(1, true), Some(3));
        assert_eq!(s.scroll_button(0, false), None);
    }
}
