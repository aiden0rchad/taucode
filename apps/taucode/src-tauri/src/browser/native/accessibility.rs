//! An owned AX container gates WebKit without changing its class or viewport.
use objc2::{define_class, msg_send, rc::Retained, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityFrameInView, NSAccessibilityGroupRole,
    NSAccessibilityLayoutChangedNotification, NSAccessibilityPostNotification,
    NSAutoresizingMaskOptions, NSView, NSWindowOrderingMode,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSObjectProtocol, NSPoint, NSRect, NSString, NSValue,
};
use objc2_web_kit::WKWebView;
use std::cell::Cell;

struct State {
    parked: Cell<bool>,
    flipped: bool,
}

fn child_attribute(attribute: &NSString) -> bool {
    matches!(
        attribute.to_string().as_str(),
        "AXChildren"
            | "AXVisibleChildren"
            | "AXContents"
            | "AXSelectedChildren"
            | "AXChildrenInNavigationOrder"
    )
}

fn empty_children() -> *mut objc2::runtime::AnyObject {
    Retained::autorelease_return(NSArray::<objc2::runtime::AnyObject>::new()).cast()
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = State]
    struct BrowserAXContainer;
    unsafe impl NSObjectProtocol for BrowserAXContainer {}
    impl BrowserAXContainer {
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool { self.ivars().flipped }

        #[unsafe(method(accessibilityIsIgnored))]
        fn ignored(&self) -> bool { false }

        #[unsafe(method(accessibilityFrame))]
        fn ax_frame(&self) -> NSRect {
            // The physical container fills its parent for Wry's coordinates;
            // its logical AX group represents only the page, never main chrome.
            if self.ivars().parked.get() { return NSRect::default(); }
            self.subviews().firstObject().map_or_else(NSRect::default, |child| NSAccessibilityFrameInView(&child, child.bounds()))
        }

        // AppKit's legacy AX methods can enumerate physical subviews despite the
        // modern children setter. Gate those paths on this owned container too.
        #[unsafe(method(accessibilityAttributeValue:))]
        fn attribute(&self, attribute: &NSString) -> *mut objc2::runtime::AnyObject {
            if attribute.to_string() == "AXPosition" { unsafe { Retained::autorelease_return(NSValue::valueWithPoint(self.accessibilityFrame().origin)).cast() } }
            else if attribute.to_string() == "AXSize" { unsafe { Retained::autorelease_return(NSValue::valueWithSize(self.accessibilityFrame().size)).cast() } }
            else if self.ivars().parked.get() && child_attribute(attribute) { empty_children() }
            else if self.ivars().parked.get() && attribute.to_string() == "AXFocusedUIElement" { std::ptr::null_mut() }
            else { unsafe { msg_send![super(self), accessibilityAttributeValue: attribute] } }
        }

        #[unsafe(method(accessibilityAttributeValue:forParameter:))]
        fn parameterized_attribute(&self, attribute: &NSString, parameter: *mut objc2::runtime::AnyObject) -> *mut objc2::runtime::AnyObject {
            if self.ivars().parked.get() && child_attribute(attribute) { empty_children() }
            else if self.ivars().parked.get() && attribute.to_string() == "AXFocusedUIElement" { std::ptr::null_mut() }
            else { unsafe { msg_send![super(self), accessibilityAttributeValue: attribute, forParameter: parameter] } }
        }

        #[unsafe(method(accessibilityArrayAttributeCount:))]
        fn count(&self, attribute: &NSString) -> usize {
            if self.ivars().parked.get() && child_attribute(attribute) { 0 }
            else { unsafe { msg_send![super(self), accessibilityArrayAttributeCount: attribute] } }
        }

        #[unsafe(method(accessibilityArrayAttributeValues:index:maxCount:))]
        fn values(&self, attribute: &NSString, index: usize, max_count: usize) -> *mut objc2::runtime::AnyObject {
            if self.ivars().parked.get() && child_attribute(attribute) { empty_children() }
            else { unsafe { msg_send![super(self), accessibilityArrayAttributeValues: attribute, index: index, maxCount: max_count] } }
        }

        #[unsafe(method(hitTest:))]
        fn hit(&self, point: NSPoint) -> *mut NSView {
            if self.ivars().parked.get() { return std::ptr::null_mut(); }
            let hit: *mut NSView = unsafe { msg_send![super(self), hitTest: point] };
            // This full-window container must never consume clicks on main chrome.
            if std::ptr::eq(hit, self as *const Self as *const NSView) { std::ptr::null_mut() } else { hit }
        }

        #[unsafe(method(accessibilityHitTest:))]
        fn ax_hit(&self, point: NSPoint) -> *mut objc2::runtime::AnyObject {
            if self.ivars().parked.get() { return std::ptr::null_mut(); }
            unsafe {
                let Some(window) = self.window() else { return std::ptr::null_mut(); };
                let Some(child) = self.subviews().firstObject() else { return std::ptr::null_mut(); };
                // AX uses screen coordinates. WK's AX hit-test ignores its point,
                // so bound it to the actual page before forwarding into WebKit.
                let local = child.convertPoint_fromView(window.convertPointFromScreen(point), None);
                if !child.mouse_inRect(local, child.bounds()) { return std::ptr::null_mut(); }
                msg_send![&*child, accessibilityHitTest: point]
            }
        }

        #[unsafe(method(accessibilityFocusedUIElement))]
        fn ax_focus(&self) -> *mut objc2::runtime::AnyObject {
            if self.ivars().parked.get() { std::ptr::null_mut() } else {
                unsafe { msg_send![super(self), accessibilityFocusedUIElement] }
            }
        }
    }
);

// These view helpers also serve the main-thread, headless AppKit integration test.
pub(super) fn wrap(child: &NSView) -> Result<(), String> {
    let parent = unsafe { child.superview() }.ok_or("Native browser parent unavailable")?;
    if parent.downcast_ref::<BrowserAXContainer>().is_some() {
        return Ok(());
    }
    let mtm = MainThreadMarker::new().ok_or("Native browser requires AppKit main thread")?;
    let frame = child.frame();
    let allocated = BrowserAXContainer::alloc(mtm).set_ivars(State {
        parked: Cell::new(true),
        flipped: parent.isFlipped(),
    });
    let container: Retained<BrowserAXContainer> =
        unsafe { msg_send![super(allocated), initWithFrame: parent.bounds()] };
    unsafe {
        container.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        container.setAccessibilityElement(true);
        container.setAccessibilityRole(Some(NSAccessibilityGroupRole));
        container.setAccessibilityLabel(Some(&NSString::from_str("Native browser page")));
        // Above the original child, then remove that child from the sibling list:
        // the container occupies exactly its former drawing and hit-test order.
        parent.addSubview_positioned_relativeTo(
            &container,
            NSWindowOrderingMode::Above,
            Some(child),
        );
        container.addSubview(child);
        // Wry computes bounds against the immediate parent's full height and flip.
        // A full-parent container preserves that coordinate system on every resize.
        if child.frame() != frame {
            child.setFrame(frame);
        }
    }
    set_exposed(child, false)?;
    Ok(())
}

pub(super) fn set_exposed(child: &NSView, exposed: bool) -> Result<(), String> {
    let parent = unsafe { child.superview() }.ok_or("Native browser parent unavailable")?;
    let container = parent
        .downcast_ref::<BrowserAXContainer>()
        .ok_or("Native browser AX container unavailable")?;
    let changed = container.ivars().parked.replace(!exposed) == exposed;
    let child: &objc2::runtime::AnyObject = child;
    let children = if exposed {
        NSArray::from_slice(&[child])
    } else {
        NSArray::new()
    };
    unsafe {
        // Unlike WKWebView, our NSView uses AppKit's ordinary AX property storage.
        // An explicit group and children list prevent ignored-view flattening.
        container.setAccessibilityChildren(Some(&children));
        let _: () = msg_send![container, setAccessibilityChildrenInNavigationOrder: &*children];
        if changed {
            NSAccessibilityPostNotification(&parent, NSAccessibilityLayoutChangedNotification);
        }
    }
    Ok(())
}

pub(super) fn unwrap(child: &NSView) {
    let Some(container) = (unsafe { child.superview() }) else {
        return;
    };
    if container.downcast_ref::<BrowserAXContainer>().is_none() {
        return;
    }
    let _ = set_exposed(child, false);
    // Keep the child in its window until Wry processes close. Removing only the
    // wrapper also leaves no empty, full-window native view after tab eviction.
    if let Some(parent) = unsafe { container.superview() } {
        parent.addSubview_positioned_relativeTo(
            child,
            NSWindowOrderingMode::Above,
            Some(&container),
        );
        container.removeFromSuperview();
    }
}

pub async fn install_view(view: &tauri::Webview) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        let _ = tx.send(wrap(wk));
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Native view closed")?
}

pub async fn expose(view: &tauri::Webview, exposed: bool) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        let _ = tx.send(set_exposed(wk, exposed));
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Native view closed")?
}

pub fn release(view: &tauri::Webview) {
    let _ = view.with_webview(move |platform| unsafe {
        unwrap(&*(platform.inner() as *const WKWebView));
    });
}
