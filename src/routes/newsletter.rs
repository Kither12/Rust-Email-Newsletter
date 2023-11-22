use actix_web::http::header;
use actix_web::http::header::HeaderValue;
use actix_web::HttpResponseBuilder;
use actix_web::{http::header::HeaderMap, web, HttpRequest, HttpResponse, Responder};
use anyhow::Context;
use argon2::password_hash::PasswordVerifier;
use argon2::{Argon2, PasswordHash};
use base64::{engine::general_purpose, Engine as _};
use lettre::Address;
use secrecy::ExposeSecret;
use secrecy::Secret;
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
    let confirmed_subscriber: Vec<Subscriber> = rows
        .into_iter()
        .filter_map(|item| {
            let x: Result<Subscriber, _> = item.try_into();
            match x {
                Ok(subscriber) => Some(subscriber),
                Err(_) => None,
            }
        })
        .collect();
    Ok(confirmed_subscriber)
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    subject: String,
    content: String,
}

fn get_unauthorized_response() -> HttpResponseBuilder {
    let mut response = HttpResponse::Unauthorized();
    let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
    response.insert_header((header::WWW_AUTHENTICATE, header_value));
    response
}

#[tracing::instrument(name = "Publish a newsletter", skip(body, pool, email_client), fields(username=tracing::field::Empty, user_id=tracing::field::Empty))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> impl Responder {
    let credentials = match basic_authentication(request.headers()) {
        Ok(credentials) => credentials,
        Err(e) => {
            tracing::error!("Failed to authorize: {}", e);
            return get_unauthorized_response();
        }
    };
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));

    let user_id = match validate_credentials(credentials, &pool).await {
        Ok(user_id) => user_id,
        Err(http_response) => {
            tracing::error!("Failed to validate credentials");
            return http_response;
        }
    };
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

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

struct Credentials {
    username: String,
    password: Secret<String>,
}
fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string")?;
    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;
    let decoded_bytes = general_purpose::STANDARD
        .decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();
    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}
#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, HttpResponseBuilder> {
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );
    let mut user_id = None;
    if let Some((stored_user_id, strored_expected_password_hash)) =
        get_stored_credentials(&credentials.username, pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get stored credentials: {}", e);
                HttpResponse::InternalServerError()
            })?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = strored_expected_password_hash;
    }
    tokio::task::spawn_blocking(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .map_err(|e| {
        tracing::error!("Failed to start blocking task: {}", e);
        HttpResponse::InternalServerError()
    })?
    .map_err(|e| {
        tracing::error!("Failed to validate password: {}", e);
        get_unauthorized_response()
    })?;
    user_id.ok_or_else(|| get_unauthorized_response())
}
#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
            SELECT user_id, password_hash
            FROM users
            WHERE username = $1
        "#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials.")?
    .map(|row| (row.user_id, Secret::new(row.password_hash)));
    Ok(row)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), argon2::password_hash::Error> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())?;
    Argon2::default().verify_password(
        password_candidate.expose_secret().as_bytes(),
        &expected_password_hash,
    )
}
