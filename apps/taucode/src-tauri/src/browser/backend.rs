use futures_util::future::BoxFuture;
use serde_json::{json, Value};

/// The local socket and MCP adapter use the same backend as the pane.
pub trait BrowserBackend: Send + Sync {
    fn request(&self, params: Value) -> BoxFuture<'_, Result<Value, String>>;
    fn navigate(&self, url: &str) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"navigate","url":url}))
    }
    fn snapshot(&self) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"script","op":"snapshot"}))
    }
    fn click(&self, reference: &str) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"script","op":"click","ref":reference}))
    }
    fn type_text(
        &self,
        reference: &str,
        text: &str,
        submit: bool,
    ) -> BoxFuture<'_, Result<Value, String>> {
        self.request(
            json!({"action":"script","op":"type","ref":reference,"text":text,"submit":submit}),
        )
    }
    fn press(&self, key: &str) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"script","op":"press","key":key}))
    }
    fn select(&self, reference: &str, values: Value) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"script","op":"select","ref":reference,"values":values}))
    }
    fn scroll(&self, params: Value) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"script","op":"scroll","x":params["x"],"y":params["y"],"ref":params["ref"]}))
    }
    fn screenshot(&self) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"screenshot"}))
    }
    fn tabs(&self, params: Value) -> BoxFuture<'_, Result<Value, String>> {
        self.request(params)
    }
    fn wait_for(&self, params: Value) -> BoxFuture<'_, Result<Value, String>> {
        self.request(json!({"action":"wait","text":params["text"],"textGone":params["textGone"],"ref":params["ref"],"time":params["time"]}))
    }
}

pub struct CdpBackend(pub tokio::sync::mpsc::Sender<super::bridge::Request>);
impl BrowserBackend for CdpBackend {
    fn request(&self, params: Value) -> BoxFuture<'_, Result<Value, String>> {
        Box::pin(async move {
            let (reply, receiver) = tokio::sync::oneshot::channel();
            self.0
                .send((params, reply))
                .await
                .map_err(|_| "Browser disconnected")?;
            tokio::time::timeout(std::time::Duration::from_secs(30), receiver)
                .await
                .map_err(|_| "Browser request timed out")?
                .map_err(|_| "Browser disconnected")?
        })
    }
}

fn reference(args: &Value) -> Result<&str, String> {
    args.get("target")
        .or_else(|| args.get("ref"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Missing target (snapshot ref or unique selector)".into())
}

pub async fn tool(backend: &dyn BrowserBackend, name: &str, args: Value) -> Result<Value, String> {
    let result = match name {
        "browser_navigate" => backend.navigate(args["url"].as_str().ok_or("Missing URL")?).await?,
        "browser_navigate_back" => backend.request(json!({"action":"back"})).await?,
        "browser_snapshot" => backend.snapshot().await?,
        "browser_click" => backend.click(reference(&args)?).await?,
        "browser_type" => backend.type_text(reference(&args)?, args["text"].as_str().ok_or("Missing text")?, args["submit"] == true).await?,
        "browser_press_key" => backend.press(args["key"].as_str().ok_or("Missing key")?).await?,
        "browser_select_option" => backend.select(reference(&args)?, args["values"].clone()).await?,
        "browser_fill_form" => backend.request(json!({"action":"script","op":"fill","fields":args["fields"]})).await?,
        "browser_evaluate" => {
            let function = args["function"].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 200000).ok_or("Invalid JavaScript function")?;
            if !args["filename"].is_null() { return Err("Native evaluation returns results inline; filename is unsupported".into()); }
            let target = if args.get("target").or_else(|| args.get("ref")).is_some() { Some(reference(&args)?) } else { None };
            backend.request(json!({"action":"evaluate","function":function,"ref":target})).await?
        },
        "browser_scroll" => backend.scroll(args).await?,
        "browser_wait_for" => backend.wait_for(args).await?,
        "browser_tabs" => backend.tabs(json!({"action":"tabs","params":args})).await?,
        "browser_close" => backend.request(json!({"action":"close"})).await?,
        "browser_take_screenshot" => return Ok(json!({"content":[{"type":"image","mimeType":"image/png","data":backend.screenshot().await?}]})),
        _ => return Err("This tool needs trusted input or CDP. Try the Headless Chromium backend (Settings → Browser)".into()),
    };
    Ok(
        json!({"content":[{"type":"text","text":serde_json::to_string(&result).map_err(|e|e.to_string())?}]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake;
    impl BrowserBackend for Fake {
        fn request(&self, params: Value) -> BoxFuture<'_, Result<Value, String>> {
            Box::pin(async { Ok(params) })
        }
    }
    #[tokio::test]
    async fn tools_share_the_backend_contract() {
        let result = tool(
            &Fake,
            "browser_type",
            json!({"ref":"e1","text":"hello","submit":true}),
        )
        .await
        .unwrap();
        let params: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            params,
            json!({"action":"script","op":"type","ref":"e1","text":"hello","submit":true})
        );
        assert!(tool(&Fake, "browser_evaluate", json!({}))
            .await
            .unwrap_err()
            .contains("Invalid JavaScript function"));
    }
    async fn dispatched(name: &str, args: Value) -> Value {
        let result = tool(&Fake, name, args).await.unwrap();
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }
    #[tokio::test]
    async fn advertised_targets_and_legacy_refs_reach_native_actions() {
        for target in [
            json!({"target":"e6"}),
            json!({"ref":"e6"}),
            json!({"target":"#login"}),
        ] {
            let expected = reference(&target).unwrap().to_string();
            assert_eq!(
                dispatched("browser_click", target).await,
                json!({"action":"script","op":"click","ref":expected})
            );
        }
        assert_eq!(
            dispatched("browser_type", json!({"target":"e4","text":"demo"})).await,
            json!({"action":"script","op":"type","ref":"e4","text":"demo","submit":false})
        );
        assert_eq!(
            dispatched(
                "browser_select_option",
                json!({"target":"select","values":["shade"]})
            )
            .await,
            json!({"action":"script","op":"select","ref":"select","values":["shade"]})
        );
        for invalid in [json!({}), json!({"target":""}), json!({"target":42})] {
            assert!(tool(&Fake, "browser_click", invalid)
                .await
                .unwrap_err()
                .contains("Missing target"));
        }
    }
    #[tokio::test]
    async fn advertised_form_and_evaluation_requests_reach_native_backend() {
        let fields = json!([
            {"target":"e4","name":"Username","type":"textbox","value":"demo"},
            {"target":"e5","name":"Password","type":"textbox","value":"demo123"}
        ]);
        assert_eq!(
            dispatched("browser_fill_form", json!({"fields":fields})).await,
            json!({"action":"script","op":"fill","fields":fields})
        );
        let function = "async () => document.title";
        assert_eq!(
            dispatched("browser_evaluate", json!({"function":function})).await,
            json!({"action":"evaluate","function":function,"ref":null})
        );
        assert_eq!(
            dispatched(
                "browser_evaluate",
                json!({"function":"el => el.id","target":"e4"})
            )
            .await,
            json!({"action":"evaluate","function":"el => el.id","ref":"e4"})
        );
        assert!(tool(
            &Fake,
            "browser_evaluate",
            json!({"function":function,"filename":"output.json"})
        )
        .await
        .unwrap_err()
        .contains("filename is unsupported"));
    }
}
