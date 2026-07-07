use tower::util::ServiceExt;
use axum::{body::Body, http::{Request, StatusCode}};

#[tokio::test]
async fn health_is_open_and_rpc_needs_token() {
    let app = donna_server::build_app(donna_server::test_state());
    let res = app.clone().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let res = app.clone().oneshot(Request::post("/rpc/list_conversations").body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = app.oneshot(Request::post("/rpc/list_conversations")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn rpc_conversation_roundtrip() {
    let app = donna_server::build_app(donna_server::test_state());

    // A real dispatch: create_conversation actually inserts a row and returns its id.
    let res = app.clone().oneshot(Request::post("/rpc/create_conversation")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let id: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // ops::create_conversation returns i64 -> a JSON number.
    assert!(id.is_i64(), "expected a numeric conversation id, got {id}");

    // The row must really exist: list_conversations returns it.
    let res = app.clone().oneshot(Request::post("/rpc/list_conversations")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let convs: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(convs.as_array().map(|a| a.len()), Some(1));

    // Unknown command -> 404.
    let res = app.oneshot(Request::post("/rpc/nonexistent_command")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rpc_trust_policies_roundtrip() {
    let app = donna_server::build_app(donna_server::test_state());

    let res = app.clone().oneshot(Request::post("/rpc/trust_policy_set")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"actionKind":"slack_send_message","mode":"auto"}"#)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app.clone().oneshot(Request::post("/rpc/trust_policies_list")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from("{}")).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let rows: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let slack_mode = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["action_kind"] == "slack_send_message")
        .and_then(|r| r["mode"].as_str())
        .unwrap();
    assert_eq!(slack_mode, "auto");

    // Invalid tool -> 400.
    let res = app.oneshot(Request::post("/rpc/trust_policy_set")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"actionKind":"not_a_tool","mode":"auto"}"#)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rpc_suggestions_and_events_roundtrip() {
    let app = donna_server::build_app(donna_server::test_state());

    // recent_events: read-only, starts empty.
    let res = app.clone().oneshot(Request::post("/rpc/recent_events")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"limit":10}"#)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let events: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(events.as_array().map(|a| a.len()), Some(0));

    // suggestions_list: pendingOnly arm smoke test.
    let res = app.oneshot(Request::post("/rpc/suggestions_list")
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"pendingOnly":true}"#)).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let suggestions: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(suggestions.as_array().map(|a| a.len()), Some(0));
}

#[tokio::test]
async fn ws_token_query_rejects_prefix_and_wrong_param_name() {
    let app = donna_server::build_app(donna_server::test_state());

    // Real token as a *prefix* of the value must NOT authenticate (old bug: substring match).
    let res = app.clone().oneshot(Request::get("/ws?token=test-tokenXYZ").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // A differently-named param ending in "token" must NOT authenticate (old bug: substring match).
    let res = app.clone().oneshot(Request::get("/ws?other_token=test-token").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // The exact token as the exact `token` param must authenticate.
    let res = app.clone().oneshot(Request::get("/ws?token=test-token").body(Body::empty()).unwrap()).await.unwrap();
    assert_ne!(res.status(), StatusCode::UNAUTHORIZED);

    // No token at all must NOT authenticate.
    let res = app.oneshot(Request::get("/ws").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
