#[cfg(target_os = "ios")]
pub(crate) fn configure_ios_webview_edge_to_edge(
    webview_window: &tauri::WebviewWindow,
) -> Result<(), String> {
    use objc2::runtime::AnyObject;

    webview_window
        .with_webview(|webview| unsafe {
            let wk_webview = webview.inner().cast::<AnyObject>();
            if !wk_webview.is_null() {
                let scroll_view: *mut AnyObject = objc2::msg_send![wk_webview, scrollView];
                if !scroll_view.is_null() {
                    // UIScrollViewContentInsetAdjustmentNever
                    let adjustment_never: isize = 2;
                    let () = objc2::msg_send![
                        scroll_view,
                        setContentInsetAdjustmentBehavior: adjustment_never
                    ];
                    let () = objc2::msg_send![
                        scroll_view,
                        setAutomaticallyAdjustsScrollIndicatorInsets: false
                    ];
                }
            }

            let view_controller = webview.view_controller().cast::<AnyObject>();
            if !view_controller.is_null() {
                // UIRectEdgeAll
                let all_edges: usize = 15;
                let () = objc2::msg_send![view_controller, setEdgesForExtendedLayout: all_edges];
                let () = objc2::msg_send![
                    view_controller,
                    setExtendedLayoutIncludesOpaqueBars: true
                ];
            }
        })
        .map_err(|error| error.to_string())
}
