use crate::types::{ProviderKind, ReasoningCapability};
use serde_json::{json, Value};

pub(crate) const LEVELS: [&str; 7] = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
pub(crate) fn capability(supported: bool, levels: &[&str], control: &str, source: &str) -> ReasoningCapability {
    let levels: Vec<String> = LEVELS.iter().filter(|l| supported && levels.contains(l)).map(|s| (*s).into()).collect();
    ReasoningCapability {
        supported, default: levels.iter().find(|s| *s == "medium").or(levels.first()).cloned(), levels,
        control: control.into(), source: source.into(),
        detected_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    }
}
fn strings(value: &Value) -> Vec<&str> {
    value.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| if s == "none" { "off" } else { s }).collect()
}
pub(crate) fn litellm(info: &Value) -> Option<ReasoningCapability> {
    let efforts = strings(&info["supported_reasoning_efforts"]);
    let params = strings(&info["supported_openai_params"]);
    let adaptive = info["supports_adaptive_thinking"] == true;
    let supported = info["supports_reasoning"].as_bool().or_else(|| (!efforts.is_empty() || adaptive).then_some(true))?;
    let control = if adaptive && efforts.is_empty() { "adaptive" } else if params.contains(&"thinking") { "thinking-toggle" } else if params.contains(&"reasoning_effort") { "effort" } else { "unknown" };
    let levels = if !efforts.is_empty() { efforts } else if adaptive { vec!["low", "medium", "high"] } else if control == "thinking-toggle" { vec!["off", "high"] } else { vec![] };
    Some(capability(supported, &levels, control, "api"))
}
pub(crate) fn detect(kind: ProviderKind, id: &str, entry: &Value) -> Option<ReasoningCapability> {
    use ProviderKind::*;
    match kind {
        Openrouter => {
            let params = entry["supported_parameters"].as_array()?;
            let supported = params.iter().any(|p| p == "reasoning" || p == "include_reasoning");
            let mut levels = vec!["off", "low", "medium", "high"];
            levels.extend(strings(&entry["supported_reasoning_efforts"]));
            Some(capability(supported, &levels, "effort", "api"))
        }
        Ollama => {
            let caps = entry["capabilities"].as_array()?;
            let supported = caps.iter().any(|c| c == "thinking");
            let oss = id.starts_with("gpt-oss");
            Some(capability(supported, if oss { &["low", "medium", "high"] } else { &["off", "high"] }, if oss { "effort" } else { "thinking-toggle" }, "api"))
        }
        Google => entry["thinking"].as_bool().map(|supported| capability(supported, &["off", "low", "medium", "high"], "budget", "api")),
        Openai | Anthropic => heuristic(kind, id),
        _ => None,
    }
}
// Conservative family fallbacks for official APIs that omit capability metadata.
// Unknown families stay unknown; API metadata always takes precedence.
pub(crate) fn heuristic(kind: ProviderKind, id: &str) -> Option<ReasoningCapability> {
    use ProviderKind::*;
    let id = id.to_lowercase();
    let (levels, control): (&[&str], &str) = match kind {
        Openai if id.starts_with("gpt-5") || id.starts_with("gpt-6") => (&["off", "minimal", "low", "medium", "high", "xhigh"], "effort"),
        Openai if ["o1", "o3", "o4"].iter().any(|p| id.starts_with(p)) => (&["low", "medium", "high"], "effort"),
        Openai if id.starts_with("gpt-4") || id.starts_with("gpt-3.5") => (&[], "unknown"),
        Anthropic if id.contains("opus-4-6") || id.contains("sonnet-4-6") || ["sonnet-5", "opus-5", "haiku-5"].iter().any(|family| id.contains(family)) => (&["low", "medium", "high"], "adaptive"),
        Anthropic if id.contains("-4") || id.contains("3-7") => (&["off", "low", "medium", "high"], "budget"),
        Anthropic if id.contains("3-5") || id.contains("claude-3-") => (&[], "unknown"),
        _ => return None,
    };
    Some(capability(!levels.is_empty(), levels, control, "heuristic"))
}
pub(crate) fn clamp(cap: &ReasoningCapability, requested: Option<&str>) -> Option<String> {
    if !cap.supported { return None; }
    let requested = requested.or(cap.default.as_deref()).unwrap_or("medium");
    let index = LEVELS.iter().position(|s| *s == requested).unwrap_or(3);
    cap.levels.iter().filter_map(|s| LEVELS.iter().position(|l| l == s).map(|i| (i.abs_diff(index), s))).min_by_key(|(distance, _)| *distance).map(|(_, s)| s.clone())
}
pub(crate) fn pi_config(model: &mut Value, cap: &ReasoningCapability, kind: ProviderKind) {
    model["reasoning"] = json!(cap.supported);
    model["thinkingLevelMap"] = Value::Object(LEVELS.iter().map(|l| ((*l).into(), if cap.supported && cap.levels.iter().any(|s| s == l) { json!(if *l == "off" { "none" } else { l }) } else { Value::Null })).collect());
    let format = if kind == ProviderKind::Openrouter { Some("openrouter") }
        else if cap.control == "thinking-toggle" && kind != ProviderKind::Ollama { Some("zai") }
        else if matches!(cap.control.as_str(), "effort" | "adaptive") && !matches!(kind, ProviderKind::Anthropic | ProviderKind::Google | ProviderKind::Ollama) { Some("openai") } else { None };
    if let Some(format) = format { model["compat"] = json!({"thinkingFormat":format}); }
}
pub(crate) fn classify_probe(status: u16, body: &Value, param: &str) -> Option<bool> {
    match status {
        200..=299 => Some(true),
        400 if body.to_string().contains(param) => Some(false),
        _ => None,
    }
}
pub(crate) fn acp(session: &Value) -> Option<(String, ReasoningCapability)> {
    let option = session["configOptions"].as_array()?.iter().find(|o| o["category"] == "thought_level" || o["id"] == "effort")?;
    let levels: Vec<&str> = option["options"].as_array()?.iter().flat_map(|o| {
        if let Some(group) = o["options"].as_array() { group.iter().filter_map(|v| v["value"].as_str()).collect() }
        else { o["value"].as_str().into_iter().collect::<Vec<_>>() }
    }).filter(|s| LEVELS.contains(s)).collect();
    if levels.is_empty() { return None; }
    let mut cap = capability(true, &levels, "effort", "api");
    if let Some(current) = option["currentValue"].as_str().filter(|s| levels.contains(s)) { cap.default = Some(current.into()); }
    Some((option["id"].as_str()?.into(), cap))
}

pub(crate) async fn catalog(providers: &[crate::types::ProviderEntry], sessions: &tokio::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<super::engines::WorkspaceSession>>>, workspace: &str, thread: Option<&str>) -> Value {
    let mut catalog = super::providers::catalog(providers);
    let session = sessions.lock().await.get(workspace).cloned();
    if let (Some(session), Some(thread)) = (session, thread) {
        if let Some((provider, model, cap)) = session.claude.reasoning_options(thread).await {
            for entry in catalog["result"]["data"].as_array_mut().unwrap() {
                if entry["providerId"] == provider && entry["modelId"] == model { entry["acpReasoning"] = json!(cap); }
            }
        }
    }
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProviderEntry, ProviderModel};
    #[test]
    fn litellm_response_shape_and_level_order() {
        let response = json!({"data":[
            {"model_name":"GPT 6 Astra","model_info":{"supports_reasoning":true,"supported_reasoning_efforts":["low","medium","high","xhigh","max"],"supported_openai_params":["reasoning_effort"]}},
            {"model_name":"GLM 5.3","model_info":{"supports_reasoning":true,"supported_reasoning_efforts":["low","high","max"],"supported_openai_params":["thinking"]}},
            {"model_name":"Claude Sonnet 5.5","model_info":{"supports_reasoning":true,"supports_adaptive_thinking":true,"supported_openai_params":["reasoning_effort"]}}
        ]});
        let caps: Vec<_> = response["data"].as_array().unwrap().iter().map(|e| litellm(&e["model_info"]).unwrap()).collect();
        assert_eq!(caps[0].levels, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(caps[1].levels, ["low", "high", "max"]);
        assert_eq!(caps[1].control, "thinking-toggle");
        assert_eq!(caps[2].control, "adaptive");
        assert_eq!(caps[2].levels, ["low", "medium", "high"]);
        assert_eq!(litellm(&json!({"supports_reasoning":true,"supported_reasoning_efforts":["max","none","low","low","unsupported"]})).unwrap().levels, ["off", "low", "max"]);
        assert!(!litellm(&json!({"supports_reasoning":false})).unwrap().supported);
        assert!(litellm(&json!({})).is_none());
    }
    #[test]
    fn api_parsers_heuristics_and_probe_classification() {
        use ProviderKind::*;
        let router = detect(Openrouter, "test", &json!({"supported_parameters":["reasoning"],"supported_reasoning_efforts":["minimal","xhigh"]})).unwrap();
        assert_eq!(router.levels, ["off", "minimal", "low", "medium", "high", "xhigh"]);
        assert!(!detect(Openrouter, "test", &json!({"supported_parameters":[]})).unwrap().supported);
        assert_eq!(detect(Ollama, "gpt-oss:20b", &json!({"capabilities":["thinking"]})).unwrap().levels, ["low", "medium", "high"]);
        assert_eq!(detect(Ollama, "qwen", &json!({"capabilities":["thinking"]})).unwrap().levels, ["off", "high"]);
        assert_eq!(detect(Google, "gemini", &json!({"thinking":true})).unwrap().control, "budget");
        assert!(!detect(Google, "gemini", &json!({"thinking":false})).unwrap().supported);
        for (kind, id, supported, control) in [(Openai,"o3",true,"effort"),(Openai,"gpt-5",true,"effort"),(Openai,"gpt-4o",false,"unknown"),(Anthropic,"claude-sonnet-4-6",true,"adaptive"),(Anthropic,"claude-sonnet-4",true,"budget"),(Anthropic,"claude-3-5-haiku",false,"unknown")] {
            let cap = heuristic(kind, id).unwrap(); assert_eq!(cap.supported, supported); assert_eq!(cap.control, control); assert_eq!(cap.source, "heuristic");
        }
        assert!(heuristic(Openai, "new-family").is_none());
        assert_eq!(classify_probe(200, &Value::Null, "reasoning_effort"), Some(true));
        assert_eq!(classify_probe(400, &json!({"error":"unsupported reasoning_effort"}), "reasoning_effort"), Some(false));
        for status in [400, 401, 429, 500] { assert_eq!(classify_probe(status, &json!({"error":"other"}), "reasoning_effort"), None); }
    }
    #[test]
    fn migration_override_clamp_and_pi_configuration() {
        let old: ProviderModel = serde_json::from_value(json!({"id":"model","enabled":true})).unwrap();
        assert!(old.reasoning.is_none());
        let cap = capability(true, &["low", "high", "max"], "thinking-toggle", "api");
        assert_eq!(clamp(&cap, Some("medium")).as_deref(), Some("low"));
        assert_eq!(clamp(&cap, Some("xhigh")).as_deref(), Some("high"));
        let mut provider: ProviderEntry = serde_json::from_value(json!({"id":"lab","name":"Lab","kind":"openai-compatible","baseUrl":"https://lab.test/v1","enabled":true,"models":[{"id":"model","enabled":true,"reasoning":cap}]})).unwrap();
        let config = crate::shared::pi::config::models(&[provider.clone()]);
        let model = &config["providers"]["taucode-lab"]["models"][0];
        assert_eq!(model["reasoning"], true);
        assert_eq!(model["compat"]["thinkingFormat"], "zai");
        assert_eq!(model["thinkingLevelMap"], json!({"off":null,"minimal":null,"low":"low","medium":null,"high":"high","xhigh":null,"max":"max"}));
        provider.models[0].reasoning = Some(capability(true, &["high"], "effort", "user"));
        crate::shared::providers::merge_models(&mut provider.models, vec![ProviderModel { reasoning: Some(cap.clone()), ..old }]);
        assert_eq!(provider.models[0].reasoning.as_ref().unwrap().source, "user");
        assert_eq!(provider.models[0].detected_reasoning, Some(cap));
        let mut model = json!({});
        pi_config(&mut model, &capability(true, &["off","low"], "effort", "api"), ProviderKind::Openrouter);
        assert_eq!(model["compat"]["thinkingFormat"], "openrouter");
        assert_eq!(model["thinkingLevelMap"]["off"], "none");
        pi_config(&mut model, &capability(true, &["low"], "effort", "api"), ProviderKind::OpenaiCompatible);
        assert_eq!(model["compat"]["thinkingFormat"], "openai");
    }
    #[test]
    fn acp_uses_advertised_session_options() {
        assert!(acp(&json!({})).is_none());
        let (id, cap) = acp(&json!({"configOptions":[{"id":"effort","category":"thought_level","options":[{"value":"default"},{"value":"low"},{"value":"high"}],"currentValue":"high"}]})).unwrap();
        assert_eq!(id,"effort"); assert_eq!(cap.levels,["low","high"]); assert_eq!(cap.default.as_deref(),Some("high"));
    }
}
