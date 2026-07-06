use axum::{extract::State, http::{Request, StatusCode}, middleware::Next, response::Response};
use crate::state::AppState;

pub async fn require_bearer(State(st): State<AppState>, req: Request<axum::body::Body>, next: Next)
    -> Result<Response, StatusCode> {
    let ok = req.headers().get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == format!("Bearer {}", st.token))
        .unwrap_or(false)
        // WS can't set headers from the browser: accept ?token= on /ws only.
        // Parsed as real query params (not substring search) so neither a
        // token-prefixed value nor a differently-named param can match.
        || (req.uri().path() == "/ws"
            && url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
                .any(|(k, v)| k == "token" && v == st.token));
    if ok { Ok(next.run(req).await) } else { Err(StatusCode::UNAUTHORIZED) }
}
