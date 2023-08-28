use rust_email_newsletter::configuration::*;
use rust_email_newsletter::telemetry::init_subscriber;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::{net::TcpListener, sync::Once};
use test_case::test_case;

static INIT_SUBSCRIBER: Once = Once::new();

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

async fn spawn_app() -> TestApp {
    if std::env::var("TEST_LOG").is_ok(){
        INIT_SUBSCRIBER.call_once(|| init_subscriber("email_newsletter", "info", std::io::stdout));
    }
    else{
        INIT_SUBSCRIBER.call_once(|| init_subscriber("email_newsletter", "info", std::io::sink));
    }

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.database.database_name = uuid::Uuid::new_v4().to_string();
    let db_pool = config_database(&configuration.database).await;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let server = rust_email_newsletter::startup::run(listener, db_pool.clone())
        .expect("Failed to bind address");
    tokio::spawn(server);
    let address = format!("http://127.0.0.1:{}", port);
    TestApp { address, db_pool }
}

async fn config_database(config: &DatabaseSettings) -> PgPool{
    PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres")
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    let db_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate the database");

    db_pool
}

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health_check", app.address))
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, response.status().as_u16());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn check_form_data_valid() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let body = "name=kither%20god&email=kither_123%40gmail.com";

    let response = client
        .post(format!("{}/subscriptions", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.name, "kither god");
    assert_eq!(saved.email, "kither_123@gmail.com");
}

#[test_case("name=kither%20god"; "form_data_missing_email")]
#[test_case("email=kither_123%40gmail.com"; "form_data_missing_name")]
#[test_case(""; "missing_both_email_and_name")]
#[tokio::test]
async fn check_form_data_unvalid(body: &'static str) {
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/subscriptions", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(400, response.status().as_u16());
}

#[test_case("name=&email=kither_123%40gmail.com"; "name_is_empty")]
#[test_case("name=sadfsdf&email="; "email_is_empty")]
#[tokio::test]
async fn check_form_present_but_invalid(body: &'static str){
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/subscriptions", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(400, response.status().as_u16());
}
