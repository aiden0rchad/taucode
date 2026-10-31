// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from cursor-overlay/{lib,motion,bezier,z_order,capture_exclusion}.rs
// and platform-macos/src/cursor/overlay.rs. See NOTICE.
use super::super::panels;
use super::bezier::{CubicBezier, Point};
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const SHARING_TYPE: u64 = 0;
pub const NONACTIVATING_PANEL: u64 = panels::NONACTIVATING;
pub const IGNORE_MOUSE: bool = true;
const HINT_SECONDS: f64 = 3.;
const CLICK_SECONDS: f64 = 0.75;
const CANVAS: f64 = 64.;
const TIP: [f64; 2] = [20., 44.];
struct Motion {
    from: [f64; 2],
    to: [f64; 2],
    began: Instant,
    click: bool,
    shown: bool,
}
pub struct Overlay {
    app: tauri::AppHandle,
    enabled: bool,
    active: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    motion: Arc<Mutex<Option<Motion>>>,
    presented: Arc<AtomicUsize>,
}
struct Panel {
    window: Retained<AnyObject>,
    view: Retained<AnyObject>,
    frame: NSRect,
}
thread_local! { static PANELS: RefCell<Vec<Panel>> = const { RefCell::new(Vec::new()) }; }

pub fn glide(from: [f64; 2], to: [f64; 2], t: f64) -> [f64; 2] {
    // The reference's 0.3 start/end handles with zero arc: an eased straight glide.
    let p = |v: [f64; 2]| Point { x: v[0], y: v[1] };
    let curve = CubicBezier {
        start: p(from),
        end: p(to),
        control1: p([
            from[0] + (to[0] - from[0]) * 0.3,
            from[1] + (to[1] - from[1]) * 0.3,
        ]),
        control2: p([
            to[0] - (to[0] - from[0]) * 0.3,
            to[1] - (to[1] - from[1]) * 0.3,
        ]),
    };
    let t = t.clamp(0., 1.);
    let point = curve.point_at(t * t * (3. - 2. * t));
    [point.x, point.y]
}
impl Overlay {
    pub fn new(app: tauri::AppHandle, enabled: bool, pid: i32) -> Self {
        let this = Self {
            app,
            enabled,
            active: Arc::new(AtomicBool::new(true)),
            visible: Arc::new(AtomicBool::new(false)),
            motion: Arc::new(Mutex::new(None)),
            presented: Arc::new(AtomicUsize::new(0)),
        };
        if enabled {
            let active = this.active.clone();
            let motion = this.motion.clone();
            let visible = this.visible.clone();
            let app = this.app.clone();
            let presented = this.presented.clone();
            std::thread::spawn(move || {
                while active.load(Ordering::SeqCst) {
                    let animating = motion
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|m| m.began.elapsed().as_secs_f64() < HINT_SECONDS);
                    let active = active.clone();
                    let motion = motion.clone();
                    let visible = visible.clone();
                    let presented = presented.clone();
                    let _ = app.run_on_main_thread(move || {
                        if !active.load(Ordering::SeqCst) { return; }
                        let mut motion = motion.lock().unwrap();
                        let elapsed = motion.as_ref().map(|m| m.began.elapsed().as_secs_f64()).unwrap_or(f64::INFINITY);
                        let point = motion.as_ref().map(|m| glide(m.from, m.to, elapsed / 0.16));
                        // Hit testing sees through transparent Dock/system windows.
                        let owner = point.filter(|_| elapsed < HINT_SECONDS).and_then(super::ax_actions::visible_owner);
                        let opacity = cursor_opacity(visible.load(Ordering::SeqCst), owner, pid, elapsed);
                        if opacity == 0. {
                            PANELS.with(|panels| {
                                for panel in panels.borrow().iter() {
                                    unsafe {
                                        let shown: bool = msg_send![&*panel.window, isVisible];
                                        if shown { let _: () = msg_send![&*panel.window, orderOut: std::ptr::null::<AnyObject>()]; }
                                    }
                                }
                            });
                            return;
                        }
                        if let Some(m) = motion.as_mut() {
                            let point = glide(m.from, m.to, elapsed / 0.16);
                            unsafe {
                                let shown = draw(
                                    point,
                                    opacity,
                                    if m.click && elapsed < CLICK_SECONDS {
                                        Some(elapsed / CLICK_SECONDS)
                                    } else {
                                        None
                                    },
                                );
                                if shown && !m.shown {
                                    m.shown = true;
                                    presented.fetch_add(1, Ordering::SeqCst);
                                }
                            }
                        }
                    });
                    std::thread::sleep(Duration::from_millis(if animating { 16 } else { 100 }));
                }
            });
        }
        this
    }
    pub fn move_to(&self, point: [f64; 2], click: bool) {
        if !self.enabled
            || !self.active.load(Ordering::SeqCst)
            || !self.visible.load(Ordering::SeqCst)
        {
            return;
        }
        let mut motion = self.motion.lock().unwrap();
        if !click && motion.as_ref().is_some_and(|m| m.to == point) {
            return;
        }
        let from = motion
            .as_ref()
            .map(|m| glide(m.from, m.to, m.began.elapsed().as_secs_f64() / 0.16))
            .unwrap_or(point);
        *motion = Some(Motion {
            from,
            to: point,
            began: Instant::now(),
            click,
            shown: false,
        });
    }
    pub fn presented(&self) -> usize {
        self.presented.load(Ordering::SeqCst)
    }
    // Cursor visibility is independent of the HUD and the session lifetime.
    pub fn set_visible(&self, visible: bool) {
        if self.visible.swap(visible, Ordering::SeqCst) == visible || visible {
            return;
        }
        *self.motion.lock().unwrap() = None;
        let visible = self.visible.clone();
        let _ = self.app.run_on_main_thread(move || {
            if !visible.load(Ordering::SeqCst) {
                PANELS.with(|panels| {
                    for panel in panels.borrow().iter() {
                        unsafe {
                            let _: () =
                                msg_send![&*panel.window, orderOut: std::ptr::null::<AnyObject>()];
                        }
                    }
                });
            }
        });
    }
    pub fn hide(&self) {
        if !self.active.swap(false, Ordering::SeqCst) {
            return;
        }
        let _ = self.app.run_on_main_thread(|| {
            PANELS.with(|panels| {
                for panel in panels.borrow_mut().drain(..) {
                    unsafe {
                        let _: () =
                            msg_send![&*panel.window, orderOut: std::ptr::null::<AnyObject>()];
                        let _: () = msg_send![&*panel.window, close];
                    }
                }
            })
        });
    }
}
impl Drop for Overlay {
    fn drop(&mut self) {
        self.hide();
    }
}

unsafe fn draw(point: [f64; 2], opacity: f64, ripple: Option<f64>) -> bool {
    let screens: Retained<NSArray<AnyObject>> = msg_send![class!(NSScreen), screens];
    let Some(main) = screens.iter().next() else {
        return false;
    };
    let main_frame: NSRect = msg_send![&*main, frame];
    let cocoa = [point[0], main_frame.size.height - point[1]];
    PANELS.with(|panels| {
        let mut panels = panels.borrow_mut();
        if panels.is_empty() {
            for screen in screens.iter() {
                let frame: NSRect = msg_send![&*screen, frame];
                let allocated: *mut AnyObject = msg_send![panels::panel_class(), alloc];
                let raw: *mut AnyObject = msg_send![allocated, initWithContentRect:frame, styleMask:NONACTIVATING_PANEL, backing:2u64, defer:false];
                let Some(window) = Retained::from_raw(raw) else { continue; };
                let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
                let _: () = msg_send![&*window, setOpaque:false];
                let _: () = msg_send![&*window, setBackgroundColor:clear];
                let _: () = msg_send![&*window, setHasShadow:false];
                let _: () = msg_send![&*window, setIgnoresMouseEvents:IGNORE_MOUSE];
                let _: () = msg_send![&*window, setAccessibilityElement:false];
                let _: () = msg_send![&*window, setSharingType:SHARING_TYPE];
                panels::configure(&window);
                let allocated: *mut AnyObject = msg_send![class!(NSImageView), alloc];
                let raw: *mut AnyObject = msg_send![allocated, initWithFrame:NSRect::new(NSPoint::new(0., 0.), NSSize::new(CANVAS, CANVAS))];
                let Some(view) = Retained::from_raw(raw) else { continue; };
                let _: () = msg_send![&*view, setAccessibilityElement:false];
                let content: *mut AnyObject = msg_send![&*window, contentView];
                let _: () = msg_send![content, addSubview:&*view];
                panels.push(Panel { window, view, frame });
            }
        }
        let allocated: *mut AnyObject = msg_send![class!(NSImage), alloc];
        let raw: *mut AnyObject = msg_send![allocated, initWithSize:NSSize::new(CANVAS, CANVAS)];
        let Some(image) = Retained::from_raw(raw) else { return false; };
        let _: () = msg_send![&*image, lockFocus];
        if let Some(t) = ripple {
            let radius = 7. + 11. * (1. - (1. - t).powi(3));
            let ring: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:NSRect::new(NSPoint::new(TIP[0] - radius, TIP[1] - radius), NSSize::new(radius * 2., radius * 2.))];
            let tint: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.66f64, green:0.57f64, blue:1.0f64, alpha:0.12 * (1. - t)];
            let _: () = msg_send![tint, set];
            let _: () = msg_send![ring, fill];
            let rim: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.78f64, green:0.72f64, blue:1.0f64, alpha:0.8 * (1. - t)];
            let _: () = msg_send![rim, set];
            let _: () = msg_send![ring, setLineWidth:2.0f64];
            let _: () = msg_send![ring, stroke];
        }
        let arrow: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPath];
        let _: () = msg_send![arrow, moveToPoint:NSPoint::new(22.8,43.)];
        let _: () = msg_send![arrow, curveToPoint:NSPoint::new(20.,39.6), controlPoint1:NSPoint::new(20.2,44.), controlPoint2:NSPoint::new(19.1,42.3)];
        let _: () = msg_send![arrow, lineToPoint:NSPoint::new(26.2,20.1)];
        let _: () = msg_send![arrow, curveToPoint:NSPoint::new(30.6,19.8), controlPoint1:NSPoint::new(27.,17.4), controlPoint2:NSPoint::new(29.5,17.2)];
        let _: () = msg_send![arrow, lineToPoint:NSPoint::new(34.5,29.4)];
        let _: () = msg_send![arrow, lineToPoint:NSPoint::new(43.5,33.6)];
        let _: () = msg_send![arrow, curveToPoint:NSPoint::new(43.2,38.3), controlPoint1:NSPoint::new(46.,34.7), controlPoint2:NSPoint::new(45.7,37.4)];
        let _: () = msg_send![arrow, closePath];
        let _: () = msg_send![arrow, setLineJoinStyle:1u64];
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let glow: Retained<AnyObject> = msg_send![class!(NSShadow), new];
        let lavender: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.70f64, green:0.52f64, blue:1.0f64, alpha:0.85f64];
        let _: () = msg_send![&*glow, setShadowColor:lavender];
        let _: () = msg_send![&*glow, setShadowOffset:NSSize::new(0.,0.)];
        let _: () = msg_send![&*glow, setShadowBlurRadius:4.0f64];
        let _: () = msg_send![&*glow, set];
        let _: () = msg_send![lavender, set];
        let _: () = msg_send![arrow, setLineWidth:1.8f64]; let _: () = msg_send![arrow, stroke];
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        let fill: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.12f64, green:0.68f64, blue:1.0f64, alpha:0.42f64];
        let _: () = msg_send![fill, set]; let _: () = msg_send![arrow, fill];
        let edge: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.82f64, green:0.93f64, blue:1.0f64, alpha:0.9f64]; let _: () = msg_send![edge, set];
        let _: () = msg_send![arrow, setLineWidth:1.0f64]; let _: () = msg_send![arrow, stroke];
        let _: () = msg_send![&*image, unlockFocus];
        let mut shown = false;
        for panel in panels.iter() {
            let _: () = msg_send![&*panel.view, setFrameOrigin:NSPoint::new(cocoa[0] - panel.frame.origin.x - TIP[0], cocoa[1] - panel.frame.origin.y - TIP[1])];
            let _: () = msg_send![&*panel.view, setImage:&*image];
            let _: () = msg_send![&*panel.window, setAlphaValue:opacity];
            // The controlled app is uncovered at this point; never activate it.
            let _: () = msg_send![&*panel.window, orderFrontRegardless];
            let visible: bool = msg_send![&*panel.window, isVisible];
            shown |= visible;
        }
        shown
    })
}

// Fade the action hint; never draw over an app the user is working in.
fn cursor_opacity(visible: bool, hit_owner: Option<i32>, target_pid: i32, elapsed: f64) -> f64 {
    if !visible || hit_owner != Some(target_pid) {
        return 0.;
    }
    ((HINT_SECONDS - elapsed) / 0.5).clamp(0., 1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_is_scoped_to_target_and_fades_after_action() {
        assert_eq!(cursor_opacity(true, Some(42), 42, 0.), 1.);
        assert_eq!(cursor_opacity(true, Some(42), 42, 2.5), 1.);
        assert_eq!(cursor_opacity(true, Some(42), 42, 2.75), 0.5);
        assert_eq!(cursor_opacity(true, Some(42), 42, 3.), 0.);
        assert_eq!(cursor_opacity(true, Some(42), 42, f64::INFINITY), 0.);
        assert_eq!(cursor_opacity(true, Some(7), 42, 0.), 0.);
        assert_eq!(cursor_opacity(true, None, 42, 0.), 0.);
        assert_eq!(cursor_opacity(false, Some(42), 42, 0.), 0.);
    }
}
