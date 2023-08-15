use rust_email_newsletter::configuration::*;
use rust_email_newsletter::startup::run;
use rust_email_newsletter::telemetry::init_subscriber;
use sqlx::PgPool;
use std::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_subscriber("email_newsletter", "info", std::io::stdout);

    let configuration = get_configuration().expect("Failed to read configuration");
    let db_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect Postgres");
    run(TcpListener::bind("127.0.0.1:8080").unwrap(), db_pool)?.await
}
