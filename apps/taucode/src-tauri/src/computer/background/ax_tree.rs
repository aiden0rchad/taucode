// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Ported from ax/{tree,snapshot}.rs and tools/get_window_state.rs; see NOTICE.
use super::{ax_actions::Element, Window};
use serde::Serialize;
use std::time::{Duration, Instant};

pub const MAX_ELEMENTS: usize = 250;
pub const SNAPSHOT_BUDGET: Duration = Duration::from_secs(1);

fn secure_field(role: &str, subrole: Option<&str>) -> bool {
    super::super::policy::secure_field(role, subrole.unwrap_or_default())
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

pub fn label(mut read: impl FnMut(&str) -> Option<String>) -> String {
    ["AXTitle", "AXDescription", "AXHelp", "AXIdentifier"]
        .into_iter()
        .find_map(|attribute| read(attribute).filter(|s| !s.trim().is_empty()))
        .unwrap_or_default()
}

pub trait Provider {
    type Handle;
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn read(&self, handle: &Self::Handle) -> Node;
    fn children(&self, handle: &Self::Handle, limit: usize) -> Vec<Self::Handle>;
}

pub struct Snapshot<H> {
    pub id: u64,
    pub elements: Vec<Node>,
    pub status: &'static str,
    pub lookup: serde_json::Value,
    pub attempts: usize,
    pub binding: Option<(i32, u32)>,
    handles: Vec<H>,
}
impl<H> Default for Snapshot<H> {
    fn default() -> Self {
        Self {
            id: 0,
            elements: vec![],
            status: "missing_window",
            lookup: serde_json::Value::Null,
            attempts: 0,
            binding: None,
            handles: vec![],
        }
    }
}
impl<H> Snapshot<H> {
    pub fn resolve_bound(
        &self,
        id: u64,
        index: usize,
        pid: i32,
        window: u32,
        alive: impl FnOnce(&H) -> bool,
    ) -> Result<(&Node, &H), String> {
        let result = self.resolve(id, index)?;
        if self.binding.map(|b| b.0) != Some(pid) {
            return Err("AX element binding check: snapshot pid mismatch".into());
        }
        if self.binding.map(|b| b.1) != Some(window) {
            return Err("AX element binding check: snapshot window mismatch".into());
        }
        if !alive(result.1) {
            return Err("AX element binding check: dead element (AXRole unavailable)".into());
        }
        // Reached from this window's root (or its bound app menu bar) in this
        // snapshot. Hosted views may have no usable AXParent/AXWindow chain.
        Ok(result)
    }
    pub fn append_menu(&mut self, mut node: Node, handle: H) {
        if self.elements.len() >= MAX_ELEMENTS {
            return;
        }
        node.index = self.elements.len();
        // A menu command has no window coordinates; this anchor is only for
        // the grounding protocol. Selected menu items may never fall to pixels.
        node.frame = Some([0., 0., 1., 1.]);
        self.elements.push(node);
        self.handles.push(handle);
    }
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
        self.lookup = serde_json::Value::Null;
        self.binding = None;
        self.elements.clear();
        self.handles.clear();
        self.status = if root.is_none() {
            "missing_window"
        } else {
            "complete"
        };
        let deadline = provider.now() + budget;
        let mut pending: Vec<_> = root.into_iter().map(|h| (h, None, 0)).collect();
        while self.elements.len() < cap && provider.now() < deadline {
            if super::ax_actions::check_walk().is_err() {
                break;
            }
            let Some((handle, parent, depth)) = pending.pop() else {
                break;
            };
            let mut node = provider.read(&handle);
            let index = self.elements.len();
            node.index = index;
            node.parent_index = parent;
            node.depth = depth;
            if depth < 25 && provider.now() < deadline {
                let remaining = cap.saturating_sub(index + 1 + pending.len());
                pending.extend(
                    provider
                        .children(&handle, remaining)
                        .into_iter()
                        .take(remaining)
                        .rev()
                        .map(|h| (h, Some(index), depth + 1)),
                );
            } else {
                self.status = if depth >= 25 {
                    "depth_cap"
                } else {
                    "time_budget"
                };
            }
            self.elements.push(node);
            self.handles.push(handle);
        }
        if self.elements.len() == cap {
            self.status = "element_cap";
        } else if !pending.is_empty() {
            self.status = "time_budget";
        } else if self.status == "complete" && self.elements.iter().any(|n| n.role.is_empty()) {
            self.status = "ax_read_error";
        }
    }
}

pub struct Native<'a>(pub &'a Window);
impl Snapshot<Element> {
    pub fn capture(&mut self, pid: i32, window: &Window) -> Result<(), String> {
        let result = super::ax_actions::retry_walk(
            || {
                let (root, lookup) = super::ax_actions::window_element(pid, window.id);
                self.replace(
                    &Native(window),
                    root,
                    MAX_ELEMENTS - super::menus::MAX_MENU_ELEMENTS,
                    SNAPSHOT_BUDGET,
                );
                self.lookup = lookup;
                super::ax_actions::check_walk()?;
                self.binding = Some((pid, window.id));
                Ok(())
            },
            std::thread::sleep,
        );
        match result {
            Ok(((), attempts)) => {
                self.attempts = attempts;
                Ok(())
            }
            Err(error) => {
                self.elements.clear();
                self.handles.clear();
                self.binding = None;
                self.status = "ax_read_error";
                Err(error)
            }
        }
    }
}
impl Provider for Native<'_> {
    type Handle = Element;
    fn read(&self, e: &Element) -> Node {
        // Snapshot reads run off the input lock.
        e.reading(|| {
            let role = e.text("AXRole").unwrap_or_default();
            let secure = secure_field(&role, e.text("AXSubrole").as_deref());
            // Most nodes have a label before AXHelp/AXIdentifier; avoid extra RPCs.
            let label = label(|attribute| e.text(attribute));
            let actions = e.actions();
            let value_settable = actions.is_empty() && e.settable("AXValue");
            Node {
                label,
                value: if secure {
                    String::new()
                } else {
                    e.text("AXValue")
                        .unwrap_or_default()
                        .chars()
                        .take(512)
                        .collect()
                },
                actions,
                frame: e
                    .frame()
                    .map(|[x, y, w, h]| [x - self.0.origin[0], y - self.0.origin[1], w, h]),
                enabled: e.enabled(),
                value_settable,
                role,
                secure,
                ..Node::default()
            }
        })
    }

    fn children(&self, e: &Element, limit: usize) -> Vec<Element> {
        e.reading(|| e.children("AXChildren", limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculator_reads_survive_latency_and_refresh_after_every_action() {
        struct Calculator {
            buttons: Vec<serde_json::Value>,
            clock: std::cell::Cell<Instant>,
        }
        impl Provider for Calculator {
            type Handle = usize;
            fn now(&self) -> Instant {
                self.clock.get()
            }
            fn read(&self, handle: &usize) -> Node {
                // A responsive app can still need >250 ms for a sequence of AX RPCs.
                self.clock.set(self.now() + Duration::from_millis(60));
                if *handle == 0 {
                    return Node {
                        role: "AXWindow".into(),
                        ..Node::default()
                    };
                }
                let row = &self.buttons[handle - 1];
                Node {
                    role: row["AXRole"].as_str().unwrap().into(),
                    label: label(|name| row[name].as_str().map(str::to_owned)),
                    actions: serde_json::from_value(row["actions"].clone()).unwrap(),
                    frame: serde_json::from_value(row["frame"].clone()).unwrap(),
                    enabled: true,
                    ..Node::default()
                }
            }
            fn children(&self, handle: &usize, limit: usize) -> Vec<usize> {
                if *handle == 0 {
                    (1..=self.buttons.len()).take(limit).collect()
                } else {
                    vec![]
                }
            }
        }
        let provider = Calculator {
            buttons: serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../engines/computer/tests/calculator.json"
            )))
            .unwrap(),
            clock: std::cell::Cell::new(Instant::now()),
        };
        let mut snapshot = Snapshot::default();
        snapshot.replace(&provider, Some(0), MAX_ELEMENTS, Duration::from_millis(250));
        assert_eq!(snapshot.status, "time_budget");
        assert!(!snapshot.elements.iter().any(|n| n.label == "All Clear"));
        for expected in ["All Clear", "1", "2", "Multiply", "7", "Equals"] {
            let old = snapshot.id;
            snapshot.replace(&provider, Some(0), MAX_ELEMENTS, SNAPSHOT_BUDGET);
            assert_eq!(snapshot.status, "complete");
            assert_eq!(snapshot.elements.len(), 7);
            let selected = snapshot
                .elements
                .iter()
                .find(|n| n.label == expected)
                .unwrap();
            assert!(snapshot.resolve(snapshot.id, selected.index).is_ok());
            assert!(snapshot.resolve(old, selected.index).is_err());
        }
    }
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
    fn walked_membership_ignores_missing_parent_links_but_requires_live_binding() {
        let mut snapshot = Snapshot::default();
        snapshot.replace(&Fake(Duration::ZERO), Some(0), 5, Duration::from_secs(1));
        snapshot.binding = Some((42, 7));
        let id = snapshot.id;
        // No ancestor AX lookup is needed: index 4 was reached from window 7.
        assert!(snapshot.resolve_bound(id, 4, 42, 7, |_| true).is_ok());
        for (pid, window, live, reason) in [
            (43, 7, true, "pid mismatch"),
            (42, 8, true, "window mismatch"),
            (42, 7, false, "AXRole unavailable"),
        ] {
            assert!(snapshot
                .resolve_bound(id, 4, pid, window, |_| live)
                .unwrap_err()
                .contains(reason));
        }
        assert!(snapshot
            .resolve_bound(id + 1, 4, 42, 7, |_| true)
            .unwrap_err()
            .contains("Stale AX snapshot"));
        assert!(snapshot
            .resolve_bound(id, 99, 42, 7, |_| true)
            .unwrap_err()
            .contains("Unknown AX element"));
        snapshot.replace(&Fake(Duration::ZERO), Some(0), 5, Duration::from_secs(1));
        snapshot.binding = Some((42, 7));
        assert!(snapshot.resolve_bound(id, 4, 42, 7, |_| true).is_err());
    }
    #[test]
    fn label_precedence() {
        for (values, expected) in [
            (["title", "description", "help", "id"], "title"),
            ([" ", "2", "help", "id"], "2"),
            (["", "", "help", "id"], "help"),
            (["", "", "", "id"], "id"),
        ] {
            let mut reads = 0;
            assert_eq!(
                label(|_| {
                    let value = values[reads];
                    reads += 1;
                    Some(value.into())
                }),
                expected
            );
            assert!(reads <= 4);
        }
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
