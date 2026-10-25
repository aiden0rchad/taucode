use crate::{shared::{engine_registry, providers}, types::*};
use super::config;
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path};

#[derive(Clone)]
pub(crate) struct EngineProfile { pub definition: EngineDefinition }
impl Default for EngineProfile {
    fn default() -> Self { Self { definition: engine_registry::registry(&AppSettings::default()).remove(0) } }
}
impl EngineProfile {
    pub fn id(&self) -> &str { &self.definition.id }
    pub fn omp(&self) -> bool { self.id() == "omp" }
    pub fn custom(&self) -> bool { !self.definition.built_in }
    pub fn prepare(&self, home: &Path, registry: &[ProviderEntry], features: &Features, env: &mut HashMap<String,String>) -> Result<(), String> {
        if let Some(key) = &self.definition.config_dir_env { env.insert(key.clone(), home.to_string_lossy().into()); }
        env.extend(self.definition.env.clone());
        if self.omp() {
            env.insert("PI_CONFIG_DIR".into(), home.to_string_lossy().into());
            env.insert("TAUCODE_OMP".into(), "1".into());
            env.insert("OTEL_SDK_DISABLED".into(), "true".into());
            // JSON is a YAML subset, avoiding a serializer dependency.
            config::write_json(&home.join("models.yml"), &omp_models(registry))?;
            config::write_json(&home.join("config.yml"), &json!({"startup":{"checkUpdate":false,"setupWizard":false},"marketplace":{"autoUpdate":false},"telemetry":{"otlpExportEnabled":false},"mcp":{"enableProjectConfig":false},"browser":{"enabled":false},"lsp":{"enabled":true,"lazy":true},"task":{"maxRecursionDepth":0}}))?;
            let origins: Vec<_> = registry.iter().filter(|p| p.enabled).filter_map(|p| reqwest::Url::parse(&providers::base_url(p)).ok().map(|url|url.origin().ascii_serialization())).collect();
            config::write_json(&home.join("taucode/network.json"), &json!(origins))?;
            // The gate uses the compatible tool_call API; only explicitly gated
            // tools are enabled. Native task/browser tools bypass taucode policy.
            let gate = include_str!("../../../../engines/pi/extensions/taucode-gate.ts");
            std::fs::write(home.join("extensions/taucode-gate.ts"), gate).map_err(|e| e.to_string())?;
        }
        env.insert("TAUCODE_FEATURES".into(), serde_json::to_string(features).map_err(|e|e.to_string())?);
        Ok(())
    }
}
pub(crate) fn omp_models(registry: &[ProviderEntry]) -> Value {
    let mut result = config::models(registry);
    for provider in registry.iter().filter(|p| p.enabled) {
        let entry = &mut result["providers"][config::provider_id(provider, registry)];
        entry.as_object_mut().unwrap().remove("name");
        entry["baseUrl"] = json!(providers::base_url(provider));
        entry["api"] = json!(match provider.kind {
            ProviderKind::Anthropic => "anthropic-messages", ProviderKind::Google => "google-generative-ai",
            ProviderKind::Openai => "openai-responses", _ if provider.api_style == Some(ApiStyle::Responses) => "openai-responses", _ => "openai-completions",
        });
        if matches!(provider.auth, ProviderAuth::None) { entry["auth"] = json!("none"); entry.as_object_mut().unwrap().remove("apiKey"); }
        else if let Some(key) = entry["apiKey"].as_str() { entry["apiKey"] = json!(key.trim_start_matches('$')); }
        for model in entry["models"].as_array_mut().unwrap() {
            model.as_object_mut().unwrap().remove("thinkingLevelMap");
            if let Some(cap) = provider.models.iter().find(|m| model["id"] == m.id).and_then(|m| m.reasoning.as_ref()).filter(|cap| cap.supported) {
                let efforts: Vec<_> = cap.levels.iter().filter(|level| level.as_str() != "off").collect();
                if !efforts.is_empty() { model["thinking"] = json!({"mode": match cap.control.as_str() { "adaptive" => "anthropic-adaptive", "budget" => "budget", _ => "effort" }, "efforts": efforts}); }
            }
        }
    }
    result
}
