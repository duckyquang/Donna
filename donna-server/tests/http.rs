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
