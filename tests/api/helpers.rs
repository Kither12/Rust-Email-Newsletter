use rust_email_newsletter::configuration::*;
use rust_email_newsletter::startup::Application;
use rust_email_newsletter::telemetry::init_subscriber;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::collections::HashSet;
use std::sync::{Arc, Once, OnceLock, RwLock};
use crate::smtp_sever::*;

static INIT_SUBSCRIBER: Once = Once::new();
static OPEN_SMTP_SEVER: Once = Once::new();
static STORAGE: OnceLock<Arc<RwLock<HashSet<MailMessage>>>> = OnceLock::new();

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub storage: Arc<RwLock<HashSet<MailMessage>>>,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: &'static str) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
    pub async fn post_newsletter(&self, body_json: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletter", &self.address))
            .json(&body_json)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

pub async fn spawn_app() -> TestApp {
    if std::env::var("TEST_LOG").is_ok() {
        INIT_SUBSCRIBER.call_once(|| init_subscriber("email_newsletter", "error", std::io::stdout));
    } else {
        INIT_SUBSCRIBER.call_once(|| init_subscriber("email_newsletter", "error", std::io::stdout));
    }

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.database_name = uuid::Uuid::new_v4().to_string();
        c.application.port = 0;
        c
    };
    let storage = STORAGE.get_or_init(|| Arc::new(RwLock::new(HashSet::default())));

    OPEN_SMTP_SEVER.call_once(|| {
        open_smtp_sever(
            (
                configuration.smtp_sever.smtp_host,
                configuration.smtp_sever.smtp_port,
            ),
            storage.clone(),
        )
        .expect("Failed to start SMTP sever");
    });

    let db_pool = config_database(&configuration.database).await;
    let application = Application::build(&configuration)
        .await
        .expect("Failed to build application");
    let address = format!("http://127.0.0.1:{}", application.port());
    tokio::spawn(application.run_until_stopped());
    TestApp { address, db_pool, storage: storage.clone() }
}

async fn config_database(config: &DatabaseSettings) -> PgPool {
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
