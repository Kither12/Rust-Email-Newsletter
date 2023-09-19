use crate::{domain::Subscriber, email_client::EmailClient, startup::ApplicationBaseUrl};
use actix_web::{web, HttpResponse, Responder};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::StatusCode;
use sqlx::PgPool;
use sqlx::{Executor, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, db_pool, email_client, base_url),
    fields(subscriber_email=%form.name,
           subscriber_name=%form.name)
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => {
            return HttpResponse::with_body(StatusCode::BAD_REQUEST, String::default());
        }
    };

    let mut transaction = match db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => {
            return HttpResponse::with_body(StatusCode::INTERNAL_SERVER_ERROR, String::default())
        }
    };

    let subscriber_id = match insert_subscriber(&new_subscriber, &mut transaction).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => {
            return HttpResponse::with_body(StatusCode::INTERNAL_SERVER_ERROR, String::default())
        }
    };

    let subscription_token = generate_subscription_token();

    if store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .is_err()
    {
        return HttpResponse::with_body(StatusCode::INTERNAL_SERVER_ERROR, String::default());
    };

    if email_client
        .send_confirmation(&new_subscriber, &base_url.0, &subscription_token)
        .await
        .is_err()
    {
        return HttpResponse::with_body(StatusCode::INTERNAL_SERVER_ERROR, String::default());
    };

    if transaction.commit().await.is_err(){
        return HttpResponse::with_body(StatusCode::INTERNAL_SERVER_ERROR, String::default());
    }

    HttpResponse::with_body(StatusCode::OK, subscription_token)
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(subscriber, transaction)
)]
async fn insert_subscriber(
    subscriber: &Subscriber,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_mail: &str = subscriber.email.as_ref();
    let subscriber_id = uuid::Uuid::new_v4();
    let query = sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at, status) 
                    VALUES ($1, $2, $3, $4, $5)"#,
        subscriber_id,
        subscriber_mail,
        subscriber.name.as_ref(),
        chrono::Utc::now(),
        "confirmed",
    );
    transaction.execute(query).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Storing token into database",
    skip(transaction, subscription_token)
)]
async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );
    transaction.execute(query).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
