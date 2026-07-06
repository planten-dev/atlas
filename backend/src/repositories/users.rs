use crate::entities::users::{ActiveModel, Column, Entity as User, Model};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
};
use uuid::Uuid;

pub struct UserRepository;

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
