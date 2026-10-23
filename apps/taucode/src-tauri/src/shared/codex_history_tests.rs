use super::*;
use crate::backend::app_server::{
    protocol_tests::{make_session, RecordingSink},
    route_app_server_message,
};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::timeout;

#[test]
fn history_pages_use_cursors_order_items_and_avoid_deprecated_hydration() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
    let session = make_session();
    let stdout = session.child.lock().await.stdout.take().unwrap();
    let mock_session = session.clone();
    let server = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let exchanges = vec![
            ("thread/resume", json!({"excludeTurns":true}), json!({"thread":{"id":"thread","turns":[]}})),
            ("thread/turns/list", json!({"cursor":null,"sortDirection":"desc","itemsView":"notLoaded"}), json!({"data":[{"id":"new","rootTurnId":"root","status":"completed"},{"id":"old","status":"completed"}],"nextCursor":"older","backwardsCursor":"back"})),
            ("thread/items/list", json!({"turnId":"old","cursor":null,"sortDirection":"asc"}), json!({"data":[{"item":{"id":"a","type":"sleep","durationMs":1}}],"nextCursor":"items-2"})),
            ("thread/items/list", json!({"turnId":"old","cursor":"items-2"}), json!({"data":[{"item":{"id":"a","type":"sleep","durationMs":1}},{"item":{"id":"b","type":"contextCompaction"}}],"nextCursor":null})),
            ("thread/items/list", json!({"turnId":"new","cursor":null}), json!({"data":[{"item":{"id":"c","type":"agentMessage","text":"done"}}],"nextCursor":null})),
            ("thread/turns/list", json!({"cursor":"older","sortDirection":"desc"}), json!({"data":[],"nextCursor":null,"backwardsCursor":null})),
            ("thread/read", json!({}), json!({"thread":{"id":"thread","turns":[]}})),
            ("thread/turns/list", json!({"cursor":null}), json!({"data":[],"nextCursor":null})),
            ("thread/fork", json!({"excludeTurns":true}), json!({"thread":{"id":"fork","turns":[]}})),
        ];
        for (method, fields, result) in exchanges {
            let request: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(request["method"], method);
            assert_eq!(request["params"]["threadId"], "thread");
            assert!(request["params"].get("includeTurns").is_none());
            for (field, value) in fields.as_object().unwrap() {
                assert_eq!(&request["params"][field], value, "{method}: {field}");
            }
            route_app_server_message(&mock_session, "fixture", &RecordingSink::default(),
                json!({"id":request["id"], "result":result})).await;
        }
    });
    let sessions = Mutex::new(HashMap::from([("fixture".into(), session.clone())]));
    timeout(Duration::from_secs(5), async {
        let response = resume_thread_core(&sessions, "fixture".into(), "thread".into()).await.unwrap();
        let thread = &response["result"]["thread"];
        assert_eq!(thread["historyCursor"], "older");
        assert_eq!(thread["turns"][0]["id"], "old");
        assert_eq!(thread["turns"][0]["items"].as_array().unwrap().len(), 2);
        assert_eq!(thread["turns"][0]["items"][1]["id"], "b");
        assert_eq!(thread["turns"][1]["rootTurnId"], "root");
        assert_eq!(thread["turns"][1]["itemsView"], "full");
        let older = list_thread_history_core(&sessions, "fixture".into(), "thread".into(), Some("older".into())).await.unwrap();
        assert!(older["nextCursor"].is_null());
        read_thread_core(&sessions, "fixture".into(), "thread".into()).await.unwrap();
        fork_thread_core(&sessions, "fixture".into(), "thread".into(), None).await.unwrap();
        server.await.unwrap();
    }).await.unwrap();
    session.child.lock().await.kill().await.unwrap();
    });
}

#[test]
fn history_rpc_errors_are_not_empty_successful_pages() {
    assert!(
        history_result(json!({"error":{"code":-32600,"message":"invalid"}}))
            .unwrap_err()
            .contains("invalid")
    );
    assert!(history_result(json!({})).is_err());
}
