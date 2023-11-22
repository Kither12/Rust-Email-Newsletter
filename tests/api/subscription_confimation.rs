use rust_email_newsletter::email_client::EmailClient;

use crate::helpers::spawn_app;

#[tokio::test]
async fn confirmation_without_token_are_rejected() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/subscriptions/confirm", app.address))
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(400, response.status().as_u16());
}
#[tokio::test]
async fn confirmation_wrong_token_are_rejected() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let confimation_link =
        EmailClient::get_confirmation_link(&app.address, "This must be wrong token");
    let response = client
        .get(confimation_link.0)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(401, response.status().as_u16());
}
