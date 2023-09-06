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

impl EmailClient {
    pub fn new(username: String, password: Secret<String>, user_mail: String) -> Self {
        let creds = Credentials::new(username.clone(), password.expose_secret().to_owned());
        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")
                .unwrap()
                .credentials(creds)
                .build();
        let user_mailbox: Mailbox = Mailbox::new(
            None,
            user_mail
                .parse::<Address>()
                .expect("Failed to parse user mail"),
        );
        EmailClient {
            mailer,
            user_mailbox,
        }
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
            .header(ContentType::TEXT_PLAIN)
            .body(text_content.to_owned())
            .expect("Failed to create email");
        self.mailer.send(email).await
    }
    pub async fn send_confirmation(
        &self,
        subscriber: &Subscriber,
    ) -> Result<lettre::transport::smtp::response::Response, lettre::transport::smtp::Error> {
        //Must be in somewhere else ig
        let subject = "Kither's newsletter email confimation";
        let text_content = "Hello, you are subscribing to my newsletter so this is a confimation mail";

        self.send_email(subscriber.name.as_ref().to_owned(), subscriber.email.clone(), subject, text_content).await
    }
}
