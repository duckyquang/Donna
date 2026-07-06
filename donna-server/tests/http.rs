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
