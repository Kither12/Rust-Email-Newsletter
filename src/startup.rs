use crate::configuration::Settings;
use crate::email_client::EmailClient;
use crate::routes::confirm;
use crate::routes::health_check;
use crate::routes::publish_newsletter;
use crate::routes::subscribe;
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}
impl Application {
    pub async fn build(configuration: &Settings) -> Result<Self, std::io::Error> {
        let db_pool = PgPool::connect_lazy_with(configuration.database.with_db());

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let mailer = match configuration.email_client.test_sever {
            false => EmailClient::get_gmail_mailer(
                &configuration.email_client.user_name,
                &configuration.email_client.password,
            ),
            true => EmailClient::get_test_mailer(
                &configuration.smtp_sever.smtp_host,
                &configuration.smtp_sever.smtp_port,
            ),
        };
        let email_client = EmailClient::new(
            &configuration.email_client.user_name,
            &configuration.email_client.user_mail,
            mailer,
        );
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            db_pool,
            email_client,
            &*configuration.application.base_url,
        )?;
        Ok(Self { port, server })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub struct ApplicationBaseUrl(pub String);

pub fn run(
    lisener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: &str,
) -> Result<Server, std::io::Error> {
    let base_url = web::Data::new(ApplicationBaseUrl(base_url.to_owned()));
    let db_pool = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let sever = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletter", web::post().to(publish_newsletter))
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
    })
    .listen(lisener)?
    .run();
    Ok(sever)
}
