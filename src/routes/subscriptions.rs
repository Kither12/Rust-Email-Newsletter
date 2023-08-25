use crate::domain::{NewSubscriber, SubscriberName};
use actix_web::{web, HttpResponse, Responder};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, db_pool),
    fields(subscriber_email=%form.name,
           subscriber_name=%form.name)
)]
pub async fn subscribe(form: web::Form<FormData>, db_pool: web::Data<PgPool>) -> impl Responder {
    let subscriber_name = match SubscriberName::parse(form.0.name) {
        Ok(name) => name,
        Err(_) => return HttpResponse::BadRequest(),
    };

    let new_subscriber = NewSubscriber {
        email: form.0.email,
        name: subscriber_name,
    };
    match insert_subscriber(&new_subscriber, &db_pool).await {
        Ok(_) => HttpResponse::Ok(),
        Err(_) => HttpResponse::InternalServerError(),
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(subscriber, db_pool)
)]
async fn insert_subscriber(
    subscriber: &NewSubscriber,
    db_pool: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at) 
                    VALUES ($1, $2, $3, $4)"#,
        uuid::Uuid::new_v4(),
        subscriber.email,
        subscriber.name.as_ref(),
        chrono::Utc::now()
    )
    .execute(db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
