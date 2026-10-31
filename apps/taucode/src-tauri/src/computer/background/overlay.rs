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
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const SHARING_TYPE: u64 = 0;
pub const NONACTIVATING_PANEL: u64 = panels::NONACTIVATING;
pub const IGNORE_MOUSE: bool = true;
struct Motion {
    from: [f64; 2],
    to: [f64; 2],
    began: Instant,
    click: bool,
}
pub struct Overlay {
    app: tauri::AppHandle,
    enabled: bool,
    active: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    motion: Arc<Mutex<Option<Motion>>>,
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
        };
        if enabled {
            let active = this.active.clone();
            let motion = this.motion.clone();
            let visible = this.visible.clone();
            let app = this.app.clone();
            std::thread::spawn(move || {
                while active.load(Ordering::SeqCst) {
                    let animating = motion
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|m| m.began.elapsed().as_secs_f64() < 1.8);
                    let active = active.clone();
                    let motion = motion.clone();
                    let visible = visible.clone();
                    let _ = app.run_on_main_thread(move || {
                        if !active.load(Ordering::SeqCst) { return; }
                        let motion = motion.lock().unwrap();
                        let elapsed = motion.as_ref().map(|m| m.began.elapsed().as_secs_f64()).unwrap_or(f64::INFINITY);
                        let point = motion.as_ref().map(|m| glide(m.from, m.to, elapsed / 0.16));
                        let uncovered = elapsed < 1.8 && point.is_some_and(|point| {
                            let ignored = PANELS.with(|panels| panels.borrow().iter().map(|panel| unsafe {
                                let number: isize = msg_send![&*panel.window, windowNumber];
                                number as u32
                            }).collect::<Vec<u32>>());
                            super::windows::point_visible(pid, point, &ignored)
                        });
                        let opacity = cursor_opacity(visible.load(Ordering::SeqCst), uncovered, elapsed);
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
                        if let Some(m) = motion.as_ref() {
                            let point = glide(m.from, m.to, elapsed / 0.16);
                            unsafe {
                                draw(
                                    point,
                                    opacity,
                                    if m.click && elapsed < 0.5 {
                                        Some(elapsed / 0.5)
                                    } else {
                                        None
                                    },
                                );
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
        });
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

unsafe fn draw(point: [f64; 2], opacity: f64, ripple: Option<f64>) {
    let screens: Retained<NSArray<AnyObject>> = msg_send![class!(NSScreen), screens];
    let Some(main) = screens.iter().next() else {
        return;
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
                let _: () = msg_send![&*window, setSharingType:SHARING_TYPE];
                panels::configure(&window);
                let allocated: *mut AnyObject = msg_send![class!(NSImageView), alloc];
                let raw: *mut AnyObject = msg_send![allocated, initWithFrame:NSRect::new(NSPoint::new(0., 0.), NSSize::new(40., 40.))];
                let Some(view) = Retained::from_raw(raw) else { continue; };
                let content: *mut AnyObject = msg_send![&*window, contentView];
                let _: () = msg_send![content, addSubview:&*view];
                panels.push(Panel { window, view, frame });
            }
        }
        let allocated: *mut AnyObject = msg_send![class!(NSImage), alloc];
        let raw: *mut AnyObject = msg_send![allocated, initWithSize:NSSize::new(40., 40.)];
        let Some(image) = Retained::from_raw(raw) else { return; };
        let _: () = msg_send![&*image, lockFocus];
        let color: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.54f64, green:0.36f64, blue:0.95f64, alpha:0.85f64];
        let _: () = msg_send![color, set];
        if let Some(t) = ripple {
            let radius = 4. + 8. * t;
            let ring: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:NSRect::new(NSPoint::new(12. - radius, 28. - radius), NSSize::new(radius * 2., radius * 2.))];
            let _: () = msg_send![ring, setLineWidth:1.0f64];
            let _: () = msg_send![ring, stroke];
        }
        let arrow: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPath];
        for (i, (x, y)) in [(12.,28.),(12.5,16.5),(15.5,19.5),(18.5,13.5),(21.,15.),(18.,21.),(22.5,21.)].iter().enumerate() {
            if i == 0 { let _: () = msg_send![arrow, moveToPoint:NSPoint::new(*x,*y)]; }
            else { let _: () = msg_send![arrow, lineToPoint:NSPoint::new(*x,*y)]; }
        }
        let _: () = msg_send![arrow, closePath]; let _: () = msg_send![arrow, fill];
        let white: *mut AnyObject = msg_send![class!(NSColor), whiteColor]; let _: () = msg_send![white, set];
        let _: () = msg_send![arrow, setLineWidth:1.0f64]; let _: () = msg_send![arrow, stroke];
        let _: () = msg_send![&*image, unlockFocus];
        for panel in panels.iter() {
            let _: () = msg_send![&*panel.view, setFrameOrigin:NSPoint::new(cocoa[0] - panel.frame.origin.x - 12., cocoa[1] - panel.frame.origin.y - 28.)];
            let _: () = msg_send![&*panel.view, setImage:&*image];
            let _: () = msg_send![&*panel.window, setAlphaValue:opacity];
            // The controlled app is uncovered at this point; never activate it.
            let _: () = msg_send![&*panel.window, orderFrontRegardless];
        }
    });
}

// Fade the action hint; never draw over an app the user is working in.
fn cursor_opacity(visible: bool, target_uncovered: bool, elapsed: f64) -> f64 {
    if !visible || !target_uncovered {
        return 0.;
    }
    ((1.8 - elapsed) / 0.4).clamp(0., 1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_is_scoped_to_target_and_fades_after_action() {
        assert_eq!(cursor_opacity(true, true, 0.), 1.);
        assert_eq!(cursor_opacity(true, true, 1.4), 1.);
        assert!((cursor_opacity(true, true, 1.6) - 0.5).abs() < 0.001);
        assert_eq!(cursor_opacity(true, true, 1.8), 0.);
        assert_eq!(cursor_opacity(true, true, f64::INFINITY), 0.);
        assert_eq!(cursor_opacity(true, false, 0.), 0.);
        assert_eq!(cursor_opacity(false, true, 0.), 0.);
    }
}
