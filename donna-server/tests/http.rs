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
