//! Observe failures without replacing Wry's navigation, download or auth policy.
//! All other delegate messages forward to the original delegate.
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject, Sel},
    DefinedClass, MainThreadOnly,
};
use objc2_foundation::{MainThreadMarker, NSError, NSObject, NSObjectProtocol};
use objc2_web_kit::{WKNavigation, WKNavigationDelegate, WKWebView};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::HashMap};
use tokio::sync::watch;

struct State {
    original: Retained<ProtocolObject<dyn WKNavigationDelegate>>,
    state: watch::Sender<Value>,
    id: String,
}
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = State]
    struct NavigationEvents;
    unsafe impl NSObjectProtocol for NavigationEvents {}
    unsafe impl WKNavigationDelegate for NavigationEvents {}
    impl NavigationEvents {
        #[unsafe(method(respondsToSelector:))]
        fn responds(&self, selector: Sel) -> bool {
            let own: bool = unsafe { msg_send![super(self), respondsToSelector: selector] };
            own || self.ivars().original.respondsToSelector(selector)
        }
        #[unsafe(method(forwardingTargetForSelector:))]
        fn forward(&self, _selector: Sel) -> *mut AnyObject {
            Retained::as_ptr(&self.ivars().original) as *mut AnyObject
        }
        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn terminated(&self, _view: &WKWebView) {
            self.failed("The page process stopped. Reload to recover.".into());
        }
        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn provisional_failed(&self, _view: &WKWebView, _navigation: Option<&WKNavigation>, error: &NSError) {
            // Cancellation is normal for redirects, downloads and user navigation.
            if !matches!(error.code(), -999 | 102) { self.failed(error.localizedDescription().to_string()); }
        }
        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn navigation_failed(&self, _view: &WKWebView, _navigation: Option<&WKNavigation>, error: &NSError) {
            if !matches!(error.code(), -999 | 102) { self.failed(error.localizedDescription().to_string()); }
        }
    }
);
impl NavigationEvents {
    fn failed(&self, error: String) {
        super::update(
            &self.ivars().state,
            &self.ivars().id,
            json!({"error":error,"loading":false,"ready":false}),
        );
    }
}
thread_local! { static DELEGATES: RefCell<HashMap<String,Retained<NavigationEvents>>> = RefCell::new(HashMap::new()); }
pub fn install(
    view: &tauri::Webview,
    id: String,
    state: watch::Sender<Value>,
) -> Result<(), String> {
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        let Some(original) = wk.navigationDelegate() else {
            return;
        };
        let allocated =
            NavigationEvents::alloc(MainThreadMarker::new().expect("WebKit main thread"))
                .set_ivars(State {
                    original,
                    state,
                    id: id.clone(),
                });
        let delegate: Retained<NavigationEvents> = msg_send![super(allocated), init];
        wk.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        DELEGATES.with(|delegates| {
            delegates.borrow_mut().insert(id, delegate);
        });
    })
    .map_err(|e| e.to_string())
}
pub fn release(view: &tauri::Webview) {
    let id = view.label().trim_start_matches("browser-").to_string();
    let _ = view.with_webview(move |platform| unsafe {
        if let Some(delegate) = DELEGATES.with(|delegates| delegates.borrow_mut().remove(&id)) {
            let wk = &*(platform.inner() as *const WKWebView);
            wk.setNavigationDelegate(Some(&delegate.ivars().original));
        }
    });
}
