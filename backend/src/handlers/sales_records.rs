use crate::{
    dto::{auth::ErrorResponse, sales_records::*},
    repositories::RepositoryError,
    services::{auth::CurrentSession, sales_records::SalesRecordError},
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

pub async fn list_sales_records(
    State(s): State<AppState>,
    q: Result<Query<ListSalesRecordsQuery>, QueryRejection>,
) -> Response {
    match q {
        Ok(Query(q)) => result(s.sales_records.list_sales_records(q).await),
        Err(e) => bad("invalid query parameters", e),
    }
}
pub async fn sales_record_detail(
    State(s): State<AppState>,
    p: Result<Path<Uuid>, PathRejection>,
) -> Response {
    match p {
        Ok(Path(id)) => result(s.sales_records.sales_record_detail(id).await),
        Err(e) => bad("invalid sales_record_id", e),
    }
}
pub async fn create_deal_record(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    r: Result<Json<CreateDealRecordRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = r else {
        return bad("invalid request body", r.unwrap_err());
    };
    let expert = r.expert_user_id;
    submit_create(
        &s,
        session.user.id,
        s.sales_records
            .prepare_deal_review(session.user.id, r)
            .await,
        expert,
    )
    .await
}
pub async fn create_pre_service_record(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    r: Result<Json<CreatePreServiceRecordRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = r else {
        return bad("invalid request body", r.unwrap_err());
    };
    let expert = r.expert_user_id;
    submit_create(
        &s,
        session.user.id,
        s.sales_records
            .prepare_pre_service_review(session.user.id, r)
            .await,
        expert,
    )
    .await
}
pub async fn create_debt_collection_record(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    r: Result<Json<CreateDebtCollectionRecordRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = r else {
        return bad("invalid request body", r.unwrap_err());
    };
    let expert = r.expert_user_id;
    submit_create(
        &s,
        session.user.id,
        s.sales_records
            .prepare_debt_collection_review(session.user.id, r)
            .await,
        expert,
    )
    .await
}
async fn submit_create(
    s: &AppState,
    actor: Uuid,
    doc: Result<crate::services::sales_records::SalesRecordReviewDoc, SalesRecordError>,
    expert: Option<Uuid>,
) -> Response {
    match doc {
        Ok(doc) => match s
            .events
            .submit_create(actor, &doc, approval_count(expert), approvers(expert))
            .await
        {
            Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Err(e) => crate::handlers::events::event_error_response(e),
        },
        Err(e) => error(e),
    }
}
pub async fn void_sales_record(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    p: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let Ok(Path(id)) = p else {
        return bad("invalid sales_record_id", p.unwrap_err());
    };
    match s.sales_records.prepare_void_record_review(id).await {
        Ok((old, new, expert)) => match s
            .events
            .submit_update(
                session.user.id,
                id,
                &old,
                &new,
                approval_count(expert),
                approvers(expert),
            )
            .await
        {
            Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Err(e) => crate::handlers::events::event_error_response(e),
        },
        Err(e) => error(e),
    }
}
pub async fn list_operation_counts(
    State(s): State<AppState>,
    q: Result<Query<ListOperationCountsQuery>, QueryRejection>,
) -> Response {
    match q {
        Ok(Query(q)) => result(s.sales_records.list_operation_counts(q).await),
        Err(e) => bad("invalid query parameters", e),
    }
}
pub async fn operation_count_detail(
    State(s): State<AppState>,
    p: Result<Path<Uuid>, PathRejection>,
) -> Response {
    match p {
        Ok(Path(id)) => result(s.sales_records.operation_count_detail(id).await),
        Err(e) => bad("invalid line id", e),
    }
}
pub async fn update_operation_count(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    p: Result<Path<Uuid>, PathRejection>,
    r: Result<Json<UpdateOperationCountRequest>, JsonRejection>,
) -> Response {
    let (Ok(Path(id)), Ok(Json(r))) = (p, r) else {
        return bad("invalid request", "invalid path or body");
    };
    match s.sales_records.prepare_count_review(id, r).await {
        Ok((record_id, old, new, expert)) => match s
            .events
            .submit_update(
                session.user.id,
                record_id,
                &old,
                &new,
                approval_count(expert),
                approvers(expert),
            )
            .await
        {
            Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Err(e) => crate::handlers::events::event_error_response(e),
        },
        Err(e) => error(e),
    }
}
pub async fn list_operation_usages(
    State(s): State<AppState>,
    q: Result<Query<ListOperationUsagesQuery>, QueryRejection>,
) -> Response {
    match q {
        Ok(Query(q)) => result(s.sales_records.list_operation_usages(q).await),
        Err(e) => bad("invalid query parameters", e),
    }
}
pub async fn operation_usage_detail(
    State(s): State<AppState>,
    p: Result<Path<Uuid>, PathRejection>,
) -> Response {
    match p {
        Ok(Path(id)) => result(s.sales_records.operation_usage_detail(id).await),
        Err(e) => bad("invalid usage id", e),
    }
}
pub async fn create_operation_usage(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    r: Result<Json<CreateOperationUsageRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = r else {
        return bad("invalid request body", r.unwrap_err());
    };
    match s.sales_records.prepare_create_usage_review(r).await {
        Ok((doc, expert)) => match s
            .events
            .submit_create(
                session.user.id,
                &doc,
                approval_count(expert),
                approvers(expert),
            )
            .await
        {
            Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Err(e) => crate::handlers::events::event_error_response(e),
        },
        Err(e) => error(e),
    }
}
pub async fn update_operation_usage(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    p: Result<Path<Uuid>, PathRejection>,
    r: Result<Json<UpdateOperationUsageRequest>, JsonRejection>,
) -> Response {
    let (Ok(Path(id)), Ok(Json(r))) = (p, r) else {
        return bad("invalid request", "invalid path or body");
    };
    submit_usage_update(
        &s,
        session.user.id,
        id,
        s.sales_records.prepare_update_usage_review(id, r).await,
    )
    .await
}
pub async fn void_operation_usage(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    p: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let Ok(Path(id)) = p else {
        return bad("invalid usage id", p.unwrap_err());
    };
    submit_usage_update(
        &s,
        session.user.id,
        id,
        s.sales_records.prepare_void_usage_review(id).await,
    )
    .await
}
async fn submit_usage_update(
    s: &AppState,
    actor: Uuid,
    id: Uuid,
    v: Result<
        (
            crate::services::sales_records::SalesOperationUsageReviewDoc,
            crate::services::sales_records::SalesOperationUsageReviewDoc,
            Option<Uuid>,
        ),
        SalesRecordError,
    >,
) -> Response {
    match v {
        Ok((old, new, expert)) => match s
            .events
            .submit_update(
                actor,
                id,
                &old,
                &new,
                approval_count(expert),
                approvers(expert),
            )
            .await
        {
            Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Err(e) => crate::handlers::events::event_error_response(e),
        },
        Err(e) => error(e),
    }
}
pub async fn delete_operation_usage(
    State(s): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    p: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let Ok(Path(id)) = p else {
        return bad("invalid usage id", p.unwrap_err());
    };
    match s.sales_records.prepare_delete_usage_review(id).await {
        Ok((old, expert)) => match s
            .events
            .submit_delete(
                session.user.id,
                id,
                &old,
                approval_count(expert),
                approvers(expert),
            )
            .await
        {
            Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Err(e) => crate::handlers::events::event_error_response(e),
        },
        Err(e) => error(e),
    }
}
fn approval_count(e: Option<Uuid>) -> i16 {
    if e.is_some() { 2 } else { 1 }
}
fn approvers(e: Option<Uuid>) -> Vec<Uuid> {
    e.into_iter().collect()
}
fn result<T: serde::Serialize>(r: Result<T, SalesRecordError>) -> Response {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => error(e),
    }
}
fn error(e: SalesRecordError) -> Response {
    let status = match &e {
        SalesRecordError::NotFound
        | SalesRecordError::LineNotFound
        | SalesRecordError::CountNotFound
        | SalesRecordError::UsageNotFound
        | SalesRecordError::CustomerNotFound
        | SalesRecordError::ProductNotFound
        | SalesRecordError::CategoryNotFound
        | SalesRecordError::UserNotFound => StatusCode::NOT_FOUND,
        SalesRecordError::CollectionExceedsOutstanding
        | SalesRecordError::CountBelowUsed
        | SalesRecordError::Voided
        | SalesRecordError::UsageVoided => StatusCode::CONFLICT,
        SalesRecordError::Repository(RepositoryError::Database(_)) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(ErrorResponse {
            error: e.code().into(),
            message: e.to_string(),
        }),
    )
        .into_response()
}
fn bad(message: &str, detail: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".into(),
            message: format!("{message}: {detail}"),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app,
        config::{AuthConfig, DatabaseConfig, DatabaseKind, DingTalkConfig, SessionConfig},
        db,
        repositories::{
            authz::AuthzRepository,
            customers::{CustomerRepository, NewCustomer},
            departments::DepartmentRepository,
            events::EventRepository,
            product_categories::ProductCategoryRepository,
            products::{NewProduct, ProductRepository},
            sales_records::SalesRecordRepository,
            sessions::{SessionRepository, hash_secret},
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            user_profiles::UserProfileRepository,
            users::UserRepository,
        },
        services::{
            auth::AuthService,
            authz::AuthzService,
            authz_catalog::PermissionCatalog,
            customers::CustomerService,
            departments::DepartmentService,
            events::EventService,
            product_categories::ProductCategoryService,
            products::ProductService,
            review::ApplierRegistry,
            sales_records::{
                SalesOperationUsageReviewDoc, SalesRecordReviewDoc, SalesRecordService,
            },
            stores::StoreService,
            systems::SystemService,
            users::UserService,
        },
        state::{AppState, AppStateParts},
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, header},
    };
    use chrono::{Duration, Utc};
    use sea_orm::entity::prelude::Decimal;
    use serde_json::{Value, json};
    use std::{path::PathBuf, sync::Arc};
    use tower::ServiceExt;

    struct TestContext {
        app: Router,
        cookie: String,
        user_id: Uuid,
        customer_id: Uuid,
        product_id: Uuid,
    }

    #[tokio::test]
    async fn three_sales_record_endpoints_apply_through_http_approval() {
        let context = test_context().await;
        let common = json!({
            "customer_id": context.customer_id,
            "record_date": "2026-07-08",
            "handler_user_id": context.user_id
        });

        let deal_id = submit_and_approve(&context, "/api/v1/sales-records/create-deal", merge(common.clone(), json!({
            "total_amount": "300.00", "received_amount": "100.00",
            "customer_type": "new", "deal_type": "non_salon",
            "lines": [{"product_id": context.product_id, "item_name": "护理项目", "operation_total_count": 3}],
            "allocations": [{"guide_user_id": context.user_id, "allocation_ratio": "100.00"}]
        }))).await;
        submit_and_approve(&context, "/api/v1/sales-records/create-pre-service", merge(common.clone(), json!({
            "total_amount": "200.00",
            "lines": [{"product_id": context.product_id, "item_name": "护理项目", "operation_total_count": 2}]
        }))).await;
        submit_and_approve(&context, "/api/v1/sales-records/create-debt-collection", merge(common, json!({
            "received_amount": "150.00",
            "allocations": [{"guide_user_id": context.user_id, "allocation_ratio": "100.00"}]
        }))).await;

        let detail_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/sales-records/detail/{deal_id}"),
                &context.cookie,
                None,
            ))
            .await
            .expect("detail request should complete");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail = response_json(detail_response).await;
        assert_eq!(detail["record_type"], "deal");
        assert_eq!(detail["debt_change"], "200.00");
        assert_eq!(detail["lines"][0]["operation_count"]["total_count"], 3);

        let customer_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/customers/detail/{}", context.customer_id),
                &context.cookie,
                None,
            ))
            .await
            .expect("customer detail request should complete");
        assert_eq!(customer_response.status(), StatusCode::OK);
        assert_eq!(
            response_json(customer_response).await["outstanding_amount"],
            "250.00"
        );
    }

    async fn submit_and_approve(context: &TestContext, path: &str, body: Value) -> Uuid {
        let response = context
            .app
            .clone()
            .oneshot(request(Method::POST, path, &context.cookie, Some(body)))
            .await
            .expect("submit request should complete");
        let status = response.status();
        let submitted = response_json(response).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{submitted}");
        let event_id =
            Uuid::parse_str(submitted["id"].as_str().expect("event id should exist")).unwrap();
        let response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/approve/{event_id}"),
                &context.cookie,
                Some(json!({})),
            ))
            .await
            .expect("approval request should complete");
        let status = response.status();
        let approved = response_json(response).await;
        assert_eq!(status, StatusCode::OK, "{approved}");
        assert_eq!(approved["approval_status"], 2);
        Uuid::parse_str(
            approved["resource_id"]
                .as_str()
                .expect("resource id should exist"),
        )
        .unwrap()
    }

    async fn test_context() -> TestContext {
        let db = db::connect_and_migrate(&DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".into(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        })
        .await
        .expect("database should initialize");
        let users = UserRepository::new(db.clone());
        let profiles = UserProfileRepository::new(db.clone());
        let sessions = SessionRepository::new(db.clone());
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db.clone());
        let customers = CustomerRepository::new(db.clone());
        let categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db.clone());
        let user = users
            .find_or_create_for_login("sales-http-user", Utc::now())
            .await
            .expect("user should create");
        let token = "sales-http-session";
        sessions
            .create_session(
                user.id,
                &hash_secret(token),
                Utc::now(),
                Utc::now() + Duration::hours(1),
            )
            .await
            .expect("session should create");
        let system = systems
            .create_system(
                NewSystem {
                    name: "sales-http-system".into(),
                    status: "active".into(),
                },
                Utc::now(),
            )
            .await
            .expect("system should create");
        let store = stores
            .create_store(
                NewStore {
                    name: "sales-http-store".into(),
                    system_id: system.id,
                    status: "active".into(),
                },
                Utc::now(),
            )
            .await
            .expect("store should create");
        let customer = customers
            .create_customer(
                NewCustomer {
                    name: "sales-http-customer".into(),
                    creator_user_id: user.id,
                    system_id: system.id,
                    store_id: store.id,
                    remark: None,
                    status: "active".into(),
                    attachments: None,
                },
                Utc::now(),
            )
            .await
            .expect("customer should create");
        let category = categories
            .list_categories(Some("active"), Some(true), 1, 50)
            .await
            .unwrap()
            .0
            .into_iter()
            .next()
            .unwrap();
        let product = products
            .create_product(
                NewProduct {
                    name: "sales-http-product".into(),
                    category_id: category.id,
                    series: None,
                    brand_name: None,
                    specification: None,
                    unit: Some("次".into()),
                    unit_price: Decimal::new(10000, 2),
                    status: "active".into(),
                },
                Utc::now(),
            )
            .await
            .expect("product should create");
        let mut catalog = PermissionCatalog::builtin();
        catalog.add_permission("sales:records", "approve", "审核", "销售记录及可操作次数");
        let authz = AuthzService::with_catalog(AuthzRepository::new(db.clone()), catalog)
            .await
            .expect("authz should initialize");
        for (object, action) in [
            ("sales:records", "read"),
            ("sales:records", "write"),
            ("sales:records", "approve"),
            ("customers", "read"),
        ] {
            authz
                .create_policy(
                    "user".into(),
                    user.id,
                    object.into(),
                    action.into(),
                    "allow".into(),
                )
                .await
                .expect("policy should create");
        }

        let dingtalk = DingTalkConfig {
            client_id: "test".into(),
            client_secret: "test".into(),
            redirect_uri: "http://127.0.0.1/callback".into(),
            auth_url: "http://127.0.0.1/auth".into(),
            token_url: "http://127.0.0.1/token".into(),
            user_info_url: "http://127.0.0.1/me".into(),
            user_getuserinfo_url: "http://127.0.0.1/getuserinfo".into(),
            corp_token_url: "http://127.0.0.1/gettoken".into(),
            department_listsub_url: "http://127.0.0.1/listsub".into(),
            user_detail_url: "http://127.0.0.1/user-detail".into(),
            getbyunionid_url: "http://127.0.0.1/getbyunionid".into(),
            scope: "openid".into(),
            corp_id: String::new(),
            external_id_fields: vec!["userId".into()],
        };
        let auth = AuthService::new(
            dingtalk.clone(),
            users.clone(),
            profiles.clone(),
            EventRepository::new(db.clone()),
            sessions.clone(),
            3600,
        );
        let sales_service = SalesRecordService::new(
            SalesRecordRepository::new(db.clone()),
            customers.clone(),
            systems.clone(),
            stores.clone(),
            categories.clone(),
            users.clone(),
        );
        let mut registry = ApplierRegistry::new();
        registry.register::<SalesRecordReviewDoc>();
        registry.register::<SalesOperationUsageReviewDoc>();
        let state = AppState::new(AppStateParts {
            auth,
            authz: authz.clone(),
            users: UserService::new(users, profiles, sessions),
            product_categories: ProductCategoryService::new(categories.clone(), products.clone()),
            products: ProductService::new(products, categories),
            systems: SystemService::new(systems.clone(), stores.clone()),
            stores: StoreService::new(stores.clone(), systems),
            customers: CustomerService::new(
                customers,
                SystemRepository::new(db.clone()),
                StoreRepository::new(db.clone()),
            ),
            sales_records: sales_service,
            events: EventService::new(EventRepository::new(db), authz, Arc::new(registry), 180),
            departments: DepartmentService::new(dingtalk, departments),
            auth_config: AuthConfig {
                frontend_callback_url: String::new(),
            },
            session_config: SessionConfig {
                ttl_seconds: 3600,
                absolute_ttl_seconds: 7200,
                renew_before_seconds: 300,
                cookie_secure: false,
            },
        });
        TestContext {
            app: app::router(state),
            cookie: format!("atlas_session={token}"),
            user_id: user.id,
            customer_id: customer.id,
            product_id: product.id,
        }
    }

    fn merge(mut base: Value, extra: Value) -> Value {
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        base
    }

    fn request(method: Method, uri: &str, cookie: &str, body: Option<Value>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, cookie);
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        builder
            .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
            .unwrap()
    }

    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }
}
