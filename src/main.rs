use rust_email_newsletter::configuration::*;
use rust_email_newsletter::startup::run;
use rust_email_newsletter::telemetry::init_subscriber;
use sqlx::PgPool;
use std::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_subscriber("email_newsletter", "info", std::io::stdout);

    let configuration = get_configuration().expect("Failed to read configuration");
    let db_pool = PgPool::connect_lazy_with(configuration.database.with_db());
    let address = format!("{}:{}", configuration.application.host, configuration.application.port);
    run(TcpListener::bind(address).unwrap(), db_pool)?.await
}
