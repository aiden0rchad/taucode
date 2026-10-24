use super::*;
use serde::de::DeserializeOwned;

fn parse_input<T: DeserializeOwned>(params: &Value) -> Result<T, String> {
    let input_value = params
        .as_object()
        .and_then(|map| map.get("input"))
        .cloned()
        .ok_or_else(|| "missing `input`".to_string())?;
    serde_json::from_value(input_value).map_err(|err| err.to_string())
}

pub(super) async fn try_handle(
    state: &DaemonState,
    method: &str,
    params: &Value,
) -> Option<Result<Value, String>> {
    match method {
        "create_side_chat" | "get_subagent_transcript" => {
            let workspace = match parse_string(params, "workspaceId") {
                Ok(value) => value, Err(err) => return Some(Err(err)),
            };
            let thread = match parse_string(params, "threadId") {
                Ok(value) => value, Err(err) => return Some(Err(err)),
            };
            let rpc = if method == "create_side_chat" { "thread/sidechat" } else { "subagent/transcript" };
            Some(codex_core::thread_feature_core(&state.sessions, workspace, rpc,
                json!({"threadId":thread,"atItemId":params["atItemId"],"nodeId":params["nodeId"]})).await)
        }
        "connector_request" => {
            let input = params["params"].clone();
            let changed = matches!(input["action"].as_str(), Some("setSecret" | "deleteSecret"));
            let result = crate::shared::connectors::request(
                input,
                &state.app_settings,
                &state.workspaces,
                state.codegraph.setup.as_ref(),
                &crate::shared::providers::Keychain,
                &state.data_dir,
            )
            .await;
            if result.is_ok() && changed {
                for session in state.sessions.lock().await.values() {
                    session.credentials_changed().await;
                }
            }
            Some(result)
        }
        "get_codex_config_path" => {
            let path = match settings_core::get_codex_config_path_core() {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(Ok(Value::String(path)))
        }
        "get_config_model" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.get_config_model(workspace_id).await)
        }
        "start_thread" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let engine = match params
                .get("engine")
                .filter(|v| !v.is_null())
                .cloned()
                .map(serde_json::from_value)
                .transpose()
            {
                Ok(v) => v,
                Err(_) => return Some(Err("Unknown engine".into())),
            };
            Some(
                state
                    .start_thread(workspace_id, engine, parse_optional_string(params, "model"))
                    .await,
            )
        }
        "resume_thread" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.resume_thread(workspace_id, thread_id).await)
        }
        "read_thread" | "list_thread_history" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            if method == "list_thread_history" {
                Some(
                    codex_core::list_thread_history_core(
                        &state.sessions,
                        workspace_id,
                        thread_id,
                        parse_optional_string(params, "cursor"),
                    )
                    .await,
                )
            } else {
                Some(state.read_thread(workspace_id, thread_id).await)
            }
        }
        "thread_live_subscribe" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.thread_live_subscribe(workspace_id, thread_id).await)
        }
        "thread_live_unsubscribe" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.thread_live_unsubscribe(workspace_id, thread_id).await)
        }
        "fork_thread" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .fork_thread(
                        workspace_id,
                        thread_id,
                        parse_optional_string(params, "model"),
                    )
                    .await,
            )
        }
        "list_threads" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let cursor = parse_optional_string(params, "cursor");
            let limit = parse_optional_u32(params, "limit");
            let sort_key = parse_optional_string(params, "sortKey");
            Some(
                state
                    .list_threads(workspace_id, cursor, limit, sort_key)
                    .await,
            )
        }
        "list_mcp_server_status" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let cursor = parse_optional_string(params, "cursor");
            let limit = parse_optional_u32(params, "limit");
            Some(
                state
                    .list_mcp_server_status(workspace_id, cursor, limit)
                    .await,
            )
        }
        "archive_thread" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.archive_thread(workspace_id, thread_id).await)
        }
        "compact_thread" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.compact_thread(workspace_id, thread_id).await)
        }
        "set_thread_access_mode" => {
            let workspace = match parse_string(params, "workspaceId") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let thread = match parse_string(params, "threadId") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let mode = match parse_string(params, "accessMode") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(
                crate::shared::codex_core::set_thread_access_mode_core(
                    &state.sessions,
                    workspace,
                    thread,
                    mode,
                )
                .await,
            )
        }
        "set_thread_name" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let name = match parse_string(params, "name") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.set_thread_name(workspace_id, thread_id, name).await)
        }
        "send_user_message" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let text = match parse_string(params, "text") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let model = parse_optional_string(params, "model");
            let effort = parse_optional_string(params, "effort");
            let service_tier = parse_optional_nullable_string(params, "serviceTier");
            let access_mode = parse_optional_string(params, "accessMode");
            let images = parse_optional_string_array(params, "images");
            let collaboration_mode = parse_optional_value(params, "collaborationMode");
            Some(
                state
                    .send_user_message(
                        workspace_id,
                        thread_id,
                        text,
                        model,
                        effort,
                        service_tier,
                        access_mode,
                        images,
                        collaboration_mode,
                    )
                    .await,
            )
        }
        "turn_interrupt" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let turn_id = match parse_string(params, "turnId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.turn_interrupt(workspace_id, thread_id, turn_id).await)
        }
        "turn_steer" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let turn_id = match parse_string(params, "turnId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let text = match parse_string(params, "text") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let images = parse_optional_string_array(params, "images");
            Some(
                state
                    .turn_steer(workspace_id, thread_id, turn_id, text, images)
                    .await,
            )
        }
        "start_review" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = match parse_string(params, "threadId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let target = match params
                .as_object()
                .and_then(|map| map.get("target"))
                .cloned()
                .ok_or("missing `target`")
            {
                Ok(value) => value,
                Err(err) => return Some(Err(err.to_string())),
            };
            let delivery = parse_optional_string(params, "delivery");
            Some(
                state
                    .start_review(workspace_id, thread_id, target, delivery)
                    .await,
            )
        }
        "model_list" | "list_models" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.model_list(workspace_id).await)
        }
        "experimental_feature_list" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let cursor = parse_optional_string(params, "cursor");
            let limit = parse_optional_u32(params, "limit");
            Some(
                state
                    .experimental_feature_list(workspace_id, cursor, limit)
                    .await,
            )
        }
        "collaboration_mode_list" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.collaboration_mode_list(workspace_id).await)
        }
        "set_codex_feature_flag" => {
            let feature_key = match parse_string(params, "featureKey") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let enabled = match parse_optional_bool(params, "enabled") {
                Some(value) => value,
                None => return Some(Err("missing or invalid `enabled`".to_string())),
            };
            Some(
                state
                    .set_codex_feature_flag(feature_key, enabled)
                    .await
                    .map(|_| json!({ "ok": true })),
            )
        }
        "get_agents_settings" => Some(
            state
                .get_agents_settings()
                .await
                .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
        ),
        "set_agents_core_settings" => {
            let input = match parse_input::<agents_config_core::SetAgentsCoreInput>(params) {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .set_agents_core_settings(input)
                    .await
                    .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
            )
        }
        "create_agent" => {
            let input = match parse_input::<agents_config_core::CreateAgentInput>(params) {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .create_agent(input)
                    .await
                    .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
            )
        }
        "update_agent" => {
            let input = match parse_input::<agents_config_core::UpdateAgentInput>(params) {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .update_agent(input)
                    .await
                    .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
            )
        }
        "delete_agent" => {
            let input = match parse_input::<agents_config_core::DeleteAgentInput>(params) {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .delete_agent(input)
                    .await
                    .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
            )
        }
        "read_agent_config_toml" => {
            let agent_name = match parse_string(params, "agentName") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .read_agent_config_toml(agent_name)
                    .await
                    .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
            )
        }
        "write_agent_config_toml" => {
            let agent_name = match parse_string(params, "agentName") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let content = match parse_string(params, "content") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .write_agent_config_toml(agent_name, content)
                    .await
                    .map(|_| json!({ "ok": true })),
            )
        }
        "provider_test_and_fetch" => {
            let id = match parse_string(params, "id") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(
                crate::shared::providers::test_and_fetch(
                    id,
                    &state.app_settings,
                    &state.settings_path,
                )
                .await,
            )
        }
        "provider_set_api_key" | "provider_has_api_key" | "provider_delete_api_key" => {
            use crate::shared::providers::{KeyStore, Keychain};
            let id = match parse_string(params, "id") {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            if !state
                .app_settings
                .lock()
                .await
                .providers
                .iter()
                .any(|p| p.id == id)
            {
                return Some(Err("Provider not found".into()));
            }
            let result = match method {
                "provider_has_api_key" => Keychain.get(&id).map(|v| json!(v.is_some())),
                "provider_delete_api_key" => Keychain.delete(&id).map(|_| Value::Null),
                _ => {
                    let key = match parse_string(params, "key") {
                        Ok(v) => v,
                        Err(e) => return Some(Err(e)),
                    };
                    Keychain.set(&id, &key).map(|_| Value::Null)
                }
            };
            if result.is_ok() && method != "provider_has_api_key" {
                for session in state.sessions.lock().await.values() {
                    session.credentials_changed().await;
                }
            }
            Some(result)
        }
        "skills_list" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.skills_list(workspace_id).await)
        }
        "respond_to_server_request" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let map = match params.as_object().ok_or("missing requestId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err.to_string())),
            };
            let request_id = match map
                .get("requestId")
                .cloned()
                .filter(|value| value.is_number() || value.is_string())
                .ok_or("missing requestId")
            {
                Ok(value) => value,
                Err(err) => return Some(Err(err.to_string())),
            };
            let result = match map.get("result").cloned().ok_or("missing `result`") {
                Ok(value) => value,
                Err(err) => return Some(Err(err.to_string())),
            };
            Some(
                state
                    .respond_to_server_request(workspace_id, request_id, result)
                    .await,
            )
        }
        "remember_approval_rule" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let command = match parse_string_array(params, "command") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.remember_approval_rule(workspace_id, command).await)
        }
        "codex_doctor" => {
            let codex_bin = parse_optional_string(params, "codexBin");
            let codex_args = parse_optional_string(params, "codexArgs");
            Some(state.codex_doctor(codex_bin, codex_args).await)
        }
        "generate_run_metadata" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let prompt = match parse_string(params, "prompt") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(state.generate_run_metadata(workspace_id, prompt).await)
        }
        "generate_agent_description" => {
            let workspace_id = match parse_string(params, "workspaceId") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let description = match parse_string(params, "description") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            Some(
                state
                    .generate_agent_description(workspace_id, description)
                    .await
                    .and_then(|value| serde_json::to_value(value).map_err(|err| err.to_string())),
            )
        }
        _ => None,
    }
}
