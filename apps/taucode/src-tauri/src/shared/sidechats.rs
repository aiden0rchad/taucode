use super::engines::WorkspaceSession;
use serde_json::{json, Value};

pub(crate) fn metadata_path(
    session: &WorkspaceSession,
    workspace: &str,
    id: &str,
) -> Result<std::path::PathBuf, String> {
    for part in [workspace, id] {
        if part.is_empty()
            || !part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid thread metadata ID".into());
        }
    }
    Ok(session
        .pi
        .data
        .as_ref()
        .ok_or("Thread metadata storage unavailable")?
        .join("thread-metadata")
        .join(workspace)
        .join(format!("{id}.json")))
}

pub(crate) fn enrich(
    session: &WorkspaceSession,
    workspace: &str,
    response: &mut Value,
) -> Result<(), String> {
    if session.pi.data.is_none() {
        return Ok(());
    }
    fn attach(
        session: &WorkspaceSession,
        workspace: &str,
        thread: &mut Value,
    ) -> Result<(), String> {
        if let Some(id) = thread["id"].as_str() {
            let path = metadata_path(session, workspace, id)?;
            let metadata = crate::storage::read_thread_metadata(&path)?;
            if metadata.get("sideOf").is_some() {
                thread["sideOf"] = metadata["sideOf"].clone();
            }
        }
        Ok(())
    }
    if let Some(thread) = response.pointer_mut("/result/thread") {
        attach(session, workspace, thread)?;
    }
    if let Some(data) = response
        .pointer_mut("/result/data")
        .and_then(Value::as_array_mut)
    {
        for thread in data {
            attach(session, workspace, thread)?;
        }
    }
    Ok(())
}

pub(crate) async fn create(
    session: &WorkspaceSession,
    workspace: &str,
    params: Value,
) -> Result<Value, String> {
    let parent = params["threadId"].as_str().ok_or("Missing parent thread")?;
    if !params["atItemId"].is_null() && !params["atItemId"].is_string() {
        return Err("Invalid message ID".into());
    }
    let side_of = json!({"threadId":parent,"workspaceId":workspace,"atItemId":params["atItemId"]});
    // Pi copies the saved branch without switching or stopping the parent's RPC process.
    // Codex currently has no safe item-level fork contract in this adapter.
    if !session.pi.owns(workspace, parent).await && params["atItemId"].is_string() {
        return Err("Codex side chats can currently fork only at the latest point".into());
    }
    let mut response = Box::pin(session.send_request_for_workspace(
        workspace,
        "thread/fork",
        json!({
            "threadId":parent,"excludeTurns":true,"sideOf":side_of,"atItemId":params["atItemId"],
            "sandbox":"read-only","approvalPolicy":"on-request"
        }),
    ))
    .await?;
    persist(session, workspace, &mut response, side_of)?;
    Ok(response)
}

pub(crate) fn persist(
    session: &WorkspaceSession,
    workspace: &str,
    response: &mut Value,
    side_of: Value,
) -> Result<(), String> {
    let thread = response
        .pointer_mut("/result/thread")
        .ok_or("Fork returned no thread")?;
    let id = thread["id"]
        .as_str()
        .ok_or("Fork returned no ID")?
        .to_owned();
    let path = metadata_path(session, workspace, &id)?;
    crate::storage::write_thread_metadata(
        &path,
        &json!({"sideOf":side_of,"accessMode":"read-only"}),
    )?;
    thread["sideOf"] = side_of;
    session.pi.emit(
        workspace,
        json!({"method":"thread/started","params":{"thread":thread}}),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn side_links_are_restored_on_reads_and_lists_after_restart() {
        let data = std::env::temp_dir().join(format!("sidechat-{}", uuid::Uuid::new_v4()));
        let session = || {
            let mut s = WorkspaceSession::lazy(
                Box::new(|| Box::pin(async { Err("not used".into()) })),
                vec![],
                Default::default(),
                None,
                None,
                "ws".into(),
            );
            s.pi = super::super::pi::PiEngine::new(
                data.clone(),
                "ws".into(),
                "/project".into(),
                None,
                Arc::new(|_, _| {}),
            );
            s
        };
        let link = json!({"threadId":"main","workspaceId":"ws","atItemId":"answer"});
        let mut response = json!({"result":{"thread":{"id":"side"}}});
        persist(&session(), "ws", &mut response, link.clone()).unwrap();
        let restarted = session();
        let mut read = json!({"result":{"thread":{"id":"side"}}});
        enrich(&restarted, "ws", &mut read).unwrap();
        assert_eq!(read["result"]["thread"]["sideOf"], link);
        let mut list = json!({"result":{"data":[{"id":"side"},{"id":"main"}]}});
        enrich(&restarted, "ws", &mut list).unwrap();
        assert_eq!(list["result"]["data"][0]["sideOf"], link);
        assert!(list["result"]["data"][1].get("sideOf").is_none());
        let saved =
            crate::storage::read_thread_metadata(&metadata_path(&restarted, "ws", "side").unwrap())
                .unwrap();
        assert_eq!(saved["accessMode"], "read-only");
        assert!(metadata_path(&restarted, "../outside", "side").is_err());
        let _ = std::fs::remove_dir_all(data);
    }
}
