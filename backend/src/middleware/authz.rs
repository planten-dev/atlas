use std::{future::Future, pin::Pin};

use axum::{
    extract::{Request, State},
    middleware::{FromFnLayer, Next, from_fn_with_state},
    response::Response,
};
use tracing::{debug, warn};

use crate::{
    handlers::error::{authz_error_response, permission_denied_response},
    services::{auth::CurrentSession, authz::AuthzError},
    state::AppState,
};

const VALID_ACTIONS: [&str; 4] = ["read", "write", "post", "approve"];

/// State carried by one permission-checking layer instance: the parsed
/// object/action this specific route requires.
#[derive(Clone)]
pub struct PermissionCheck {
    state: AppState,
    object: &'static str,
    action: &'static str,
}

type EnforceFuture = Pin<Box<dyn Future<Output = Response> + Send>>;
type EnforceFn = fn(State<PermissionCheck>, Request, Next) -> EnforceFuture;

/// The concrete layer type returned by [`require_permission`], nameable so
/// it can be used with `Handler::layer` at route-declaration sites.
pub type PermissionLayer =
    FromFnLayer<EnforceFn, PermissionCheck, (State<PermissionCheck>, Request)>;

/// Builds a per-route permission layer from a `scope1:scope2:...:action`
/// permission string. Must run inside a router group protected by
/// `require_auth`, which inserts `CurrentSession` into request extensions.
///
/// Panics on a malformed permission string or one missing from the
/// permission catalog: this runs at router build time, so a bad
/// declaration fails startup (same failure mode as an invalid axum route
/// path). The catalog check keeps route declarations and the catalog —
/// which the permission panel and effective-permission queries enumerate —
/// from drifting apart.
pub fn require_permission(state: &AppState, permission: &'static str) -> PermissionLayer {
    let (object, action) = parse_permission(permission)
        .unwrap_or_else(|reason| panic!("invalid route permission `{permission}`: {reason}"));
    if !state.authz.catalog().contains(object, action) {
        panic!(
            "route permission `{permission}` is not in the permission catalog; \
             add it to PermissionCatalog::builtin"
        );
    }

    from_fn_with_state(
        PermissionCheck {
            state: state.clone(),
            object,
            action,
        },
        enforce_permission_boxed as EnforceFn,
    )
}

fn enforce_permission_boxed(
    check: State<PermissionCheck>,
    request: Request,
    next: Next,
) -> EnforceFuture {
    Box::pin(enforce_permission(check, request, next))
}

async fn enforce_permission(
    State(check): State<PermissionCheck>,
    request: Request,
    next: Next,
) -> Response {
    let Some(session) = request.extensions().get::<CurrentSession>() else {
        warn!(
            object = check.object,
            action = check.action,
            "permission check reached without an authenticated session; route is likely missing require_auth"
        );
        return authz_error_response(AuthzError::MissingSession);
    };
    let user_id = session.user.id;

    match check
        .state
        .authz
        .check(user_id, check.object, check.action)
        .await
    {
        Ok(true) => {
            debug!(
                user_id = %user_id,
                object = check.object,
                action = check.action,
                "permission granted"
            );
            next.run(request).await
        }
        Ok(false) => {
            warn!(
                user_id = %user_id,
                object = check.object,
                action = check.action,
                "permission denied"
            );
            permission_denied_response(check.object, check.action)
        }
        Err(error) => authz_error_response(error),
    }
}

/// Splits `scope1:...:action` at the last `:`. The action must be one of
/// read/write/post/approve, every scope segment must be non-empty, and
/// wildcards are not allowed at declaration sites. Shared with the review
/// framework, which validates `ReviewableResource::APPROVAL_PERMISSION`
/// against the same grammar at registration time.
pub(crate) fn parse_permission(
    permission: &'static str,
) -> Result<(&'static str, &'static str), String> {
    let (object, action) = permission
        .rsplit_once(':')
        .ok_or_else(|| "expected at least `scope:action`".to_string())?;

    if !VALID_ACTIONS.contains(&action) {
        return Err(format!(
            "action `{action}` must be one of {VALID_ACTIONS:?}"
        ));
    }
    if object.split(':').any(|segment| segment.is_empty()) {
        return Err("object segments must not be empty".to_string());
    }
    if object.split(':').any(|segment| segment.contains('*')) {
        return Err("wildcards are not allowed in route permissions".to_string());
    }

    Ok((object, action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_permissions() {
        assert_eq!(
            parse_permission("system:permissions:read").expect("should parse"),
            ("system:permissions", "read")
        );
        assert_eq!(
            parse_permission("finance:invoices:q3:approve").expect("should parse"),
            ("finance:invoices:q3", "approve")
        );
        assert_eq!(
            parse_permission("sales:performance:post").expect("should parse"),
            ("sales:performance", "post")
        );
        assert_eq!(
            parse_permission("health:write").expect("should parse"),
            ("health", "write")
        );
    }

    #[test]
    fn rejects_invalid_action() {
        assert!(parse_permission("system:permissions:delete").is_err());
        assert!(parse_permission("system:permissions:*").is_err());
    }

    #[test]
    fn rejects_missing_separator() {
        assert!(parse_permission("read").is_err());
    }

    #[test]
    fn rejects_empty_segments() {
        assert!(parse_permission(":read").is_err());
        assert!(parse_permission("system::read").is_err());
    }

    #[test]
    fn rejects_wildcard_object() {
        assert!(parse_permission("system:*:read").is_err());
        assert!(parse_permission("*:read").is_err());
    }
}
