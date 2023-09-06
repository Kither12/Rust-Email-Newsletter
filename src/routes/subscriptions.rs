use crate::{domain::Subscriber, email_client::EmailClient};
use actix_web::{web, HttpResponse, Responder};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, db_pool, email_client),
    fields(subscriber_email=%form.name,
           subscriber_name=%form.name)
)]
pub async fn subscribe(form: web::Form<FormData>, db_pool: web::Data<PgPool>, email_client: web::Data<EmailClient>) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(err) => {
            tracing::error!("Failed to parsing form data: {}", err);
            return HttpResponse::BadRequest();
        }
    };

    let _ = match insert_subscriber(&new_subscriber, &db_pool).await {
        Ok(_) => (),
        Err(err) => {
            tracing::error!("Failed to insert subscriber: {}", err);
            return HttpResponse::InternalServerError();
        }
    };
    
    let _ = match email_client.send_confirmation(&new_subscriber).await{
        Ok(_) =>(),
        Err(err) => {
            println!("Failed to sending email: {}", err);
            return HttpResponse::InternalServerError();
        }
    };
    return HttpResponse::Ok();
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(subscriber, db_pool)
)]
async fn insert_subscriber(
    subscriber: &Subscriber,
    db_pool: &PgPool,
) -> Result<(), sqlx::Error> {
    let subscriber_mail: &str = subscriber.email.as_ref();
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at) 
                    VALUES ($1, $2, $3, $4)"#,
        uuid::Uuid::new_v4(),
        subscriber_mail,
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
