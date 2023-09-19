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
    let confimation_link = EmailClient::get_confirmation_link(&app.address, "12345");
    let response = client
        .get(confimation_link)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(401, response.status().as_u16());
}
#[tokio::test]
async fn check_confirmation_work_after_send_in_email() {
    let app = spawn_app().await;
    let response = app
        .post_subscriptions("name=kither123%20god&email=superminecraft2509%40gmail.com")
        .await;
    let subscription_token = response.text().await.expect("Failed to get response");
    let confimation_link = EmailClient::get_confirmation_link(&app.address, &subscription_token);
    let client = reqwest::Client::new();
    let response = client
        .get(confimation_link)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(response.status().as_u16(), 200);
}