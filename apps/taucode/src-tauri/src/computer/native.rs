// Background mouse timing adapted from trycua/cua input/mouse.rs.
// Copyright (c) 2025 Cua AI, Inc. (MIT); see background/NOTICE.
use super::{
    control::{Event, Input},
    policy::{Context, Display},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use core_graphics::{
    display::CGDisplay,
    event::{
        CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField,
        ScrollEventUnit,
    },
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::CGPoint,
};
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSBundle, NSData, NSString, NSURL};
use screencapturekit::{
    cm::{CMSampleBufferExt, SCFrameStatus},
    prelude::*,
};
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
    fn AXUIElementCopyAttributeValue(
        element: *const c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    fn AXUIElementSetMessagingTimeout(element: *const c_void, seconds: f32) -> i32;
    fn AXUIElementCreateSystemWide() -> *const c_void;
    fn AXUIElementCopyElementAtPosition(
        element: *const c_void,
        x: f32,
        y: f32,
        result: *mut *const c_void,
    ) -> i32;
    fn AXUIElementGetPid(element: *const c_void, pid: *mut i32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: *const c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(value: *const c_void);
}
pub(super) struct NativePermissions;
impl super::PermissionRequests for NativePermissions {
    fn request_screen(&self) -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }
    fn request_accessibility(&self) -> Result<bool, String> {
        unsafe {
            // Both entries are system constants and outlive the dictionary.
            let options = CFDictionaryCreate(
                std::ptr::null(),
                &kAXTrustedCheckOptionPrompt,
                &kCFBooleanTrue,
                1,
                std::ptr::null(),
                std::ptr::null(),
            );
            if options.is_null() {
                return Err("Cannot create Accessibility permission options".into());
            }
            let trusted = AXIsProcessTrustedWithOptions(options);
            CFRelease(options);
            Ok(trusted)
        }
    }
}
pub fn permissions() -> serde_json::Value {
    unsafe {
        serde_json::json!({"screen":CGPreflightScreenCaptureAccess(),"accessibility":AXIsProcessTrusted()})
    }
}
pub fn context() -> Result<Context, String> {
    let front = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .ok_or("Cannot identify frontmost app")?;
    let app = front
        .bundleIdentifier()
        .ok_or("Frontmost app has no bundle ID")?
        .to_string();
    // Unknown AX focus is treated as secure, never as evidence that typing is safe.
    let secure = unsafe {
        let root = AXUIElementCreateApplication(front.processIdentifier());
        if root.is_null() {
            true
        } else {
            AXUIElementSetMessagingTimeout(root, 0.02);
            let mut focus = std::ptr::null();
            let attr = NSString::from_str("AXFocusedUIElement");
            let status =
                AXUIElementCopyAttributeValue(root, (&*attr as *const NSString).cast(), &mut focus);
            CFRelease(root);
            if status != 0 || focus.is_null() {
                true
            } else {
                AXUIElementSetMessagingTimeout(focus, 0.02);
                let mut role = std::ptr::null();
                let attr = NSString::from_str("AXSubrole");
                let status = AXUIElementCopyAttributeValue(
                    focus,
                    (&*attr as *const NSString).cast(),
                    &mut role,
                );
                CFRelease(focus);
                if status != 0 || role.is_null() {
                    true
                } else {
                    let expected = NSString::from_str("AXSecureTextField");
                    let equal: bool =
                        msg_send![role as *const AnyObject, isEqualToString:&*expected];
                    CFRelease(role);
                    equal
                }
            }
        }
    };
    Ok(Context { app, secure })
}
pub fn target(app_or_file: &str) -> Result<(String, Retained<NSURL>), String> {
    let ws = NSWorkspace::sharedWorkspace();
    if app_or_file.starts_with('/') {
        let path = std::path::Path::new(app_or_file);
        if !path.is_file() {
            return Err("Open requires a regular file or application bundle ID".into());
        }
        let url = NSURL::fileURLWithPath(&NSString::from_str(app_or_file));
        let app_url = ws
            .URLForApplicationToOpenURL(&url)
            .ok_or("No application for file")?;
        let bundle = NSBundle::bundleWithURL(&app_url)
            .and_then(|b| b.bundleIdentifier())
            .ok_or("Cannot identify application for file")?
            .to_string();
        Ok((bundle, url))
    } else {
        let url = ws
            .URLForApplicationWithBundleIdentifier(&NSString::from_str(app_or_file))
            .ok_or("Application bundle ID not installed")?;
        Ok((app_or_file.into(), url))
    }
}
pub fn open_app(app_or_file: &str, expected: &str) -> Result<(), String> {
    let (bundle, url) = target(app_or_file)?;
    if bundle != expected {
        return Err("File association changed after approval".into());
    }
    if NSWorkspace::sharedWorkspace().openURL(&url) {
        Ok(())
    } else {
        Err("Cannot open application or file".into())
    }
}
pub fn event_context(event: &Event) -> Result<Context, String> {
    let c = context()?;
    let point = match event {
        Event::Mouse { point, .. } | Event::Move(point, _) => Some(point),
        _ => None,
    };
    if let Some(p) = point {
        unsafe {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return Err("Cannot inspect target application".into());
            }
            AXUIElementSetMessagingTimeout(system, 0.02);
            let mut element = std::ptr::null();
            let result =
                AXUIElementCopyElementAtPosition(system, p[0] as f32, p[1] as f32, &mut element);
            CFRelease(system);
            if result != 0 || element.is_null() {
                return Err("Cannot identify target application".into());
            }
            let mut pid = 0;
            let result = AXUIElementGetPid(element, &mut pid);
            CFRelease(element);
            let front = NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .ok_or("No frontmost application")?;
            if result != 0
                || pid != front.processIdentifier()
                || front
                    .bundleIdentifier()
                    .is_none_or(|a| a.to_string() != c.app)
            {
                return Err(
                    "Pointer targets another application; switch apps explicitly first".into(),
                );
            }
        }
    }
    Ok(c)
}
pub struct Frame {
    pub png: String,
    pub thumbnail: String,
    pub display: Display,
}
pub fn capture(allowed: &[String]) -> Result<Frame, String> {
    if permissions()["screen"] != true {
        return Err("Grant Screen Recording in System Settings".into());
    }
    let content = SCShareableContent::get().map_err(|e| e.to_string())?;
    let main = CGDisplay::main();
    let display = content
        .displays()
        .into_iter()
        .find(|d| d.display_id() == main.id)
        .ok_or("Main display unavailable")?;
    // Explicit window exclusions include HUD, approval UI, and all windows owned by taucode.
    let windows: Vec<_> = content
        .windows()
        .into_iter()
        .filter(|w| {
            w.owning_application().is_some_and(|a| {
                a.process_id() == std::process::id() as i32
                    || a.bundle_identifier() == "dev.taucode.desktop"
                    || !allowed.contains(&a.bundle_identifier())
            })
        })
        .collect();
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&windows.iter().collect::<Vec<_>>())
        .build()
        .map_err(|e| e.to_string())?;
    let width = main.pixels_wide() as u32;
    let height = main.pixels_high() as u32;
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_shows_cursor(false);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let mut stream = SCStream::new(&filter, &config).map_err(|e| e.to_string())?;
    stream
        .add_output_handler(
            move |sample: CMSampleBuffer, kind: SCStreamOutputType| {
                if kind == SCStreamOutputType::Screen
                    && sample.frame_status() == Some(SCFrameStatus::Complete)
                {
                    if let Ok(image) = sample.cg_image() {
                        let result = unsafe {
                            encode_png(image.as_ptr().cast()).and_then(|png| {
                                thumbnail(image.as_ptr().cast(), width, height)
                                    .map(|thumb| (png, thumb))
                            })
                        };
                        let _ = tx.try_send(result);
                    }
                }
            },
            SCStreamOutputType::Screen,
        )
        .map_err(|e| e.to_string())?;
    stream.start_capture().map_err(|e| e.to_string())?;
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "Screen capture timed out".to_string());
    let stopped = stream.stop_capture().map_err(|e| e.to_string());
    let (png, thumbnail) = result??;
    stopped?;
    Ok(Frame {
        png: STANDARD.encode(png),
        thumbnail: STANDARD.encode(thumbnail),
        display: Display {
            origin: [0., 0.],
            width: main.bounds().size.width,
            height: main.bounds().size.height,
            pixel_width: width,
            pixel_height: height,
        },
    })
}
pub(super) unsafe fn encode_png(image: *const c_void) -> Result<Vec<u8>, String> {
    let allocated: *mut AnyObject = msg_send![class!(NSBitmapImageRep), alloc];
    let bitmap: *mut AnyObject = msg_send![allocated, initWithCGImage:image];
    let bitmap = Retained::from_raw(bitmap).ok_or("Cannot create PNG encoder")?;
    let properties: Retained<AnyObject> = msg_send![class!(NSDictionary), new];
    let data: Option<Retained<NSData>> =
        msg_send![&*bitmap, representationUsingType:4usize, properties:&*properties];
    data.map(|d| d.to_vec())
        .ok_or("Cannot encode screenshot PNG".into())
}
#[derive(Default)]
pub struct NativeInput {
    pub session: Option<std::sync::Arc<super::background::Session>>,
    flags: CGEventFlags,
    point: [f64; 2],
    group: i64,
}
impl NativeInput {
    pub fn background(session: std::sync::Arc<super::background::Session>) -> Self {
        Self {
            session: Some(session),
            ..Default::default()
        }
    }
}
impl Input for NativeInput {
    fn stop(&mut self) {
        if let Some(session) = &self.session {
            session.stop();
        }
    }

    fn post(&mut self, event: &Event) -> Result<(), String> {
        let background = self
            .session
            .as_ref()
            .filter(|s| !s.foreground.load(std::sync::atomic::Ordering::SeqCst));
        if let Some(session) = &self.session {
            session.check()?;
        }
        if let Some(session) = background {
            session.guard.arm();
            match event {
                Event::Mouse { point, .. } | Event::Move(point, _) => {
                    self.point = *point;
                    session
                        .overlay
                        .move_to(*point, matches!(event, Event::Mouse { down: true, .. }));
                }
                _ => (),
            }
        }
        let source = CGEventSource::new(if background.is_some() {
            CGEventSourceStateID::Private
        } else {
            CGEventSourceStateID::HIDSystemState
        })
        .map_err(|_| "Cannot create event source")?;
        let e = match event {
            Event::Key(code, down) => {
                let flag = match code {
                    55 => CGEventFlags::CGEventFlagCommand,
                    56 => CGEventFlags::CGEventFlagShift,
                    58 => CGEventFlags::CGEventFlagAlternate,
                    59 => CGEventFlags::CGEventFlagControl,
                    _ => CGEventFlags::empty(),
                };
                if *down {
                    self.flags.insert(flag);
                } else {
                    self.flags.remove(flag);
                }
                CGEvent::new_keyboard_event(source, *code, *down)
            }
            Event::Text(text) => {
                let e = CGEvent::new_keyboard_event(source, 0, true)
                    .map_err(|_| "Cannot create text event")?;
                e.set_string(text);
                Ok(e)
            }
            Event::Mouse {
                point,
                button,
                down,
                clicks,
            } => {
                let (button, kind) = match (*button, *down) {
                    (0, true) => (CGMouseButton::Left, CGEventType::LeftMouseDown),
                    (0, false) => (CGMouseButton::Left, CGEventType::LeftMouseUp),
                    (1, true) => (CGMouseButton::Right, CGEventType::RightMouseDown),
                    (1, false) => (CGMouseButton::Right, CGEventType::RightMouseUp),
                    (_, true) => (CGMouseButton::Center, CGEventType::OtherMouseDown),
                    (_, false) => (CGMouseButton::Center, CGEventType::OtherMouseUp),
                };
                let e = CGEvent::new_mouse_event(
                    source,
                    kind,
                    CGPoint::new(point[0], point[1]),
                    button,
                )
                .map_err(|_| "Cannot create mouse event")?;
                e.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, *clicks as i64);
                Ok(e)
            }
            Event::Move(p, drag) => CGEvent::new_mouse_event(
                source,
                if *drag {
                    CGEventType::LeftMouseDragged
                } else {
                    CGEventType::MouseMoved
                },
                CGPoint::new(p[0], p[1]),
                CGMouseButton::Left,
            ),
            Event::Scroll(n, horizontal) => CGEvent::new_scroll_event(
                source,
                ScrollEventUnit::LINE,
                2,
                if *horizontal { 0 } else { *n },
                if *horizontal { *n } else { 0 },
                0,
            ),
        }
        .map_err(|_| "Cannot create input event")?;
        e.set_flags(self.flags);
        if let Some(session) = background {
            let window = session
                .window
                .lock()
                .unwrap()
                .clone()
                .ok_or("Missing target window")?;
            let keyboard = matches!(event, Event::Key(..) | Event::Text(_));
            if matches!(event, Event::Mouse { down: true, .. }) {
                self.group = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as i64;
                // Copyright (c) 2025 Cua AI, Inc. MIT. Port of mouse.rs's
                // post_mouse_moved_primer and click tracking intervals (background/NOTICE).
                let source = CGEventSource::new(CGEventSourceStateID::Private)
                    .map_err(|_| "Cannot create mouse primer source")?;
                let primer = CGEvent::new_mouse_event(
                    source,
                    CGEventType::MouseMoved,
                    CGPoint::new(self.point[0], self.point[1]),
                    CGMouseButton::Left,
                )
                .map_err(|_| "Cannot create mouse primer")?;
                primer.set_flags(self.flags);
                primer.set_integer_value_field(7, 3);
                super::background::post_event(
                    session, &primer, false, self.point, window.id, self.group,
                );
                std::thread::sleep(std::time::Duration::from_millis(12));
            }
            if matches!(event, Event::Mouse { .. })
                && !session.dragging.load(std::sync::atomic::Ordering::SeqCst)
            {
                e.set_integer_value_field(7, 3);
            }
            super::background::post_event(session, &e, keyboard, self.point, window.id, self.group);
            if matches!(event, Event::Mouse { down: true, .. }) {
                std::thread::sleep(std::time::Duration::from_millis(28));
            }
            if matches!(event, Event::Text(_)) {
                e.set_type(CGEventType::KeyUp);
                super::background::post_event(session, &e, true, self.point, window.id, self.group);
            }
        } else {
            e.post(CGEventTapLocation::HID);
            if matches!(event, Event::Text(_)) {
                e.set_type(CGEventType::KeyUp);
                e.post(CGEventTapLocation::HID);
            }
        }
        Ok(())
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> *const c_void;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        bytes: usize,
        space: *const c_void,
        info: u32,
    ) -> *const c_void;
    fn CGContextDrawImage(
        context: *const c_void,
        rect: core_graphics::geometry::CGRect,
        image: *const c_void,
    );
    fn CGBitmapContextCreateImage(context: *const c_void) -> *const c_void;
}
pub(super) unsafe fn thumbnail(
    image: *const c_void,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    use core_graphics::geometry::{CGRect, CGSize};
    let h = (320. * height as f64 / width as f64).round().max(1.) as usize;
    let space = CGColorSpaceCreateDeviceRGB();
    if space.is_null() {
        return Err("Cannot create thumbnail color space".into());
    }
    let context = CGBitmapContextCreate(std::ptr::null_mut(), 320, h, 8, 0, space, 1);
    CFRelease(space);
    if context.is_null() {
        return Err("Cannot create thumbnail context".into());
    }
    CGContextDrawImage(
        context,
        CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(320., h as f64)),
        image,
    );
    let thumb = CGBitmapContextCreateImage(context);
    CFRelease(context);
    if thumb.is_null() {
        return Err("Cannot create thumbnail".into());
    }
    let result = encode_png(thumb);
    CFRelease(thumb);
    result
}
