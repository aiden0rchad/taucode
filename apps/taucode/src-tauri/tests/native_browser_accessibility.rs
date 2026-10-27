//! Run on the process main thread with unshown AppKit windows; never activates UI.
//! cargo test --test native_browser_accessibility --offline
#[cfg(target_os = "macos")]
#[path = "../src/browser/native/accessibility.rs"]
#[allow(dead_code)]
mod accessibility;

#[cfg(target_os = "macos")]
mod checks {
    use super::accessibility;
    use objc2::{
        define_class, msg_send,
        rc::{autoreleasepool, Retained},
        DefinedClass, MainThreadOnly,
    };
    use objc2_app_kit::{
        NSAccessibility, NSApplication, NSApplicationActivationPolicy, NSBackingStoreType,
        NSObjectNSAccessibility, NSView, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{
        MainThreadMarker, NSArray, NSKeyValueObservingOptions, NSObject, NSObjectProtocol, NSPoint,
        NSRect, NSSize, NSString, NSValue,
    };
    use std::{cell::Cell, ffi::c_void};

    define_class!(
        #[unsafe(super = NSView)]
        #[thread_kind = MainThreadOnly]
        struct FlippedView;
        unsafe impl NSObjectProtocol for FlippedView {}
        impl FlippedView {
            #[unsafe(method(isFlipped))]
            fn flipped(&self) -> bool { true }
        }
    );
    define_class!(
        #[unsafe(super = NSView)]
        #[thread_kind = MainThreadOnly]
        struct PageView;
        unsafe impl NSObjectProtocol for PageView {}
        impl PageView {
            // WebKit returns its remote child regardless of the queried point.
            // The container must reject background before calling this method.
            #[unsafe(method(accessibilityHitTest:))]
            fn ax_hit(&self, _: NSPoint) -> *mut objc2::runtime::AnyObject {
                self as *const Self as *mut objc2::runtime::AnyObject
            }
        }
    );
    define_class!(
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[ivars = Cell<usize>]
        struct Observer;
        unsafe impl NSObjectProtocol for Observer {}
        impl Observer {
            #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
            fn observe(&self, _: *mut NSString, _: *mut objc2::runtime::AnyObject, _: *mut objc2::runtime::AnyObject, _: *mut c_void) {
                self.ivars().set(self.ivars().get() + 1);
            }
        }
    );

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    #[allow(deprecated)]
    fn assert_ax_frame(container: &NSView, child: &NSView, window: &NSWindow, exposed: bool) {
        unsafe {
            let expected = if exposed {
                window.convertRectToScreen(child.convertRect_toView(child.bounds(), None))
            } else {
                NSRect::default()
            };
            assert_eq!(
                container.accessibilityFrame(),
                expected,
                "AX frame represents the page, not its full-window coordinate container"
            );
            let position = container
                .accessibilityAttributeValue(&NSString::from_str("AXPosition"))
                .unwrap()
                .downcast::<NSValue>()
                .unwrap();
            let size = container
                .accessibilityAttributeValue(&NSString::from_str("AXSize"))
                .unwrap()
                .downcast::<NSValue>()
                .unwrap();
            assert_eq!(
                position.pointValue(),
                expected.origin,
                "legacy AX position follows logical page geometry"
            );
            assert_eq!(
                size.sizeValue(),
                expected.size,
                "legacy AX size follows logical page geometry"
            );
        }
    }

    #[allow(deprecated)]
    pub fn run() {
        let mtm =
            MainThreadMarker::new().expect("AppKit regression must run on process main thread");
        autoreleasepool(|_| unsafe {
            let app = NSApplication::sharedApplication(mtm);
            // Command-line test hosts may reject policy changes. No code below
            // activates the process or orders a window, regardless of that policy.
            let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
            for flipped in [false, true] {
                let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                    NSWindow::alloc(mtm),
                    rect(if flipped { 410. } else { 200. }, 180., 1000., 700.),
                    NSWindowStyleMask::Borderless,
                    NSBackingStoreType::Buffered,
                    true,
                );
                assert!(!window.isVisible());
                let parent: Retained<NSView> = if flipped {
                    let view: Retained<FlippedView> = msg_send![FlippedView::alloc(mtm), initWithFrame: rect(0., 0., 1000., 700.)];
                    view.into_super()
                } else {
                    NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 1000., 700.))
                };
                window.setContentView(Some(&parent));
                let before = NSView::initWithFrame(NSView::alloc(mtm), rect(0., 0., 1., 1.));
                let child: Retained<PageView> =
                    msg_send![PageView::alloc(mtm), initWithFrame: rect(100., 140., 320., 240.)];
                let child: Retained<NSView> = child.into_super();
                child.setAccessibilityElement(true);
                child.setAccessibilityRole(Some(objc2_app_kit::NSAccessibilityGroupRole));
                let after = NSView::initWithFrame(NSView::alloc(mtm), rect(999., 699., 1., 1.));
                parent.addSubview(&before);
                parent.addSubview(&child);
                parent.addSubview(&after);
                let allocated = Observer::alloc(mtm).set_ivars(Cell::new(0));
                let observer: Retained<Observer> = msg_send![super(allocated), init];
                let key = NSString::from_str("frame");
                let _: () = msg_send![&*child, addObserver: &*observer, forKeyPath: &*key, options: NSKeyValueObservingOptions::New, context: std::ptr::null_mut::<c_void>()];
                let original_class = child.class();
                let original_frame = child.frame();
                accessibility::wrap(&child).unwrap();
                let container = child.superview().unwrap();
                assert_eq!(
                    child.class(),
                    original_class,
                    "KVO-enrolled child's class must stay untouched"
                );
                assert_eq!(child.frame(), original_frame);
                assert_eq!(container.frame(), parent.bounds());
                assert_eq!(container.isFlipped(), flipped);
                assert_eq!(parent.subviews().objectAtIndex(1), container);
                assert_eq!(
                    container.accessibilityLabel().unwrap().to_string(),
                    "Native browser page"
                );
                assert!(!container.accessibilityIsIgnored());

                for exposed in [false, true, false, true] {
                    accessibility::set_exposed(&child, exposed).unwrap();
                    assert_ax_frame(&container, &child, &window, exposed);
                    let count = usize::from(exposed);
                    assert_eq!(container.accessibilityChildren().unwrap().len(), count);
                    assert_eq!(
                        container
                            .accessibilityChildrenInNavigationOrder()
                            .unwrap()
                            .len(),
                        count
                    );
                    let attr = NSString::from_str("AXChildren");
                    let legacy = container
                        .accessibilityAttributeValue(&attr)
                        .unwrap()
                        .downcast::<NSArray>()
                        .unwrap();
                    assert_eq!(legacy.len(), count);
                    let parameterized: Option<Retained<objc2::runtime::AnyObject>> = msg_send![&*container, accessibilityAttributeValue: &*attr, forParameter: std::ptr::null_mut::<objc2::runtime::AnyObject>()];
                    if !exposed {
                        assert!(parameterized
                            .is_none_or(|value| value.downcast::<NSArray>().unwrap().is_empty()));
                    }
                    assert_eq!(container.accessibilityArrayAttributeCount(&attr), count);
                    assert_eq!(
                        container
                            .accessibilityArrayAttributeValues_index_maxCount(&attr, 0, 10)
                            .len(),
                        count
                    );
                    assert!(
                        container.hitTest(NSPoint::new(700., 400.)).is_none(),
                        "full-window background must pass clicks through"
                    );
                    let screen_point =
                        |point| window.convertPointToScreen(child.convertPoint_toView(point, None));
                    let inside = screen_point(NSPoint::new(50., 40.));
                    let hit = container.accessibilityHitTest(inside);
                    assert_eq!(
                        hit.is_some(),
                        exposed,
                        "AX screen hit must reach the actual page when exposed"
                    );
                    if let Some(hit) = hit {
                        assert_eq!(Retained::as_ptr(&hit), Retained::as_ptr(&child).cast());
                    }
                    for point in [NSPoint::new(370., 40.), NSPoint::new(50., 290.)] {
                        assert!(container.accessibilityHitTest(screen_point(point)).is_none(), "AX background must pass through, even though page hit-test ignores its point");
                    }
                    let hit = container.hitTest(NSPoint::new(150., 180.));
                    assert_eq!(hit.is_some(), exposed);
                    if let Some(hit) = hit {
                        assert!(hit.isDescendantOf(&child));
                    }
                    if !exposed {
                        assert!(container.accessibilityFocusedUIElement().is_none());
                    }
                    child.setFrame(rect(100., 140., 320., 240.));
                    assert_eq!(child.class(), original_class);
                }
                assert!(
                    observer.ivars().get() >= 4,
                    "KVO CGRect setters still notify after wrapping"
                );
                window.setContentSize(NSSize::new(1440., 900.));
                assert_eq!(
                    container.frame(),
                    parent.bounds(),
                    "container tracks window-sized parent"
                );
                assert_eq!(
                    child.frame(),
                    original_frame,
                    "parent resize must preserve child viewport"
                );
                assert_eq!(container.isFlipped(), flipped);
                assert_ax_frame(&container, &child, &window, true);
                window.setFrameOrigin(NSPoint::new(470., 260.));
                assert_ax_frame(&container, &child, &window, true);
                accessibility::set_exposed(&child, false).unwrap();
                assert_ax_frame(&container, &child, &window, false);
                accessibility::unwrap(&child);
                assert!(container.accessibilityChildren().unwrap().is_empty());
                assert!(container.superview().is_none());
                assert_eq!(child.superview().unwrap(), parent);
                assert_eq!(
                    parent.subviews().objectAtIndex(1),
                    child,
                    "close restores sibling order without orphan container"
                );
                assert_eq!(child.frame(), original_frame);
                let _: () = msg_send![&*child, removeObserver: &*observer, forKeyPath: &*key];
                child.removeFromSuperview();
                assert_eq!(parent.subviews().len(), 2);
                assert!(
                    !window.isVisible(),
                    "fixture must never order a window onto the screen"
                );
                window.setContentView(None);
            }
        });
        println!("native browser AX container: main-thread AppKit/KVO/geometry/hit-test/cleanup checks passed");
    }
}

#[cfg(target_os = "macos")]
fn main() {
    checks::run();
}
#[cfg(not(target_os = "macos"))]
fn main() {
    println!("native browser AX container: macOS only");
}
