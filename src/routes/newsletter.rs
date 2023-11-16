use actix_web::{web, HttpResponse, Responder};
use lettre::Address;
use sqlx::PgPool;

use crate::{
    domain::{Subscriber, SubscriberName},
    email_client::EmailClient,
};

struct Row {
    email: String,
    name: String,
}
impl TryInto<Subscriber> for Row {
    type Error = String;
    fn try_into(self) -> Result<Subscriber, Self::Error> {
        let email = self.email.parse::<Address>().map_err(|x| format!("{x}"))?;
        let name = SubscriberName::parse(self.name)?;
        Ok(Subscriber { email, name })
    }
}
#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(pool: &PgPool) -> Result<Vec<Subscriber>, sqlx::Error> {
    let rows: Vec<Row> = sqlx::query_as!(
        Row,
        r#"
            SELECT email, name
            FROM subscriptions
            WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get all confirmed subscriber: {}", e);
        e
    })?;
    let confirmed_subscriber: Vec<Subscriber> = rows.into_iter().filter_map(|item| {
        let x: Result<Subscriber, _> = item.try_into();
        match x {
            Ok(subscriber) => Some(subscriber),
            Err(_) => None,
        }
    }).collect();
    Ok(confirmed_subscriber)
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    subject: String,
    content: String,
}

#[tracing::instrument(name = "Publish a newsletter", skip(body, pool, email_client))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> impl Responder {
    let subscribers = match get_confirmed_subscribers(&pool).await {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::InternalServerError(),
    };
    for subscriber in subscribers {
        email_client
            .send_email(
                subscriber.name.as_ref().to_owned(),
                subscriber.email,
                &body.subject,
                &body.content,
            )
            .await
            .expect("Failed to send email");
    }
    HttpResponse::Ok()
}
