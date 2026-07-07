use axum::{
    extract::{Request, State},
    http::{HeaderMap, header},
    middleware::Next,
    response::Response,
};
use tracing::debug;

use crate::{handlers::error::auth_error_response, state::AppState};

pub const SESSION_COOKIE_NAME: &str = "atlas_session";

pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let session_token = session_cookie_value(request.headers());

    match state
        .auth
        .authenticate_session(session_token.as_deref())
        .await
    {
        Ok(current_session) => {
            debug!(
                user_id = %current_session.user.id,
                session_id = %current_session.session_id,
                "authenticated request"
            );
            request.extensions_mut().insert(current_session);
            next.run(request).await
        }
        Err(error) => auth_error_response(error),
    }
}

fn session_cookie_value(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if name == SESSION_COOKIE_NAME && !value.trim().is_empty() {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}
