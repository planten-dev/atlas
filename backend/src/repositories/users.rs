use crate::entities::users::{ActiveModel, Column, Entity as User, Model, UserStatus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, Order,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, Set, Unchanged,
};
use uuid::Uuid;

pub struct UserRepository;

pub struct UserListFilter {
    pub statuses: Vec<UserStatus>,
    pub keyword: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UserSortBy {
    CreatedAt,
    UpdatedAt,
    Username,
    Phone,
    Status,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UserSortOrder {
    Asc,
    Desc,
}

impl UserRepository {
    #[tracing::instrument(name = "users.repository.create", skip(db, user))]
    pub async fn create(db: &DatabaseConnection, user: ActiveModel) -> Result<Model, DbErr> {
        tracing::debug!("inserting user");
        let result = user.insert(db).await;

        if let Ok(user) = &result {
            tracing::debug!(user_id = %user.id, "user inserted");
        }

        result
    }

    #[tracing::instrument(name = "users.repository.find_by_id", skip(db), fields(user_id = %id))]
    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Model>, DbErr> {
        let result = User::find_by_id(id).one(db).await;
        log_lookup_result(&result);

        result
    }

    #[tracing::instrument(
        name = "users.repository.find_by_username",
        skip(db, username),
        fields(username_len = username.chars().count())
    )]
    pub async fn find_by_username(
        db: &DatabaseConnection,
        username: &str,
    ) -> Result<Option<Model>, DbErr> {
        let result = User::find()
            .filter(Column::Username.eq(username))
            .one(db)
            .await;
        log_lookup_result(&result);

        result
    }

    #[tracing::instrument(
        name = "users.repository.find_by_phone",
        skip(db, phone),
        fields(phone_len = phone.chars().count())
    )]
    pub async fn find_by_phone(
        db: &DatabaseConnection,
        phone: &str,
    ) -> Result<Option<Model>, DbErr> {
        let result = User::find().filter(Column::Phone.eq(phone)).one(db).await;
        log_lookup_result(&result);

        result
    }

    #[tracing::instrument(
        name = "users.repository.find_by_login_identifier",
        skip(db, identifier),
        fields(identifier_kind = %login_identifier_kind(identifier), identifier_len = identifier.chars().count())
    )]
    pub async fn find_by_login_identifier(
        db: &DatabaseConnection,
        identifier: &str,
    ) -> Result<Option<Model>, DbErr> {
        let result = User::find()
            .filter(
                Condition::any()
                    .add(Column::Username.eq(identifier))
                    .add(Column::Phone.eq(identifier)),
            )
            .one(db)
            .await;
        log_lookup_result(&result);

        result
    }

    #[tracing::instrument(
        name = "users.repository.list",
        skip(db, filter),
        fields(
            status_count = filter.statuses.len(),
            has_keyword = filter.keyword.is_some(),
            offset,
            limit,
            sort_by = ?sort_by,
            sort_order = ?sort_order
        )
    )]
    pub async fn list(
        db: &DatabaseConnection,
        filter: &UserListFilter,
        sort_by: UserSortBy,
        sort_order: UserSortOrder,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        let result = apply_sort(apply_filter(User::find(), filter), sort_by, sort_order)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await;

        match &result {
            Ok(users) => tracing::debug!(count = users.len(), "user list loaded"),
            Err(err) => tracing::error!(error = %err, "user list failed"),
        }

        result
    }

    #[tracing::instrument(
        name = "users.repository.count",
        skip(db, filter),
        fields(status_count = filter.statuses.len(), has_keyword = filter.keyword.is_some())
    )]
    pub async fn count(db: &DatabaseConnection, filter: &UserListFilter) -> Result<u64, DbErr> {
        let result = apply_filter(User::find(), filter).count(db).await;

        match &result {
            Ok(total) => tracing::debug!(total, "user count loaded"),
            Err(err) => tracing::error!(error = %err, "user count failed"),
        }

        result
    }

    #[tracing::instrument(
        name = "users.repository.update_profile",
        skip(db, username, phone),
        fields(user_id = %id, has_username = username.is_some(), has_phone = phone.is_some())
    )]
    pub async fn update_profile(
        db: &DatabaseConnection,
        id: Uuid,
        username: Option<String>,
        phone: Option<String>,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Model, DbErr> {
        let mut user = ActiveModel {
            id: Unchanged(id),
            updated_at: Set(updated_at),
            ..Default::default()
        };

        if let Some(username) = username {
            user.username = Set(username);
        }

        if let Some(phone) = phone {
            user.phone = Set(phone);
        }

        let result = user.update(db).await;
        log_update_result(&result);

        result
    }

    #[tracing::instrument(name = "users.repository.update_status", skip(db), fields(user_id = %id, status = ?status))]
    pub async fn update_status(
        db: &DatabaseConnection,
        id: Uuid,
        status: crate::entities::users::UserStatus,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Model, DbErr> {
        let user = ActiveModel {
            id: Unchanged(id),
            status: Set(status),
            updated_at: Set(updated_at),
            ..Default::default()
        };

        let result = user.update(db).await;
        log_update_result(&result);

        result
    }

    #[tracing::instrument(name = "users.repository.update_password_hash", skip(db, password_hash), fields(user_id = %id))]
    pub async fn update_password_hash(
        db: &DatabaseConnection,
        id: Uuid,
        password_hash: String,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Model, DbErr> {
        let user = ActiveModel {
            id: Unchanged(id),
            password_hash: Set(password_hash),
            updated_at: Set(updated_at),
            ..Default::default()
        };

        let result = user.update(db).await;
        log_update_result(&result);

        result
    }
}

fn apply_filter(mut query: Select<User>, filter: &UserListFilter) -> Select<User> {
    if !filter.statuses.is_empty() {
        let mut condition = Condition::any();
        for status in &filter.statuses {
            condition = condition.add(Column::Status.eq(status.clone()));
        }
        query = query.filter(condition);
    }

    if let Some(keyword) = &filter.keyword {
        query = query.filter(
            Condition::any()
                .add(Column::Username.contains(keyword.as_str()))
                .add(Column::Phone.contains(keyword.as_str())),
        );
    }

    query
}

fn apply_sort(query: Select<User>, sort_by: UserSortBy, sort_order: UserSortOrder) -> Select<User> {
    let column = match sort_by {
        UserSortBy::CreatedAt => Column::CreatedAt,
        UserSortBy::UpdatedAt => Column::UpdatedAt,
        UserSortBy::Username => Column::Username,
        UserSortBy::Phone => Column::Phone,
        UserSortBy::Status => Column::Status,
    };
    let order = match sort_order {
        UserSortOrder::Asc => Order::Asc,
        UserSortOrder::Desc => Order::Desc,
    };

    query.order_by(column, order)
}

fn log_lookup_result(result: &Result<Option<Model>, DbErr>) {
    match result {
        Ok(Some(user)) => {
            tracing::debug!(found = true, user_id = %user.id, "user lookup completed")
        }
        Ok(None) => tracing::debug!(found = false, "user lookup completed"),
        Err(err) => tracing::error!(error = %err, "user lookup failed"),
    }
}

fn login_identifier_kind(identifier: &str) -> &'static str {
    if identifier
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '+')
    {
        "phone"
    } else {
        "username"
    }
}

fn log_update_result(result: &Result<Model, DbErr>) {
    match result {
        Ok(user) => tracing::debug!(user_id = %user.id, "user update completed"),
        Err(err) => tracing::error!(error = %err, "user update failed"),
    }
}
