use axum::{
    extract::{Request, State},
    http::{HeaderMap, header},
    middleware::Next,
    response::Response,
};
use tracing::debug;

use crate::{
    handlers::{auth::session_cookie, error::auth_error_response},
    state::AppState,
};

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
            let renewal = current_session.renewal.clone();
            debug!(
                user_id = %current_session.user.id,
                session_id = %current_session.session_id,
                "authenticated request"
            );
            request.extensions_mut().insert(current_session);
            let mut response = next.run(request).await;
            if let (Some(session_token), Some(renewal)) = (session_token.as_deref(), renewal)
                && response.status().is_success()
                && !response.headers().contains_key(header::SET_COOKIE)
            {
                let cookie = session_cookie(
                    session_token,
                    renewal.max_age_seconds,
                    state.session_config.cookie_secure,
                );
                response.headers_mut().insert(header::SET_COOKIE, cookie);
            }
            response
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
