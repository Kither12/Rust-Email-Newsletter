use std::{time::Duration, net::IpAddr};

use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use secrecy::{ExposeSecret, Secret};

use crate::domain::Subscriber;

pub struct EmailClient {
    user_mailbox: Mailbox,
    mailer: AsyncSmtpTransport<Tokio1Executor>,
}
pub struct ConfirmationLink(pub String);

impl EmailClient {
    pub fn new(
        username: &String,
        user_mail: &String,
        mailer: AsyncSmtpTransport<Tokio1Executor>,
    ) -> Self {
        let user_mailbox: Mailbox = Mailbox::new(
            Some(username.to_owned()),
            user_mail
                .parse::<Address>()
                .expect("Failed to parse user mail"),
        );
        EmailClient {
            mailer,
            user_mailbox,
        }
    }
    pub fn get_gmail_mailer(
        username: &String,
        password: &Secret<String>,
    ) -> AsyncSmtpTransport<Tokio1Executor> {
        let creds = Credentials::new(username.to_owned(), password.expose_secret().to_owned());
        AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")
            .expect("Failed to connect to gmail SMTP port")
            .credentials(creds)
            .build()
    }
    pub fn get_test_mailer(
        smtp_host: &IpAddr,
        smtp_port: &u16,
    ) -> AsyncSmtpTransport<Tokio1Executor> {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host.to_string())
            .expect("Failed to connect to mailcrab SMTP port")
            .tls(lettre::transport::smtp::client::Tls::None)
            .timeout(Some(Duration::from_secs(5)))
            .port(*smtp_port)
            .build()
    }
    pub async fn send_email(
        &self,
        recipent_name: String,
        recipent_mail: Address,
        subject: &str,
        text_content: &str,
    ) -> Result<lettre::transport::smtp::response::Response, lettre::transport::smtp::Error> {
        let email = Message::builder()
            .from(self.user_mailbox.clone())
            .to(Mailbox::new(Some(recipent_name), recipent_mail))
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(text_content.to_owned())
            .expect("Failed to create email");
        match self.mailer.send(email).await {
            Ok(e) => Ok(e),
            Err(e) => {
                tracing::error!("Failed to send email: {}", e);
                Err(e)
            }
        }
    }
    pub fn get_confirmation_link(base_url: &str, subscription_token: &str) -> ConfirmationLink {
        ConfirmationLink(format!(
            "{}/subscriptions/confirm?subscription_token={}",
            base_url, subscription_token
        ))
    }
    pub async fn send_confirmation(
        &self,
        subscriber: &Subscriber,
        base_url: &String,
        subscription_token: &String,
    ) -> Result<lettre::transport::smtp::response::Response, lettre::transport::smtp::Error> {
        let confimation_link = EmailClient::get_confirmation_link(base_url, subscription_token);
        let subject = "Kither's newsletter email confimation";
        let html_body = format!(
            "Welcome to our newsletter!<br />Click <a href=\"{}\">here</a> to confirm your subscription.",
            confimation_link.0
        );

        match self
            .send_email(
                subscriber.name.as_ref().to_owned(),
                subscriber.email.clone(),
                subject,
                &html_body,
            )
            .await
        {
            Ok(e) => Ok(e),
            Err(e) => {
                tracing::error!("Failed to send confimation: {}", e);
                Err(e)
            }
        }
    }
}
