// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from cursor-overlay/{lib,motion,bezier,z_order,capture_exclusion}.rs
// and platform-macos/src/cursor/overlay.rs. See NOTICE.
use super::super::panels;
use super::bezier::{CubicBezier, Point};
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};
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
    pub fn new(app: tauri::AppHandle, enabled: bool) -> Self {
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
                    let active = active.clone();
                    let motion = motion.clone();
                    let visible = visible.clone();
                    let _ = app.run_on_main_thread(move || {
                        if !active.load(Ordering::SeqCst) || !visible.load(Ordering::SeqCst) {
                            return;
                        }
                        if let Some(m) = motion.lock().unwrap().as_ref() {
                            let elapsed = m.began.elapsed().as_secs_f64();
                            let point = glide(m.from, m.to, elapsed / 0.16);
                            unsafe {
                                draw(
                                    point,
                                    if m.click && elapsed < 0.5 {
                                        Some(elapsed / 0.5)
                                    } else {
                                        None
                                    },
                                );
                            }
                        }
                    });
                    std::thread::sleep(Duration::from_millis(16));
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

unsafe fn draw(point: [f64; 2], ripple: Option<f64>) {
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
                let raw: *mut AnyObject = msg_send![allocated, initWithFrame:NSRect::new(NSPoint::new(0., 0.), NSSize::new(100., 80.))];
                let Some(view) = Retained::from_raw(raw) else { continue; };
                let content: *mut AnyObject = msg_send![&*window, contentView];
                let _: () = msg_send![content, addSubview:&*view];
                panels.push(Panel { window, view, frame });
            }
        }
        let allocated: *mut AnyObject = msg_send![class!(NSImage), alloc];
        let raw: *mut AnyObject = msg_send![allocated, initWithSize:NSSize::new(100., 80.)];
        let Some(image) = Retained::from_raw(raw) else { return; };
        let _: () = msg_send![&*image, lockFocus];
        let color: *mut AnyObject = msg_send![class!(NSColor), colorWithCalibratedRed:0.54f64, green:0.36f64, blue:0.95f64, alpha:1.0f64];
        let _: () = msg_send![color, set];
        if let Some(t) = ripple {
            let radius = 6. + 18. * t;
            let ring: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPathWithOvalInRect:NSRect::new(NSPoint::new(24. - radius, 52. - radius), NSSize::new(radius * 2., radius * 2.))];
            let _: () = msg_send![ring, setLineWidth:2.0f64];
            let _: () = msg_send![ring, stroke];
        }
        let arrow: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPath];
        for (i, (x, y)) in [(24.,52.),(25.,29.),(31.,35.),(37.,23.),(42.,26.),(36.,38.),(45.,38.)].iter().enumerate() {
            if i == 0 { let _: () = msg_send![arrow, moveToPoint:NSPoint::new(*x,*y)]; }
            else { let _: () = msg_send![arrow, lineToPoint:NSPoint::new(*x,*y)]; }
        }
        let _: () = msg_send![arrow, closePath]; let _: () = msg_send![arrow, fill];
        let white: *mut AnyObject = msg_send![class!(NSColor), whiteColor]; let _: () = msg_send![white, set];
        let _: () = msg_send![arrow, setLineWidth:1.0f64]; let _: () = msg_send![arrow, stroke];
        let _: () = msg_send![color, set];
        let pill: *mut AnyObject = msg_send![class!(NSBezierPath), bezierPathWithRoundedRect:NSRect::new(NSPoint::new(44., 11.), NSSize::new(56., 20.)), xRadius:6.0f64, yRadius:6.0f64];
        let _: () = msg_send![pill, fill];
        let attributes: *mut AnyObject = msg_send![class!(NSDictionary), dictionaryWithObject:white, forKey:&*NSString::from_str("NSColor")];
        let _: () = msg_send![&*NSString::from_str("taucode"), drawAtPoint:NSPoint::new(49., 15.), withAttributes:attributes];
        let _: () = msg_send![&*image, unlockFocus];
        for panel in panels.iter() {
            let _: () = msg_send![&*panel.view, setFrameOrigin:NSPoint::new(cocoa[0] - panel.frame.origin.x - 24., cocoa[1] - panel.frame.origin.y - 52.)];
            let _: () = msg_send![&*panel.view, setImage:&*image];
            // Reassert on each tick without activation, as in upstream z_order.
            let _: () = msg_send![&*panel.window, orderFrontRegardless];
        }
    });
}
