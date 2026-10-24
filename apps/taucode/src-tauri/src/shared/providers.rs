use crate::types::*;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) fn kind_info(kind: ProviderKind) -> (&'static str, &'static str) {
    use ProviderKind::*;
    match kind {
        Openai => ("https://api.openai.com/v1", "authorization"),
        Anthropic => ("https://api.anthropic.com/v1", "x-api-key"),
        Google => (
            "https://generativelanguage.googleapis.com/v1beta",
            "x-goog-api-key",
        ),
        Openrouter => ("https://openrouter.ai/api/v1", "authorization"),
        Groq => ("https://api.groq.com/openai/v1", "authorization"),
        Mistral => ("https://api.mistral.ai/v1", "authorization"),
        Xai => ("https://api.x.ai/v1", "authorization"),
        Deepseek => ("https://api.deepseek.com", "authorization"),
        Zai => ("https://api.z.ai/api/paas/v4", "authorization"),
        Ollama => ("http://localhost:11434/v1", ""),
        Lmstudio => ("http://localhost:1234/v1", ""),
        OpenaiCompatible => ("", "authorization"),
    }
}
pub(crate) fn base_url(p: &ProviderEntry) -> &str {
    p.base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(kind_info(p.kind).0)
}
pub(crate) fn codex_usable(p: &ProviderEntry) -> bool {
    matches!(
        p.kind,
        ProviderKind::Openai | ProviderKind::Openrouter | ProviderKind::Ollama
    ) || (p.kind == ProviderKind::OpenaiCompatible && p.api_style == Some(ApiStyle::Responses))
}
pub(crate) fn claude_usable(p: &ProviderEntry) -> bool {
    p.kind == ProviderKind::Anthropic || p.anthropic_compatible
}
pub(crate) fn valid_slug(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}
pub(crate) fn validate(providers: &[ProviderEntry], policy: &AgentPolicy) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut env_names = HashSet::new();
    for p in providers {
        if !valid_slug(&p.id) || !ids.insert(&p.id) {
            return Err(
                "Provider IDs must be unique slugs of 1–32 lowercase letters, digits, _ or -"
                    .into(),
            );
        }
        if !env_names.insert(crate::shared::engines::key_env(&p.id)) {
            return Err("Provider IDs must have distinct credential environment names (hyphens and underscores collide)".into());
        }
        let url = reqwest::Url::parse(base_url(p))
            .map_err(|_| format!("Enter a valid base URL for {}", p.name))?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("Base URLs must be HTTP(S), without credentials, query or fragment".into());
        }
        if p.kind != ProviderKind::OpenaiCompatible && p.api_style.is_some() {
            return Err("API style applies only to openai-compatible providers".into());
        }
        for (name, value) in &p.extra_headers {
            let lower = name.to_ascii_lowercase();
            if lower.contains("auth")
                || lower.contains("key")
                || lower.contains("token")
                || lower == "cookie"
            {
                return Err("Store credentials using provider auth, not extra headers".into());
            }
            reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "Invalid header name")?;
            reqwest::header::HeaderValue::from_str(value).map_err(|_| "Invalid header value")?;
        }
        if let ProviderAuth::Env { var } = &p.auth {
            if matches!(
                var.as_str(),
                "CODEX_HOME" | "CODEX_APP_SERVER_TEST_USER_CONFIG_FILE"
            ) {
                return Err(
                    "Use a credential environment variable, not a Codex configuration variable"
                        .into(),
                );
            }
            if var.is_empty()
                || !var.bytes().enumerate().all(|(i, c)| {
                    c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err("Enter a valid environment variable name".into());
            }
        }
        if let ProviderAuth::Command { command, timeout_ms, .. } = &p.auth {
            if command.trim().is_empty() {
                return Err("Enter a token command".into());
            }
            if *timeout_ms == Some(0) {
                return Err("Auth command timeout must be greater than zero".into());
            }
        }
        let mut models = HashSet::new();
        if p.models
            .iter()
            .any(|m| m.id.trim().is_empty() || !models.insert(&m.id))
        {
            return Err("Model IDs must be non-empty and unique within a provider".into());
        }
    }
    let s = &policy.subagents;
    if !(1..=16).contains(&s.max_concurrent)
        || !(1..=32).contains(&s.max_per_turn)
        || !(1..=3).contains(&s.max_depth)
    {
        return Err("Subagent limits: concurrent 1–16, per turn 1–32, depth 1–3".into());
    }
    let check_model = |m: &ModelRef| {
        providers.iter().any(|p| {
            p.id == m.provider_id
                && p.enabled
                && p.models.iter().any(|v| v.id == m.model_id && v.enabled)
        })
    };
    for m in policy
        .default_model
        .iter()
        .chain(s.default_model.iter())
        .chain(s.allowed_models.iter())
        .chain(s.roles.iter().filter_map(|r| r.model.as_ref()))
    {
        if !check_model(m) {
            return Err(format!(
                "Enable provider and model {}/{}",
                m.provider_id, m.model_id
            ));
        }
    }
    let mut names = HashSet::new();
    for role in &s.roles {
        if !valid_slug(&role.name) || !names.insert(&role.name) {
            return Err("Role names must be unique slugs".into());
        }
        if role
            .model
            .as_ref()
            .is_some_and(|m| !s.allowed_models.is_empty() && !s.allowed_models.contains(m))
        {
            return Err(format!("Role {} model is not in allowed models", role.name));
        }
    }
    if s.default_model
        .as_ref()
        .is_some_and(|m| !s.allowed_models.is_empty() && !s.allowed_models.contains(m))
    {
        return Err("Default subagent model is not in allowed models".into());
    }
    Ok(())
}
pub(crate) fn effective_policy(
    base: &AgentPolicy,
    over: Option<&AgentPolicyOverride>,
) -> AgentPolicy {
    let mut p = base.clone();
    if let Some(o) = over {
        if let Some(m) = &o.default_model {
            p.default_model = Some(m.clone());
        }
        if let Some(e) = o.default_engine {
            p.default_engine = e;
        }
        if let Some(s) = &o.subagents {
            macro_rules! merge { ($($field:ident),*) => { $(if let Some(v) = &s.$field { p.subagents.$field = v.clone(); })* }; }
            merge!(
                enabled,
                max_concurrent,
                max_per_turn,
                max_depth,
                allowed_models,
                roles
            );
            if let Some(m) = &s.default_model {
                p.subagents.default_model = Some(m.clone());
            }
        }
    }
    p
}
pub(crate) fn catalog(providers: &[ProviderEntry]) -> Value {
    json!({ "result": { "data": providers.iter().filter(|p| p.enabled).flat_map(|p| p.models.iter().filter(|m| m.enabled).map(move |m| json!({
        "id": format!("{}/{}",p.id,m.id), "model": format!("{}/{}",p.id,m.id),
        "providerId": p.id, "providerName": p.name, "modelId": m.id,
        "displayName": m.display_name.as_ref().unwrap_or(&m.id), "description": p.name,
        "engines": ([Some("pi"), codex_usable(p).then_some("codex"), claude_usable(p).then_some("claude")].into_iter().flatten().collect::<Vec<_>>()),
        "supportedReasoningEfforts": [], "defaultReasoningEffort": null, "isDefault": false,
        "contextWindow": m.context_window
    }))).collect::<Vec<_>>() } })
}
pub(crate) trait KeyStore: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<String>, String>;
    fn set(&self, id: &str, key: &str) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}
pub(crate) struct Keychain;
fn key_entry(id: &str) -> Result<keyring::Entry, String> {
    let account = if let Some(rest) = id.strip_prefix("mcp:") {
        let (server, name) = rest.split_once(':').ok_or("Invalid connector credential")?;
        super::connectors::account(server, name)?
    } else {
        if !valid_slug(id) {
            return Err("Invalid provider ID".into());
        }
        format!("provider:{id}")
    };
    keyring::Entry::new("dev.taucode.desktop", &account)
        .map_err(|_| "Cannot access provider Keychain".into())
}
impl KeyStore for Keychain {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        match key_entry(id)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("Cannot read provider Keychain".into()),
        }
    }
    fn set(&self, id: &str, key: &str) -> Result<(), String> {
        key_entry(id)?
            .set_password(key)
            .map_err(|_| "Cannot save provider key".into())
    }
    fn delete(&self, id: &str) -> Result<(), String> {
        match key_entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Cannot delete provider key".into()),
        }
    }
}
pub(crate) fn delete_removed_keys(
    old: &[ProviderEntry],
    new: &[ProviderEntry],
    keys: &dyn KeyStore,
) -> Result<(), String> {
    for p in old {
        if !new.iter().any(|v| v.id == p.id) {
            keys.delete(&p.id)?;
        }
    }
    Ok(())
}
pub(crate) async fn token(
    p: &ProviderEntry,
    keys: &dyn KeyStore,
) -> Result<Option<String>, String> {
    match &p.auth {
        ProviderAuth::None => Ok(None),
        ProviderAuth::ApiKey => keys
            .get(&p.id)?
            .map(Some)
            .ok_or_else(|| "Save an API key first".into()),
        ProviderAuth::Env { var } => std::env::var(var)
            .map(Some)
            .map_err(|_| format!("Set environment variable {var}")),
        ProviderAuth::Command { command, args, timeout_ms } => {
            let duration = Duration::from_millis(timeout_ms.unwrap_or(15_000));
            if duration.is_zero() { return Err("Auth command timeout must be greater than zero".into()); }
            let mut child = tokio::process::Command::new(command);
            child.args(args).stdin(std::process::Stdio::null()).kill_on_drop(true);
            let output = tokio::time::timeout(duration, child.output())
                .await
                .map_err(|_| format!("The auth command took longer than {} s. If the script lives in an iCloud-synced folder (Desktop/Documents), move it or read the key file directly.", duration.as_secs_f64()))?
                .map_err(|_| "Cannot run token command")?;
            if !output.status.success() {
                return Err("Token command failed".into());
            }
            let text =
                String::from_utf8(output.stdout).map_err(|_| "Token command must return UTF-8")?;
            if text.trim().is_empty() {
                return Err("Token command returned no token".into());
            }
            Ok(Some(text.trim().into()))
        }
    }
}
pub(crate) struct ModelRequest {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}
pub(crate) trait ModelHttp: Send + Sync {
    fn get(&self, request: ModelRequest) -> BoxFuture<'_, Result<(u16, Value), String>>;
}
pub(crate) struct Http;
impl ModelHttp for Http {
    fn get(&self, request: ModelRequest) -> BoxFuture<'_, Result<(u16, Value), String>> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Cannot create HTTP client")?;
            let mut req = client.get(&request.url);
            for (k, v) in request.headers {
                req = req.header(k, v);
            }
            let response = req.send().await.map_err(|e| {
                if e.is_connect() {
                    format!("Is the server running at {}?", request.url)
                } else {
                    "Request failed or timed out; check the endpoint".into()
                }
            })?;
            let status = response.status().as_u16();
            let bytes = response
                .bytes()
                .await
                .map_err(|_| "Cannot read model list")?;
            Ok((
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            ))
        })
    }
}
pub(crate) fn merge_models(existing: &mut Vec<ProviderModel>, fetched: Vec<ProviderModel>) {
    for mut model in fetched {
        if let Some(old) = existing.iter_mut().find(|m| m.id == model.id) {
            model.enabled = old.enabled;
            *old = model;
        } else {
            existing.push(model);
        }
    }
}
pub(crate) async fn fetch(
    p: &ProviderEntry,
    keys: &dyn KeyStore,
    http: &dyn ModelHttp,
) -> Result<Vec<ProviderModel>, String> {
    let mut headers = p.extra_headers.clone();
    let (_, auth_header) = kind_info(p.kind);
    if let Some(token) = token(p, keys).await? {
        if !auth_header.is_empty() {
            headers.insert(
                auth_header.into(),
                if auth_header == "authorization" {
                    format!("Bearer {token}")
                } else {
                    token
                },
            );
        }
    }
    if p.kind == ProviderKind::Anthropic {
        headers.insert("anthropic-version".into(), "2023-06-01".into());
    }
    let mut models = vec![];
    let mut page: Option<String> = None;
    let mut seen = HashSet::new();
    loop {
        let mut url = reqwest::Url::parse(&format!("{}/models", base_url(p).trim_end_matches('/')))
            .map_err(|_| "Invalid provider URL")?;
        if let Some(page) = &page {
            url.query_pairs_mut().append_pair("pageToken", page);
        }
        let (status, body) = http
            .get(ModelRequest {
                url: url.into(),
                headers: headers.clone(),
            })
            .await?;
        match status {
            200..=299 => (),
            401 | 403 => return Err("API key rejected".into()),
            404 => return Err("This endpoint doesn't list models; add model IDs manually".into()),
            _ => {
                return Err(format!(
                    "Model listing failed (HTTP {status}); check the endpoint and credentials"
                ))
            }
        }
        let entries = body
            .get(if p.kind == ProviderKind::Google {
                "models"
            } else {
                "data"
            })
            .and_then(Value::as_array)
            .ok_or("Invalid model list; add model IDs manually")?;
        for entry in entries {
            if let Some(id) = entry
                .get("id")
                .or_else(|| entry.get("name"))
                .and_then(Value::as_str)
            {
                let id = if p.kind == ProviderKind::Google {
                    id.strip_prefix("models/").unwrap_or(id)
                } else {
                    id
                };
                models.push(ProviderModel {
                    id: id.into(),
                    display_name: entry
                        .get("displayName")
                        .or_else(|| entry.get("display_name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    enabled: true,
                    context_window: entry
                        .get("context_length")
                        .or_else(|| entry.get("inputTokenLimit"))
                        .and_then(Value::as_u64),
                    supports_tools: None,
                    supports_images: None,
                });
            }
        }
        page = if p.kind == ProviderKind::Google {
            body.get("nextPageToken")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        } else {
            None
        };
        if let Some(next) = &page {
            if !seen.insert(next.clone()) {
                return Err("Model listing repeated a page token".into());
            }
        } else {
            break;
        }
    }
    Ok(models)
}
pub(crate) async fn test_and_fetch(
    id: String,
    settings: &Mutex<AppSettings>,
    path: &std::path::PathBuf,
) -> Result<Value, String> {
    let provider = settings
        .lock()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or("Provider not found")?;
    let start = Instant::now();
    let auth_budget = match &provider.auth {
        ProviderAuth::Command { timeout_ms, .. } => timeout_ms.unwrap_or(15_000),
        _ => 0,
    };
    match tokio::time::timeout(Duration::from_millis(auth_budget.saturating_add(15_000)), fetch(&provider, &Keychain, &Http))
        .await
        .unwrap_or_else(|_| Err("Model listing timed out".into()))
    {
        Ok(models) => {
            let mut current = settings.lock().await;
            let mut next = current.clone();
            let target = next
                .providers
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("Provider was deleted")?;
            if serde_json::to_value(&provider).ok() != serde_json::to_value(&*target).ok() {
                return Err("Provider changed; test again".into());
            }
            merge_models(&mut target.models, models);
            target.models_fetched_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            let result =
                json!({"ok":true, "latencyMs":start.elapsed().as_millis(), "models":target.models});
            crate::storage::write_settings(path, &next)?;
            *current = next;
            Ok(result)
        }
        Err(error) => Ok(
            json!({"ok":false, "latencyMs":start.elapsed().as_millis(), "models":[], "error":error}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    #[derive(Default)]
    struct Keys(StdMutex<BTreeMap<String, String>>);
    impl KeyStore for Keys {
        fn get(&self, id: &str) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().get(id).cloned())
        }
        fn set(&self, id: &str, key: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(id.into(), key.into());
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), String> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }
    struct FakeHttp {
        replies: StdMutex<std::collections::VecDeque<Result<(u16, Value), String>>>,
        requests: StdMutex<Vec<ModelRequest>>,
    }
    impl ModelHttp for FakeHttp {
        fn get(&self, request: ModelRequest) -> BoxFuture<'_, Result<(u16, Value), String>> {
            self.requests.lock().unwrap().push(request);
            let response = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected HTTP request");
            Box::pin(async { response })
        }
    }
    fn http(replies: Vec<Result<(u16, Value), String>>) -> FakeHttp {
        FakeHttp {
            replies: StdMutex::new(replies.into()),
            requests: StdMutex::new(vec![]),
        }
    }
    fn provider(kind: ProviderKind) -> ProviderEntry {
        serde_json::from_value(json!({"id":"test","name":"Test","kind":kind,"auth":{"type":"apiKey"},"models":[{"id":"model","enabled":true}],"enabled":true,"baseUrl": if kind == ProviderKind::OpenaiCompatible { Some("http://localhost:1234/v1") } else { None }})).unwrap()
    }
    #[tokio::test]
    async fn command_auth_timeout_is_configurable_and_actionable() {
        let mut p = provider(ProviderKind::Openai);
        p.auth = serde_json::from_value(json!({"type":"command","command":"/bin/sh","args":["-c","printf token"]})).unwrap();
        assert!(matches!(p.auth, ProviderAuth::Command { timeout_ms: None, .. }));
        assert_eq!(token(&p, &Keys::default()).await.unwrap().as_deref(), Some("token"));
        p.auth = serde_json::from_value(json!({"type":"command","command":"/bin/sleep","args":["1"],"timeoutMs":20})).unwrap();
        assert_eq!(token(&p, &Keys::default()).await.unwrap_err(), "The auth command took longer than 0.02 s. If the script lives in an iCloud-synced folder (Desktop/Documents), move it or read the key file directly.");
        let (config, _) = crate::shared::engines::codex_config(&[p.clone()], &AgentPolicy::default(), Path::new("/managed")).unwrap();
        assert!(config.contains("TAUCODE_PROVIDER_TEST_KEY"));
        assert!(!config.contains("/bin/sleep"));
        assert_eq!(crate::shared::pi::config::models(&[p])["providers"]["openai"]["apiKey"], "$TAUCODE_PROVIDER_TEST_KEY");
    }
    #[tokio::test]
    async fn kind_table_urls_and_headers_and_google_paging() {
        use ProviderKind::*;
        let table = [
            (Openai, "https://api.openai.com/v1", "authorization"),
            (Anthropic, "https://api.anthropic.com/v1", "x-api-key"),
            (
                Google,
                "https://generativelanguage.googleapis.com/v1beta",
                "x-goog-api-key",
            ),
            (Openrouter, "https://openrouter.ai/api/v1", "authorization"),
            (Groq, "https://api.groq.com/openai/v1", "authorization"),
            (Mistral, "https://api.mistral.ai/v1", "authorization"),
            (Xai, "https://api.x.ai/v1", "authorization"),
            (Deepseek, "https://api.deepseek.com", "authorization"),
            (Zai, "https://api.z.ai/api/paas/v4", "authorization"),
            (Ollama, "http://localhost:11434/v1", ""),
            (Lmstudio, "http://localhost:1234/v1", ""),
            (OpenaiCompatible, "", "authorization"),
        ];
        let keys = Keys::default();
        keys.set("test", "secret").unwrap();
        for (kind, url, header) in table {
            assert_eq!(kind_info(kind), (url, header));
            let fake = if kind == Google {
                http(vec![
                    Ok((
                        200,
                        json!({"models":[{"name":"models/one"}],"nextPageToken":"a +/"}),
                    )),
                    Ok((200, json!({"models":[{"name":"models/two"}]}))),
                ])
            } else {
                http(vec![Ok((200, json!({"data":[{"id":"one"}]})))])
            };
            let p = provider(kind);
            let models = fetch(&p, &keys, &fake).await.unwrap();
            assert_eq!(models[0].id, "one");
            let requests = fake.requests.lock().unwrap();
            assert_eq!(requests[0].url, format!("{}/models", base_url(&p)));
            if !header.is_empty() {
                assert_eq!(
                    requests[0].headers[header],
                    if header == "authorization" {
                        "Bearer secret"
                    } else {
                        "secret"
                    }
                );
            } else {
                assert!(requests[0].headers.is_empty());
            }
            if kind == Anthropic {
                assert_eq!(requests[0].headers["anthropic-version"], "2023-06-01");
            }
            if kind == Google {
                assert_eq!(models[1].id, "two");
                assert!(requests[1].url.contains("pageToken=a+%2B%2F"));
            }
        }
    }
    #[tokio::test]
    async fn actionable_errors_do_not_echo_response_secrets() {
        let keys = Keys::default();
        keys.set("test", "secret").unwrap();
        for (status, expected) in [
            (401, "API key rejected"),
            (
                404,
                "This endpoint doesn't list models; add model IDs manually",
            ),
        ] {
            assert_eq!(
                fetch(
                    &provider(ProviderKind::Openai),
                    &keys,
                    &http(vec![Ok((status, json!({"error":"secret"})))])
                )
                .await
                .unwrap_err(),
                expected
            );
        }
        let error = "Is the server running at http://localhost:1234/v1?";
        assert_eq!(
            fetch(
                &provider(ProviderKind::Openai),
                &keys,
                &http(vec![Err(error.into())])
            )
            .await
            .unwrap_err(),
            error
        );
    }
    #[test]
    fn merge_keeps_flags_manual_models_and_delete_removes_key() {
        let mut p = provider(ProviderKind::Openai);
        p.models[0].enabled = false;
        let mut fetched = p.models[0].clone();
        fetched.enabled = true;
        fetched.display_name = Some("New".into());
        let mut manual = fetched.clone();
        manual.id = "manual".into();
        p.models.push(manual);
        merge_models(&mut p.models, vec![fetched]);
        assert!(!p.models[0].enabled);
        assert_eq!(p.models.len(), 2);
        assert_eq!(p.models[0].display_name.as_deref(), Some("New"));
        let keys = Keys::default();
        keys.set("test", "secret").unwrap();
        delete_removed_keys(&[p], &[], &keys).unwrap();
        assert!(keys.get("test").unwrap().is_none());
    }
    #[test]
    fn policy_merge_and_validation() {
        let p = provider(ProviderKind::Openai);
        let over: AgentPolicyOverride = serde_json::from_value(
            json!({"defaultEngine":"codex","subagents":{"maxConcurrent":7}}),
        )
        .unwrap();
        let mut policy = effective_policy(&AgentPolicy::default(), Some(&over));
        assert_eq!(policy.default_engine, EngineKind::Codex);
        assert_eq!(policy.subagents.max_concurrent, 7);
        assert_eq!(policy.subagents.max_per_turn, 8);
        assert_eq!(policy.subagents.roles.len(), 3);
        assert!(validate(&[p.clone()], &policy).is_ok());
        policy.subagents.max_depth = 4;
        assert!(validate(&[p.clone()], &policy).is_err());
        policy.subagents.max_depth = 1;
        policy.subagents.roles[0].model = Some(ModelRef {
            provider_id: "disabled".into(),
            model_id: "model".into(),
        });
        assert!(validate(&[p], &policy)
            .unwrap_err()
            .contains("Enable provider"));
    }
    #[test]
    fn registry_only_catalog_and_codex_config_are_secret_free() {
        let mut p = provider(ProviderKind::Openai);
        p.name = "Name \"quoted\"\nline".into();
        let mut policy = AgentPolicy::default();
        policy.default_model = Some(ModelRef {
            provider_id: "test".into(),
            model_id: "model".into(),
        });
        let catalog = catalog(&[p.clone()]);
        assert_eq!(catalog["result"]["data"][0]["id"], "test/model");
        let (config, files) =
            crate::shared::engines::codex_config(&[p], &policy, Path::new("/managed")).unwrap();
        let parsed = config.parse::<toml_edit::Document>().unwrap();
        assert_eq!(
            parsed["model_providers"]["test"]["requires_openai_auth"].as_bool(),
            Some(false)
        );
        assert_eq!(
            parsed["cli_auth_credentials_store"].as_str(),
            Some("ephemeral")
        );
        assert_eq!(parsed["analytics"]["enabled"].as_bool(), Some(false));
        assert_eq!(parsed["apps"]["_default"]["enabled"].as_bool(), Some(false));
        assert_eq!(
            parsed["plugins"]["_default"]["enabled"].as_bool(),
            Some(false)
        );
        assert!(parsed.get("forced_login_method").is_none());
        assert!(parsed.get("chatgpt_base_url").is_none());
        assert!(config.contains("TAUCODE_PROVIDER_TEST_KEY"));
        assert!(!config.contains("super-secret"));
        assert_eq!(files.len(), 3);
        for (_, content) in files {
            assert!(content.parse::<toml_edit::Document>().is_ok());
        }
    }
    #[test]
    fn keychain_values_only_reach_child_environment() {
        let p = provider(ProviderKind::Openai);
        let keys = Keys::default();
        keys.set("test", "super-secret").unwrap();
        let home = std::env::temp_dir().join(format!("taucode-registry-{}", uuid::Uuid::new_v4()));
        let env = crate::shared::engines::prepare_codex(
            &home,
            &[p.clone()],
            &AgentPolicy::default(),
            &keys,
        )
        .unwrap();
        assert_eq!(
            env,
            vec![("TAUCODE_PROVIDER_TEST_KEY".into(), "super-secret".into())]
        );
        let args = crate::shared::engines::codex_config_args(&[p], &AgentPolicy::default(), &home)
            .unwrap();
        assert!(!args.join(" ").contains("super-secret"));
        assert!(!std::fs::read_to_string(home.join("config.toml"))
            .unwrap()
            .contains("super-secret"));
        assert!(!home.join("auth.json").exists());
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn codex_capability_and_auth_mapping() {
        for kind in [
            ProviderKind::Openai,
            ProviderKind::Anthropic,
            ProviderKind::Google,
            ProviderKind::Openrouter,
            ProviderKind::Groq,
            ProviderKind::Mistral,
            ProviderKind::Xai,
            ProviderKind::Deepseek,
            ProviderKind::Zai,
            ProviderKind::Ollama,
            ProviderKind::Lmstudio,
            ProviderKind::OpenaiCompatible,
        ] {
            let mut p = provider(kind);
            let expected = matches!(
                kind,
                ProviderKind::Openai | ProviderKind::Openrouter | ProviderKind::Ollama
            );
            assert_eq!(codex_usable(&p), expected);
            if kind == ProviderKind::OpenaiCompatible {
                p.api_style = Some(ApiStyle::Responses);
                assert!(codex_usable(&p));
            }
        }
        for auth in [
            ProviderAuth::None,
            ProviderAuth::Env {
                var: "MY_KEY".into(),
            },
            ProviderAuth::Command {
                command: "token-helper".into(),
                args: vec!["a \"quoted\" argument".into()],
                timeout_ms: None,
            },
        ] {
            let mut p = provider(ProviderKind::Openai);
            p.auth = auth;
            let (config, _) = crate::shared::engines::codex_config(
                &[p],
                &AgentPolicy::default(),
                Path::new("/managed"),
            )
            .unwrap();
            let parsed = config.parse::<toml_edit::Document>().unwrap();
            assert_eq!(
                parsed["model_providers"]["test"]["requires_openai_auth"].as_bool(),
                Some(false)
            );
            assert!(!config.contains("super-secret"));
        }
    }
    use std::path::Path;
}
